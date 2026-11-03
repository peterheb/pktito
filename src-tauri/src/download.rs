//! A model's download as the window runs it: on a thread of its own, saying
//! how it goes as it goes, trying again by itself while the network is away
//! (2 s after it went, then twice as long each time, 30 s at most), and
//! paused, resumed or cancelled from the window.

use crate::model::{self, Ctl, Entry, Stop, Wanted};
use crate::strings::Problem;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

/// How a download stands, for the window.
#[derive(Clone, Serialize)]
pub struct State {
    pub name: String,
    /// "downloading", "checking", "paused", "failed", "done" or "cancelled"
    pub step: &'static str,
    pub got: u64,
    pub total: u64,
    /// Bytes a second over the last few seconds; 0 until known.
    pub speed: f64,
    pub host: String,
    /// A failure: what happened.
    pub problem: Option<Problem>,
    /// A failure it is trying again after, by itself.
    pub retry: bool,
    /// After a failure, trying again now: the failure stands until bytes come.
    pub trying: bool,
}

pub struct Job {
    pub ctl: Ctl,
    /// Try again now, not after the wait.
    now: AtomicBool,
    alive: AtomicBool,
    /// the bytes of the files before the one under way, where a download
    /// is of several (model::edition): its progress is the whole's
    before: AtomicU64,
    state: Mutex<State>,
}

impl Job {
    /// A job for e's download on this system (model::edition): one file, or
    /// on a Mac the two of its Neural Engine edition, counted as one.
    pub fn new(e: &Entry, dir: &Path) -> Job {
        let files = model::edition(dir, e);
        let job = Job::for_file(&files[0]);
        job.set(|s| {
            s.total = files.iter().map(|w| w.bytes).sum();
            s.got = model::kept(dir, e);
        });
        job
    }

    /// A job for files of one model's, in turn, counted as one: what the
    /// Android app chose to download of it, the edition for the phone it
    /// is on (the whole .pkf, or the slim one and the graph for its NPU),
    /// or for a whole .pkf here the graph alone (android.rs, download).
    pub fn files(files: &[Wanted]) -> Job {
        let job = Job::for_file(&files[0]);
        job.set(|s| {
            s.total = files.iter().map(|w| w.bytes).sum();
            s.got = files
                .iter()
                .map(|w| if w.here() { w.bytes } else { w.partial() })
                .sum();
        });
        job
    }

    /// A job for any file models.pktito.net serves (model::Wanted).
    pub fn for_file(w: &Wanted) -> Job {
        Job {
            ctl: Ctl::default(),
            now: AtomicBool::new(false),
            alive: AtomicBool::new(false),
            before: AtomicU64::new(0),
            state: Mutex::new(State {
                name: w.name.clone(),
                step: "downloading",
                got: w.partial(),
                total: w.bytes,
                speed: 0.0,
                host: model::host(&w.url).into(),
                problem: None,
                retry: false,
                trying: false,
            }),
        }
    }

    pub fn state(&self) -> State {
        self.state.lock().unwrap().clone()
    }

    pub fn name(&self) -> String {
        self.state.lock().unwrap().name.clone()
    }

    /// Whether its thread is running: downloading, or waiting to try again.
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// For one whose thread has ended: cancelled, its part deleted by the caller.
    pub fn set_step(&self, step: &'static str) {
        self.set(|s| s.step = step);
    }

    /// Go on: now, if it is waiting to try again; else the window starts it.
    pub fn resume_now(&self) {
        self.ctl.pause.store(false, Ordering::Relaxed);
        self.now.store(true, Ordering::Relaxed);
    }

    /// Run the download on a thread, telling `tell` each change of state
    /// (progress at most ten times a second); `done` gets the checked file.
    pub fn start(
        self: &std::sync::Arc<Job>,
        e: &'static Entry,
        dir: PathBuf,
        tell: impl Fn(&State) + Send + 'static,
        done: impl FnOnce(PathBuf) + Send + 'static,
    ) {
        self.alive.store(true, Ordering::Release);
        self.ctl.pause.store(false, Ordering::Relaxed);
        let job = self.clone();
        struct Over(std::sync::Arc<Job>);
        impl Drop for Over {
            fn drop(&mut self) {
                self.0.alive.store(false, Ordering::Release);
            }
        }
        thread::spawn(move || {
            let over = Over(job);
            let job = &over.0;
            if let Some(p) = job.run(e, &dir, &tell) {
                done(p);
            }
            tell(&job.state());
        });
    }

    /// A job for e's Core ML encoder alone, for a whole model file that is
    /// here without it (model::encoder_offer).
    pub fn encoder(e: &Entry, dir: &Path) -> Job {
        Job::for_file(&Wanted::part(dir, e, &e.coreml.encoder))
    }

    /// Run encoder()'s download on a thread: the archive, then unpacked;
    /// `done` gets the encoder.
    pub fn start_encoder(
        self: &std::sync::Arc<Job>,
        e: &'static Entry,
        dir: PathBuf,
        tell: impl Fn(&State) + Send + 'static,
        done: impl FnOnce(PathBuf) + Send + 'static,
    ) {
        let archive = Wanted::part(&dir, e, &e.coreml.encoder);
        let dst = model::encoder_in(&dir, e);
        let job = self.clone();
        self.start_file(archive, tell, move |got| {
            job.set(|s| s.step = "checking");
            match model::unpack_encoder(&got, &dst) {
                Ok(()) => {
                    job.set(|s| s.step = "done");
                    done(dst);
                }
                Err(why) => job.set(|s| {
                    (s.step, s.speed, s.retry, s.trying) = ("failed", 0.0, false, false);
                    s.problem = Some(Problem::new("download.damaged").detail(why));
                }),
            }
        });
    }

    /// start() for any file models.pktito.net serves.
    pub fn start_file(
        self: &std::sync::Arc<Job>,
        w: Wanted,
        tell: impl Fn(&State) + Send + 'static,
        done: impl FnOnce(PathBuf) + Send + 'static,
    ) {
        self.alive.store(true, Ordering::Release);
        self.ctl.pause.store(false, Ordering::Relaxed);
        let job = self.clone();
        // Over when the thread is: after done has the file, so a waiter that
        // sees alive() false finds it, and on a panic in tell or done too
        // (an eprintln! to a closed stderr), where a waiter would otherwise
        // wait forever.
        struct Over(std::sync::Arc<Job>);
        impl Drop for Over {
            fn drop(&mut self) {
                self.0.alive.store(false, Ordering::Release);
            }
        }
        thread::spawn(move || {
            let over = Over(job);
            let job = &over.0;
            if let Some(p) = job.run_file(&w, &tell) {
                done(p);
            }
            tell(&job.state());
        });
    }

    /// The download on this thread: `tell` hears each change of state as it
    /// goes, and state() says how it ended; the checked file, or None where
    /// it stopped short. start runs it on a thread of its own.
    pub fn run(&self, e: &'static Entry, dir: &Path, tell: &impl Fn(&State)) -> Option<PathBuf> {
        let files = model::edition(dir, e);
        let mut before = 0;
        for w in &files {
            if !w.here() {
                self.before.store(before, Ordering::Relaxed);
                if self.run_file(w, tell).is_none() {
                    // cancelled: what it had of the files before goes too
                    if self.state().step == "cancelled" {
                        let _ = model::remove_parts(dir, e);
                    }
                    return None;
                }
            }
            before += w.bytes;
        }
        self.before.store(0, Ordering::Relaxed);
        // the Neural Engine's encoder out of its archive, said as the check
        // is, in the while it takes (seconds)
        if model::NEURAL && !model::have_encoder(dir, e) {
            self.set(|s| s.step = "checking");
            tell(&self.state());
            if let Err(why) = model::unpack_encoder(&files[1].dst, &model::encoder_in(dir, e)) {
                self.set(|s| {
                    (s.step, s.speed, s.retry, s.trying) = ("failed", 0.0, false, false);
                    s.problem = Some(Problem::new("download.damaged").detail(why));
                });
                return None;
            }
            self.set(|s| s.step = "done");
        }
        model::file_of(dir, e)
    }

    /// files()'s download on this thread, as run() downloads an edition:
    /// each file not here in turn, the progress the whole's. A cancel
    /// deletes every file of it, done or not, and one at a file's place
    /// that is not the catalog's: an older graph an update of the app
    /// left, its catalog's graph a new one. The last file's path once all
    /// are here. The Android app runs it on a thread of its own
    /// (android.rs, download), which a job or a service keeps alive for it.
    pub fn run_files(&self, files: &[Wanted], tell: &impl Fn(&State)) -> Option<PathBuf> {
        let mut before = 0;
        for w in files {
            if !w.here() {
                self.before.store(before, Ordering::Relaxed);
                if self.run_file(w, tell).is_none() {
                    if self.state().step == "cancelled" {
                        for f in files {
                            let _ = std::fs::remove_file(&f.part);
                            let _ = std::fs::remove_file(&f.dst);
                        }
                    }
                    return None;
                }
            }
            before += w.bytes;
        }
        self.before.store(0, Ordering::Relaxed);
        files.last().map(|w| w.dst.clone())
    }

    /// run() for any file models.pktito.net serves.
    pub fn run_file(&self, w: &Wanted, tell: &impl Fn(&State)) -> Option<PathBuf> {
        let mut wait = Duration::from_secs(2);
        loop {
            let started = self.state.lock().unwrap().got;
            let (mut t0, mut b0) = (Instant::now(), started);
            let mut shown = Instant::now() - Duration::from_secs(1);
            // after a failure the window keeps saying so until bytes come, not
            // "downloading" for each try at a network that is still away
            self.set(|s| {
                if s.step == "failed" {
                    s.trying = true;
                } else {
                    s.step = "downloading";
                }
            });
            tell(&self.state());
            let r = model::download_file(w, &self.ctl, |got, checking| {
                let now = Instant::now();
                let dt = now.duration_since(t0).as_secs_f64();
                let got = self.before.load(Ordering::Relaxed) + got;
                self.set(|s| {
                    s.got = got;
                    s.step = if checking { "checking" } else { "downloading" };
                    (s.retry, s.trying) = (false, false);
                    // a rate over a second or more, smoothed over a few
                    if dt >= 1.0 {
                        let rate = got.saturating_sub(b0) as f64 / dt;
                        s.speed = if s.speed > 0.0 {
                            0.7 * s.speed + 0.3 * rate
                        } else {
                            rate
                        };
                    }
                });
                if dt >= 1.0 {
                    (t0, b0) = (now, got);
                }
                if checking || now.duration_since(shown) >= Duration::from_millis(100) {
                    shown = now;
                    tell(&self.state());
                }
            });
            match r {
                Ok(p) => {
                    self.set(|s| s.step = "done");
                    return Some(p);
                }
                Err(Stop::Paused) => {
                    self.set(|s| {
                        s.step = "paused";
                        s.speed = 0.0;
                    });
                    return None;
                }
                Err(Stop::Cancelled) => {
                    let _ = std::fs::remove_file(&w.part);
                    self.set(|s| s.step = "cancelled");
                    return None;
                }
                Err(Stop::Hard(p)) => {
                    let got = self.before.load(Ordering::Relaxed) + w.partial();
                    self.set(|s| {
                        (s.step, s.got, s.speed, s.retry, s.trying) =
                            ("failed", got, 0.0, false, false);
                        s.problem = Some(p);
                    });
                    return None;
                }
                Err(Stop::Net(p)) => {
                    let got = self.before.load(Ordering::Relaxed) + w.partial();
                    if got > started {
                        // it was getting somewhere: the next wait starts short again
                        wait = Duration::from_secs(2);
                    }
                    self.set(|s| {
                        (s.step, s.got, s.speed, s.retry, s.trying) =
                            ("failed", got, 0.0, true, false);
                        s.problem = Some(p);
                    });
                    tell(&self.state());
                    let until = Instant::now() + wait;
                    while Instant::now() < until && !self.now.swap(false, Ordering::Relaxed) {
                        if self.ctl.cancel.load(Ordering::Relaxed) {
                            let _ = std::fs::remove_file(&w.part);
                            self.set(|s| s.step = "cancelled");
                            return None;
                        }
                        if self.ctl.pause.load(Ordering::Relaxed) {
                            self.set(|s| s.step = "paused");
                            return None;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    wait = (wait * 2).min(Duration::from_secs(30));
                }
            }
        }
    }

    fn set(&self, f: impl FnOnce(&mut State)) {
        f(&mut self.state.lock().unwrap());
    }
}

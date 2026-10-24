//! A model's download as the window runs it: on a thread of its own, saying
//! how it goes as it goes, trying again by itself while the network is away
//! (2 s after it went, then twice as long each time, 30 s at most), and
//! paused, resumed or cancelled from the window.

use crate::model::{self, Ctl, Entry, Stop};
use crate::strings::Problem;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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
    state: Mutex<State>,
}

impl Job {
    pub fn new(e: &Entry, dir: &Path) -> Job {
        Job {
            ctl: Ctl::default(),
            now: AtomicBool::new(false),
            alive: AtomicBool::new(false),
            state: Mutex::new(State {
                name: e.name.into(),
                step: "downloading",
                got: model::partial(dir, e),
                total: e.bytes,
                speed: 0.0,
                host: model::host(&model::download_url(e)).into(),
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
            if let Some(p) = job.run(e, &dir, &tell) {
                done(p);
            }
            tell(&job.state());
        });
    }

    fn run(&self, e: &'static Entry, dir: &Path, tell: &impl Fn(&State)) -> Option<PathBuf> {
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
            let r = model::download(e, dir, &self.ctl, |got, checking| {
                let now = Instant::now();
                let dt = now.duration_since(t0).as_secs_f64();
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
                    let _ = std::fs::remove_file(model::part_in(dir, e));
                    self.set(|s| s.step = "cancelled");
                    return None;
                }
                Err(Stop::Hard(p)) => {
                    let got = model::partial(dir, e);
                    self.set(|s| {
                        (s.step, s.got, s.speed, s.retry, s.trying) =
                            ("failed", got, 0.0, false, false);
                        s.problem = Some(p);
                    });
                    return None;
                }
                Err(Stop::Net(p)) => {
                    let got = model::partial(dir, e);
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
                            let _ = std::fs::remove_file(model::part_in(dir, e));
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

//! pktito-cli: the app's transcription from a terminal. It reads every file
//! the window reads, through the same decoders (decode.rs), uses the model
//! the app is set to unless told another, and writes Save's formats with
//! the window's own code (format.rs), so its files and the window's are the
//! same bytes. It comes with the app, beside the window's program: the
//! bundles carry every program in src/bin.

use pktito_app::{decode, download, format, model, pktito, transcribe};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const USAGE: &str = "\
usage: pktito-cli [options] FILE...    transcribe audio or video: one file's
                                      transcript on the terminal, several's
                                      each beside its file
       pktito-cli models               the models, which are here, which is in use
       pktito-cli download MODEL       a model, for the app and pktito-cli both
       pktito-cli bench PATH...        how fast this computer transcribes: each
                                      file, or a folder's audio and video,
                                      timed, and the whole as times real time
  -m, --model MODEL    a model's name (pktito-cli models lists them) or a .pkf
                       file (default: the one the app uses)
  -f, --format F       txt, md, srt, vtt or json (default txt); or --txt, --md,
                       --srt, --vtt, --json
      --times          each sentence's time before it, in txt and md
  -o, --output PATH    one file's transcript to PATH; several's into the
                       folder PATH
  -t, --threads N      threads (default: one a core)
  -q, --quiet          no progress
bench takes -m, -t, -q and --json (its results as JSON, each file's with a
SHA-256 of its transcript).
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(match run(&args) {
        Ok(code) => code,
        Err(why) => {
            eprintln!("pktito-cli: {why}");
            1
        }
    });
}

struct Opts {
    files: Vec<String>,
    model: Option<String>,
    ext: String,
    times: bool,
    output: Option<PathBuf>,
    threads: i32,
    quiet: bool,
}

fn run(args: &[String]) -> Result<i32, String> {
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => {
            print!("{USAGE}");
            return Ok(0);
        }
        Some("--version") => {
            println!("pktito-cli {}", env!("CARGO_PKG_VERSION"));
            return Ok(0);
        }
        Some("models") => return models(),
        Some("download") => {
            return fetch(
                args.get(1)
                    .ok_or("download needs a model name (pktito-cli models shows the list)")?,
            )
        }
        _ => {}
    }
    let bench = args[0] == "bench";
    let args = if bench { &args[1..] } else { args };
    let mut o = Opts {
        files: vec![],
        model: None,
        ext: "txt".into(),
        times: false,
        output: None,
        threads: transcribe::threads_wanted(),
        quiet: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |flag: &str| it.next().cloned().ok_or(format!("{flag} needs a value"));
        match a.as_str() {
            "-m" | "--model" => o.model = Some(value(a)?),
            "-f" | "--format" => o.ext = value(a)?.to_lowercase(),
            "--txt" | "--md" | "--srt" | "--vtt" | "--json" => o.ext = a[2..].into(),
            "--times" => o.times = true,
            "-o" | "--output" => o.output = Some(PathBuf::from(value(a)?)),
            "-t" | "--threads" => {
                o.threads = value(a)?
                    .parse()
                    .map_err(|_| format!("{a} needs a number"))?
            }
            "-q" | "--quiet" => o.quiet = true,
            "-" => return Err("reads files, not standard input".into()),
            f if f.starts_with('-') => return Err(format!("no option {f}\n{USAGE}")),
            f => o.files.push(f.into()),
        }
    }
    if !format::FORMATS.contains(&o.ext.as_str()) {
        return Err(format!("no format {}: txt, md, srt, vtt or json", o.ext));
    }
    if o.files.is_empty() {
        return Err(format!("no files given\n{USAGE}"));
    }
    if bench {
        return bench_all(&o);
    }
    transcribe_all(&o)
}

/// Each file transcribed and written: on to the next after one that fails,
/// which says why; 1 if any did.
fn transcribe_all(o: &Opts) -> Result<i32, String> {
    let path = model_path(o.model.as_deref())?;
    let model = pktito::Model::load(&path)?;
    let mut asr = pktito::Asr::new(&model, o.threads)?;
    let name = model.name();
    let show = !o.quiet && std::io::stderr().is_terminal();
    let mut failed = 0;
    for f in &o.files {
        let file = Path::new(f);
        let label = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| f.clone());
        let done = (|| -> Result<Option<PathBuf>, String> {
            if show {
                status(&format!("{label}: reading the audio"));
            }
            let x = decode::decode(file).map_err(|e| e.message())?;
            let (segs, _) = transcribe::run(
                &model,
                &mut asr,
                &x,
                |done, _| {
                    if show {
                        status(&format!("{label}: transcribing {:.0}%", done * 100.0));
                    }
                },
                || false,
            )?;
            let t = format::Transcript {
                name: &label,
                duration: x.len() as f64 / decode::RATE as f64,
                model: Some(name.as_str()),
                segments: &segs,
            };
            let text = format::text(&t, &o.ext, o.times)?;
            let to = destination(o, file);
            if show {
                status("");
            }
            match &to {
                None => {
                    let mut out = std::io::stdout().lock();
                    out.write_all(text.as_bytes())
                        .and_then(|_| out.flush())
                        .map_err(|e| e.to_string())?;
                }
                Some(p) => fs::write(p, &text)
                    .map_err(|e| format!("couldn't write {}: {e}", p.display()))?,
            }
            Ok(to)
        })();
        match done {
            Ok(Some(p)) if !o.quiet => eprintln!("{label}: {}", p.display()),
            Ok(_) => {}
            Err(why) => {
                if show {
                    status("");
                }
                eprintln!("pktito-cli: {label}: {why}");
                failed += 1;
            }
        }
    }
    Ok(if failed > 0 { 1 } else { 0 })
}

/// The files bench times: each one named, and a folder's audio and video
/// (decode::MEDIA), by name; not the folders inside it.
fn bench_files(paths: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for p in paths.iter().map(PathBuf::from) {
        if !p.is_dir() {
            files.push(p);
            continue;
        }
        let mut here: Vec<PathBuf> = fs::read_dir(&p)
            .map_err(|e| format!("couldn't read {}: {e}", p.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|f| f.is_file())
            .filter(|f| {
                f.extension().is_some_and(|x| {
                    decode::MEDIA.contains(&x.to_string_lossy().to_lowercase().as_str())
                })
            })
            .collect();
        if here.is_empty() {
            return Err(format!("no audio or video in {}", p.display()));
        }
        here.sort();
        files.extend(here);
    }
    Ok(files)
}

/// How fast this computer transcribes: each file's transcription timed, from
/// its samples to its segments (the chunking, the model and the sentences,
/// as the window runs them), and the whole as the audio's length over the
/// time it took. Reading the file is left out of the time: its decoder is
/// the OS's or ffmpeg's, not pktito, and it is a small part. A file that
/// can't be read is said and left out; 1 if any was.
fn bench_all(o: &Opts) -> Result<i32, String> {
    let files = bench_files(&o.files)?;
    let path = model_path(o.model.as_deref())?;
    let model = pktito::Model::load(&path)?;
    let mut asr = pktito::Asr::new(&model, o.threads)?;
    let json = o.ext == "json";
    let show = !o.quiet && std::io::stderr().is_terminal();
    let (version, name, backend, compiler, threads) = (
        env!("CARGO_PKG_VERSION"),
        model.name(),
        pktito::backend(),
        pktito::compiler(),
        asr.threads(),
    );
    if !json {
        println!(
            "pktito-cli {version}\nmodel    {name}\nbackend  {backend}\ncompiler {compiler}\nthreads  {threads}"
        );
    }
    let (mut audio, mut took, mut failed, mut results) = (0.0, 0.0, 0, Vec::new());
    for file in &files {
        let label = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let done = (|| -> Result<(f64, f64, String), String> {
            if show {
                status(&format!("{label}: reading the audio"));
            }
            let x = decode::decode(file).map_err(|e| e.message())?;
            let start = Instant::now();
            let (segs, _) = transcribe::run(
                &model,
                &mut asr,
                &x,
                |done, _| {
                    if show {
                        status(&format!("{label}: transcribing {:.0}%", done * 100.0));
                    }
                },
                || false,
            )?;
            let secs = start.elapsed().as_secs_f64();
            let words = serde_json::to_string(&segs).map_err(|e| e.to_string())?;
            Ok((
                x.len() as f64 / decode::RATE as f64,
                secs,
                sha256_hex(words.as_bytes()),
            ))
        })();
        if show {
            status("");
        }
        match done {
            Ok((a, t, sha)) => {
                if !json {
                    println!("{:>9}  {:>9.2} s  {:>7.1}x  {label}", clock(a), t, a / t);
                }
                results.push(serde_json::json!({ "file": label, "audio_s": a, "seconds": t, "x_real_time": a / t, "transcript_sha256": sha }));
                audio += a;
                took += t;
            }
            Err(why) => {
                eprintln!("pktito-cli: {label}: {why}");
                failed += 1;
            }
        }
    }
    if results.is_empty() {
        return Ok(1);
    }
    if json {
        let all = serde_json::json!({
            "pktito_cli": version, "model": name, "backend": backend, "compiler": compiler,
            "threads": threads,
            "files": results, "audio_s": audio, "seconds": took, "x_real_time": audio / took,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?
        );
    } else {
        let n = results.len();
        println!(
            "{:>9}  {:>9.2} s  {:>7.1}x  all {n} file{}",
            clock(audio),
            took,
            audio / took,
            if n == 1 { "" } else { "s" }
        );
    }
    Ok(if failed > 0 { 1 } else { 0 })
}

/// Seconds as h:mm:ss, or m:ss under an hour
fn clock(s: f64) -> String {
    let s = s.round() as u64;
    match s / 3600 {
        0 => format!("{}:{:02}", s / 60, s % 60),
        h => format!("{h}:{:02}:{:02}", s / 60 % 60, s % 60),
    }
}

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// A line of progress on stderr, written over the last: padded with spaces
/// rather than cleared with an escape, which an older Windows console would
/// print as it is; "" clears it.
fn status(line: &str) {
    if line.is_empty() {
        eprint!("\r{:79}\r", "");
    } else {
        eprint!("\r{line:<79}");
    }
}

/// Where a file's transcript goes: the terminal, for one file and no -o;
/// -o itself for one file, unless it is a folder; else into -o's folder,
/// or beside the file, by its name and never over another (format.rs,
/// free_path, as the window saves)
fn destination(o: &Opts, file: &Path) -> Option<PathBuf> {
    let one = o.files.len() == 1;
    let stem = format::stem_of(file);
    match &o.output {
        None if one => None,
        None => Some(format::free_path(
            file.parent().unwrap_or(Path::new("")),
            &stem,
            &o.ext,
        )),
        Some(p) if one && !p.is_dir() && !p.to_string_lossy().ends_with(['/', '\\']) => {
            Some(p.clone())
        }
        Some(dir) => {
            let _ = fs::create_dir_all(dir);
            Some(format::free_path(dir, &stem, &o.ext))
        }
    }
}

/// The model to transcribe with: a .pkf named, or a catalog model by name,
/// downloaded; else the one the app uses (model::usable)
fn model_path(arg: Option<&str>) -> Result<PathBuf, String> {
    let data = model::data_dir().ok_or("no data folder for the app on this system")?;
    let Some(m) = arg else {
        return model::usable(&data, &model::load_settings(&data)).ok_or_else(|| {
            "no model yet: choose one in the app, or pktito-cli download parakeet-tdt-0.6b-v2"
                .into()
        });
    };
    if Path::new(m).is_file() {
        return Ok(PathBuf::from(m));
    }
    let e = entry(m)?;
    let dir = data.join("models");
    if !model::have(&dir, e) {
        return Err(format!(
            "{} isn't downloaded: pktito-cli download {}",
            e.name, e.name
        ));
    }
    Ok(model::file_in(&dir, e))
}

/// A catalog model by its name, its label ("english", "multilingual") or its
/// name's last part ("v2")
fn entry(m: &str) -> Result<&'static model::Entry, String> {
    model::CATALOG
        .iter()
        .find(|e| {
            e.name == m || e.label.eq_ignore_ascii_case(m) || e.name.rsplit('-').next() == Some(m)
        })
        .ok_or_else(|| format!("no model {m} (pktito-cli models shows the list)"))
}

/// The catalog, each model's size and whether it is here, and the one in use
fn models() -> Result<i32, String> {
    let data = model::data_dir().ok_or("no data folder for the app on this system")?;
    let s = model::load_settings(&data);
    let using = model::usable(&data, &s);
    let dir = data.join("models");
    for e in model::CATALOG {
        let here = model::have(&dir, e);
        let state = if !here {
            "not downloaded"
        } else if using.as_deref() == Some(model::file_in(&dir, e).as_path()) {
            "in use"
        } else {
            "downloaded"
        };
        println!(
            "{:<22}{:<14}{:<24}{:>4} MB  {state}",
            e.name,
            e.label,
            e.about,
            (e.bytes as f64 / 1e6).round()
        );
    }
    if let Some(own) = s.model_file.as_ref().filter(|p| p.is_file()) {
        println!("{:<22}{}  in use", "your own file", own.display());
    }
    Ok(0)
}

/// A model downloaded into the app's models folder, as the window does it:
/// resuming what a stopped one kept, trying again, and checked against its
/// size and SHA-256. The app then uses it if it had none.
fn fetch(m: &str) -> Result<i32, String> {
    let e = entry(m)?;
    let data = model::data_dir().ok_or("no data folder for the app on this system")?;
    let dir = data.join("models");
    if model::have(&dir, e) {
        // here already, from the window or a copy: in use, too, if none is,
        // as one just downloaded is (with no settings, the model there was
        // "no model yet" after a download of it said where it was)
        use_if_none(&data, e)?;
        println!("{}", model::file_in(&dir, e).display());
        return Ok(0);
    }
    fs::create_dir_all(&dir).map_err(|err| format!("couldn't make {}: {err}", dir.display()))?;
    let job = Arc::new(download::Job::new(e, &dir));
    let (tx, rx) = std::sync::mpsc::channel();
    let show = std::io::stderr().is_terminal();
    // The window tries again for as long as it is open, where a person sees
    // it and can stop it; a command line in a script could wait forever (a
    // sandbox's TLS that failed the same way every try ran it until killed,
    // 2026-10-24). So after TRIES failures in a row with no bytes between
    // them, about a minute of waits (2, 4, 8, 16 s), it stops, keeping what
    // it has for the next `download` to go on from.
    const TRIES: u32 = 5;
    let failed = Arc::new(AtomicU32::new(0));
    let (fails, stop) = (failed.clone(), job.clone());
    job.start(
        e,
        dir.clone(),
        move |s| match s.step {
            // each failure once, not again as its next try starts
            "failed" if !s.trying => {
                let n = if s.retry {
                    fails.fetch_add(1, Ordering::Relaxed) + 1
                } else {
                    0
                };
                let again = s.retry && n < TRIES;
                if s.retry && !again {
                    stop.ctl.pause.store(true, Ordering::Relaxed);
                }
                // the last failure is the error fetch returns, said once
                if again {
                    if show {
                        status("");
                    }
                    eprintln!(
                        "{} (trying again)",
                        s.problem.as_ref().map(|p| p.line()).unwrap_or_default()
                    )
                }
            }
            "failed" => {}
            step => {
                // bytes came: the count starts again
                if step == "downloading" || step == "checking" {
                    fails.store(0, Ordering::Relaxed);
                }
                if show {
                    status(&format!(
                        "{} {}: {} of {} MB",
                        s.step,
                        s.name,
                        s.got / 1_000_000,
                        s.total / 1_000_000
                    ));
                }
            }
        },
        move |p| {
            let _ = tx.send(p);
        },
    );
    while job.alive() {
        std::thread::sleep(Duration::from_millis(100));
    }
    if show {
        status("");
    }
    let path = rx.try_recv().map_err(|_| {
        let line = job.state().problem.map(|p| p.line()).unwrap_or_default();
        if failed.load(Ordering::Relaxed) >= TRIES {
            format!(
                "{line}\n{TRIES} tries in a row failed; `pktito-cli download {}` again \
                 goes on from where this stopped",
                e.name
            )
        } else {
            line
        }
    })?;
    use_if_none(&data, e)?;
    println!("{}", path.display());
    Ok(0)
}

/// The app's model from now on, if it has none.
fn use_if_none(data: &Path, e: &model::Entry) -> Result<(), String> {
    let mut s = model::load_settings(data);
    if s.model.is_none() && s.model_file.is_none() {
        s.model = Some(e.name.to_string());
        model::save_settings(data, &s)?;
    }
    Ok(())
}

//! pktito-cli: the app's transcription from a terminal. It reads every file
//! the window reads, through the same decoders (decode.rs), uses the model
//! the app is set to unless told another, and writes Save's formats with
//! the window's own code (format.rs), so its files and the window's are the
//! same bytes. It comes with the app, beside the window's program: the
//! bundles carry every program in src/bin.

use pktito_app::{awake, decode, download, format, model, pktito, score, transcribe};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const USAGE: &str = "\
usage: pktito-cli [options] FILE...        transcribe
       pktito-cli bench [options] PATH...  time a transcription
       pktito-cli models                   list the models, here and in use
       pktito-cli download [--cpu] MODEL   download a model
FILE is audio or video; one goes to stdout, several each beside its file.
PATH is a file or a folder of them.

  -m, --model MODEL   name (see pktito-cli models) or .pkf file
                      (default: the app's)
  -f, --format F      txt, md, srt, vtt or json (txt); or --txt, --md, ...
      --times         each sentence's start time, in txt and md
  -o, --output PATH   write to PATH; with several files, into folder PATH
  -t, --threads N     threads (default: one per core)
      --cpu           on a Mac, the CPU instead of the Neural Engine; with
                      download, the model for any CPU
  -q, --quiet         no progress

bench takes -m, -t, -q, --cpu and:
      --json          results as JSON, with each transcript's SHA-256
      --ref DIR       score each against DIR/NAME.txt (word error rate and
                      character edits)
";

fn main() {
    #[cfg(target_os = "android")]
    if !decode::binder_pool() {
        eprintln!("pktito-cli: no binder thread pool, which Android's decoder answers through");
    }
    // its downloads say they are pktito-cli's, not the window's
    model::set_client("pktito-cli");
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
    /// the encoder on the CPU even where the Neural Engine's is here
    cpu: bool,
    /// bench's references, a folder of NAME.txt
    reference: Option<PathBuf>,
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
            let cpu = args[1..].iter().any(|a| a == "--cpu");
            let name = args[1..]
                .iter()
                .find(|a| *a != "--cpu")
                .ok_or("download needs a model name (see pktito-cli models)")?;
            return fetch(name, cpu || !cfg!(target_os = "macos"));
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
        cpu: false,
        reference: None,
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
            "--cpu" => o.cpu = true,
            "--ref" if bench => o.reference = Some(PathBuf::from(value(a)?)),
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
    // the computer awake until the last file is written (awake.rs)
    let _awake = awake::awake("pktito-cli is transcribing");
    let path = model_path(o.model.as_deref())?;
    let model = pktito::Model::load(&path)?;
    let mut asr = pktito::Asr::new(&model, o.threads)?;
    encoder(&mut asr, &model, &path, o)?;
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
            let (segs, windows, _) = transcribe::run_windows(
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
            // json with pktito's chunks, for measuring its cuts
            let text = format::text_windows(&t, &o.ext, o.times, &windows)?;
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
    // awake through the timing too: a sleep in the middle would be timed
    let _awake = awake::awake("pktito-cli is timing a transcription");
    let files = bench_files(&o.files)?;
    let path = model_path(o.model.as_deref())?;
    let model = pktito::Model::load(&path)?;
    let mut asr = pktito::Asr::new(&model, o.threads)?;
    let encoder = encoder(&mut asr, &model, &path, o)?;
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
            "pktito-cli {version}\nmodel    {name}\nencoder  {encoder}\nbackend  {backend}\ncompiler {compiler}\nthreads  {threads}"
        );
    }
    let (mut audio, mut took, mut failed, mut results) = (0.0, 0.0, 0, Vec::new());
    // the scored files' reference words, word errors, character edits,
    // those identical, and how many
    let mut total = (0usize, 0usize, 0usize, 0usize, 0usize);
    for file in &files {
        let label = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let done = (|| -> Result<(f64, f64, String, String), String> {
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
            let text: Vec<&str> = segs.iter().map(|s| s.text.as_str()).collect();
            Ok((
                x.len() as f64 / decode::RATE as f64,
                secs,
                sha256_hex(words.as_bytes()),
                text.join("\n"),
            ))
        })();
        if show {
            status("");
        }
        match done {
            Ok((a, t, sha, text)) => {
                // against its reference, where there is a folder of them
                let scored = o.reference.as_ref().map(|dir| {
                    let stem = file
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    fs::read_to_string(dir.join(format!("{stem}.txt")))
                        .ok()
                        .map(|r| score::score(&text, &r))
                });
                if !json {
                    let said = match &scored {
                        None => String::new(),
                        Some(None) => "  no reference".into(),
                        Some(Some(s)) => format!("  {}", said(s)),
                    };
                    println!(
                        "{:>9}  {:>9.2} s  {:>7.1}x  {label}{said}",
                        clock(a),
                        t,
                        a / t
                    );
                }
                let mut r = serde_json::json!({ "file": label, "audio_s": a, "seconds": t, "x_real_time": a / t, "transcript_sha256": sha });
                if let Some(s) = &scored {
                    r["reference"] = serde_json::to_value(s).map_err(|e| e.to_string())?;
                }
                if let Some(Some(s)) = scored {
                    total.0 += s.words;
                    total.1 += s.word_errors;
                    total.2 += s.char_edits;
                    total.3 += usize::from(s.identical);
                    total.4 += 1;
                }
                results.push(r);
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
        let mut all = serde_json::json!({
            "pktito_cli": version, "model": name, "encoder": encoder, "backend": backend, "compiler": compiler,
            "threads": threads,
            "files": results, "audio_s": audio, "seconds": took, "x_real_time": audio / took,
        });
        if total.4 > 0 {
            all["reference"] = serde_json::json!({
                "files": total.4, "identical": total.3, "words": total.0,
                "word_errors": total.1, "wer": wer(total.1, total.0), "char_edits": total.2,
            });
        }
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
        if total.4 > 0 {
            let (words, errors, edits, same, scored) = total;
            if same == scored {
                println!("{scored} of {scored} transcripts identical to their references");
            } else {
                println!(
                    "{same} of {scored} transcripts identical to their references; WER {:.3}% ({errors} of {words} words), {edits} character edit{}",
                    wer(errors, words),
                    if edits == 1 { "" } else { "s" }
                );
            }
        }
    }
    Ok(if failed > 0 { 1 } else { 0 })
}

/// A transcript's distance from its reference, in a few words.
fn said(s: &score::Score) -> String {
    if s.identical {
        "identical".into()
    } else {
        format!(
            "WER {:.3}%, {} character edit{}",
            s.wer,
            s.char_edits,
            if s.char_edits == 1 { "" } else { "s" }
        )
    }
}

/// Word errors over words, in percent.
fn wer(errors: usize, words: usize) -> f64 {
    if words == 0 {
        0.0
    } else {
        100.0 * errors as f64 / words as f64
    }
}

/// The encoder a transcription runs: on a Mac, the Neural Engine's where the
/// model is a catalog one whose Core ML encoder is beside it (the ane fork,
/// core/ane.m) and --cpu doesn't say otherwise, else the CPU's, pktito's
/// own. Core ML compiles the encoder for this Mac the first time this
/// program loads it, which takes a while, once (about 10 s for v2 on the
/// M4 Pro). Where the Neural Engine can't run it, Core ML's reason is said
/// and the CPU's runs, if the model file has one: the slim file, the
/// Neural Engine edition's, has none, and says what to download. "Neural
/// Engine" or "CPU", for bench to say.
#[cfg(target_os = "macos")]
fn encoder(
    asr: &mut pktito::Asr,
    model: &pktito::Model,
    path: &Path,
    o: &Opts,
) -> Result<&'static str, String> {
    let whole = model.has_encoder();
    let cpu_or = |why: String| if whole { Ok("CPU") } else { Err(why) };
    let Some(data) = model::data_dir() else {
        return cpu_or("no data folder for the app on this system".into());
    };
    let dir = data.join("models");
    let e = model::CATALOG.iter().find(|e| {
        model::same_file(&model::file_in(&dir, e), path)
            || model::same_file(&model::slim_in(&dir, e), path)
    });
    let Some(e) = e else {
        return cpu_or(format!(
            "{} has no encoder of its own and isn't a catalog model's",
            path.display()
        ));
    };
    let again = format!(
        "pktito-cli download {} --cpu downloads the one for the CPU",
        e.name
    );
    if o.cpu {
        return cpu_or(format!(
            "{} here is the Neural Engine's edition, with no encoder for the CPU: {again}",
            e.name
        ));
    }
    if !model::have_encoder(&dir, e) {
        return cpu_or(format!(
            "{}'s Core ML encoder isn't here: pktito-cli download {}",
            e.name, e.name
        ));
    }
    let show = !o.quiet && std::io::stderr().is_terminal();
    if show {
        status("loading the Neural Engine's encoder (the first time, Core ML compiles it for this Mac)");
    }
    let r = asr.use_ane(Some(&model::encoder_in(&dir, e)));
    if show {
        status("");
    }
    match r {
        Ok(()) => Ok("Neural Engine"),
        Err(why) => {
            // the model's own encoder again
            let _ = asr.use_ane(None);
            if whole {
                eprintln!("pktito-cli: the Neural Engine can't run the encoder here ({why}); using the CPU");
            }
            cpu_or(format!(
                "the Neural Engine can't run the encoder here ({why}): {again}"
            ))
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn encoder(
    _asr: &mut pktito::Asr,
    _model: &pktito::Model,
    _path: &Path,
    _o: &Opts,
) -> Result<&'static str, String> {
    Ok("CPU")
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
    let dir = data.join("models");
    let Some(m) = arg else {
        let s = model::load_settings(&data);
        return model::usable(&data, &s)
            .or_else(|| {
                // the app's model as its Neural Engine edition, where only that is here
                let e = model::entry(s.model.as_deref()?)?;
                neural(&dir, e).then(|| model::slim_in(&dir, e))
            })
            .ok_or_else(|| {
                "no model yet: choose one in the app, or pktito-cli download parakeet-tdt-0.6b-v2"
                    .into()
            });
    };
    if Path::new(m).is_file() {
        return Ok(PathBuf::from(m));
    }
    let e = entry(m)?;
    if model::have(&dir, e) {
        return Ok(model::file_in(&dir, e));
    }
    if neural(&dir, e) {
        return Ok(model::slim_in(&dir, e));
    }
    Err(format!(
        "{} isn't downloaded: pktito-cli download {}",
        e.name, e.name
    ))
}

/// Whether e's Neural Engine edition is in dir, on a Mac, the one place it runs.
fn neural(dir: &Path, e: &model::Entry) -> bool {
    cfg!(target_os = "macos") && model::have_neural(dir, e)
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
        let (whole, ne) = (model::have(&dir, e), neural(&dir, e));
        let editions = match (whole, ne) {
            (true, true) => " (Neural Engine and CPU)",
            (false, true) => " (Neural Engine)",
            (true, false) if cfg!(target_os = "macos") => " (CPU)",
            _ => "",
        };
        let in_use = using.as_deref().is_some_and(|u| {
            model::same_file(u, &model::file_in(&dir, e))
                || model::same_file(u, &model::slim_in(&dir, e))
        }) || (using.is_none() && ne && s.model.as_deref() == Some(e.name));
        let state = if !whole && !ne {
            "not downloaded".to_string()
        } else if in_use {
            format!("in use{editions}")
        } else {
            format!("downloaded{editions}")
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
fn fetch(m: &str, cpu: bool) -> Result<i32, String> {
    let e = entry(m)?;
    let data = model::data_dir().ok_or("no data folder for the app on this system")?;
    let dir = data.join("models");
    // on a Mac, as the iOS app has it: the model file without its encoder
    // and its Core ML encoder, 589 MB for v2 where the whole is 624, the
    // encoder on the Neural Engine (3x the CPU's speed on an M4 Pro,
    // AliceBench); --cpu, or anywhere else, the whole model
    let here = if cpu {
        model::have(&dir, e)
    } else {
        model::have_neural(&dir, e)
    };
    let at = if cpu {
        model::file_in(&dir, e)
    } else {
        model::slim_in(&dir, e)
    };
    if !here {
        fs::create_dir_all(&dir)
            .map_err(|err| format!("couldn't make {}: {err}", dir.display()))?;
        if cpu {
            fetch_file(&model::Wanted::model(&dir, e))?;
        } else {
            let slim = model::Wanted::part(&dir, e, &e.coreml.slim);
            if !slim.here() {
                fetch_file(&slim)?;
            }
            let archive = model::Wanted::part(&dir, e, &e.coreml.encoder);
            if !model::have_encoder(&dir, e) {
                let got = fetch_file(&archive)?;
                unpack(&got, &model::encoder_in(&dir, e))?;
            }
        }
    }
    // here already, from the window or a copy: in use, too, if none is, as
    // one just downloaded is (with no settings, the model there was "no
    // model yet" after a download of it said where it was)
    use_if_none(&data, e)?;
    println!("{}", at.display());
    Ok(0)
}

/// The Core ML encoder out of its archive (model::unpack_encoder), saying so.
fn unpack(archive: &Path, dst: &Path) -> Result<(), String> {
    let show = std::io::stderr().is_terminal();
    if show {
        status("unpacking the Neural Engine's encoder");
    }
    let r = model::unpack_encoder(archive, dst);
    if show {
        status("");
    }
    r
}

/// One file downloaded and checked, saying how it goes; its path.
fn fetch_file(w: &model::Wanted) -> Result<PathBuf, String> {
    let job = Arc::new(download::Job::for_file(w));
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
    job.start_file(
        w.clone(),
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
                w.name
            )
        } else {
            line
        }
    })?;
    Ok(path)
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

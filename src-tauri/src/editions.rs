//! A model's editions: the files one kind of hardware runs it from, which
//! of them this device runs, whether the ones here are the catalog's, and
//! what Manage models offers. One rule for every platform (docs/EDITIONS.md):
//! the desktop's window, pktito-cli, and through ios.rs and android.rs the
//! iOS and Android apps, which tell it the hardware they have (Device) and
//! download and run what it names.

use crate::model::{self, marker_of, Ctl, Entry, Part, CATALOG};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One way to run a model: the model file it loads and, for an NPU, the
/// encoder built for it.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Edition {
    /// Stable, for refusals and logs: "cpu", "coreml", "qnn-sm8650".
    pub id: &'static str,
    pub runtime: Runtime,
    /// The model file it loads: the whole .pkf for the CPU's, else the
    /// slim one (`pktito-tools slim`), the model without its encoder.
    pub model: Part,
    /// The NPU's encoder: a compiled Core ML model in an Apple Archive, or a
    /// QNN graph; None for the CPU's.
    pub encoder: Option<Part>,
}

impl Edition {
    pub fn npu(&self) -> bool {
        self.encoder.is_some()
    }
}

/// What an edition runs on.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Runtime {
    /// pktito's own encoder, on any CPU.
    Cpu,
    /// Apple's Neural Engine, from these systems on (model::COREML's), on
    /// every device of theirs: each has one.
    CoreMl { macos: (u32, u32), ios: (u32, u32) },
    /// One Snapdragon's Hexagon, the chip in QNN's words for it
    /// (soc,arch,vtcm,dlbc,signed: pk_qnn_chip), on these systems
    /// (std::env::consts::OS): a phone never takes a PC's graph of its
    /// Hexagon, nor a PC a phone's.
    Qnn {
        chip: &'static str,
        systems: &'static [&'static str],
    },
}

/// The hardware the core chooses for, as each platform describes it.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Device {
    /// std::env::consts::OS: "macos", "ios", "android", "windows", "linux"
    pub system: String,
    /// The system's version: macOS's and iOS's major and minor, Android's
    /// (SDK_INT, 0). Only Core ML's floors compare it.
    #[serde(default)]
    pub os: (u32, u32),
    /// QNN's chip, where QNN found an NPU it runs on.
    #[serde(default)]
    pub qnn: Option<String>,
    /// What a refusal holds for: the system's build and the app's, and on
    /// Windows on Arm the NPU driver's, as each platform can name them. The
    /// core only compares it.
    #[serde(default)]
    pub here: String,
}

impl Device {
    /// This computer, for the window and pktito-cli: asked once.
    pub fn this() -> &'static Device {
        static THIS: OnceLock<Device> = OnceLock::new();
        THIS.get_or_init(|| {
            let system = std::env::consts::OS.to_string();
            let os = os_version();
            Device {
                here: format!(
                    "{system} {}.{} {}{}",
                    os.0,
                    os.1,
                    env!("CARGO_PKG_VERSION"),
                    npu_here()
                ),
                system,
                os,
                qnn: model::chip().map(str::to_string),
            }
        })
    }
}

/// This Mac's macOS version, from the system's own record of it; (0, 0)
/// where it can't be read and on other systems, whose version nothing
/// compares.
fn os_version() -> (u32, u32) {
    #[cfg(target_os = "macos")]
    {
        let plist = fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist");
        if let Some(v) = plist.ok().and_then(|p| {
            let after = p.split("<key>ProductVersion</key>").nth(1)?;
            let v = after.split("<string>").nth(1)?.split("</string>").next()?;
            Some(v.to_string())
        }) {
            let mut n = v.split('.').map(|x| x.parse().unwrap_or(0));
            return (n.next().unwrap_or(0), n.next().unwrap_or(0));
        }
    }
    (0, 0)
}

/// On Windows on Arm, the rest of what a refusal holds for: the OS build
/// with its update revision, and the NPU driver's version, which Windows
/// Update and the PC's maker update apart from the OS (snappy, 2026-11-08:
/// " build 26300.9818 npu 30.0.220.3000"). Either is "?" where it can't be
/// read, which still holds a refusal until the app's version changes.
#[cfg(all(windows, target_arch = "aarch64"))]
fn npu_here() -> String {
    let os = reg::os_build().unwrap_or_else(|| "?".into());
    let npu = reg::npu_driver().unwrap_or_else(|| "?".into());
    format!(" build {os} npu {npu}")
}

#[cfg(not(all(windows, target_arch = "aarch64")))]
fn npu_here() -> String {
    String::new()
}

/// The registry values npu_here reads, through advapi32 itself: the
/// windows crate's registry module would be a dependency for four calls.
#[cfg(all(windows, target_arch = "aarch64"))]
mod reg {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    type Hkey = *mut c_void;
    // HKEY_LOCAL_MACHINE is (HKEY)(LONG)0x80000002, sign-extended
    const HKLM: Hkey = 0x8000_0002u32 as i32 as isize as Hkey;
    const KEY_READ: u32 = 0x2_0019;
    const RRF_RT_REG_SZ: u32 = 0x2;
    const RRF_RT_REG_DWORD: u32 = 0x10;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(key: Hkey, sub: *const u16, opts: u32, sam: u32, out: *mut Hkey) -> i32;
        fn RegEnumKeyExW(
            key: Hkey,
            index: u32,
            name: *mut u16,
            len: *mut u32,
            reserved: *mut u32,
            class: *mut u16,
            class_len: *mut u32,
            time: *mut c_void,
        ) -> i32;
        fn RegGetValueW(
            key: Hkey,
            sub: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut c_void,
            len: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: Hkey) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    fn get(
        key: Hkey,
        sub: &str,
        value: &str,
        flags: u32,
        data: *mut c_void,
        len: &mut u32,
    ) -> bool {
        let (sub, value) = (wide(sub), wide(value));
        unsafe {
            RegGetValueW(
                key,
                sub.as_ptr(),
                value.as_ptr(),
                flags,
                null_mut(),
                data,
                len,
            ) == 0
        }
    }

    fn string(key: Hkey, sub: &str, value: &str) -> Option<String> {
        let mut buf = [0u16; 256];
        let mut len = std::mem::size_of_val(&buf) as u32;
        // len counts bytes, the terminating NUL included
        get(
            key,
            sub,
            value,
            RRF_RT_REG_SZ,
            buf.as_mut_ptr().cast(),
            &mut len,
        )
        .then(|| String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]))
    }

    fn dword(key: Hkey, sub: &str, value: &str) -> Option<u32> {
        let mut v = 0u32;
        let mut len = 4u32;
        get(
            key,
            sub,
            value,
            RRF_RT_REG_DWORD,
            (&mut v as *mut u32).cast(),
            &mut len,
        )
        .then_some(v)
    }

    /// "26300.9818": CurrentBuildNumber and UBR, the build and the update
    /// revision winver shows.
    pub fn os_build() -> Option<String> {
        let nt = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        let build = string(HKLM, nt, "CurrentBuildNumber")?;
        Some(match dword(HKLM, nt, "UBR") {
            Some(ubr) => format!("{build}.{ubr}"),
            None => build,
        })
    }

    /// The driver version of the ComputeAccelerator device whose
    /// description names a Hexagon, as Device Manager shows it: on snappy
    /// "Snapdragon(R) X Elite - X1E78100 - Qualcomm(R) Hexagon(TM) NPU",
    /// 30.0.220.3000, under the class's key 0000.
    pub fn npu_driver() -> Option<String> {
        let class =
            wide(r"SYSTEM\CurrentControlSet\Control\Class\{f01a9d53-3ff6-48d2-9f97-c8a7004be10c}");
        let mut key: Hkey = null_mut();
        if unsafe { RegOpenKeyExW(HKLM, class.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            return None;
        }
        let mut found = None;
        for i in 0.. {
            let mut name = [0u16; 64];
            let mut len = name.len() as u32;
            let r = unsafe {
                RegEnumKeyExW(
                    key,
                    i,
                    name.as_mut_ptr(),
                    &mut len,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if r != 0 {
                break;
            }
            let sub = String::from_utf16_lossy(&name[..len as usize]);
            if string(key, &sub, "DriverDesc").is_some_and(|d| d.contains("Hexagon")) {
                found = string(key, &sub, "DriverVersion");
                break;
            }
        }
        unsafe { RegCloseKey(key) };
        found
    }
}

/// How well edition fits device: 3 for one built for it (Core ML on a
/// system at least its floor, QNN for its exact chip), 2 for QNN built for
/// another SoC of its Hexagon with no more VTCM than it has, which QNN may
/// run or refuse, 1 for the CPU's, 0 for none.
pub fn fit(edition: &Edition, d: &Device) -> u8 {
    match edition.runtime {
        Runtime::Cpu => 1,
        Runtime::CoreMl { macos, ios } => match d.system.as_str() {
            "macos" if d.os >= macos => 3,
            "ios" if d.os >= ios => 3,
            _ => 0,
        },
        Runtime::Qnn { chip, systems } => {
            let Some(mine) = d.qnn.as_deref() else {
                return 0;
            };
            if !systems.contains(&d.system.as_str()) {
                return 0;
            }
            if mine == chip {
                return 3;
            }
            match (numbers(mine), numbers(chip)) {
                (Some(m), Some(t)) if m[1] == t[1] && t[2] <= m[2] => 2,
                _ => 0,
            }
        }
    }
}

/// A chip's five numbers in QNN's words; None for any other.
fn numbers(chip: &str) -> Option<Vec<u64>> {
    let n: Vec<u64> = chip
        .split(',')
        .map(|x| x.parse().ok())
        .collect::<Option<_>>()?;
    (n.len() == 5).then_some(n)
}

/// The edition of e this device runs: the best fit that isn't refused
/// here. The CPU's always fits, so there is always one. Never the catalog's
/// order: two at the top is a mistake in the catalog, which a test catches
/// (no_edition_ties_another).
pub fn choose(e: &Entry, d: &Device, r: &Refusals) -> &'static Edition {
    best(e, d, |ed| r.holds(e, ed, d).is_none())
}

fn best(e: &Entry, d: &Device, allowed: impl Fn(&Edition) -> bool) -> &'static Edition {
    let mut top: Option<(&'static Edition, u8)> = None;
    for ed in e.editions {
        let f = fit(ed, d);
        if f > 0 && allowed(ed) && top.is_none_or(|(_, t)| f > t) {
            top = Some((ed, f));
        }
    }
    top.map(|(ed, _)| ed).unwrap_or(e.cpu())
}

/// How a file of the catalog's stands in the models folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Have {
    /// Here, and the catalog's: its record (.sha256) is the catalog's
    /// SHA-256, or it has none yet but the catalog's size (adopt).
    Current,
    /// Here, but another: its record is another SHA-256, or its size isn't
    /// the catalog's, or for an unpacked encoder it has no record. It still
    /// runs.
    Older,
    /// Not in place: a download kept this many bytes, or (all of them) its
    /// download checked and waits to be put in place (settle).
    Partial(u64),
    Absent,
}

impl Have {
    pub fn here(self) -> bool {
        matches!(self, Have::Current | Have::Older)
    }
}

/// Whether p is unpacked once downloaded: a Core ML encoder's archive.
fn unpacked(p: &Part) -> bool {
    p.file.ends_with(".mlmodelc.aar")
}

/// Where p is once it is in place in dir: the file, or for a Core ML
/// encoder's archive the folder it unpacks to.
pub fn installed(dir: &Path, p: &Part) -> PathBuf {
    match p.file.strip_suffix(".aar").filter(|_| unpacked(p)) {
        Some(folder) => dir.join(folder),
        None => dir.join(p.file),
    }
}

/// Where p's download goes once it checks: in place, or beside the file it
/// replaces (`.next`) until that file is let go (settle). A Core ML
/// archive goes beside its folder under its own name, and settle unpacks it.
pub fn landing(dir: &Path, p: &Part) -> PathBuf {
    let file = dir.join(p.file);
    if !unpacked(p) && file.is_file() {
        next_of(&file)
    } else {
        file
    }
}

fn next_of(path: &Path) -> PathBuf {
    with_suffix(path, ".next")
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(suffix);
    PathBuf::from(p)
}

/// The SHA-256 recorded beside path, if any.
fn record(path: &Path) -> Option<String> {
    let s = fs::read_to_string(marker_of(path)).ok()?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Whether a download of p checked and waits to be put in place.
pub fn waiting(dir: &Path, p: &Part) -> bool {
    let at = if unpacked(p) {
        dir.join(p.file)
    } else {
        next_of(&dir.join(p.file))
    };
    at.is_file() && record(&at).as_deref() == Some(p.sha256)
}

/// How p stands in dir (Have).
pub fn have(dir: &Path, p: &Part) -> Have {
    let at = installed(dir, p);
    let present = if unpacked(p) {
        at.join("model.mil").is_file()
    } else {
        at.is_file()
    };
    if present {
        // a swap cut short after its file's rename: the record it was to
        // get is still beside the .next that has gone (recover moves it)
        let next = next_of(&at);
        if !unpacked(p) && !next.exists() && record(&next).as_deref() == Some(p.sha256) {
            return Have::Current;
        }
        return match record(&at) {
            Some(sha) if sha == p.sha256 => Have::Current,
            Some(_) => Have::Older,
            // before 0.9.6, no record: a file of the catalog's size is
            // taken for it until adopt reads it; a folder can't be read back
            None if !unpacked(p) && size(&at) == Some(p.bytes) => Have::Current,
            None => Have::Older,
        };
    }
    if waiting(dir, p) {
        return Have::Partial(p.bytes);
    }
    match size(&with_suffix(&dir.join(p.file), ".part")) {
        Some(n) => Have::Partial(n.min(p.bytes)),
        None => Have::Absent,
    }
}

fn size(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|m| m.len())
}

/// The files of the catalog's e may have, each once: the editions share
/// their slim file.
pub fn parts(e: &Entry) -> Vec<&'static Part> {
    let mut out: Vec<&'static Part> = Vec::new();
    for ed in e.editions {
        for p in [Some(&ed.model), ed.encoder.as_ref()].into_iter().flatten() {
            if !out.iter().any(|q| q.file == p.file) {
                out.push(p);
            }
        }
    }
    out
}

/// Every catalog file in dir without a record read once and given one: its
/// own SHA-256, the catalog's or not, so that a file from before 0.9.6 is
/// Current or Older by what it holds. About 2 s for 624 MB on a Mac and on
/// a Galaxy S24 (0.45 s from the page cache, android-9950x, 2026-11-07).
/// Swaps cut short are finished first, and 0.9.6's refusals deleted (they
/// were keyed on its build, which no 0.9.7 is). Off the main thread, at
/// start.
pub fn adopt(dir: &Path) {
    recover(dir);
    if let Ok(files) = fs::read_dir(dir) {
        for f in files.flatten() {
            if f.file_name().to_string_lossy().ends_with(".refused") {
                let _ = fs::remove_file(f.path());
            }
        }
    }
    for e in CATALOG {
        for p in parts(e) {
            let at = dir.join(p.file);
            if unpacked(p) || !at.is_file() || record(&at).is_some() {
                continue;
            }
            if size(&at) != Some(p.bytes) {
                continue;
            }
            if let Ok(mut f) = File::open(&at) {
                if let Ok(sha) = model::sha256(&mut f, &Ctl::default()) {
                    let _ = fs::write(marker_of(&at), format!("{sha}\n"));
                }
            }
        }
    }
}

/// A swap cut short, put right: a file renamed into place whose record
/// wasn't moved after it gets that record (settle moves the file first, so
/// the record of a replacement is never beside a file it doesn't describe);
/// an encoder's old folder put back where its new one isn't whole yet,
/// else deleted; and half an unpacking gone.
fn recover(dir: &Path) {
    for e in CATALOG {
        for p in parts(e).into_iter().filter(|p| !unpacked(p)) {
            let (file, next) = (dir.join(p.file), next_of(&dir.join(p.file)));
            if !next.exists() && marker_of(&next).exists() && file.is_file() {
                let _ = fs::rename(marker_of(&next), marker_of(&file));
            }
        }
    }
    for e in CATALOG {
        for p in parts(e).into_iter().filter(|p| unpacked(p)) {
            let at = installed(dir, p);
            let old = with_suffix(&at, ".old");
            let _ = fs::remove_dir_all(with_suffix(&at, ".unpacking"));
            if !old.exists() {
                continue;
            }
            if at.join("model.mil").is_file() {
                let _ = fs::remove_dir_all(&old);
                let _ = fs::remove_file(marker_of(&old));
            } else {
                let _ = fs::remove_dir_all(&at);
                let _ = fs::rename(&old, &at);
                let _ = fs::rename(marker_of(&old), marker_of(&at));
            }
        }
    }
}

/// Whether e has a download that checked and waits to be put in place.
pub fn pending(dir: &Path, e: &Entry) -> bool {
    parts(e).into_iter().any(|p| waiting(dir, p))
}

/// e's downloads that checked, put in place: a file over the one it
/// replaces, with its record; a Core ML archive unpacked over its encoder,
/// the old one kept aside until the new one is whole. Only while no engine
/// holds e's files, which the caller makes sure of: Windows renames no
/// file that is open, and Core ML may still read a folder it loaded.
pub fn settle(dir: &Path, e: &Entry) -> Result<(), String> {
    for p in parts(e) {
        if !waiting(dir, p) {
            continue;
        }
        if unpacked(p) {
            #[cfg(target_os = "macos")]
            model::unpack_encoder(&dir.join(p.file), &installed(dir, p))?;
            continue;
        }
        let (file, next) = (dir.join(p.file), next_of(&dir.join(p.file)));
        fs::rename(&next, &file)
            .map_err(|err| format!("couldn't put {} in place: {err}", file.display()))?;
        let _ = fs::rename(marker_of(&next), marker_of(&file));
    }
    Ok(())
}

/// Settle, for downloads that replace nothing: a new edition's encoder,
/// unpacked where none is in place. What replaces a file in use waits for
/// settle.
pub fn settle_new(dir: &Path, e: &Entry) -> Result<(), String> {
    for p in parts(e) {
        if unpacked(p) && waiting(dir, p) && !installed(dir, p).exists() {
            #[cfg(target_os = "macos")]
            model::unpack_encoder(&dir.join(p.file), &installed(dir, p))?;
        }
    }
    Ok(())
}

/// That an NPU refused one of a model's editions here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Refusal {
    pub model: String,
    pub edition: String,
    /// The SHA-256 of the encoder it refused.
    pub encoder: String,
    pub here: String,
    pub why: String,
}

/// The refusals recorded in a models folder (refusals.json).
#[derive(Default)]
pub struct Refusals(pub Vec<Refusal>);

fn refusals_in(dir: &Path) -> PathBuf {
    dir.join("refusals.json")
}

impl Refusals {
    pub fn read(dir: &Path) -> Refusals {
        Refusals(
            fs::read(refusals_in(dir))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default(),
        )
    }

    /// Why ed of e was refused, where that holds: for this device's system,
    /// app and driver (here), and this encoder. A new one of any tries again.
    pub fn holds(&self, e: &Entry, ed: &Edition, d: &Device) -> Option<&str> {
        let enc = ed.encoder.as_ref()?;
        self.0
            .iter()
            .find(|r| {
                r.model == e.name
                    && r.edition == ed.id
                    && r.encoder == enc.sha256
                    && r.here == d.here
            })
            .map(|r| r.why.as_str())
    }

    fn write(&self, dir: &Path) {
        let _ = fs::create_dir_all(dir);
        if let Ok(b) = serde_json::to_vec_pretty(&self.0) {
            let _ = fs::write(refusals_in(dir), b);
        }
    }
}

/// An NPU refused ed of e here (why): its encoder deleted, 714 MB for
/// v2's graph, and recorded (Refusals) where it was the catalog's. An
/// Older one refused says nothing of the catalog's, so it is only deleted,
/// and Update or Download offers the catalog's again (android-9950x,
/// 2026-11-08). Its model file stays: the engine may have it open, and it
/// goes with the model.
pub fn refuse(dir: &Path, e: &Entry, ed: &Edition, d: &Device, why: &str) {
    let Some(enc) = ed.encoder.as_ref() else {
        return;
    };
    if have(dir, enc) == Have::Current {
        let mut r = Refusals::read(dir);
        r.0.retain(|x| !(x.model == e.name && x.edition == ed.id));
        r.0.push(Refusal {
            model: e.name.into(),
            edition: ed.id.into(),
            encoder: enc.sha256.into(),
            here: d.here.clone(),
            why: why.into(),
        });
        r.write(dir);
        // nothing runs its slim file while the refusal holds: beside a
        // whole .pkf it waits for a retry, counted in the model's size;
        // alone it would be a file Manage models never shows (snappy,
        // 2026-11-08), so it goes, and a retry fetches it again (12 MB
        // for v2)
        if !have(dir, e.whole()).here() {
            remove_part(dir, &ed.model);
        }
    }
    remove_part(dir, enc);
}

/// Whether the test-only switch PKTITO_REFUSE_NPU=1 is set: every NPU then
/// refuses, so that a refusal's path can be walked on any device.
pub fn refusing_for_test() -> bool {
    std::env::var("PKTITO_REFUSE_NPU").is_ok_and(|v| v == "1")
}

/// What runs: an edition, the model file it loads and its encoder.
#[derive(Clone, Debug)]
pub struct Runs {
    pub edition: &'static Edition,
    /// The file it loads: its own, or for an NPU's the whole .pkf.
    pub model_part: &'static Part,
    pub model: PathBuf,
    pub encoder: Option<PathBuf>,
}

/// ed of e as it can run from dir: its encoder here, and a model file it
/// can load, its own or for an NPU's the CPU's whole one, which the engine
/// runs with an encoder beside it as well. None where they aren't here.
pub fn here_of(dir: &Path, e: &Entry, ed: &'static Edition) -> Option<Runs> {
    let encoder = match &ed.encoder {
        Some(p) if have(dir, p).here() => Some(installed(dir, p)),
        Some(_) => return None,
        None => None,
    };
    let whole = e.whole();
    let model_part = if have(dir, &ed.model).here() {
        &ed.model
    } else if ed.npu() && have(dir, whole).here() {
        whole
    } else {
        return None;
    };
    Some(Runs {
        edition: ed,
        model_part,
        model: dir.join(model_part.file),
        encoder,
    })
}

/// What e runs on here: the chosen edition where it is here, else the
/// CPU's where its file is; None for nothing to run.
pub fn runs(dir: &Path, e: &Entry, d: &Device, r: &Refusals) -> Option<Runs> {
    here_of(dir, e, choose(e, d, r)).or_else(|| here_of(dir, e, e.cpu()))
}

/// What Manage models offers for a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Nothing runs: the chosen edition's files.
    Download,
    /// The CPU's runs and an NPU's is chosen: its slim file and encoder.
    Faster,
    /// The chosen one runs and a file of it is Older: the catalog's.
    Update,
}

/// An offer: what it is, its files, the bytes they come to and how many of
/// those a download that stopped kept.
#[derive(Clone, Debug)]
pub struct Offer {
    pub kind: Kind,
    pub files: Vec<&'static Part>,
    pub bytes: u64,
    pub kept: u64,
}

/// What Manage models offers for e, if anything. Only ever started by its
/// user: nothing downloads by itself (Peter, 2026-11-05).
pub fn offer(dir: &Path, e: &Entry, d: &Device, r: &Refusals) -> Option<Offer> {
    let chosen = choose(e, d, r);
    let own = |ed: &'static Edition| -> Vec<&'static Part> {
        [Some(&ed.model), ed.encoder.as_ref()]
            .into_iter()
            .flatten()
            .filter(|p| have(dir, p) != Have::Current && !waiting(dir, p))
            .collect()
    };
    let (kind, files) = match runs(dir, e, d, r) {
        None => (Kind::Download, own(chosen)),
        Some(now) if now.edition.id != chosen.id => (Kind::Faster, own(chosen)),
        Some(now) => {
            let used = [Some(now.model_part), now.edition.encoder.as_ref()];
            let older = used
                .into_iter()
                .flatten()
                .filter(|p| have(dir, p) == Have::Older && !waiting(dir, p))
                .collect();
            (Kind::Update, older)
        }
    };
    if files.is_empty() {
        return None;
    }
    let kept = files
        .iter()
        .map(|p| match have(dir, p) {
            Have::Partial(n) => n,
            _ => 0,
        })
        .sum();
    let bytes = files.iter().map(|p| p.bytes).sum();
    Some(Offer {
        kind,
        files,
        bytes,
        kept,
    })
}

/// A transcription ran on ed with the model file at model: once an NPU's
/// edition has run with its own slim file, the CPU's whole .pkf goes, on
/// every platform (Peter, 2026-11-07): v2 then takes 727 MB, where the
/// whole file beside it takes 1,339. One that ran from the whole file keeps
/// it. Whether the whole file went.
pub fn ran(dir: &Path, e: &Entry, ed: &Edition, model: &Path) -> bool {
    if !ed.npu() {
        return false;
    }
    // it ran: a refusal of it recorded before an update is spent
    let mut r = Refusals::read(dir);
    let n = r.0.len();
    r.0.retain(|x| !(x.model == e.name && x.edition == ed.id));
    if r.0.len() != n {
        r.write(dir);
    }
    if !model::same_file(model, &dir.join(ed.model.file)) {
        return false;
    }
    if have(dir, &ed.model) != Have::Current {
        return false;
    }
    let whole = e.whole();
    if !dir.join(whole.file).exists() {
        return false;
    }
    remove_part(dir, whole);
    true
}

/// Every trace of p in dir: in place (file or folder), its record, its
/// download's part, and a replacement waiting or half put in place.
pub fn remove_part(dir: &Path, p: &Part) {
    let at = installed(dir, p);
    let file = dir.join(p.file);
    for f in [
        file.clone(),
        with_suffix(&file, ".part"),
        next_of(&file),
        at.clone(),
    ] {
        let _ = fs::remove_file(&f);
        let _ = fs::remove_file(marker_of(&f));
    }
    for d in [
        at.clone(),
        with_suffix(&at, ".old"),
        with_suffix(&at, ".unpacking"),
    ] {
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_file(marker_of(&d));
    }
}

/// e's refusals forgotten, as when it is deleted.
pub fn forget(dir: &Path, e: &Entry) {
    let mut r = Refusals::read(dir);
    let n = r.0.len();
    r.0.retain(|x| x.model != e.name);
    if r.0.len() != n {
        r.write(dir);
    }
}

/// A model as every platform's Manage models shows it.
#[derive(Clone, Debug, Serialize)]
pub struct View {
    #[serde(flatten)]
    pub entry: &'static Entry,
    /// The edition this device runs (choose).
    pub chosen: &'static str,
    pub runs: Option<RunsView>,
    /// Why the NPU refused the edition that fits best, where it did here.
    pub refused: Option<String>,
    pub offer: Option<OfferView>,
    /// The bytes it takes here, where it runs: its files here. Where
    /// nothing runs, the chosen edition's, which its Download fetches.
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunsView {
    pub edition: &'static str,
    pub model: PathBuf,
    pub encoder: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OfferView {
    pub kind: Kind,
    /// All its files' bytes, and those a download that stopped kept.
    pub bytes: u64,
    pub kept: u64,
    pub files: Vec<&'static str>,
}

/// The view of e in dir for d.
pub fn view(dir: &Path, e: &'static Entry, d: &Device) -> View {
    let r = Refusals::read(dir);
    let chosen = choose(e, d, &r);
    let unrefused = best(e, d, |_| true);
    let refused = r.holds(e, unrefused, d).map(str::to_string);
    let runs = runs(dir, e, d, &r);
    let here: u64 = parts(e)
        .into_iter()
        .filter(|p| have(dir, p).here())
        .map(|p| p.bytes)
        .sum();
    // where nothing runs, a part here (a download stopped, or a refused
    // NPU's) isn't what Download fetches (snappy, 2026-11-08: "12 MB" on
    // the Download of the whole 624 MB file)
    let bytes = if runs.is_some() {
        here
    } else {
        [Some(&chosen.model), chosen.encoder.as_ref()]
            .into_iter()
            .flatten()
            .map(|p| p.bytes)
            .sum()
    };
    View {
        entry: e,
        chosen: chosen.id,
        runs: runs.map(|x| RunsView {
            edition: x.edition.id,
            model: x.model,
            encoder: x.encoder,
        }),
        refused,
        offer: offer(dir, e, d, &r).map(|o| OfferView {
            kind: o.kind,
            bytes: o.bytes,
            kept: o.kept,
            files: o.files.iter().map(|p| p.file).collect(),
        }),
        bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pktito-editions-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn device(system: &str, os: (u32, u32), qnn: Option<&str>) -> Device {
        Device {
            system: system.into(),
            os,
            qnn: qnn.map(str::to_string),
            here: format!("{system} test"),
        }
    }

    /// Every device we know of, as its platform describes it
    fn known() -> Vec<(&'static str, Device)> {
        vec![
            ("an M1-M5 Mac", device("macos", (15, 0), None)),
            ("a Mac on macOS 14", device("macos", (14, 7), None)),
            ("a Mac on macOS 12", device("macos", (12, 0), None)),
            ("an iPhone 16e (A18)", device("ios", (26, 7), None)),
            ("an M1 iPad", device("ios", (26, 5), None)),
            ("the S24", device("android", (36, 0), Some("57,75,8,1,0"))),
            ("the S25", device("android", (36, 0), Some("69,79,8,1,0"))),
            ("the S26", device("android", (36, 0), Some("87,81,8,1,0"))),
            ("an SM8845", device("android", (36, 0), Some("97,81,8,1,0"))),
            (
                "the Googlebook (X1E80100)",
                device("android", (37, 0), Some("60,73,8,1,1")),
            ),
            (
                "an Android phone with no NPU",
                device("android", (34, 0), None),
            ),
            (
                "snappy (X1E, no SoC named)",
                device("windows", (0, 0), Some("2147483647,73,8,1,1")),
            ),
            (
                "an X2 Elite",
                device("windows", (0, 0), Some("2147483647,81,8,1,1")),
            ),
            ("the 7c (no NPU QNN runs)", device("windows", (0, 0), None)),
            ("an x86 PC", device("windows", (0, 0), None)),
            ("a Pi", device("linux", (0, 0), None)),
        ]
    }

    /// No device has two editions at the top: which one is never the
    /// catalog's order
    #[test]
    fn no_edition_ties_another() {
        for e in CATALOG {
            for (what, d) in known() {
                let fits: Vec<u8> = e.editions.iter().map(|ed| fit(ed, &d)).collect();
                let top = *fits.iter().max().unwrap();
                let n = fits.iter().filter(|&&f| f == top).count();
                assert_eq!(n, 1, "{} on {what}: {n} editions fit at {top}", e.name);
            }
        }
    }

    /// Every model has one CPU edition, and every NPU edition the slim file
    #[test]
    fn every_model_has_one_cpu_edition() {
        for e in CATALOG {
            let cpus = e.editions.iter().filter(|ed| !ed.npu()).count();
            assert_eq!(cpus, 1, "{}", e.name);
            assert_eq!(e.whole().file, format!("{}.pkf", e.name));
            for ed in e.editions.iter().filter(|ed| ed.npu()) {
                assert_eq!(ed.model.file, format!("{}-slim.pkf", e.name), "{}", ed.id);
            }
            let mut ids: Vec<&str> = e.editions.iter().map(|ed| ed.id).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), e.editions.len(), "{}: an id twice", e.name);
        }
    }

    /// What each device gets
    #[test]
    fn each_device_gets_its_edition() {
        let r = Refusals::default();
        for e in CATALOG {
            let got: Vec<(&str, &str)> = known()
                .iter()
                .map(|(what, d)| (*what, choose(e, d, &r).id))
                .collect();
            let want = [
                "coreml",
                "cpu",
                "cpu",
                "coreml",
                "coreml",
                "qnn-sm8650",
                "qnn-sm8750",
                "qnn-sm8850",
                // another V81 phone takes the S26's, never the X2's
                "qnn-sm8850",
                "qnn-x1e80100",
                "cpu",
                "qnn-x1e80100",
                "qnn-sc8480xp",
                "cpu",
                "cpu",
                "cpu",
            ];
            for ((what, id), want) in got.iter().zip(want) {
                assert_eq!(*id, want, "{} on {what}", e.name);
            }
        }
        // a Mac below the floor, an iPhone below iOS 18; no phone's graph on
        // Windows, nor one where the Hexagon has less VTCM than it was built for
        let v2 = &CATALOG[0];
        for d in [
            device("macos", (11, 0), None),
            device("ios", (17, 0), None),
            device("windows", (0, 0), Some("2147483647,75,8,1,1")),
            device("windows", (0, 0), Some("2147483647,73,4,1,1")),
            device("linux", (0, 0), Some("60,73,8,1,1")),
            device("android", (36, 0), Some("snapdragon")),
        ] {
            assert_eq!(choose(v2, &d, &r).id, "cpu", "{d:?}");
        }
    }

    /// A refusal holds for its device's here and its encoder: another app,
    /// system or driver, or a rebuilt encoder, tries the NPU again
    #[test]
    fn a_refusal_is_of_one_here_and_encoder() {
        let dir = scratch("refusal");
        let v2 = &CATALOG[0];
        let s24 = device("android", (36, 0), Some("57,75,8,1,0"));
        let g = choose(v2, &s24, &Refusals::default());
        let graph = g.encoder.as_ref().unwrap();
        put(&dir, graph, Some(graph.sha256));
        refuse(&dir, v2, g, &s24, "QNN: no");
        let r = Refusals::read(&dir);
        assert_eq!(choose(v2, &s24, &r).id, "cpu");
        assert_eq!(r.holds(v2, g, &s24), Some("QNN: no"));
        let updated = Device {
            here: "android test, the next build".into(),
            ..s24.clone()
        };
        assert_eq!(choose(v2, &updated, &r).id, g.id);
        let mut rebuilt = Refusals::read(&dir);
        rebuilt.0[0].encoder = "0".repeat(64);
        assert_eq!(choose(v2, &s24, &rebuilt).id, g.id);
        // the view says why, apart from a model never downloaded
        let v = view(&dir, v2, &s24);
        assert_eq!(v.refused.as_deref(), Some("QNN: no"));
        assert!(v.runs.is_none());
        assert_eq!(v.offer.unwrap().kind, Kind::Download);
        forget(&dir, v2);
        assert!(Refusals::read(&dir).0.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A refusal recorded with no whole .pkf here leaves no file Manage
    /// models can't show, and the view's size is the Download's; beside a
    /// whole file the slim one waits for a retry. A run spends a refusal.
    #[test]
    fn a_refusal_with_nothing_else_here_leaves_nothing() {
        let dir = scratch("refusal-alone");
        let v2 = &CATALOG[0];
        let s24 = device("android", (36, 0), Some("57,75,8,1,0"));
        let g = choose(v2, &s24, &Refusals::default());
        let (slim, graph) = (&g.model, g.encoder.as_ref().unwrap());
        put(&dir, slim, Some(slim.sha256));
        put(&dir, graph, Some(graph.sha256));
        refuse(&dir, v2, g, &s24, "QNN: no");
        assert!(!dir.join(slim.file).exists() && !dir.join(graph.file).exists());
        let v = view(&dir, v2, &s24);
        assert!(v.runs.is_none());
        assert_eq!(v.refused.as_deref(), Some("QNN: no"));
        assert_eq!(v.bytes, v2.whole().bytes);
        let o = v.offer.unwrap();
        assert_eq!(
            (o.kind, o.files.clone()),
            (Kind::Download, vec![v2.whole().file])
        );
        // beside the whole file, the slim one stays
        let whole = v2.whole();
        put(&dir, whole, Some(whole.sha256));
        put(&dir, slim, Some(slim.sha256));
        put(&dir, graph, Some(graph.sha256));
        refuse(&dir, v2, g, &s24, "QNN: no");
        assert!(dir.join(slim.file).exists() && !dir.join(graph.file).exists());
        let v = view(&dir, v2, &s24);
        assert_eq!(v.runs.unwrap().edition, "cpu");
        assert_eq!(v.bytes, whole.bytes + slim.bytes);
        // the NPU's edition runs again after an update: its refusal goes
        assert!(!Refusals::read(&dir).0.is_empty());
        put(&dir, graph, Some(graph.sha256));
        ran(&dir, v2, g, &dir.join(slim.file));
        assert!(Refusals::read(&dir).0.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// An Older encoder refused is deleted and not recorded: the catalog's
    /// is offered again, and tried
    #[test]
    fn an_older_encoder_refused_blocks_nothing() {
        let dir = scratch("older-refused");
        let v2 = &CATALOG[0];
        let s24 = device("android", (36, 0), Some("57,75,8,1,0"));
        let g = choose(v2, &s24, &Refusals::default());
        let (slim, graph) = (&g.model, g.encoder.as_ref().unwrap());
        put(&dir, slim, Some(slim.sha256));
        put(&dir, graph, Some(&"0".repeat(64)));
        refuse(&dir, v2, g, &s24, "QNN: no");
        assert!(Refusals::read(&dir).0.is_empty());
        assert!(!dir.join(graph.file).exists());
        assert_eq!(choose(v2, &s24, &Refusals::read(&dir)).id, g.id);
        let o = offer(&dir, v2, &s24, &Refusals::read(&dir)).unwrap();
        assert_eq!((o.kind, o.files[0].file), (Kind::Download, graph.file));
        let _ = fs::remove_dir_all(&dir);
    }

    /// A file of e's here as a download leaves it: of its size (sparse),
    /// with the record given
    fn put(dir: &Path, p: &Part, sha: Option<&str>) {
        let f = dir.join(p.file);
        File::create(&f).unwrap().set_len(p.bytes).unwrap();
        if let Some(sha) = sha {
            fs::write(marker_of(&f), format!("{sha}\n")).unwrap();
        }
    }

    /// A Core ML encoder unpacked, with the record given
    fn put_encoder(dir: &Path, p: &Part, sha: Option<&str>) {
        let at = installed(dir, p);
        fs::create_dir_all(&at).unwrap();
        fs::write(at.join("model.mil"), b"program").unwrap();
        if let Some(sha) = sha {
            fs::write(marker_of(&at), format!("{sha}\n")).unwrap();
        }
    }

    #[test]
    fn a_files_record_says_whether_it_is_the_catalogs() {
        let dir = scratch("states");
        let v2 = &CATALOG[0];
        let (whole, mac) = (v2.whole(), device("macos", (15, 0), None));
        let coreml = choose(v2, &mac, &Refusals::default());
        let enc = coreml.encoder.as_ref().unwrap();
        assert_eq!(have(&dir, whole), Have::Absent);
        put(&dir, whole, Some(whole.sha256));
        assert_eq!(have(&dir, whole), Have::Current);
        put(&dir, whole, Some(&"0".repeat(64)));
        assert_eq!(have(&dir, whole), Have::Older);
        // no record, the catalog's size: taken for it until adopt reads it
        fs::remove_file(marker_of(&dir.join(whole.file))).unwrap();
        assert_eq!(have(&dir, whole), Have::Current);
        // an encoder folder with no record: 0.9.5's, Older
        put_encoder(&dir, enc, None);
        assert_eq!(have(&dir, enc), Have::Older);
        put_encoder(&dir, enc, Some(enc.sha256));
        assert_eq!(have(&dir, enc), Have::Current);
        let _ = fs::remove_dir_all(&dir);
    }

    /// adopt gives a file with no record its own SHA-256: the catalog's
    /// file becomes Current, any other Older
    #[test]
    fn adopt_reads_a_file_with_no_record_once() {
        let dir = scratch("adopt");
        let v2 = &CATALOG[0];
        let slim = &v2.editions[1].model;
        put(&dir, slim, None);
        assert_eq!(have(&dir, slim), Have::Current);
        adopt(&dir);
        let zeros = format!("{:x}", Sha256::digest(vec![0u8; slim.bytes as usize]));
        assert_eq!(record(&dir.join(slim.file)), Some(zeros));
        assert_eq!(have(&dir, slim), Have::Older);
        // 0.9.6's refusals go
        fs::write(dir.join("parakeet-tdt-0.6b-v2-x1e80100.qnn.refused"), "x").unwrap();
        adopt(&dir);
        assert!(!dir
            .join("parakeet-tdt-0.6b-v2-x1e80100.qnn.refused")
            .exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn offers_download_faster_and_update() {
        let dir = scratch("offers");
        let v2 = &CATALOG[0];
        let s25 = device("android", (36, 0), Some("69,79,8,1,0"));
        let r = Refusals::default();
        let npu = choose(v2, &s25, &r);
        let (slim, graph) = (&npu.model, npu.encoder.as_ref().unwrap());
        let o = offer(&dir, v2, &s25, &r).unwrap();
        assert_eq!(o.kind, Kind::Download);
        assert_eq!(o.bytes, slim.bytes + graph.bytes);
        // the whole file here: Faster, the slim file and the graph
        put(&dir, v2.whole(), Some(v2.whole().sha256));
        assert_eq!(runs(&dir, v2, &s25, &r).unwrap().edition.id, "cpu");
        let o = offer(&dir, v2, &s25, &r).unwrap();
        assert_eq!(o.kind, Kind::Faster);
        assert_eq!(
            o.files.iter().map(|p| p.file).collect::<Vec<_>>(),
            [slim.file, graph.file]
        );
        // both here: the NPU's runs from its own slim file, nothing offered
        put(&dir, slim, Some(slim.sha256));
        put(&dir, graph, Some(graph.sha256));
        let now = runs(&dir, v2, &s25, &r).unwrap();
        assert_eq!((now.edition.id, now.model_part.file), (npu.id, slim.file));
        assert!(offer(&dir, v2, &s25, &r).is_none());
        // a rebuilt graph in the catalog: Update, the graph alone
        put(&dir, graph, Some(&"0".repeat(64)));
        let o = offer(&dir, v2, &s25, &r).unwrap();
        assert_eq!(
            (o.kind, o.files[0].file, o.bytes),
            (Kind::Update, graph.file, graph.bytes)
        );
        // its download lands beside the old one, which runs until settle
        let next = landing(&dir, graph);
        assert_eq!(next, dir.join(format!("{}.next", graph.file)));
        File::create(&next).unwrap().set_len(graph.bytes).unwrap();
        fs::write(marker_of(&next), format!("{}\n", graph.sha256)).unwrap();
        assert!(pending(&dir, v2));
        assert!(offer(&dir, v2, &s25, &r).is_none());
        assert_eq!(have(&dir, graph), Have::Older);
        settle(&dir, v2).unwrap();
        assert!(!pending(&dir, v2));
        assert_eq!(have(&dir, graph), Have::Current);
        assert!(!next.exists());
        // run once on the NPU from its slim file: the whole file goes
        assert!(ran(&dir, v2, npu, &dir.join(slim.file)));
        assert_eq!(have(&dir, v2.whole()), Have::Absent);
        assert_eq!(runs(&dir, v2, &s25, &r).unwrap().edition.id, npu.id);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A Mac or iPhone that put an encoder beside the whole file before
    /// 0.9.7 runs the Neural Engine from it, keeps it, and is offered the
    /// 0.9.6 encoder in place of 0.9.5's
    #[test]
    fn an_encoder_beside_the_whole_file_is_updated_and_kept() {
        let dir = scratch("mac-whole");
        let v2 = &CATALOG[0];
        let mac = device("macos", (15, 0), None);
        let r = Refusals::default();
        let coreml = choose(v2, &mac, &r);
        let enc = coreml.encoder.as_ref().unwrap();
        put(&dir, v2.whole(), None);
        put_encoder(&dir, enc, None);
        let now = runs(&dir, v2, &mac, &r).unwrap();
        assert_eq!(
            (now.edition.id, now.model_part.file),
            ("coreml", v2.whole().file)
        );
        let o = offer(&dir, v2, &mac, &r).unwrap();
        assert_eq!(
            (o.kind, o.files.len(), o.files[0].file),
            (Kind::Update, 1, enc.file)
        );
        assert!(!ran(&dir, v2, coreml, &dir.join(v2.whole().file)));
        assert!(dir.join(v2.whole().file).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A file swap cut short between the file's rename and its record's:
    /// the record follows it at the next start
    #[test]
    fn a_file_swap_cut_short_gets_its_record() {
        let dir = scratch("recover-file");
        let graph = CATALOG[0].editions[2].encoder.as_ref().unwrap();
        put(&dir, graph, Some(&"0".repeat(64)));
        let next = next_of(&dir.join(graph.file));
        fs::write(marker_of(&next), format!("{}\n", graph.sha256)).unwrap();
        // read as it is even before recover runs, so that no first read
        // offers it again
        assert_eq!(have(&dir, graph), Have::Current);
        recover(&dir);
        assert_eq!(have(&dir, graph), Have::Current);
        assert!(!marker_of(&next).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// An encoder swap cut short: the old folder comes back where the new
    /// one isn't whole, and goes where it is
    #[test]
    fn a_swap_cut_short_is_put_right() {
        let dir = scratch("recover");
        let enc = CATALOG[0].editions[1].encoder.as_ref().unwrap();
        let at = installed(&dir, enc);
        let old = with_suffix(&at, ".old");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("model.mil"), b"old").unwrap();
        recover(&dir);
        assert_eq!(fs::read(at.join("model.mil")).unwrap(), b"old");
        fs::create_dir_all(&old).unwrap();
        recover(&dir);
        assert!(!old.exists() && at.join("model.mil").is_file());
        let _ = fs::remove_dir_all(&dir);
    }
}

//! Compiles pktito (vendor/pktito, scripts/vendor-pktito.sh) the way its
//! Makefile does: the core, ref and the dispatcher at the family's baseline,
//! and each SIMD backend of the target's family with its own instruction
//! set, so one binary runs on every CPU of the family and picks the fastest
//! backend it can (pktito's backend/kernels.c). Then the platform's decoder
//! (AVFoundation on macOS and iOS, Media Foundation on Windows) and the mix
//! that every decoder hands its samples to (native/mix.h). On the Mac and
//! iOS the encoder can also run on the Neural Engine through Core ML
//! (pktito's npu/ane.m, as its Makefile's ANE=1 builds it). On Android and
//! Windows on Arm it can run on a Snapdragon's NPU through QNN (npu/qnn.c,
//! QNN=1), in a build with QNN_SDK set. Either reads the slim model file,
//! whose only encoder is the NPU's; elsewhere, and without QNN_SDK, pktito
//! runs on the CPU alone.
//!
//! It also writes the catalog's editions from pktito's models.json, which
//! model.rs includes (catalog, below).
//!
//! Everything goes into one archive, and the system's libraries come after
//! it. GNU ld reads each archive once, left to right, taking only what
//! resolves a reference it has already seen; within one archive it keeps
//! resolving until nothing new turns up.

use std::env;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const CORE: &[&str] = &[
    "model.c",
    "os.c",
    "pool.c",
    "mel.c",
    "encoder.c",
    "decoder.c",
    "tokens.c",
    "asr.c",
    "transcribe.c",
    "vad.c",
    "segment.c",
    "wav.c",
    "resample.c",
    "captions.c",
    "language.c",
    "sentences.c",
];

/// The instructions a backend's assembly names for itself, which its flags
/// say nothing of, as pktito's Makefile probes its compiler for them
/// (PROBE_<id>): neon-sve's SVE and I8MM, neon-i8mm's I8MM.
fn probe(name: &str) -> Option<&'static str> {
    match name {
        "neon-sve" => Some(
            r".arch_extension sve\n.arch_extension i8mm\nmovprfx z0, z1\nsmmla z0.s, z1.b, z2.b",
        ),
        "neon-i8mm" => Some(r".arch_extension i8mm\nsmmla v0.4s, v1.16b, v2.16b"),
        _ => None,
    }
}

/// Whether this compiler's assembler takes a backend's probe, under the
/// backend's flags; a backend it doesn't is left out.
fn assembles(os: &str, root: &PathBuf, name: &str, flags: &[&str], asm: &str) -> bool {
    let dir = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(format!("probe-{name}"));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("probe.c");
    let mut b = base(os, root);
    for f in flags {
        b.flag(f);
    }
    std::fs::write(
        &file,
        format!("void pk_probe(void) {{ __asm__(\"{asm}\"); }}\n"),
    )
    .is_ok()
        && b.out_dir(&dir)
            .file(&file)
            .cargo_warnings(false)
            .try_compile_intermediates()
            .is_ok()
}

/// The catalog's editions, from pktito's models.json, which `pktito-c
/// models` reads too, so the library and the app offer the same files. For
/// each model a const of its editions in models.json's order, named for it
/// in capitals (parakeet-tdt-0.6b-v2's PARAKEET_TDT_0_6B_V2), which
/// model.rs's CATALOG gives with what the window says of the model; MODELS,
/// every model's name, which a test holds CATALOG to; and DEFAULT_BASE,
/// where the files are. A runtime this doesn't know stops the build, since
/// the app would offer an edition it can't run.
fn catalog(json: &Path, out: &Path) {
    use serde_json::Value;
    let text = std::fs::read_to_string(json).expect("pktito's models.json");
    let v: Value = serde_json::from_str(&text).expect("models.json isn't JSON");
    let s = |v: &Value, what: &str| -> String {
        v.as_str()
            .unwrap_or_else(|| panic!("models.json: {what} isn't a string"))
            .to_string()
    };
    let part = |p: &Value| {
        let bytes = p["bytes"].as_u64().expect("models.json: a file's bytes");
        let (file, sha) = (s(&p["file"], "a file"), s(&p["sha256"], "a sha256"));
        format!("Part {{ file: {file:?}, bytes: {bytes}, sha256: {sha:?} }}")
    };
    let version = |v: &Value| {
        let n: Vec<u64> = v
            .as_array()
            .expect("models.json: a system's version")
            .iter()
            .map(|n| n.as_u64().expect("models.json: a version's number"))
            .collect();
        format!("({}, {})", n[0], n[1])
    };
    let mut rs = String::new();
    let _ = writeln!(
        rs,
        "// build.rs's, from pktito's models.json: change that, not this\n"
    );
    let _ = writeln!(
        rs,
        "pub const DEFAULT_BASE: &str = {:?};\n",
        s(&v["base"], "base")
    );
    let mut names = Vec::new();
    for m in v["models"].as_array().expect("models.json: models") {
        let name = s(&m["name"], "a model's name");
        let ident: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect();
        let _ = writeln!(rs, "pub const {ident}: &[Edition] = &[");
        for ed in m["editions"].as_array().expect("models.json: editions") {
            let id = s(&ed["id"], "an edition's id");
            let r = &ed["runtime"];
            let runtime = match r["kind"].as_str() {
                Some("cpu") => "Runtime::Cpu".to_string(),
                Some("coreml") => format!(
                    "Runtime::CoreMl {{ macos: {}, ios: {} }}",
                    version(&r["macos"]),
                    version(&r["ios"])
                ),
                Some("qnn") => {
                    let systems: Vec<String> = r["systems"]
                        .as_array()
                        .expect("models.json: a QNN edition's systems")
                        .iter()
                        .map(|x| s(x, "a system"))
                        .collect();
                    format!(
                        "Runtime::Qnn {{ chip: {:?}, systems: &{systems:?} }}",
                        s(&r["chip"], "a chip")
                    )
                }
                k => panic!("models.json: {name}'s {id} runs on {k:?}, which the app doesn't know"),
            };
            let encoder = ed
                .get("encoder")
                .map_or("None".to_string(), |p| format!("Some({})", part(p)));
            let _ = writeln!(
                rs,
                "    // {}\n    Edition {{ id: {id:?}, runtime: {runtime}, model: {}, encoder: {encoder} }},",
                s(&ed["about"], "an edition's about"),
                part(&ed["model"]),
            );
        }
        let _ = writeln!(rs, "];\n");
        names.push(name);
    }
    let _ = writeln!(rs, "#[cfg(test)]\npub const MODELS: &[&str] = &{names:?};");
    std::fs::write(out, rs).expect("OUT_DIR/catalog.rs");
}

fn base(os: &str, root: &PathBuf) -> cc::Build {
    let mut b = cc::Build::new();
    // pktito's numbers are the same bits on every backend only without
    // contraction, and it is slow without optimization, debug build or not.
    // Without errno, sqrtf is the CPU's square root instruction (the same
    // correctly rounded bits) and not a call into libm, whose sqrtf alone
    // held a build on Ubuntu 26.04 to GLIBC_2.43
    b.std("c11")
        .opt_level(3)
        .debug(false)
        .flag("-ffp-contract=off")
        .flag("-fno-math-errno")
        .warnings(false)
        .include(root.join("core"))
        .include(root.join("backend"))
        // the NPU encoders' headers, which core/asr.c and core/encoder.c
        // include in every build, as the Makefile's -Inpu
        .include(root.join("npu"));
    match os {
        "macos" | "ios" => {
            b.define("_DARWIN_C_SOURCE", None).define("PK_ANE", None);
        }
        "windows" => {
            b.define("_WIN32_WINNT", "0x0A00")
                .define("__USE_MINGW_ANSI_STDIO", "1");
        }
        _ => {
            b.define("_DEFAULT_SOURCE", None)
                .define("_POSIX_C_SOURCE", "200809L");
        }
    }
    b
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let root = manifest.join("../vendor/pktito");
    println!("cargo:rerun-if-changed=../vendor/pktito");
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-env-changed=QNN_SDK");
    catalog(
        &root.join("models.json"),
        &PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("catalog.rs"),
    );
    // Qualcomm's AI Runtime SDK, where QNN_SDK names it: the encoder on a
    // Snapdragon's NPU too (npu/qnn.c), on Android's Arm64 and Windows's.
    // Only its headers are read here; its libraries are loaded at run time
    // (dlopen; LoadLibraryEx on Windows) from the app's own native ones, or
    // the DLLs beside the exe
    let qnn = env::var_os("QNN_SDK")
        .filter(|_| (os == "android" || os == "windows") && arch == "aarch64")
        .map(PathBuf::from);

    let family: Vec<(&str, Vec<&str>)> = match arch.as_str() {
        // neon-dp's three variants, each its C with another int8 product,
        // on its flags as pktito's Makefile builds them (pktito's
        // backend/neon-dp/CLIMB.md, by date):
        // - neon-sve, a loop of SVE and I8MM assembly (SMMLA): 15 to 16%
        //   over neon-dp on the GB10's ten X925s, 24% on one (2026-11-02);
        //   run only where the system says the CPU has both (core/os.c,
        //   cpu_has).
        // - neon-i8mm, that loop without SVE: test.wav 12.4% the quicker on
        //   the S24 and 21.5% on the S25 (2026-11-07).
        // - neon-dp-a76, SDOT in the Cortex-A76's tile: 8.9% on the Pi 5,
        //   7.0% on Graviton2 (2026-11-08).
        // The last two run only where a probe of the product, on all of
        // the fastest cores at once, finds them 3% the faster, once, by the
        // first model load (backend/kernels.c). Apple's chips get none of
        // the three: they have no SVE, and the probe keeps neon-dp there,
        // the M4's neon-i8mm at 1.119 of its time and neon-dp-a76 at 0.976
        // to 0.977 (2026-11-08), the M5 Ultra's neon-i8mm at 1.173 to 1.190
        // (2026-11-07).
        "aarch64" if os != "macos" && os != "ios" => vec![
            ("neon-sve", vec!["-march=armv8.2-a+dotprod"]),
            ("neon-i8mm", vec!["-march=armv8.2-a+dotprod"]),
            ("neon-dp-a76", vec!["-march=armv8.2-a+dotprod"]),
            ("neon-dp", vec!["-march=armv8.2-a+dotprod"]),
            ("neon", vec!["-march=armv8-a"]),
        ],
        "aarch64" => vec![
            (
                "neon-dp",
                if os == "macos" {
                    vec!["-mcpu=apple-m1"]
                } else {
                    vec!["-march=armv8.2-a+dotprod"]
                },
            ),
            ("neon", vec!["-march=armv8-a"]),
        ],
        "x86_64" => vec![
            (
                "amx-int8",
                vec![
                    "-mavx2",
                    "-mfma",
                    "-mavx512f",
                    "-mavx512vnni",
                    "-mavx512vl",
                    "-mamx-tile",
                    "-mamx-int8",
                ],
            ),
            (
                "avx512-vnni",
                vec!["-mavx2", "-mfma", "-mavx512f", "-mavx512vnni", "-mavx512vl"],
            ),
            ("avx2-vnni", vec!["-mavx2", "-mfma", "-mavxvnni"]),
            ("avx2", vec!["-mavx2", "-mfma"]),
        ],
        _ => vec![],
    };

    // the backends this compiler can build, as pktito's Makefile asks
    let mut objects = Vec::new();
    let mut built = Vec::new();
    for (name, flags) in &family {
        let mut b = base(&os, &root);
        if flags
            .iter()
            .all(|f| b.is_flag_supported(f).unwrap_or(false))
            && probe(name).map_or(true, |asm| assembles(&os, &root, name, flags, asm))
        {
            for f in flags {
                b.flag(f);
            }
            b.file(root.join("backend").join(name).join("kernels.c"));
            objects.extend(b.compile_intermediates());
            built.push(*name);
        } else {
            println!("cargo:warning=pktito: this compiler cannot build {name}; left out");
        }
    }

    // the mix every decoder's samples go through, ffmpeg's too
    objects.extend(
        base(&os, &root)
            .file(manifest.join("native/mix.c"))
            .compile_intermediates(),
    );
    match os.as_str() {
        // the Mac's decoder, which iOS's AVFoundation runs as it is, and the
        // encoder on the Neural Engine
        "macos" | "ios" => {
            objects.extend(
                cc::Build::new()
                    .file(manifest.join("native/decode_macos.m"))
                    .flag("-fobjc-arc")
                    .opt_level(2)
                    .compile_intermediates(),
            );
            objects.extend(
                base(&os, &root)
                    .file(root.join("npu/ane.m"))
                    .flag("-fobjc-arc")
                    .compile_intermediates(),
            );
        }
        "windows" => objects.extend(
            base(&os, &root)
                .file(manifest.join("native/decode_windows.c"))
                .compile_intermediates(),
        ),
        "android" => objects.extend(
            base(&os, &root)
                .file(manifest.join("native/decode_android.c"))
                .compile_intermediates(),
        ),
        _ => {}
    }

    let mut core = base(&os, &root);
    for f in CORE {
        core.file(root.join("core").join(f));
    }
    core.file(root.join("backend/ref/kernels.c"))
        .file(root.join("backend/kernels.c"));
    if let Some(sdk) = &qnn {
        core.file(root.join("npu/qnn.c"))
            .define("PK_QNN", None)
            .include(sdk.join("include/QNN"));
    }
    for name in &built {
        core.define(&format!("PK_HAVE_{}", name.replace('-', "_")), None);
    }
    for o in &objects {
        core.object(o);
    }
    core.compile("pktito");

    // after the archive, for the same reason
    let system: &[&str] = match os.as_str() {
        "macos" | "ios" => &[
            "framework=AVFoundation",
            "framework=AudioToolbox",
            "framework=CoreMedia",
            "framework=CoreML",
            "framework=Foundation",
        ],
        // Media Foundation's DLLs are loaded when a file needs them
        // (native/decode_windows.c), so the exe starts on a Windows without
        // them; mfuuid is its GUIDs, a static library
        "windows" => &["mfuuid", "ole32", "uuid"],
        // bionic holds pthreads itself, and has no libpthread to link;
        // mediandk is the NDK's media framework (native/decode_android.c)
        "android" => &["m", "mediandk"],
        _ => &["pthread", "m"],
    };
    for lib in system {
        println!("cargo:rustc-link-lib={lib}");
    }
    if qnn.is_some() && os == "android" {
        println!("cargo:rustc-link-lib=dl");
    }
    // the engine's pool runs on pthreads, which Rust's std brings along on
    // the GNU targets and not on the LLVM ones (the Arm64 build's
    // aarch64-pc-windows-gnullvm): winpthreads, linked in, since a plain -l
    // took libwinpthread-1.dll, which Windows does not have. Not bundled into
    // the library crate (src/lib.rs): rustc would have to find libpthread.a
    // itself, and only the linker knows llvm-mingw's lib folder
    if os == "windows" && env::var("CARGO_CFG_TARGET_ABI").as_deref() == Ok("llvm") {
        println!("cargo:rustc-link-lib=static:-bundle=pthread");
    }

    // Tauri's own manifest, and the process's code page UTF-8 (Windows 10
    // 1903 and later): the engine opens the model, and its reader a WAV,
    // with CreateFileA and fopen on the UTF-8 path Rust hands it, which in
    // the old code page names another file, or none, past ASCII - a
    // %APPDATA% under C:\Users\José. Android and iOS have no Tauri (Cargo.toml)
    if os == "android" || os == "ios" {
        return;
    }
    let windows =
        tauri_build::WindowsAttributes::new().app_manifest(include_str!("windows/app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("tauri-build");
}

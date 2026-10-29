//! Compiles pktito (vendor/pktito, scripts/vendor-pktito.sh) the way its
//! Makefile does: the core, ref and the dispatcher at the family's baseline,
//! and each SIMD backend of the target's family with its own instruction
//! set, so one binary runs on every CPU of the family and picks the fastest
//! backend it can (pktito's backend/kernels.c). Then the platform's decoder
//! (AVFoundation on macOS, Media Foundation on Windows) and the mix that
//! every decoder hands its samples to (native/mix.h).
//!
//! Everything goes into one archive, and the system's libraries come after
//! it. GNU ld reads each archive once, left to right, taking only what
//! resolves a reference it has already seen; within one archive it keeps
//! resolving until nothing new turns up.

use std::env;
use std::path::PathBuf;

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
];

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
        .include(root.join("backend"));
    match os {
        "macos" => {
            b.define("_DARWIN_C_SOURCE", None);
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
    let root = manifest.join("../vendor/pktito");
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    println!("cargo:rerun-if-changed=../vendor/pktito");
    println!("cargo:rerun-if-changed=native");

    let family: Vec<(&str, Vec<&str>)> = match arch.as_str() {
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
        "macos" => objects.extend(
            cc::Build::new()
                .file(manifest.join("native/decode_macos.m"))
                .flag("-fobjc-arc")
                .opt_level(2)
                .compile_intermediates(),
        ),
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
    for name in &built {
        core.define(&format!("PK_HAVE_{}", name.replace('-', "_")), None);
    }
    for o in &objects {
        core.object(o);
    }
    core.compile("pktito");

    // after the archive, for the same reason
    let system: &[&str] = match os.as_str() {
        "macos" => &[
            "framework=AVFoundation",
            "framework=AudioToolbox",
            "framework=CoreMedia",
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
    // %APPDATA% under C:\Users\José. Android has no Tauri (Cargo.toml)
    if os == "android" {
        return;
    }
    let windows =
        tauri_build::WindowsAttributes::new().app_manifest(include_str!("windows/app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("tauri-build");
}

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
    "model.c", "os.c", "pool.c", "mel.c", "encoder.c", "decoder.c", "tokens.c", "asr.c", "vad.c",
    "segment.c", "wav.c", "resample.c",
];

fn base(os: &str, root: &PathBuf) -> cc::Build {
    let mut b = cc::Build::new();
    // pktito's numbers are the same bits on every backend only without
    // contraction, and it is slow without optimization, debug build or not
    b.std("c11")
        .opt_level(3)
        .debug(false)
        .flag("-ffp-contract=off")
        .warnings(false)
        .include(root.join("core"))
        .include(root.join("backend"));
    match os {
        "macos" => {
            b.define("_DARWIN_C_SOURCE", None);
        }
        "windows" => {
            b.define("_WIN32_WINNT", "0x0A00").define("__USE_MINGW_ANSI_STDIO", "1");
        }
        _ => {
            b.define("_DEFAULT_SOURCE", None).define("_POSIX_C_SOURCE", "200809L");
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
            ("neon-dp", if os == "macos" { vec!["-mcpu=apple-m1"] } else { vec!["-march=armv8.2-a+dotprod"] }),
            ("neon", vec!["-march=armv8-a"]),
        ],
        "x86_64" => vec![
            ("avx2-vnni", vec!["-mavx2", "-mfma", "-mavxvnni"]),
            ("avx512-vnni", vec!["-mavx2", "-mfma", "-mavx512vnni", "-mavx512vl"]),
            ("avx2", vec!["-mavx2", "-mfma"]),
        ],
        _ => vec![],
    };

    // the backends this compiler can build, as pktito's Makefile asks
    let mut objects = Vec::new();
    let mut built = Vec::new();
    for (name, flags) in &family {
        let mut b = base(&os, &root);
        if flags.iter().all(|f| b.is_flag_supported(f).unwrap_or(false)) {
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
    objects.extend(base(&os, &root).file(manifest.join("native/mix.c")).compile_intermediates());
    match os.as_str() {
        "macos" => objects.extend(
            cc::Build::new()
                .file(manifest.join("native/decode_macos.m"))
                .flag("-fobjc-arc")
                .opt_level(2)
                .compile_intermediates(),
        ),
        "windows" => {
            objects.extend(base(&os, &root).file(manifest.join("native/decode_windows.c")).compile_intermediates())
        }
        _ => {}
    }

    let mut core = base(&os, &root);
    for f in CORE {
        core.file(root.join("core").join(f));
    }
    core.file(root.join("backend/ref/kernels.c")).file(root.join("backend/kernels.c"));
    for name in &built {
        core.define(&format!("PK_HAVE_{}", name.replace('-', "_")), None);
    }
    for o in &objects {
        core.object(o);
    }
    core.compile("pktito");

    // after the archive, for the same reason
    let system: &[&str] = match os.as_str() {
        "macos" => &["framework=AVFoundation", "framework=AudioToolbox", "framework=CoreMedia", "framework=Foundation"],
        "windows" => &["mfplat", "mfreadwrite", "mfuuid", "ole32", "uuid"],
        _ => &["pthread", "m"],
    };
    for lib in system {
        println!("cargo:rustc-link-lib={lib}");
    }

    tauri_build::build();
}

//! Compiles pktito (vendor/pktito, scripts/vendor-pktito.sh) the way its
//! Makefile does: the core, ref and the dispatcher in one archive, and each
//! SIMD backend of the target's family in an archive of its own with its own
//! instruction set, so one binary runs on every CPU of the family and picks
//! the fastest backend it can (pktito's backend/kernels.c). Then the
//! platform's decoder: AVFoundation on macOS, Media Foundation on Windows.

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
    let mut built = Vec::new();
    for (name, flags) in &family {
        let mut probe = base(&os, &root);
        if flags.iter().all(|f| probe.is_flag_supported(f).unwrap_or(false)) {
            for f in flags {
                probe.flag(f);
            }
            probe.file(root.join("backend").join(name).join("kernels.c"));
            probe.compile(&format!("pktito_{}", name.replace('-', "_")));
            built.push(*name);
        } else {
            println!("cargo:warning=pktito: this compiler cannot build {name}; left out");
        }
    }

    let mut core = base(&os, &root);
    for f in CORE {
        core.file(root.join("core").join(f));
    }
    core.file(root.join("backend/ref/kernels.c")).file(root.join("backend/kernels.c"));
    for name in &built {
        core.define(&format!("PK_HAVE_{}", name.replace('-', "_")), None);
    }
    core.compile("pktito");

    match os.as_str() {
        "macos" => {
            cc::Build::new()
                .file(manifest.join("native/decode_macos.m"))
                .flag("-fobjc-arc")
                .opt_level(2)
                .compile("pkapp_decode");
            for fw in ["AVFoundation", "CoreMedia", "Foundation"] {
                println!("cargo:rustc-link-lib=framework={fw}");
            }
        }
        "windows" => {
            let mut b = base(&os, &root);
            b.file(manifest.join("native/decode_windows.c")).compile("pkapp_decode");
            for lib in ["mfplat", "mfreadwrite", "mfuuid", "ole32"] {
                println!("cargo:rustc-link-lib={lib}");
            }
        }
        _ => {}
    }
    if os == "linux" {
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
    }

    tauri_build::build();
}

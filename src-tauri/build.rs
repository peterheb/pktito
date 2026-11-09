//! Compiles pktito (vendor/pktito, scripts/vendor-pktito.sh) the way its
//! Makefile does: the core, ref and the dispatcher at the family's baseline,
//! and each SIMD backend of the target's family with its own instruction
//! set, so one binary runs on every CPU of the family and picks the fastest
//! backend it can (pktito's backend/kernels.c). Then the platform's decoder
//! (AVFoundation on macOS and iOS, Media Foundation on Windows) and the mix
//! that every decoder hands its samples to (native/mix.h). On the Mac and
//! iOS pktito is its ane fork (vendor/pktito-ane), whose encoder can run on
//! the Neural Engine through Core ML (core/ane.m), and runs on the CPU as
//! pktito's does where it isn't given one. On Android it is that fork with
//! the qnn branch's (vendor/pktito-qnn), whose encoder can run on a
//! Snapdragon's NPU (core/qnn.c), in a build with QNN_SDK set, and which
//! reads the fork's slim model file, whose only encoder is the NPU's graph;
//! without QNN_SDK it runs on the CPU as pktito's does. Windows on Arm is
//! the same: the qnn fork, its NPU where QNN_SDK is set, a Snapdragon X's.
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
    let root = manifest.join(match os.as_str() {
        "macos" | "ios" => "../vendor/pktito-ane",
        "android" => "../vendor/pktito-qnn",
        "windows" if arch == "aarch64" => "../vendor/pktito-qnn",
        _ => "../vendor/pktito",
    });
    println!("cargo:rerun-if-changed=../vendor/pktito");
    println!("cargo:rerun-if-changed=../vendor/pktito-ane");
    println!("cargo:rerun-if-changed=../vendor/pktito-qnn");
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-env-changed=QNN_SDK");
    // Qualcomm's AI Runtime SDK, where QNN_SDK names it: the encoder on a
    // Snapdragon's NPU too (core/qnn.c), on Android's Arm64 and Windows's.
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
        // ane fork's encoder on the Neural Engine
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
                    .file(root.join("core/ane.m"))
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
        core.file(root.join("core/qnn.c"))
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

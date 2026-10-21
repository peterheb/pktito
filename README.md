# pktito, the desktop app

Drop an audio or video file and read its transcript. The audio comes out
through the media libraries the operating system already has, and it is
transcribed on this computer by [pktito](https://pktito.net), a C11 port of
NVIDIA's Parakeet-TDT models. Nothing leaves the computer except the model's
one download.

- **macOS 12 and later**: AVFoundation reads the file.
- **Windows 10 and later**: Media Foundation reads it. (The N editions of
  Windows have no Media Foundation without the Media Feature Pack.)
- **Linux**: there is no one media library, so ffmpeg is required: install
  it with your package manager.

On macOS and Windows, a file the OS cannot read is handed to ffmpeg when
ffmpeg is on the PATH. `PKTITO_DECODER=ffmpeg` uses ffmpeg for everything;
`PKTITO_DECODER=os` the OS alone.

## Formats

One minute of speech in each of these, made by `scripts/media-matrix.sh`
and transcribed by `scripts/media-check.sh`, on macOS 27.0.1 (M4 Pro),
Windows 11 Pro 26200 (Core 3 304) and Ubuntu 26.04 (ffmpeg 8.0.1; 9.0 on
the other two), 2026-10-20:

| File | macOS | Windows | ffmpeg |
|---|:-:|:-:|:-:|
| MP4, MOV, M4A, 3GP: AAC, HE-AAC v1 and v2, ALAC, PCM; H.264, HEVC, ProRes | ✓ | ✓ | ✓ |
| MP3, constant and variable rate | ✓ | ✓ | ✓ |
| WAV: 16 and 24-bit, float, µ-law, IMA ADPCM | ✓ | ✓ | ✓ |
| FLAC, 16-bit 44.1 kHz and 24-bit 96 kHz | ✓ | ✓ | ✓ |
| MPEG-TS (H.264, AAC), MPEG-PS (MPEG-2, MP2) | ✓ | ✓ | ✓ |
| AVI: MJPEG with PCM | ✓ | ✓ | ✓ |
| AC-3 5.1, alone or in M2TS | ✓ | – | ✓ |
| AIFF; CAF with ALAC | ✓ | – | ✓ |
| Ogg Opus and Vorbis, .opus voice messages | ✓ | – | ✓ |
| WebM: Opus or Vorbis, with VP8, VP9 or AV1 | – | ✓ | ✓ |
| Matroska with AAC | – | ✓ | ✓ |
| Matroska with AC-3, E-AC-3 or DTS 5.1 | – | – | ✓ |
| WMA, WMV | – | ✓ | ✓ |
| AVI: MPEG-4 with MP3 | – | ✓ | ✓ |
| FLV; Ogg Theora | – | – | ✓ |
| An MP3 named .m4a | – | ✓ | ✓ |
| An M4A cut off halfway | – | first half | first half |

With ffmpeg on the PATH, every file reads on every system. A file with
several audio tracks gives its first, as all three decoders take it.

Every decoder hands over its samples as they are, at their own rate and in
all their channels, and one mix (`src-tauri/native/mix.h`) brings them to
one channel and 16 kHz with pktito's resampler. So a lossless file gives
the same transcript, byte for byte, through AVFoundation, Media Foundation
and ffmpeg on all three systems, and the same file the same transcript on
every run. (AVFoundation's own resampler did neither: its output changed
from run to run.) Lossy formats differ only as far as their decoders do;
HE-AAC most, by a word or two in a minute.

## The model

On first run the app downloads one model from models.pktito.net, about
624 MB for English (parakeet-tdt-0.6b-v2) or 648 MB for 25 European
languages (parakeet-tdt-0.6b-v3), and checks its size and SHA-256 against
the ones built into the app (src-tauri/src/model.rs) before using it. A
`.pkf` already on the computer can be used instead ("Use a model file on
this computer").

Each file lives under the first twelve digits of its SHA-256, as
`https://models.pktito.net/8bfa6750b83c/parakeet-tdt-0.6b-v2.pkf`, so an
address never serves two different files and a CDN can keep each one
forever. `scripts/models-tree.sh` lays converted models out that way, for
uploading or for a local server; `PKTITO_MODEL_BASE` points the download at
another server, such as a mirror or that local one:

```sh
scripts/models-tree.sh ../pktito/models/*.pkf /tmp/models
(cd /tmp/models && python3 -m http.server 8765) &
PKTITO_MODEL_BASE=http://127.0.0.1:8765/ cargo tauri dev
```

The models are NVIDIA's (CC-BY-4.0), converted by pktito-tools.

## Building

Rust, the Tauri CLI (`cargo install tauri-cli --version "^2" --locked`) and
a C compiler: Xcode's command line tools on macOS, MSYS2's MinGW-w64 gcc on
Windows (with Rust's `x86_64-pc-windows-gnu` toolchain), gcc or clang on
Linux.

```sh
cargo tauri dev             # run it
cargo tauri build           # the installer: .dmg, .msi/.exe, .deb/.AppImage
```

pktito's sources are copied into `vendor/pktito` by
`scripts/vendor-pktito.sh` (`vendor/pktito/VENDORED` says which commit); the
build compiles every SIMD backend of the target's family, and the CPU picks
the fastest it can run, so one build runs on any CPU of its kind.

For checking a build without the window:

```sh
src-tauri/target/debug/pktito-app --transcribe talk.mp4 --model parakeet-tdt-0.6b-v2.pkf
PKTITO_MODEL_BASE=http://127.0.0.1:8765/ src-tauri/target/debug/pktito-app --download parakeet-tdt-0.6b-v2 /tmp/m
```

and for checking the decoders, the formats above from any minute or so of
speech (ffmpeg makes them; on macOS, afconvert adds HE-AAC), then one line
a file: what happened, the seconds decoded, and the word error rate
against the speech's own transcript:

```sh
scripts/media-matrix.sh speech.wav /tmp/matrix
PKTITO_DECODER=os scripts/media-check.sh src-tauri/target/debug/pktito-app parakeet-tdt-0.6b-v2.pkf speech.wav /tmp/matrix
```

## Design

The interface follows the pktito design system: `design/` holds its tokens,
brand book and app icon, `ui/bundle.css` its components, and `ui/fonts`
Archivo and Commit Mono with their SIL Open Font License texts.
`scripts/tokens.py` turns `design/tokens.json` into `ui/tokens.css`; the
icon set in `src-tauri/icons` comes from `cargo tauri icon design/app-icon.svg`.

## License

MIT (LICENSE). pktito is MIT; the fonts are under the SIL Open Font License
1.1; the models are NVIDIA's, under CC-BY-4.0.

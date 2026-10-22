# pktito, the desktop app

Drop audio or video files and read their transcripts as they fill in; then
play one back with the word being spoken underlined, search it, and save it
as text, subtitles, Markdown or JSON. The audio comes out through the media
libraries the operating system already has, and it is transcribed on this
computer by [pktito](https://pktito.net), a C11 port of NVIDIA's
Parakeet-TDT models. Nothing leaves the computer, and the one thing the app
fetches is its model, once.

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
the other two), 2026-10-21:

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

On its first run the window asks for a model, with the one that speaks the
system's language picked: parakeet-tdt-0.6b-v2 (624 MB) for English, or
parakeet-tdt-0.6b-v3 (648 MB) for 25 European languages. It downloads from
models.pktito.net and can be paused. A download that stops keeps what it
has, and the next asks only for the rest (an HTTP range request); after a
lost connection it tries again by itself, 2 s later and then twice as long
each time, 30 s at most. The model is checked against the size and SHA-256
built into the app (src-tauri/src/model.rs) before it is used. The model
menu on the start screen changes it later, and a `.pkf` already on the
computer can be used instead.

Each file lives under the first twelve digits of its SHA-256, as
`https://models.pktito.net/8bfa6750b83c/parakeet-tdt-0.6b-v2.pkf`, so an
address never serves two different files and a CDN can keep each one
forever. models.pktito.net is a bunny.net pull zone, and
`scripts/models-upload.sh` puts converted models in its storage that way
(site/README.md). `scripts/models-tree.sh` lays them out the same way for a
local server, and `PKTITO_MODEL_BASE` points the download at another
server, such as a mirror or that local one:

```sh
scripts/models-tree.sh ../pktito/models/*.pkf /tmp/models
(cd /tmp/models && python3 -m http.server 8765) &
PKTITO_MODEL_BASE=http://127.0.0.1:8765/ cargo tauri dev
```

(Python's server ignores ranges, so a download from it that stops starts
over; the app takes the whole file when a server sends it whole.)

The models are NVIDIA's (CC-BY-4.0), converted by pktito-tools.

## Transcripts

Text appears as each part of a recording finishes (whole up to 30 s, else
cut at its silences), and Stop ends it after the part it is on, keeping the
text so far. Each finished transcript stays on the recent list, the 50
newest in the app's data folder, with its audio as pktito heard it: a 16 kHz
16-bit WAV in the cache, so that playback lines up with the timestamps
whatever the original's format, and whether or not the webview could play
it. The cache keeps 2 GB of these, letting the oldest go first; a transcript
whose audio is gone reads its original again to play.

The arrow at the top left, or Esc, goes back to the start screen. A
finished transcript closes, staying on the recent list; a file still being
transcribed goes on, at the top of that list, and leads back to itself.
Files dropped on the window join the queue from any screen.

About opens from the wordmark at the top of the start and setup screens, or
from the app menu on macOS. It credits the models and Silero VAD, and its
Everything else lists each other part the app is built from, the typefaces,
the icons and the Rust crates of the system it runs on, each with its
copyright and license, and then the licenses' texts. `scripts/notices.py`
writes that list, `ui/notices.js`, from what Cargo says each system's build
links; run it when Cargo.lock changes. The mascot, a budgie of voxels that
turns when dragged and chirps when clicked, is WebGL 2: `scripts/mascot.py`
(numpy) builds its model into `ui/mascot-model.js`, and `ui/mascot.js` draws
it. Each loads the first time it is shown, the mascot 29 KB of the app's
assets and the list 20 KB, compressed. Without WebGL 2, About shows the
credits alone.

The window shows no shortcuts; tooltips name them. Only macOS has a menu
bar. On Windows and Linux the page takes the shortcuts itself: Ctrl+O adds
files, Ctrl+S and Ctrl+Shift+S save as text and as subtitles, Ctrl+F finds.
Space plays and pauses.

Saving writes `.txt`, `.srt`, `.vtt`, `.md`, or `.json` with every word's
start and end, beside the original or where the dialog says. It never writes
over a file: a second save is "name 2.txt" ("name (2).txt" on Windows, as
Explorer numbers copies).

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

For checking a build without the window (`--json` prints the segments and
their words as the window gets them; `--download` resumes and retries as the
window does):

```sh
src-tauri/target/debug/pktito-app --transcribe talk.mp4 --model parakeet-tdt-0.6b-v2.pkf [--json]
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
brand book, app icon and the components the site uses (`bundle.css`), and
`ui/fonts` Archivo and Commit Mono with their SIL Open Font License texts.
The window, as redesigned on 2026-10-21, is `ui/app.css` on `ui/tokens.css`,
which `scripts/tokens.py` makes from `design/tokens.json`; the icon set in
`src-tauri/icons` comes from `cargo tauri icon design/app-icon.svg`.

## The site

`site/` builds pktito.net from the same design system, and its Cloudflare
Worker serves the installers (site/README.md).

## License

MIT (LICENSE). pktito is MIT; the fonts are under the SIL Open Font License
1.1; the models are NVIDIA's, under CC-BY-4.0. About lists every other part
the app is built from, with its license (scripts/notices.py).

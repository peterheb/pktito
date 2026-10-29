# pktito

Drop audio or video files and read their transcripts as they fill in; then
play one back with the word being spoken underlined, search it, and save it
as text, subtitles, Markdown or JSON. The audio comes out through the media
libraries the operating system already has, and it is transcribed on this
computer by [libpktito](https://github.com/peterheb/libpktito), a C11 port of NVIDIA's
Parakeet-TDT models. Nothing leaves the computer, and the one thing the app
fetches is its model, once.

On every system pktito's own reader takes WAV and AIFF (PCM, float,
A-law and mu-law; RF64, and AIFF-C's uncompressed kinds; up to two
channels, and more where no other reader knows the speakers), so a server
needs nothing more for them, and Windows, whose Media Foundation has no
AIFF reader, reads a Mac's AIFF. Everything else:

- **macOS 12 and later**: AVFoundation reads the file.
- **Windows 10 and later**: Media Foundation reads it. Windows Server and
  the N editions have no Media Foundation without the Media Feature Pack;
  pktito still starts and reads WAV there.
- **Linux**: there is no one media library, so ffmpeg, which the .deb
  depends on.

On macOS and Windows, a file the OS cannot read is handed to ffmpeg when
ffmpeg is on the PATH. `PKTITO_DECODER=ffmpeg` uses ffmpeg for everything;
`PKTITO_DECODER=os` the OS alone.

## Formats

One minute of speech in each of these, made by `scripts/media-matrix.sh`
and transcribed by `scripts/media-check.sh`, on macOS 27.0.1 (M4 Pro),
Windows 11 Pro 26200 (Core 3 304) and Ubuntu 26.04 (ffmpeg 8.0.1; 9.0 on
the other two), 2026-10-21:

| File                                                                      | macOS |  Windows   |   ffmpeg   |
| ------------------------------------------------------------------------- | :---: | :--------: | :--------: |
| MP4, MOV, M4A, 3GP: AAC, HE-AAC v1 and v2, ALAC, PCM; H.264, HEVC, ProRes |   ✓   |     ✓      |     ✓      |
| MP3, constant and variable rate                                           |   ✓   |     ✓      |     ✓      |
| WAV: 16 and 24-bit, float, µ-law, IMA ADPCM                               |   ✓   |     ✓      |     ✓      |
| FLAC, 16-bit 44.1 kHz and 24-bit 96 kHz                                   |   ✓   |     ✓      |     ✓      |
| MPEG-TS (H.264, AAC), MPEG-PS (MPEG-2, MP2)                               |   ✓   |     ✓      |     ✓      |
| AVI: MJPEG with PCM                                                       |   ✓   |     ✓      |     ✓      |
| AC-3 5.1, alone or in M2TS                                                |   ✓   |     –      |     ✓      |
| AIFF                                                                      |   ✓   |     ✓      |     ✓      |
| CAF with ALAC                                                             |   ✓   |     –      |     ✓      |
| Ogg Opus and Vorbis, .opus voice messages                                 |   ✓   |     –      |     ✓      |
| WebM: Opus or Vorbis, with VP8, VP9 or AV1                                |   –   |     ✓      |     ✓      |
| Matroska with AAC                                                         |   –   |     ✓      |     ✓      |
| Matroska with AC-3, E-AC-3 or DTS 5.1                                     |   –   |     –      |     ✓      |
| WMA, WMV                                                                  |   –   |     ✓      |     ✓      |
| AVI: MPEG-4 with MP3                                                      |   –   |     ✓      |     ✓      |
| FLV; Ogg Theora                                                           |   –   |     –      |     ✓      |
| An MP3 named .m4a                                                         |   –   |     ✓      |     ✓      |
| An M4A cut off halfway                                                    |   –   | first half | first half |

AIFF on Windows is pktito's own reader's, since 0.9.1: the 9950X's smoke
test of 0.9.1 transcribed one there (2026-10-24); the rest of the table is
the run above.

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
computer can be used instead: from that menu, or double-clicked, which
opens the app, or goes to the one already open, on the start screen with
it in use. A file pktito cannot load leaves the model in use as it was and
says why. Model files are a type of their own, with their icon, on every
system: the Windows installer registers it (src-tauri/windows/hooks.nsh),
the app's Info.plist on macOS, and on Linux the package's MIME type and
desktop file (src-tauri/linux).

Each file lives under the first twelve digits of its SHA-256, as
`https://models.pktito.net/8bfa6750b83c/parakeet-tdt-0.6b-v2.pkf`, so an
address never serves two different files and a CDN can keep each one
forever. models.pktito.net is a bunny.net storage zone behind a pull zone
with that hostname (a CNAME in Cloudflare's DNS, not proxied, so that bunny
can issue its certificate), which answers the range requests that resume a
download. `scripts/models-upload.sh` puts converted models in the storage
zone that way, through the zone's own endpoint (`ny.storage.bunnycdn.com`;
the plain `storage.bunnycdn.com` refuses its password), with the password
read from a file; bunny checks each file against its SHA-256, and the
script checks what the zone then holds. `scripts/models-tree.sh` lays them
out the same way for a local server, and `PKTITO_MODEL_BASE` points the
download at another server, such as a mirror or that local one:

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
text so far. A stretch in which Silero heard speech but the model wrote
no words is transcribed again on its own: 2 s of speech inside a sentence,
1 s between sentences, and a part with no words at all cut in two, if its
speech is within 4 dB of the recording's, so that a voice in the
background stays out. The
English model, NeMo's own included, drops the LibriVox notices, credits
and titles at the start of seven of AliceBench's chapters as they are
cut, and these give them back (pktito's `pk_asr_transcribe`,
`vendor/pktito/core/transcribe.c`). Each finished transcript stays on the recent list, the 50
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
links; run it when Cargo.lock changes, and with `--check` before every
release (Releasing, below). The mascot, a budgie of voxels that
turns when dragged and chirps when clicked, is WebGL 2: `scripts/mascot.py`
(numpy) builds its model into `ui/mascot-model.js`, and `ui/mascot.js` draws
it. Each loads the first time it is shown, the mascot 29 KB of the app's
assets and the list 20 KB, compressed. Without WebGL 2, About shows the
credits alone.

The window shows no shortcuts; tooltips name them. Only macOS has a menu
bar. On Windows and Linux the page takes the shortcuts itself: Ctrl+O adds
files, Ctrl+S and Ctrl+Shift+S save as text and as subtitles, Ctrl+F finds.
Space plays and pauses; < and > slow playback down and speed it up, from
0.75x to 2x, which the word under the play button shows and opens.

A finished transcript reads as its sentences, as the model wrote them, or as
Captions: its subtitle track, cut into cues of one or two lines of 42
characters by pktito's own pk_captions (vendor/pktito/core/captions.c), as
the CLI's --srt is, each cue shown with when it starts and ends and a mark
where it reads faster than 20 characters a second. Playback, find and its
ticks work in either. Saving writes `.txt`, `.srt`, `.vtt`, `.md`, or
`.json` beside the original or where the dialog says: the subtitles are the
captions' cues, the text and Markdown the sentences, and the JSON both, with
every word's start and end. It never writes over a file: a second save is
"name 2.txt" ("name (2).txt" on Windows, as Explorer numbers copies).

## Command line

`pktito-cli` is the app's transcription from a terminal, and comes with it:
the same decoders, the model the app is set to, and Save's five formats,
written by the same code as the window's (`src-tauri/src/format.rs`), so
that the two write the same bytes.

```sh
pktito-cli talk.mp4                      # the transcript, on the terminal
pktito-cli --srt *.mp4                   # subtitles beside each file
pktito-cli -m multilingual -f json -o out interview.m4a
pktito-cli models                        # which are here, which is in use
pktito-cli download parakeet-tdt-0.6b-v3
```

`-m` takes a model's name, its label (`english`, `multilingual`) or a
`.pkf` file; `--times` puts each sentence's time before it in `.txt` and
`.md`; `-o` names the file, or for several files the folder, and nothing is
written over. A model downloaded from here is the app's too, and the one it
uses if it had none. Progress goes to stderr when that is a terminal. The
bundles carry it beside the window's program (`src-tauri/src/bin`), and on
the PATH: the Linux package in `/usr/bin`; on Windows the installer adds its
folder to the user's PATH, and the uninstaller takes it out
(`src-tauri/windows/path.ps1`); on macOS, where a disk image installs
nothing, the app menu's Install Command Line Tool links it into
`/usr/local/bin`, asking for an administrator's password where that folder
is the system's.

### How fast: bench and AliceBench

`pktito-cli bench PATH...` times the transcription of each file named, or
of a folder's audio and video, and gives the whole as times real time: the
audio's length over the time it took, from its samples to its sentences.
Reading the file is left out, being the OS's decoder or ffmpeg rather than
pktito. `--json` adds each transcript's SHA-256.

AliceBench is the one number to compare computers by:
`scripts/alicebench.sh` (`alicebench.ps1` on Windows) downloads LibriVox's
_Alice's Adventures in Wonderland_, twelve chapters and 2:58:51 as 16 kHz
FLAC (200 MB, once; on Linux the same samples as 330 MB of WAV, which
needs no ffmpeg), benches it with the English model, and checks every
chapter's transcript against the one pktito gives on every machine. FLAC
rather than LibriVox's MP3s, because each OS decodes an MP3 to other
samples and so to a slightly different transcript, where a FLAC is the
same samples through all of them. The M4 Pro, in use: 106.1x real time
(2026-10-23).

## The window's words

Every word the window shows is in `ui/locales/en.json`, under a key:
`"save.failed": "Couldn't save {name}"`, with `{name}` filled in where it is
shown, and a key that counts as an object of plural forms (`"one"`,
`"other"`, as `Intl.PluralRules` names them). The Rust side embeds the file
and hands it to the page before its scripts run (`src-tauri/src/strings.rs`);
the page words its own HTML by `data-t` attributes and the rest with `t(key,
values)` (`ui/app.js`), and Rust reads the same file for what it draws
itself: the macOS menu bar, the Windows system menu, the file dialogs'
filters, macOS's password prompt. Something that went wrong comes from Rust
as a key, its values, and the particulars as the OS or server said them,
which the window folds into a tooltip or a Details. pktito-cli and the
engine stay English.

English is the only catalog so far. A translation is another file of the
same keys; adding the first means choosing it from the system's language in
`strings.rs`, where the catalog is read, with English under it for any key it
lacks, and setting the page's `lang`. `cargo test` holds the catalog to the code: every key named is in it,
and nothing in it is named nowhere. Brand names, the model names, and the
copyright notices stay as they are.

## Building

Rust, the Tauri CLI (`cargo install tauri-cli --version "^2" --locked`) and
a C compiler: Xcode's command line tools on macOS,
[llvm-mingw](https://github.com/mstorsjo/llvm-mingw)'s clang on Windows
(below), gcc or clang on Linux.

```sh
cargo tauri dev             # run it
cargo tauri build           # the installer: .dmg, .msi/.exe, .deb/.AppImage
```

The Rust is rustfmt's and the page's JavaScript, CSS, HTML and JSON
Prettier's, each with its defaults: `cargo fmt` in `src-tauri` and `npx
prettier@3.6.2 --write .` at the top before a commit (`--check` says whether
either is due). `.prettierignore` leaves out what scripts write and the
design system's own files.

Windows builds on x64 Windows with rustup's
`stable-x86_64-pc-windows-gnullvm` toolchain and llvm-mingw's `bin` on
`PATH` (its clang compiles and links; `src-tauri/.cargo/config.toml` names
it and links everything statically): `x86_64-pc-windows-gnullvm` for x64,
and the `aarch64-pc-windows-gnullvm` target, cross-compiled, for Windows on
Arm. Each exe needs only Windows' own DLLs and its architecture's
`WebView2Loader.dll` beside it.

```sh
cargo +stable-x86_64-pc-windows-gnullvm build --release --target x86_64-pc-windows-gnullvm
cargo +stable-x86_64-pc-windows-gnullvm build --release --target aarch64-pc-windows-gnullvm
```

Each installer names the DLL as a resource: tauri-bundler 2.10.4 adds
`WebView2Loader.dll` only for targets ending in `-gnu`, and without it the
installed app does not start. tauri-build copies it into the target's
folder, but only after it has checked the resources, so a first build in a
fresh folder fails on the DLL not being there yet: copy it in from
webview2-com-sys, which carries Microsoft's loader for each architecture
(its version is Cargo.lock's), `x64` or `arm64`:

```powershell
Copy-Item (Resolve-Path "$env:USERPROFILE\.cargo\registry\src\index.crates.io-*\webview2-com-sys-0.39.1\x64\WebView2Loader.dll") src-tauri\target\x86_64-pc-windows-gnullvm\release\
```

```sh
cargo tauri build --target x86_64-pc-windows-gnullvm --bundles nsis --config \
  '{"bundle":{"resources":{"target/x86_64-pc-windows-gnullvm/release/WebView2Loader.dll":"WebView2Loader.dll"}}}'
cargo tauri build --target aarch64-pc-windows-gnullvm --bundles nsis --config \
  '{"bundle":{"resources":{"target/aarch64-pc-windows-gnullvm/release/WebView2Loader.dll":"WebView2Loader.dll"}}}'
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

## Releasing

Before a version is tagged and its builds are made:

```sh
python3 scripts/notices.py --check
```

It builds About's list from what Cargo says each system links, without
writing it, and fails naming each crate added, removed or changed since
`ui/notices.js` was written; then run it without `--check` and commit the
file. It needs no installed targets, so any machine can run it. Cargo knows
only crates: a typeface, an icon set or a file shipped beside the app (as
`WebView2Loader.dll` is) added since the last release goes into the
script's `EXTRAS` by hand, its license text into `scripts/licenses`.
pktito-site's README (Installers) posts the builds.

## Design

The interface follows the pktito design system: `design/` holds its tokens,
brand book, app icon and the components the site uses (`bundle.css`), and
`ui/fonts` Archivo and Commit Mono with their SIL Open Font License texts.
The window, as redesigned on 2026-10-21, is `ui/app.css` on `ui/tokens.css`,
which `scripts/tokens.py` makes from `design/tokens.json`; the icon set in
`src-tauri/icons` comes from `cargo tauri icon design/app-icon.svg`.

## The site

pktito.net is its own repository,
[pktito-site](https://github.com/peterheb/pktito-site), built from this
design system; its Cloudflare Worker hands out the installers. `npm run
sync` there copies the design system's files and the screenshots
(`design/screenshots`) from a checkout of this repository beside it.

## License

MIT (LICENSE). pktito is MIT; the fonts are under the SIL Open Font License
1.1; the models are NVIDIA's, under CC-BY-4.0. About lists every other part
the app is built from, with its license (scripts/notices.py).

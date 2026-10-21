# pktito, the desktop app

Drop an audio or video file and read its transcript. The audio comes out
through the media libraries the operating system already has, and it is
transcribed on this computer by [pktito](https://pktito.net), a C11 port of
NVIDIA's Parakeet-TDT models. Nothing leaves the computer except the model's
one download.

- **macOS 12 and later**: AVFoundation reads the file (mp3, mp4, m4a, mov,
  wav, aiff, caf, aac, flac, ogg, and whatever else macOS plays).
- **Windows 10 and later**: Media Foundation reads it (mp3, mp4, m4a, wma,
  wav, aac, and more with Microsoft's codec extensions).
- **Linux**: there is no one media library, so ffmpeg is required: install
  it with your package manager.

On macOS and Windows, a file the OS cannot read (WebM and Matroska, for
example) is handed to ffmpeg when ffmpeg is on the PATH. `PKTITO_DECODER=ffmpeg`
uses ffmpeg for everything; `PKTITO_DECODER=os` the OS alone.

## The model

On first run the app downloads one model from pktito.net, about 624 MB for
English (parakeet-tdt-0.6b-v2) or 648 MB for 25 European languages
(parakeet-tdt-0.6b-v3), and checks its size and SHA-256 against the ones
built into the app (src-tauri/src/model.rs) before using it. A `.pkf` already
on the computer can be used instead ("Use a model file on this computer").
`PKTITO_MODEL_BASE` points the download at another server, such as a mirror
or a local one while testing:

```sh
(cd ../pktito/models && python3 -m http.server 8765) &
PKTITO_MODEL_BASE=http://127.0.0.1:8765/ cargo tauri dev
```

The models are NVIDIA's (CC-BY-4.0), converted by pktito-tools; pktito.net
serves the files named in the catalog.

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

## Design

The interface follows the pktito design system: `design/` holds its tokens,
brand book and app icon, `ui/bundle.css` its components, and `ui/fonts`
Archivo and Commit Mono with their SIL Open Font License texts.
`scripts/tokens.py` turns `design/tokens.json` into `ui/tokens.css`; the
icon set in `src-tauri/icons` comes from `cargo tauri icon design/app-icon.svg`.

## License

MIT (LICENSE). pktito is MIT; the fonts are under the SIL Open Font License
1.1; the models are NVIDIA's, under CC-BY-4.0.

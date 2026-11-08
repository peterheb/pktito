# Editions

A model in the catalog comes in editions: the files one kind of hardware
runs it from. Every model has a CPU edition, its whole `.pkf`. Some have
an edition for an NPU as well: a slim `.pkf` (the model without its
encoder) and an encoder built for that NPU, a Core ML model for Apple's
Neural Engine or a QNN graph for one Snapdragon's Hexagon.

This is the design every platform follows from 0.9.7 (PKT-2). Desktop
(macOS, Windows x64 and Arm64, Linux), iOS and Android each use it, and
none decides alone which edition fits, whether a file is current, or what
to offer. The Rust core decides; each platform tells the core what
hardware it has, downloads what the core names, and runs it.

## What 0.9.6 does, and why it changes

- **Choosing.** Three copies of the rule: model.rs `graph_on` (Windows on
  Arm), Kotlin's `Models.graphFor` (Android), and Swift's
  `Editions.preferred`, which takes the Neural Engine's edition on every
  device. The Kotlin copy reads the catalog's JSON and must be kept in step
  with the Rust one by hand.
- **Knowing a file is current.** An encoder counts as here if
  `model.mil` exists in its folder, and every other file if its size is
  the catalog's. 0.9.6's encoder replaced 0.9.5's (the position term folded
  in, 16% less time a call on an M4, the ane fork's tools/ane/CLIMB.md,
  2026-11-03), and a Mac or iPhone that has 0.9.5's keeps it. The S24's v2
  and v3 graphs are both 714,506,856 bytes, so a rebuilt graph of the same
  size can't be told from the old one at all. 0.9.6 started writing
  `<file>.sha256` beside every checked download (model.rs `marker_of`, app
  ca854d8; iOS `Store.marker`, 82c6479), and nothing reads them yet.
- **Refusals.** Remembered three ways: desktop's `<graph>.refused` files
  in the models folder; Android's `npu_failed.<model>` preference;
  iOS's `neuralEngineFailed.<model>` default. Each is keyed differently.
- **"Faster" for a whole file already here.** The desktop downloads the
  encoder alone and keeps the whole file, which the NPU edition then loads
  (`encoder_offer`). Android downloads the slim file and the graph, and
  deletes the whole file once the NPU has run a transcription.

## The catalog

`Entry.coreml` and `Entry.qnn` give way to one list of editions:

```rust
pub struct Entry {
    pub name: &'static str,
    // key, label, about, languages, accuracy as before
    pub editions: &'static [Edition],
}

pub struct Edition {
    /// Stable, for refusals and logs: "cpu", "coreml", "qnn-sm8650".
    pub id: &'static str,
    pub runtime: Runtime,
    /// The model file it loads: the whole .pkf for "cpu", the slim one else.
    pub model: Part,
    /// The NPU's encoder file: a Core ML archive or a QNN graph; None for "cpu".
    pub encoder: Option<Part>,
}

pub enum Runtime {
    Cpu,
    /// Apple's Neural Engine, from these systems on.
    CoreMl { macos: (u32, u32), ios: (u32, u32) },
    /// One chip's Hexagon, in QNN's words (soc,arch,vtcm,dlbc,signed), on
    /// these systems (std::env::consts::OS).
    Qnn { chip: &'static str, systems: &'static [&'static str] },
}
```

`Part` stays as it is: a file name, its bytes and its SHA-256, served at
`<first 12 of the SHA>/<file>`. A rebuilt encoder is a new `Part` under
the same edition id, and so a new address.

## The device

Each platform builds one `Device` and passes it to the core:

```rust
pub struct Device {
    pub system: &'static str,        // std::env::consts::OS, "ios" on iOS
    pub os: (u32, u32),              // the system's version
    pub qnn: Option<String>,         // QNN's chip, where QNN found an NPU
    /// What a refusal holds for: the system's build and the app's,
    /// as each platform can name them.
    pub here: String,
}
```

| Platform           | `qnn`                                       | `here`                                            |
| ------------------ | ------------------------------------------- | ------------------------------------------------- |
| macOS              | None                                        | macOS version + app version                       |
| iOS / iPadOS       | None                                        | `UIDevice.systemVersion` + `CFBundleVersion`      |
| Android            | `Engine.npuChip`, as Editions.chip asks now | `Build.FINGERPRINT` + `lastUpdateTime`            |
| Windows on Arm     | `pktito::qnn_chip`, as `model::chip` asks   | OS build + app version + the NPU driver's version |
| Windows x64, Linux | None                                        | app version                                       |

`here` is a string the core only compares, so each platform puts in it
whatever can change what its NPU runs. On Windows on Arm that is also the
Hexagon NPU driver's version, which Windows Update and the PC's maker
update apart from the OS build: a refusal a driver caused shouldn't hold
past the driver that fixed it (pktito-9950x, 2026-11-07). Android's
fingerprint already covers the vendor's drivers.

`os` is the system's version as each names it: macOS and iOS their
major and minor, Android `(Build.VERSION.SDK_INT, 0)`, (36, 0) on the S24.
Only Core ML's floors compare it today.

## Choosing

`choose(e, device, refusals) -> &Edition`, in Rust only. Each edition
scores a fit to the device, and the highest that isn't refused wins:

| Fit | When                                                                                       |
| --- | ------------------------------------------------------------------------------------------ |
| 3   | Core ML on a system at least its `macos` / `ios`; QNN whose `chip` is the device's exactly |
| 2   | QNN for another SoC of the device's Hexagon with no more VTCM, on one of its `systems`     |
| 1   | CPU: always                                                                                |

The CPU always fits, so `choose` always answers. The catalog's order never
chooses: a test runs `choose` for every device in a table of the ones we
know (M1-M5, A18, S24, S25, S26, X1E80100 on Android and on Windows,
whose one `qnn-x1e80100` edition is for both systems, X2, 7c, x86, Pi) and fails on a tie at the top. It replaces
`one_graph_a_hexagon_a_system`, which held the same line for QNN alone.

## What is here

Each file of an edition is in one of four states, from the file and its
marker (`<file>.sha256`, and for an unpacked encoder `<folder>.sha256`
beside the folder, as 0.9.6 writes them):

| State   | Meaning                                                                                              |
| ------- | ---------------------------------------------------------------------------------------------------- |
| Current | Here, and its marker is the catalog's SHA-256                                                        |
| Older   | Here, but its marker is another SHA-256, or it has none (see below), or its size isn't the catalog's |
| Partial | Only its `.part`, with this many bytes                                                               |
| Absent  | Nothing                                                                                              |

A flat file with the catalog's size and no marker (any download before
0.9.6) is hashed once to adopt it. That runs off the main thread the first
time the catalog is read, and takes about 2 s for 624 MB on a Mac. A match
writes the marker and makes it Current; a mismatch makes it Older. An
unpacked encoder can't be hashed back to its archive, so one with no marker
is Older: the case PKT-2 is about.

An edition is here when its encoder is Current or Older and a model file
it can load is: its own, or for an NPU edition the CPU's whole file, which
the engine runs with an encoder beside it as well as the slim one.

## What runs, and what is offered

- **Runs:** the chosen edition if it is here, else the CPU edition if it
  is here, else nothing. An Older file runs: an update is never needed to
  transcribe.
- **Offered** in Manage models, one at a time, and only when the user
  taps it (no download starts by itself; Peter, 2026-11-05):

| Offer           | When                                                   | Downloads                                                      |
| --------------- | ------------------------------------------------------ | -------------------------------------------------------------- |
| Download · N MB | No edition is here                                     | The chosen edition's files                                     |
| Faster · N MB   | The CPU edition runs and an NPU edition is chosen      | The chosen edition's files not here: its slim file and encoder |
| Update · N MB   | The chosen edition runs, and one of its files is Older | Its Older files                                                |

N is what the download fetches, less what a stopped one kept.

Once an NPU edition has run a transcription here with its own slim file
Current, the CPU's whole `.pkf` is deleted, on every platform (Peter,
2026-11-07): v2 takes 727 MB that way, where the whole file kept beside
it takes 1,339. An NPU edition that runs from the whole file (a Mac or
iPhone that added an encoder to it before 0.9.7) keeps it.

## Updating

An update downloads beside the file it replaces (`.part`, then checked),
and the old file runs until the swap:

1. The new file is checked against its size and SHA-256, and its marker
   written as `<file>.next.sha256`.
2. The swap waits until no engine holds the model: the app lets go of a
   loaded engine it isn't transcribing with, and a transcription under way
   finishes first. On macOS, Linux, iOS and Android a rename over an open
   file would work anyway, but Windows refuses it, and one rule is simpler.
3. A flat file is renamed over the old one, and its marker after. An
   encoder is unpacked to `<folder>.unpacking`, the old folder renamed to
   `<folder>.old`, the new one into place, and the old deleted.
4. A swap cut short (the app killed between steps) is finished at the next
   catalog read: a `.next` checked file or a `.old` folder says how far it
   got.

On Apple's systems Core ML compiles the new encoder on its first load,
16 s on an M1 iPad, which the setup screen says while it lasts
(`Editions.compiled` keys on the encoder's SHA-256 already). The old
encoder's compiled copy is the system's to evict, about 1.2 GB each
(our measurement, 2026-10-31).

## Refusals

When an NPU won't run its edition, the core records it in one file,
`models/refusals.json`: the model, the edition id, its encoder's SHA-256,
the device's `here`, and why. A refusal holds while all of those match,
so a new app, a new system or a rebuilt encoder tries the NPU again.
The refused edition's encoder is deleted (714 MB for a v2 graph). Only
the catalog's own encoder is recorded: an Older one that fails says
nothing of the catalog's, so it is deleted alone, and Update or Download
offers the catalog's again (android-9950x, 2026-11-08). Its
model file stays if the CPU edition doesn't have one; then `choose`
answers the CPU edition, and Manage models offers Download for it.

A refusal can leave nothing to run: once the Faster rule has deleted the
whole `.pkf`, an NPU that stops (the S24's NPU wedging mid-job, or a system
update) leaves the slim file alone. Then the job ends with that as its
error, not a CPU retry, and the view says why nothing runs
(`"runs": null, "refused": "<why>"`), so the screen can say the NPU
stopped and offer the CPU edition's Download, apart from a model never
downloaded (`"runs": null` with no `refused`). Where the whole file is
still here, the job goes on on the CPU, as Android's does since 0.9.6.

0.9.6's three stores are not carried over: each is keyed on the app's
build, so 0.9.7 would ignore them anyway. They are deleted on the first
catalog read.

## Deleting

Delete takes every edition's files of the model, their markers, `.part`,
`.next` and `.old` files, and its refusals.

## The bridges

The core gives every platform the same view of each model, as JSON:

```json
{
  "name": "parakeet-tdt-0.6b-v2",
  "key": "english",
  "languages": ["en"],
  "chosen": "qnn-sm8650",
  "runs": {
    "edition": "cpu",
    "model": "/…/parakeet-tdt-0.6b-v2.pkf",
    "encoder": null
  },
  "offer": {
    "kind": "faster",
    "bytes": 726622380,
    "files": [
      "parakeet-tdt-0.6b-v2-slim.pkf",
      "parakeet-tdt-0.6b-v2-sm8650.qnn"
    ]
  },
  "kept": 0,
  "bytes": 624480196
}
```

- Desktop: main.rs `Offer` becomes this view; the window shows `offer`.
- iOS: `pkapp_catalog(device)` returns it, and Swift's `Editions.preferred`
  and `Store.have(e, edition)` go. Swift keeps the Core ML load, its retry
  and `compiled`.
- Android: `Engine.catalog(dir, device)` returns it, and Kotlin's
  `graphFor` and `Offer.edition` / `usable` / `faster` go. Kotlin keeps the
  chip question, the skels and the QNN load.
- Downloads take `offer.files` as they are (Android's `Engine.download`
  already takes a list of names).
- Until each app reads the view, ios.rs and android.rs give it 0.9.6's
  catalog JSON (`model::legacy`), so a phone built from this branch works
  as 0.9.6's until its port lands.

The core is src-tauri/src/editions.rs: `Device`, `fit`, `choose`, `have`,
`adopt`, `settle`, `Refusals`, `runs`, `offer`, `ran` and `view`. model.rs
holds the catalog and builds its helpers (`usable`, `encoder_of`,
`wanted`, `delete`) on it. The desktop puts a waiting download in place
(`settle`) when it reloads the engine and, from `status`, whenever nothing
is transcribing or downloading (main.rs `settle_idle`).

## Words

New strings: "Update · {size}" and one line under it for why ("A newer
build for this {device}'s Neural Engine" / "…NPU"), in ui/locales/en.json,
strings.xml and the iOS catalog, each a whole sentence, with en-XA made
again (scripts/pseudo.py).

## Who does what

| Part                                                                                                                      | Who           |
| ------------------------------------------------------------------------------------------------------------------------- | ------------- |
| This document, the core (catalog, `choose`, states, adoption, offers, updates, refusals, the view) and its tests; main.rs | pktito-mac    |
| macOS window and iOS (ios.rs, Swift)                                                                                      | pktito-mac    |
| Windows: `Device` from `qnn_chip`, the swap on Windows, x64 and Arm64 builds, snappy                                      | pktito-9950x  |
| Android: android.rs, Kotlin, Play build, S24/S25                                                                          | android-9950x |

The core lands first on this branch (`editions`), with `cargo test` and
desktop builds on macOS, Windows and Linux. Then the three platforms go at
once, each on its own branch off it, and merge back here. Main gets the
whole when every platform passes.

## Checks

On each of the release machines (the 9950X, the M4 Pro, snappy, the Pi 5,
the S24, the iPhone 16e) and the M1 iPad:

1. A fresh install: Download, then it runs on the chosen edition.
2. A 0.9.5 install (no markers, 0.9.5's encoder): Update offered for the
   encoder, nothing for the `.pkf` (adopted by its hash); after it, the
   0.9.6 encoder runs.
3. A 0.9.6 install: nothing offered.
4. A catalog bump: a local server (`PKTITO_MODEL_BASE`) serving a
   re-signed copy of one encoder under a new SHA-256: Update offered, the
   old one runs until the swap, the new one after; killed mid-swap, it
   finishes at the next launch.
5. A refusal (`PKTITO_REFUSE_NPU=1`, a test-only switch): CPU chosen,
   Download offered where the whole file isn't here; a new app version
   tries the NPU again.

## Decided

**"Faster" for a whole file already here** (Peter, 2026-11-07): Android's
rule, everywhere. Faster downloads the slim file and the encoder, and the
whole `.pkf` is deleted once the NPU edition has run (see "What runs").
In 0.9.6 the desktop downloaded the encoder alone and kept the whole file.
The cost: an NPU that refuses later, after a system update, needs the
624 MB CPU edition downloaded again.

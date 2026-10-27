pktito turns speech into text on your computer. It is a C11 library of NVIDIA's Parakeet-TDT speech models, and a desktop app for macOS, Windows and Linux built on it with Tauri. This system serves both of pktito's faces: the app, and the site, pktito.net, which shows the app and sends the reader to the download or the source.

The idea: **the transcript is the picture.** The app is a waveform strip over a transcript, and the site shows that app, with a real transcript in it, rather than describing it.

pktito is the sibling of pinfer and shares its type (Archivo and Commit Mono). The colors are its own.

## Voice

pktito is a free tool, written by people who use it. Write the way a knowledgeable friend would describe it: plain, specific, and finished when the point is made.

1. **Say it once.** If the picture, the heading or the sentence before already says it, cut it. Don't caption what the reader can see. A section answers its question and stops.
2. **People act; things don't.** pktito can be the subject of what it literally does: transcribes, saves, opens. Files, servers, operating systems and toasts don't speak, try or decide. Write "The download was damaged", not "the file didn't match its checksum"; "Retrying automatically", not "Retries by itself". Use the passive voice or a state when nobody is acting.
3. **Show, then stop.** Let the screenshot and the measured numbers do the persuading. No superlatives and no hype words (powerful, seamless, effortless, blazing), no urgency ("Get started now"), no exclamation marks, no emoji. Confident, not loud: pktito is described, not sold.
4. **Numbers have a source, in one short line.** A speed figure names its benchmark and the pktito version. The machines and the method live where that line links, not in a paragraph under the chart.
5. **No disclaimers in running copy.** Legal terms live in one place, the privacy page and the footer. Hedge ("may", "in most cases") only what is uncertain, and then say what it depends on.
6. **Errors say what happened and what to do.** The headline is the fact ("File not found") or the fix ("Install ffmpeg to read audio and video here"). Add a remedy only when the reader can act on it ("Close other apps and try again"). The system's own words (an error code, a server's answer) go in the details, never the headline.
7. **Use the reader's words.** Files, recordings, videos and transcripts; not decoders, chunks or VAD. Name formats, platforms and models exactly as published: MP3, Windows on Arm, parakeet-tdt-0.6b-v2.

Mechanics:

- Sentence case everywhere but the macOS menu bar, where Apple's Title Case applies ("Add Files…", "Save as Text").
- Lowercase **pktito**, at the start of a sentence too.
- Buttons are verbs ("Download", "Save as", "Try again"). An ellipsis marks one that opens a dialog ("Choose files…", "Open a model file…").
- A status names the step: "Reading the audio", "Transcribing 42%", "Done".
- Contractions are fine ("Can't", "isn't"). American spelling.
- Punctuation as a careful writer uses it: curly quotes, em dashes where a sentence turns. Other symbols only where they are the convention: × for times real time, → after a link, · between items in a status line, … in a button that opens a dialog.

Before and after, from the app and the privacy page:

| Before | After |
| --- | --- |
| pktito pulls the audio out of an mp3 or mp4 and transcribes it. | pktito transcribes audio and video on your computer. |
| Uninstalling pktito removes the app. Its data folder … stays until you delete it. | Uninstalling pktito leaves its data folder … for you to delete. |
| If it changes, this copy changes with the next update of pktito. | *(cut)* |
| Couldn't open it: Windows said 42 | Couldn't open the link |
| File changed since | Can't play: the original file has changed since it was transcribed |

Every word the app shows is in `ui/locales/en.json`, by key; write and review copy there.

## Color

One ground, one ink, one accent, one mark. Every color is a token with a light and a dark value, and components use tokens. The exceptions are the Windows caption buttons, which use Windows' own colors (close is #c42b1c), and a few color mixes for search highlights.

- **Grounds.** `ground` is the page, the app window, menus, sheets and dialogs. `surface` is one tone step off it: the model cards, the sentence being played, a sheet's header, About's mascot stage, the site's footer and tiles. `sunk` is two steps off: buttons at rest, the Find field, the switch's track, empty progress tracks, key caps, Details boxes.
- **Ink.** `ink` (night navy, never black) sets text and figures. It also fills the site's Download buttons and marks a chosen card (a 2 px ring) or the current search hit. `ink-muted` is for secondary text: timestamps, sizes, status lines, descriptions, credits, unselected labels, legal text. Never for the transcript itself.
- **Plume**, the accent. It colors the primary button, the site's stage, the played part of a waveform, search ticks and highlights. Text on it is `on-plume`, never white, and plume is never text on the ground. In light, progress fills with `wave`, since plume is 1.5:1 against the light track and a state's cue needs 3:1. `plume-press` fills the drop overlay.
- **Beak**, a mark, not an area. It colors the wordmark's square, the playhead, the square after the playing row's time and the playing word's underline.
- **Term**, the dark field in both themes: the waveform strip and the toast. On it go `term-ink`, `wave` and `wave-played`. The app's text there is all `term-ink`, 14.3:1, which label leads told by its place rather than a dimmer shade for the rest (`term-dim` is the site's). `term-line` is a quiet fill there: the play button, a speed changed from 1×, and the idle bars. On the dark theme `term` sits a step above the ground, so that the strip's edge shows, and `term-line` as far above it as in light.
- **State.** `ok` and `err` always sit beside a word: "Done", "Downloaded", or the failure. `err` also sets the word on the one destructive button.
- **Scrim.** `scrim` lies behind a sheet or the ask dialog.
- **Focus.** `focus` draws a 2 px solid ring: ink in light, plume in dark, and plume on `term` in both. The offset is 2 px in the app and 3 px on the site.
- **Hover and press.** One rule for every control on both surfaces: `tint`, then `tint-press`, laid over whatever the control sits on or is filled with, so it darkens in light and lightens in dark. On `term`, and on an ink fill, use `tint-term` and `tint-term-press`. Links in running text thicken their underline instead.

Areas are told apart by tone, not by lines. Outlines mark only a state or where a file goes: a chosen card's ring, the chosen switch segment's 1 px `ink-muted` edge, the drop overlay's dashed edge and the start screen's drop zone, dashed as it is, a menu's ring. A menu separates its groups with a divider: 1 px of `sunk`, inset to the items' text. Shadows only on what floats: `shadow-float` for menus, sheets, dialogs and the toast.

## Type

Two families, shipped as files (`ui/fonts`, and the site's `fonts/`) with their SIL Open Font License texts beside them. Never load a face from a CDN: the app works offline, and so do the site's pages.

- **Archivo**, at 400, 500 and 700 only. Use no other weight, since a weight without a file is faked. It sets everything that is not code, a timestamp or a measurement.
  - Site: `display` for the hero, `lead` for section headings, `body` for running text, `small` for nav and legal text.
  - App: `ui` (14 px/500) for controls and file names, `transcript` (17 px at 1.6) for the text people read, `lead` for a screen's heading, `figure` at 64 px for the download's percentage, `small` for status lines.
- **Commit Mono**, at 400 and 700, sets what is literally code, time or a measurement:
  - commands and file paths
  - every timestamp: `hh:mm:ss`, `hh:mm:ss.cc`, or SMPTE `hh:mm:ss:ff` and drop-frame `hh:mm:ss;ff` when frames are chosen
  - playback speed, frame rates, download sizes and match counts
- Tabular figures (`tabular-nums`) wherever numbers line up.
- Headings use `text-wrap: balance`. Running text stays near `measure`.
- The floor is 12 px, for secondary detail only (stats, chips, license texts). Running text is 13 px or more.

## Layout

**Site.** The page runs to `page-max` with `margin-page` outer margins (40 px; 20 px on a phone, with the safe-area insets). The hero sits on a 12-column grid; sections below use 3 or 4 columns that stack on a phone. Sections stand `space-7` apart. They are told apart by tone, alternating `ground`, `surface` and the `term` field for one dark band. Corners are square (`radius-0`).

**App.** One window and one screen at a time:

- setup: choose a model, its download, or the download stopped
- start: a drop zone, "Drop audio or video here" and Choose files… on `surface` inside a 2 px dashed `ink-muted` edge, and the recent list
- one file's transcript
- a batch: the files dropped together, how far it has got and its pace so far (over the rows' lengths, in their type), each row with its progress, opening its transcript, and Save all

About lies over whichever screen opened it, and the Privacy and license sheets over About. The ask dialog, the drop overlay, menus and the toast come and go over any screen.

- **Size.** The window opens at 680 × 800 (portrait, about 65 characters of transcript a line and 20 lines) and is at least `window-min` (480 px) each way.
- **Side.** From 920 px wide, the start screen's top row and its recent list keep a column at the window's left, the side, beside the drop zone or the file or batch open, the work. 920 is the side at its narrowest, 240, beside the 680 the window opens at. Under that, or at the start with nothing on the list, there is no side. The side is 300 px wide. Dragging its edge sets it anywhere from 240 to 480, never leaving the work under 680, and a double-click there fits the longest name, as a Finder column does. The work's narrow rules follow its own width, not the window's, so beside the side it looks as a window of its width would.
- **Title bar.** Each screen's header row is the title bar, and its empty space drags the window:
  - macOS: the system's traffic lights at its left end, with no title bar drawn and no lights in full screen.
  - Windows: the app draws minimize, maximize and close at its right to Windows 11's measure (46 × 32, Segoe Fluent Icons, close red, dimmed while the window is inactive), with the system menu on Alt+Space.
  - Linux: the desktop's own title bar.
- **Corners.** Controls and containers use `radius-ui` (8 px), as the OSs' own controls do. Items inside a container use `radius-sm` (6 px): switch segments, menu items, chips, a row's remove button.
- **Icon-only buttons.** Allowed only for actions everyone reads: back, close, copy, play and pause, remove, previous and next match. Each has a tooltip and an accessible name, with its shortcut where there is one.

## Imagery

pktito's pictures are screenshots of the app itself, taken by script so they can be retaken whenever the window changes. The standard one is chapter 1 of LibriVox's *Alice's Adventures in Wonderland*:

- transcribed by the multilingual model
- the window at its first size
- scrolled past the LibriVox notice
- in light and dark, on each system

The recording is `wonderland_ch_01_64kb.mp3` from archive.org's `alice_in_wonderland_librivox`, scrolled so that "Alice's Adventures in Wonderland by Lewis Carroll." is the first row. The shots live in `design/screenshots/`, and pktito-site copies them (`npm run sync` there). A visitor sees their own system's; a phone sees its nearest (Android the Windows one, iPhone the Mac's), and Linux the Linux one.

| System  | Taken on              | How                                         | Files                                   |
| ------- | --------------------- | ------------------------------------------- | --------------------------------------- |
| macOS   | the Mac               | `scripts/screenshots-mac.sh`                | `mac-light.png`, `mac-dark.png`         |
| Windows | the 9950X, x64        | `scripts/screenshots-windows.ps1`           | `windows-light.png`, `windows-dark.png` |
| Linux   | pi5a's desktop, labwc | not yet: the same shot, under its title bar | `linux-light.png`, `linux-dark.png`     |

Why the multilingual model and the MP3: the English model runs the notice, the title and "Chapter one" together as one row, so no row is the title; and the first 16 rows of the MP3 are the same through macOS's decoder and ffmpeg, Linux's (2026-10-23, 0.8.5), and through Media Foundation since MP3s are cut to where their sound starts (`src-tauri/src/mp3.rs`).

The Mac's script quits pktito, sets its settings for the shot and puts them back, opens the chapter (downloaded once and checked), scrolls three rows by simulated trackpad pixels, and captures the window with macOS's shadow in light and then dark, putting the system's appearance back after. It needs Screen Recording and Accessibility for the terminal it runs in. Retake the shots after any change to the window's look; if the row spacing changes, the script's `SCROLL` (three one-line rows, 43 px each) changes with it. The Mac's display is 2560×1440 at 1×, so its shots are 1× (792×912 with the shadow); a Retina display would give 2×.

The Windows script does the same with three differences. It scrolls through WebView2's debugging port, opened on 127.0.0.1 for the run, since the window takes no injected mouse input. It captures the screen inside the window's frame, so the rounded corners and border are Windows' own, with a window of the site's page color behind (`--ground`) so the corners show the page. And it switches the app theme (`AppsUseLightTheme`) and puts it back after. The 9950X's display is 2560×1440 at 100%, so its shots are 682×832; Windows keeps the transcript's scrollbar showing, where macOS's fades.

- Show real transcripts of audio that may be published. Never put the app in a device mock-up, or shrink it until it's unreadable.
- No stock microphones, headphones or AI brains.
- The one exception is the mascot: a voxel budgie beside a speech bubble whose waveform moves when it talks, and a transcript card, drawn in WebGL (`ui/mascot.js`, from `scripts/mascot.py`). It belongs to the About pages, the app's and the site's, and nowhere else.

## Motion

Motion in pktito follows the audio or answers a hand. Nothing moves to decorate.

- In the app:
  - Progress moves along the waveform, and the transcript appears as each part finishes.
  - In playback the playhead moves, the playing word is underlined, and the transcript scrolls to keep the playing sentence in view.
  - A spinner beside the file's name marks a file at work.
  - The mascot turns and hops when it's dragged or clicked.
- Controls answer hover and press with the tint (see Color), and press moves a button 1 px.
- The site has no animation beyond that: a link's arrow moves 3 px on hover.
- With `prefers-reduced-motion`, transitions are off, the spinner stays still, playback's scroll jumps instead of gliding, and the mascot stays put.

## Iconography

Line icons after Feather (MIT): a 24 px grid and 2 px round strokes, drawn in `currentColor`, 16 px in the app.

- **The set:**
  - files: music (an audio file), film (a video file), file-text (a transcript), folder
  - actions: copy, check, download, plus, x, back, search
  - choosers: chevron down and up
  - alert, for an error toast
  - external-link: 12 px, after an outbound link's last word, held to it so it never starts a line
  - zap: 12 px, marks a caption that asks more than 20 characters a second of its reader
- **Play and pause** are filled, at 20 px.
- **On the site**, an outbound link ends in the → glyph. No emoji, and no icons in tinted squares.

## Logo

There is no pictorial mark. The name is the logo: the **Wordmark** sets "pktito" in Archivo 700, lowercase, followed by a `beak` square half an em wide on the baseline, like a playhead at the end of a line. In the app it is the button to About.

The app icon (`design/app-icon.svg`) is a `plume` rounded square with four `ink` waveform bars ending in the `beak` square. Its 230/1024 corner suits macOS; the Windows .ico and the rest of the Tauri icon set are made from the same SVG, and it is the site's favicon. Never recolor it, add a gradient, or set it on `plume`.

The model icon (`design/model-icon.svg`) marks pktito's model files, .pkf, in Explorer and Finder: a white sheet in the window's proportion, 1 by √2, its corner folded, with the app icon's four bars cut into steps, as int8 weights are, and the beak at their end. Below 128 px each size has its own drawing on whole pixels (`model-icon-64.svg` for 48 and 64, then `-32`, `-24` and `-16`, the last two with three bars); `scripts/model-icon.py` makes the .ico and .icns from them.

## Components

The site's components are plain HTML and CSS in `design/bundle.css` (classes `pk-*`): Button, Link, Wordmark and Stage. The app has its own stylesheet, `ui/app.css`, on the same tokens. Its patterns are documented here, so that a picture of the app or a new screen matches it:

- **Strip:** the waveform on `term`, play at its left and the speed under play; under the waveform the time (seconds or frames), the volume (a speaker that mutes, filled with `beak` while playback is silent, and a slider) and the length, with search ticks between. While a file is transcribed a ring in play's place fills with its progress, the percent inside, and the pace takes the time's place, so that the waveform keeps its pixels when it finishes. Its small buttons, the speed, the time and the speaker, are one box: 25 px tall, 6 px round their words, 14 px apart.
- **Transcript:** rows of a start time in `stamp` and a sentence in `transcript` type. The playing row sits on `surface`, with the beak square after its time and the playing word underlined. Clicking a time plays from it; a finger's tap plays the row from anywhere on it. Sentences | Captions switches the view.
- **Row:** a file on the start screen's recent list or in a batch: its icon, name, length and step, with a progress bar while it works and a remove × on hover.
- **Menu:** `ground`, `radius-ui`, `shadow-float` and a 1 px `sunk` ring. Its items are 14 px/500 with an optional second line, a check for the chosen one, key caps, rows of chips (speeds, frame rates), and dividers between groups.
- **Toast:** a few words on `term` at the bottom of the window, a check or an alert icon, and an optional Show in Finder (or File Explorer, or folder). It stays 3.5 s, or 6 s with an error or a file to show. An error's details are its tooltip.
- **Sheet:** a box on `ground` over the `scrim`, its header on `surface` with a close ×, for Privacy and the licenses.
- **Ask:** the question before something is destroyed: the destructive word in `err` on `sunk`, and Cancel in `plume` as the default, in the platform's order (Cancel first on macOS).
- **Drop overlay:** the whole window in `plume-press` while files are dragged over it, inset by a 2 px dashed `on-plume` edge, saying how many it will transcribe and listing their names.
- **Progress:** a 4 px track of `sunk` on `radius-dot`, filled with `wave` in light and `plume` in dark, and with `ink-muted` while paused.
- **Switch:** two segments on a `sunk` track; the chosen one is raised to `ground` with a 1 px `ink-muted` edge, since the tone step alone is 1.2:1.

pktito is an open-source speech-to-text engine: a small C11 port of NVIDIA's Parakeet-TDT models. It has two faces that share this system. The site, pktito.net, shows that it works and sends the reader to the source or the download. The desktop app, built with Tauri for macOS and Windows, does one thing: you pick mp3 or mp4 files, it pulls the audio out with the operating system's own media libraries (AVFoundation on macOS, Media Foundation on Windows), and it transcribes them on your machine.

The idea: **the transcript is the picture.** The site's hero is a transcript panel filling in word by word over the waveform it came from, on a plume-blue stage. The app is the same thing at working size: the waveform strip over the transcript, and in playback the word being spoken underlined. Neither describes the product when it can show a real fragment of it.

pktito is the sibling of pinfer and shares its type (Archivo and Commit Mono) and its plain, measured voice. The colors are its own.

## Voice

Plain, exact and checkable, as a good README is.

- **Say what happens to the file.** "pktito pulls the audio out of the mp4 and transcribes it on this computer." Name the formats (mp3, mp4), the platforms (macOS, Windows) and the model exactly as published (parakeet-tdt-0.6b-v2).
- **Credit the model plainly.** "A C11 port of NVIDIA's Parakeet-TDT models." Never use NVIDIA's logo, its green, or language that suggests endorsement.
- **Numbers only when measured.** A speed or accuracy figure carries its machine, model, file and date, or it is not shown. No "real-time", "fast" or "accurate" without a number beside it.
- **Describe the code as it stands.** No roadmap promises; a thing not yet done says so.
- **App copy is short and states what happens.** Buttons are verbs ("Add files", "Copy text", "Save as .txt"). Status says the step: "Extracting audio", "Transcribing 42%", "Done". Errors say what went wrong and what to try: "No audio track in this file. Try the original recording."
- American spelling. Sentence case everywhere. No exclamation marks, no emoji, no superlatives.
- Lowercase **pktito** in running text and in the wordmark.

Lines to model copy on:

> Drop a file. Read it.
> Nothing leaves your computer.
> The audio comes out through the media libraries your OS already has.

## Color

One ground, one ink, one accent, one mark. Every color is a token with a light and a dark value; components use tokens, never literals.

- `ground` is the page and the app window; `surface` and `sunk` are one and two tone steps off it. The app's file queue sits on `surface`; code blocks, the resting drop zone and empty progress tracks on `sunk`. Separate areas by that step, never by a rule or a shadow.
- `ink` (night navy, never black) sets all running and transcript text. `ink-muted` is for timestamps, durations, file sizes and legal text only.
- `plume` is the accent: the primary button, the site's stage, progress fill and the played part of a waveform. Text on it is `on-plume`, never white. It is never text on the ground.
- `beak` is a mark, not a color for areas: the square in the wordmark, the playhead, the token being decoded. Blue and orange stay distinct for color-blind readers because they also differ in lightness.
- `term` is the dark field of the stage's transcript panel and the waveform strip in both themes; `term-ink`, `term-dim`, `wave` and `wave-played` sit on it.
- `ok` and `err` mark a finished or failed file and always sit beside a word.
- Keyboard focus is `focus`, 2px solid: ink in light, plume in dark.
- Hover and press are one rule for every control, the wordmark included: `tint`, then `tint-press`, laid over whatever the control sits on or is filled with (ground, surface, sunk, plume), so it darkens in light and lightens in dark; on `term`, the strip and the toast, `tint-term` and `tint-term-press`. Links in running text thicken their underline instead.

## Type

Two families, the same files as pinfer.

- **Archivo** (400, 500, 700) for everything that is not code or a timestamp. Site: `display` for the hero, `figure` for the one number a section is about, `lead` for section headings, `body` for running text, `small` for nav and legal. App: `ui` (14px/500) for controls and file names, `transcript` (17px at 1.6) for the text people read, `figure` at 64px for a file's length on its detail view, `small` for status lines.
- **Commit Mono** (400, 700) only for what is literally code or time: commands, file names, flags, and every timestamp (`stamp`, always `hh:mm:ss` or `hh:mm:ss.cc`; or, with frames chosen from the strip's time, SMPTE's `hh:mm:ss:ff`, `hh:mm:ss;ff` at the drop-frame 29.97 and 59.94).
- Tabular figures (`tabular-nums`) wherever numbers line up: durations, percentages, timestamps.
- Headings use `text-wrap: balance`. Running text stays near `measure`.
- The fonts ship as files (`fonts/`), on the site and inside the app bundle. Never load a face from a CDN: the app must work offline, and so should the site's pages.
- Both faces are under the SIL Open Font License 1.1; keep the license texts (`Licenses`) beside the font files wherever they go.

## Layout

**Site.** One 12-column grid, outer margins `margin-page` (40px, 20px on a phone), sections `space-8` apart. Corners are square (`radius-0`); `radius-ui` appears only inside a `data-ui` fragment, which is a real piece of the app shown at true size.

**App.** One window, two panes: the file queue on `surface` at `sidebar` width, the transcript on `ground`. Minimum width `window-min` (480px); below 880px the queue becomes a strip above the transcript. Each screen's header row is the title bar, as in the apps of 2026: on macOS the system's own traffic lights sit at its left end, the window drawing no title bar; on Windows the app draws minimize, maximize and close at its right to Windows 11's measure (46 × 32, Segoe Fluent Icons, close red), as Chrome, Terminal and Notepad do; Linux keeps its desktop's title bar. The header's empty space drags the window. Never draw traffic lights. Controls use `radius-ui` so they sit with the OS's own. No toolbar icons without labels, but for copying, whose icon everyone reads: its words are its tooltip.

**The one ornament** is the `frame`: one 8px step for 80 ms of audio, the encoder's frame. The site's waveform bars sit a frame apart, and the duration blocks under a transcript are a whole number of frames wide, because TDT predicts each token with its duration. Use it on timelines only, never as decoration elsewhere. The app's waveform strip is the exception: it is a tool for finding a place in a recording, so it draws its bars as finely as the screen shows them, 2px wide and 1px apart.

No eyebrow labels, no 01 / 02 / 03, no thin dividers, no gradients, no glass, no shadows at rest (`shadow-float` is for menus, dialogs and the app window shown on the site).

## Imagery

pktito's pictures are its own interface, built in HTML at true size and marked `data-ui`: the transcript panel, a waveform strip, the app window, a command and its output. Show real transcripts of audio you have the right to publish, and say what the audio is. Never put the app inside a laptop mock-up, never shrink it until unreadable. No stock microphones, headphones or AI brains.

The one exception is the mascot on the app's About, its one kitschy corner: a voxel budgie beside a speech bubble, whose waveform moves when it talks, and a transcript card, made by `scripts/mascot.py`. It is an easter egg for whoever opens About, not a picture of the product. Neither the mascot nor its animation goes anywhere else, in the app or on the site.

## Motion

- The signature move: the stage's transcript fills in a token at a time while the playhead (`beak`) moves along the waveform, and each token's duration block lights from `wave` to `wave-played`. That is the one loud moment on the site.
- In the app, progress moves along the waveform and the transcript appears as segments finish; in playback the playhead moves and the underline follows the speech. Nothing else animates.
- Section parts on the site enter once, from a visible resting state, 60–90ms apart.
- Every control answers hover, press and focus, with transform and opacity only.
- With `prefers-reduced-motion`, the transcript shows whole, the playhead sits at the end, and progress jumps instead of sliding.

## Iconography

Few, and always with a label.

- The `Icons` group holds line icons after Feather (MIT): music (an audio file), film (a video file), file-text (a transcript), folder, copy, check, download, x. 24px grid, 2px round strokes, drawn in `ink`; inline, use `currentColor`. Show them at 16px in the app and 20px on the site.
- Arrows in running text are the `→` glyph. No emoji, no icons in tinted squares.

## Logo

There is no pictorial mark. The name is the logo: the **Wordmark** sets "pktito" in Archivo 700 followed by a `beak` square the size of the x-height, like a playhead at the end of a line.

The app icon (`Logos/app-icon.svg`) is a `plume` rounded square with four `ink` waveform bars ending in the `beak` square. Its 230/1024 corner suits macOS; generate the Windows .ico and the rest of the Tauri icon set from the same SVG. Never recolor it, add a gradient, or set it on `plume`.

## Components

- **Button**: square on the site, `radius-ui` in the app; primary in `plume`.
- **Wordmark**: the name and the beak square.
- **Stage**: the site's hero, a transcript panel filling in over its waveform.
- **DropZone**: where files enter the app.
- **FileQueue**: each file's step and progress.
- **Transcript**: timestamped segments, the thing people read and copy.
- **AppWindow**: the queue beside the transcript, once there are several files.

Components are plain HTML and CSS in `components/bundle.css` (classes `pk-*`); there is no script bundle. The Stage preview carries the few lines of script it needs.

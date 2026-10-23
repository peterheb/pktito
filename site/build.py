#!/usr/bin/env python3
"""pktito.net: the pages, built into site/public, which the Worker in
site/worker.js serves along with the installers (site/README.md). The look
is the design system the app uses: tokens.css from design/tokens.json, its
components from design/bundle.css, and the app's fonts from ui/fonts.

    python3 site/build.py                 # site/public, for wrangler
    python3 site/build.py --artifact DIR  # a preview: relative links, index.html without its head
"""
import base64
import html
import json
import pathlib
import shutil
import sys

SITE = pathlib.Path(__file__).resolve().parent
APP = SITE.parent
ARTIFACT = "--artifact" in sys.argv
OUT = pathlib.Path(sys.argv[sys.argv.index("--artifact") + 1]) if ARTIFACT else SITE / "public"
OUT.mkdir(parents=True, exist_ok=True)
GITHUB = "https://github.com/peterheb/pktito-app"  # the app, and this site
LIBRARY = "https://github.com/peterheb/pktito"     # the C library under it
e = html.escape


def page(name):
    """A page's address: clean on the Worker, the file itself in a preview."""
    if ARTIFACT:
        return f"{name or 'index'}.html"
    return f"/{name}"


ASSETS = "" if ARTIFACT else "/"  # root-relative on the site, where 404.html answers any path


def installer(key):
    return ("https://pktito.net" if ARTIFACT else "") + f"/download/{key}"


# ------------------------------------------------------------- tokens.css --
t = json.loads((APP / "design/tokens.json").read_text())


def cv(v):
    return f"var(--{v[1:-1]})" if v.startswith("{") else v


light, dark = [], []
for fam in ("color", "shadow"):
    for tok in t[fam]["tokens"]:
        v = tok["value"]
        lv, dv = (v, v) if isinstance(v, str) else (v["light"], v.get("dark", v["light"]))
        light.append(f"  --{tok['name']}: {cv(lv)};")
        if dv != lv:
            dark.append(f"  --{tok['name']}: {cv(dv)};")
for fam in ("spacing", "radius", "layout"):
    for tok in t[fam]["tokens"]:
        light.append(f"  --{tok['name']}: {tok['value']};")
for k, v in t["type"]["families"].items():
    light.append(f"  --font-{k}: {v};")
faces = "\n".join(
    f'@font-face {{ font-family: "{f["family"]}"; src: url("{f["file"]}") format("woff2"); '
    f'font-weight: {f["weight"]}; font-style: normal; font-display: swap; }}'
    for f in t["type"]["fonts"])
nl = "\n"
tokens_css = f"""/* pktito tokens, from the design system's tokens.json. Light on :root,
   dark under the system setting unless the page is set light, and again
   when it is set dark. */

{faces}

:root {{
  color-scheme: light;
{nl.join(light)}
}}

@media (prefers-color-scheme: dark) {{
  :root:not([data-theme="light"]) {{
    color-scheme: dark;
{nl.join("  " + d for d in dark)}
  }}
}}

:root[data-theme="dark"] {{
  color-scheme: dark;
{nl.join(dark)}
}}

/* a phone: margins 20px, sections 96px apart (tokens.json, usage) */
@media (max-width: 640px) {{
  :root {{ --margin-page: 20px; --space-8: 96px; }}
}}
"""

# ----------------------------------------------------------------- pieces --
NAV = [("download", "Download"), ("models", "Models"), ("about", "About")]


def header(current):
    links = []
    for name, label in NAV:
        here = ' aria-current="page"' if name == current else ""
        links.append(f'<a href="{page(name)}"{here}>{label}</a>')
    links.append(f'<a href="{GITHUB}">GitHub</a>')
    return f"""<header class="site-head">
  <div class="wrap">
    <a class="pk-wordmark" href="{page("")}" aria-label="pktito, home">pktito</a>
    <nav class="site-nav" aria-label="Site">{"".join(links)}</nav>
  </div>
</header>"""


def footer():
    return f"""<footer class="site-foot">
  <div class="wrap">
    <a class="pk-wordmark" href="{page("")}" aria-label="pktito, home">pktito</a>
    <p class="legal">MIT. The models are NVIDIA's Parakeet-TDT, under CC-BY-4.0; pktito is not affiliated with NVIDIA. This site sets no cookies, and counts installer downloads and nothing else. <a href="{page("privacy")}">Privacy</a></p>
  </div>
</footer>"""


def write(name, title, description, main, script=""):
    styles = "\n".join(f'<link rel="stylesheet" href="{ASSETS}{c}">' for c in ("tokens.css", "bundle.css", "site.css"))
    body = f"{header(name)}\n\n{main}\n\n{footer()}\n{script}"
    if ARTIFACT and name == "":
        doc = (f'<title>{title}</title>\n<meta name="claude:viewport-fit-sides" content="cover">\n'
               f'<meta name="description" content="{e(description)}">\n{styles}\n\n{body}\n')
    else:
        doc = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{title}</title>
<meta name="description" content="{e(description)}">
<link rel="icon" href="{ASSETS}favicon.svg" type="image/svg+xml">
{styles}
</head>
<body>
{body}
</body>
</html>
"""
    (OUT / f"{name or 'index'}.html").write_text(doc)


def arrow(text, href):
    return f'<a class="pk-link" href="{href}">{text} <span class="pk-arrow" aria-hidden="true">&rarr;</span></a>'


ICO = {
    "back": '<line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/>',
    "check": '<polyline points="20 6 9 17 4 12"/>',
    "copy": '<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>',
    "search": '<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>',
    "down": '<polyline points="6 9 12 15 18 9"/>',
}


def ico(name, width=2):
    return (f'<svg class="pk-ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="{width}" '
            f'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{ICO[name]}</svg>')


def stamp(s):
    cs = int(s * 100 + 0.5)
    return f"{cs // 360000:02d}:{cs // 6000 % 60:02d}:{cs // 100 % 60:02d}.{cs % 100:02d}"


# ------------------------------------------------------------------ home --
# The app's own transcript, sentence by sentence as it shows one: LibriSpeech
# test-clean 1089-134686's nine recordings joined, 64.8 s, read by
# `pktito-app --transcribe` with parakeet-tdt-0.6b-v2 in 0.580 s on an M4 Pro,
# 2026-10-20. The seventh's missing full stop is the model's. The window shows
# it paused at 00:00:17.32, on "lamps", which pktito times at 17.15 s.
JOYCE = [
    (0.51, "He hoped there would be stew for dinner, turnips and carrots and bruised potatoes and fat mutton pieces to be ladled out in thick, peppered, flour fattened sauce."),
    (11.39, "Stuff it into you, his belly counselled him."),
    (15.15, "After early nightfall the yellow lamps would light up, here and there, the squalid quarter of the brothels."),
    (22.35, "Hello, Bertie, any good in your mind?"),
    (25.55, "Number ten."),
    (26.91, "Fresh Nelly is waiting on you."),
    (29.47, "Good night, husband The music came nearer and he recalled the words the words of Shelley's fragment upon the moon wandering companionless, pale for weariness."),
    (41.79, "The dull light fell more faintly upon the page, whereon another equation began to unfold itself slowly and to spread abroad its widening tail."),
    (52.99, "A cold, lucid indifference reigned in his soul."),
    (57.79, "The chaos in which his ardour extinguished itself was a cold, indifferent knowledge of himself."),
]
AT, NOW = 17.32, 2  # the playhead, and the segment under it


def seg(i, start, text):
    body = e(text).replace("lamps", '<span class="as-now">lamps</span>', 1) if i == NOW else e(text)
    return f'<div class="as-seg{" is-current" if i == NOW else ""}"><span class="as-t">{stamp(start)}</span><p>{body}</p></div>'


segs = "\n".join(seg(i, s, x) for i, (s, x) in enumerate(JOYCE))
# The waveform as the app draws it (ui/app.js, drawWave): its outline of
# the file, audio.rs's 2400 slices, each its RMS in dB over the 48 dB below
# the loudest as 0 to 255 (from the app's recent list, 2026-10-22); a bar
# the mean of the slices under it, over the outline's range from its 10th
# percentile to its 99.5th; 3 to 60 px, the playhead 64.
OUTLINE = base64.b64decode(
    "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"
)
DURATION = 64.8250625


def outline_range():
    s = sorted(OUTLINE)
    lo, hi = s[int(len(s) * 0.1)], s[min(len(s) - 1, int(len(s) * 0.995))]
    return lo, max(hi, lo + 16)


def wave(n, cls):
    """n bars 2 px wide: as many as the app puts in a strip 3n - 1 to
    4n - 2 px wide, the page spreading them to its strip's width."""
    lo, hi = outline_range()
    hs = []
    for i in range(n):
        a = i * len(OUTLINE) // n
        b = max(a + 1, (i + 1) * len(OUTLINE) // n)
        v = min(1.0, max(0.0, (sum(OUTLINE[a:b]) / (b - a) - lo) / (hi - lo)))
        hs.append(3 + int(v * 57 + 0.5))
    head = int(AT / DURATION * n)
    bars = "".join(f'<i class="{"on" if i < head else "head" if i == head else ""}" style="height:{64 if i == head else h}px"></i>'
                   for i, h in enumerate(hs))
    return f'<span class="as-wave {cls}">{bars}</span>'


# one for each strip width the page gives it (site.css, .as-wN)
BARS = (289, 217, 163, 122, 92, 69)
waves = "".join(wave(n, f"as-w{n}") for n in BARS)
FILE = "librispeech-1089-134686.wav"
PLAY = '<svg class="pk-ico" viewBox="0 0 24 24" aria-hidden="true"><polygon points="6 3 20 12 6 21 6 3" fill="currentColor"/></svg>'

home = f"""<main>
<section class="hero wrap">
  <h1 class="display">Drop a file. Read it.</h1>
  <div class="hero-side">
    <p class="strong">Nothing leaves your computer.</p>
    <p class="measure">pktito pulls the audio out of an mp3 or mp4 and transcribes it. For macOS, Windows and Linux.</p>
    <div class="actions">
      <a class="pk-btn pk-btn-primary" href="{page("download")}">Download</a>
      {arrow("Source on GitHub", GITHUB)}
    </div>
  </div>
</section>

<section class="pk-stage home-stage" aria-label="The app, with a transcript it made">
  <div class="wrap">
    <div class="app-shot" data-ui>
      <div class="as-head">
        <span class="as-back">{ico("back")}</span><h2>{FILE}</h2><span class="as-grow"></span>
        <span class="as-find">{ico("search")}<span>Find</span></span>
        <span class="pk-btn pk-btn-quiet as-ico">{ico("copy")}</span>
        <span class="pk-btn pk-btn-primary">Save as{ico("down")}</span>
      </div>
      <div class="as-strip">
        <span class="as-play">{PLAY}</span>
        <span class="as-lane">{waves}<span class="as-ticks"></span>
          <span class="as-times"><span class="as-time">{stamp(AT)}</span><span class="as-time as-dim">00:01:04</span></span></span>
      </div>
      <div class="as-views"><span class="is-on">Sentences</span><span>Captions</span></div>
      <div class="as-transcript">
{segs}
      </div>
    </div>
  </div>
</section>
<div class="wrap">
  <p class="provenance after-stage">The app as it is, with its transcript of James Joyce read for LibriVox (LibriSpeech test-clean 1089-134686, CC BY 4.0): 64.8 s, transcribed in 0.58 s on an M4 Pro with parakeet-tdt-0.6b-v2, 2026-10-20.</p>
</div>

<section class="facts wrap">
  <div>
    <h2>On your computer</h2>
    <p>The audio never leaves it. The app downloads its model once, when you first open it, and then works offline.</p>
  </div>
  <div>
    <h2>The files you have</h2>
    <p>Your system's own media libraries read mp3, mp4, m4a, mov, wav and flac. With ffmpeg installed, so does everything else we tried.</p>
  </div>
  <div>
    <h2>Open source</h2>
    <p>MIT licensed. The engine is a C11 library for anything with a C compiler, a Raspberry Pi included.</p>
    <p>{arrow("github.com/peterheb/pktito", LIBRARY)}</p>
  </div>
</section>
</main>"""

write("", "pktito.net",
      "pktito transcribes audio and video on your computer: a C11 port of NVIDIA's Parakeet-TDT models, as an app for macOS, Windows and Linux.",
      home)

# -------------------------------------------------------------- download --
download = f"""<main class="wrap">
<section class="page-head">
  <h1 class="display">Download</h1>
  <p class="measure">On its first run the app downloads one model, chosen for your system's language: 624 MB for English, 648 MB for 25 European languages.</p>
</section>

<section class="part">
  <div class="tiles">
    <div class="tile">
      <h2 class="lead">macOS</h2>
      <p>macOS 12 or later, Apple silicon.</p>
      <div class="buttons"><a class="pk-btn pk-btn-ink" href="{installer("macos")}">Download for macOS</a></div>
    </div>
    <div class="tile">
      <h2 class="lead">Windows</h2>
      <p>Windows 10 or later, 64-bit Intel or AMD.</p>
      <div class="buttons"><a class="pk-btn pk-btn-ink" href="{installer("windows")}">Download for Windows</a></div>
    </div>
    <div class="tile">
      <h2 class="lead">Linux</h2>
      <p>64-bit Intel or AMD, with ffmpeg: <code>sudo apt install ffmpeg</code></p>
      <div class="buttons"><a class="pk-btn pk-btn-ink" href="{installer("linux-deb")}">Download .deb</a><a class="pk-btn pk-btn-quiet" href="{installer("linux-appimage")}">AppImage</a></div>
    </div>
  </div>
</section>

<section class="part">
  <h2 class="lead">Files it reads</h2>
  <dl class="kv">
    <dt>Every system</dt><dd>MP4, MOV, M4A, MP3, WAV, FLAC, MPEG-TS</dd>
    <dt>Also macOS</dt><dd>AIFF, CAF, Ogg, Opus, AC-3</dd>
    <dt>Also Windows</dt><dd>WebM, Matroska, WMA, WMV</dd>
    <dt>With ffmpeg</dt><dd>All of these on every system, and FLV and DTS</dd>
  </dl>
</section>
</main>"""

write("download", "Download pktito",
      "The pktito app for macOS, Windows and Linux: drop audio or video files and read their transcripts, on your computer.",
      download)

# ---------------------------------------------------------------- models --
LANGS = ("Bulgarian, Croatian, Czech, Danish, Dutch, English, Estonian, Finnish, French, German, Greek, "
         "Hungarian, Italian, Latvian, Lithuanian, Maltese, Polish, Portuguese, Romanian, Russian, Slovak, "
         "Slovenian, Spanish, Swedish and Ukrainian")
MODELS = [  # the app's catalog (src-tauri/src/model.rs)
    ("parakeet-tdt-0.6b-v2", "English.", 624_480_196, "8bfa6750b83cd7c447bad82057c7a12473563b72b3d35487fcca6da0786d83c7"),
    ("parakeet-tdt-0.6b-v3", f"25 European languages: {LANGS}.", 647_552_644,
     "35a59c9a433d4da2289640db93e73c42478b576e49a293090ef61b4ae595152a"),
]


def model_html(name, speaks, size, sha):
    url = f"https://models.pktito.net/{sha[:12]}/{name}.pkf"
    return f"""<article class="model">
    <h2 class="lead"><code>{name}</code></h2>
    <p>{speaks}</p>
    <dl class="kv">
      <dt>File</dt><dd><code>{url}</code><button class="copy" type="button" data-copy="{url}">Copy</button></dd>
      <dt>Size</dt><dd class="pk-num">{size:,} bytes</dd>
      <dt>SHA-256</dt><dd><code>{sha}</code><button class="copy" type="button" data-copy="{sha}">Copy</button></dd>
    </dl>
  </article>"""


models = f"""<main class="wrap">
<section class="page-head">
  <h1 class="display">Models</h1>
  <p class="measure">The app downloads the one you choose when it first opens; the C library reads either. Each file's address holds its SHA-256, so it never changes.</p>
</section>

<section class="part">
  {model_html(*MODELS[0])}
  {model_html(*MODELS[1])}
  <p class="provenance">NVIDIA's <a href="https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2">parakeet-tdt-0.6b-v2</a> and <a href="https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3">-v3</a>, under CC-BY-4.0, with their weights rounded to int8 and rearranged by pktito-tools. Each file also holds Silero VAD (MIT).</p>
</section>
</main>"""

COPY_JS = """<script>
/* Copy buttons: the clipboard where the browser allows it, else the text selected for Ctrl+C. */
document.querySelectorAll(".copy").forEach((b) => b.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(b.dataset.copy);
    b.textContent = "Copied";
  } catch {
    const r = document.createRange();
    r.selectNodeContents(b.previousElementSibling);
    getSelection().removeAllRanges();
    getSelection().addRange(r);
    b.textContent = "Selected";
  }
  setTimeout(() => { b.textContent = "Copy"; }, 1600);
}));
</script>"""

write("models", "pktito models",
      "The two models pktito reads, parakeet-tdt-0.6b-v2 (English) and -v3 (25 European languages): addresses, sizes and SHA-256.",
      models, COPY_JS)

# ----------------------------------------------------------------- about --
about = f"""<main class="wrap">
<section class="page-head">
  <h1 class="display">About</h1>
  <p class="measure">pktito is a C11 port of NVIDIA's Parakeet-TDT speech recognizers: an app for macOS, Windows and Linux, and a library for anything with a C compiler. Both are MIT licensed, on GitHub: {arrow("the app", GITHUB)} and {arrow("the library", LIBRARY)}</p>
</section>

<section class="part">
  <h2 class="lead">Privacy</h2>
  <p class="measure">The app sends nothing anywhere. It downloads the model you choose from models.pktito.net, once; there is no account and no telemetry. This site sets no cookies. It counts how many times each installer is downloaded, a day at a time, and keeps nothing else: no addresses, nothing that tells one visitor from another. {arrow("The privacy policy", page("privacy"))}</p>
</section>

<section class="part">
  <h2 class="lead">Licenses</h2>
  <dl class="kv">
    <dt>pktito</dt><dd>MIT</dd>
    <dt>The models</dt><dd>CC-BY-4.0, NVIDIA</dd>
    <dt>Silero VAD</dt><dd>MIT, Copyright (c) 2020-present Silero Team</dd>
    <dt>Typefaces</dt><dd>Archivo and Commit Mono, SIL Open Font License 1.1 (<a href="{ASSETS}fonts/Archivo-OFL.txt">Archivo</a>, <a href="{ASSETS}fonts/CommitMono-OFL.txt">Commit Mono</a>)</dd>
    <dt>Icons</dt><dd>After Feather, MIT</dd>
    <dt>The recording</dt><dd>LibriSpeech, CC BY 4.0; LibriVox, public domain</dd>
  </dl>
  <p class="measure">pktito is not affiliated with or endorsed by NVIDIA.</p>
</section>
</main>"""

write("about", "About pktito",
      "What pktito is, what it sends where (nothing, past a model download), and the licenses of its code, models and typefaces.",
      about)

# --------------------------------------------------------------- privacy --
# the app's About > Privacy (ui/index.html) says the same of the app; the two
# change together, and keep the date
privacy = f"""<main class="wrap">
<section class="page-head">
  <h1 class="display">Privacy</h1>
  <p class="measure">pktito turns speech into text on your computer. Your recordings, transcripts and settings stay there. The app doesn't send them anywhere, and it has no accounts, ads or tracking. This page says what does reach the internet, and what happens to it.</p>
  <p class="measure legal-date">Last updated October 23, 2026</p>
</section>

<section class="part">
  <h2 class="lead">Downloading a model</h2>
  <p class="measure">The app downloads its speech model from models.pktito.net, which bunny.net, a content delivery network, serves. As with any website, the server receives your IP address, the time, the file requested and a standard user-agent string. The app sends nothing else: no name, account or device ID, and nothing from your files.</p>
  <p class="measure">The server logs these requests to keep the service secure and working. We use the logs only to count downloads in aggregate. We don't use them to identify you, combine them with other data, or sell or share them. To skip the download, use a model file you already have: choose “Open a model file…” in the app.</p>
</section>

<section class="part">
  <h2 class="lead">This website</h2>
  <p class="measure">pktito.net sets no cookies and runs no analytics. Cloudflare serves it, and like any web host it handles your IP address and browser details to deliver the pages and protect the site from abuse. When an installer download starts, the site adds one to that file’s count for the day. It keeps nothing else: no addresses, nothing that tells one visitor from another.</p>
</section>

<section class="part">
  <h2 class="lead">Links</h2>
  <p class="measure">Links in the app and on this site go to other sites, such as GitHub and Hugging Face. Their own privacy policies apply there.</p>
</section>

<section class="part">
  <h2 class="lead">Your data</h2>
  <p class="measure">Everything the app keeps is on your computer, in its data folder: your settings, the models and the recent transcripts. Clearing the recent list deletes its transcripts. Uninstalling the app removes the app, and the data folder stays until you delete it. We hold nothing about you to look up, correct or delete.</p>
</section>

<section class="part">
  <h2 class="lead">Changes and questions</h2>
  <p class="measure">If this policy changes, this page changes with it, under a new date, and the app’s copy comes with its next update. Ask questions {arrow("on GitHub", GITHUB + "/issues")}</p>
</section>
</main>"""

write("privacy", "pktito: privacy",
      "What pktito sends where: the app transcribes on your computer, and a model download and this site are the only things that reach the internet.",
      privacy)

# ------------------------------------------------------------------- 404 --
if not ARTIFACT:
    write("404", "Not here",
          "Nothing lives at this address on pktito.net.",
          f"""<main class="wrap">
<section class="page-head">
  <h1 class="display">Not here.</h1>
  <p class="measure">Nothing lives at this address. {arrow("pktito.net", page(""))}</p>
</section>
</main>""")

# ------------------------------------------------------------------ soon --
# where /download/<system> goes while no release names a file for it
if not ARTIFACT:
    write("soon", "pktito: coming soon",
          "The first release of the pktito app is on its way.",
          f"""<main class="wrap">
<section class="page-head">
  <h1 class="display">Coming soon.</h1>
  <p class="measure">The first release is on its way. {arrow("The source is on GitHub", GITHUB)}</p>
</section>
</main>""")

# ---------------------------------------------------------- the rest of it --
(OUT / "fonts").mkdir(parents=True, exist_ok=True)
for f in (APP / "ui/fonts").iterdir():
    if f.suffix in (".woff2", ".txt"):
        shutil.copy(f, OUT / "fonts" / f.name)
(OUT / "tokens.css").write_text(tokens_css)
shutil.copy(APP / "design/bundle.css", OUT / "bundle.css")
shutil.copy(SITE / "site.css", OUT / "site.css")
shutil.copy(APP / "design/app-icon.svg", OUT / "favicon.svg")
print("\n".join(sorted(str(p.relative_to(OUT)) for p in OUT.rglob("*") if p.is_file())), file=sys.stderr)

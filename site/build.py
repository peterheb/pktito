#!/usr/bin/env python3
"""pktito.net: the pages, built into site/public, which the Worker in
site/worker.js serves along with the installers (site/README.md). The look
is the design system the app uses: tokens.css from design/tokens.json,
bundle.css and the fonts from ui/.

    python3 site/build.py                 # site/public, for wrangler
    python3 site/build.py --artifact DIR  # a preview: relative links, index.html without its head
"""
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
GITHUB = "https://github.com/peterheb/pktito"
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
    <p class="legal">MIT. The models are NVIDIA's Parakeet-TDT, under CC-BY-4.0; pktito is not affiliated with NVIDIA. This site sets no cookies and runs no analytics.</p>
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
    "music": '<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
    "check": '<polyline points="20 6 9 17 4 12"/>',
    "copy": '<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>',
    "plus": '<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>',
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
# 2026-10-20. The seventh's missing full stop is the model's.
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
segs = "\n".join(f'<div class="pk-seg"><span class="pk-stamp">{stamp(s)}</span><p>{e(x)}</p></div>' for s, x in JOYCE)
FILE = "librispeech-1089-134686.wav"

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
    <div class="pk-app app-shot" data-ui>
      <aside class="pk-app-side">
        <div class="pk-drop pk-drop-sm">{ico("plus")}<strong>Add files</strong></div>
        <ul class="pk-queue" aria-label="Files">
          <li class="pk-file" aria-selected="true">{ico("music")}<span class="pk-name">{FILE}</span><span class="pk-stamp">00:01:04</span><span class="pk-status ok">{ico("check", 2.4)}Done</span></li>
        </ul>
        <div class="app-model">
          <span class="app-label">Language</span><span class="app-select">English</span>
          <p>parakeet-tdt-0.6b-v2, on this computer</p>
        </div>
      </aside>
      <div class="pk-app-main">
        <div class="pk-app-head">
          <h2>{FILE}</h2>
          <span class="pk-btn pk-btn-quiet">{ico("copy")}Copy text</span>
          <span class="pk-btn pk-btn-quiet">Save as .txt</span>
          <span class="pk-btn pk-btn-quiet">Save as .srt</span>
        </div>
        <div class="pk-transcript">
{segs}
        </div>
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
    <p>The audio never leaves it. The model downloads once, the first time a file needs it, and then the app works offline.</p>
  </div>
  <div>
    <h2>The files you have</h2>
    <p>Your system's own media libraries read mp3, mp4, m4a, mov, wav and flac. With ffmpeg installed, so does everything else we tried.</p>
  </div>
  <div>
    <h2>Open source</h2>
    <p>MIT licensed. The engine is a C11 library for anything with a C compiler, a Raspberry Pi included.</p>
    <p>{arrow("github.com/peterheb/pktito", GITHUB)}</p>
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
  <p class="measure">The first file you add downloads its model once: 624 MB for English, 648 MB for 25 European languages.</p>
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
  <p class="measure">The app downloads the one it needs the first time a file needs it; the C library reads either. Each file's address holds its SHA-256, so it never changes.</p>
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
  <p class="measure">pktito is a C11 port of NVIDIA's Parakeet-TDT speech recognizers: an app for macOS, Windows and Linux, and a library for anything with a C compiler. It is MIT licensed, and its source is on {arrow("GitHub", GITHUB)}</p>
</section>

<section class="part">
  <h2 class="lead">Privacy</h2>
  <p class="measure">The app sends nothing anywhere. It downloads a model from models.pktito.net the first time it needs one; there is no account and no telemetry. This site sets no cookies and runs no analytics.</p>
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

# ---------------------------------------------------------- the rest of it --
(OUT / "fonts").mkdir(parents=True, exist_ok=True)
for f in (APP / "ui/fonts").iterdir():
    if f.suffix in (".woff2", ".txt"):
        shutil.copy(f, OUT / "fonts" / f.name)
(OUT / "tokens.css").write_text(tokens_css)
shutil.copy(APP / "ui/bundle.css", OUT / "bundle.css")
shutil.copy(SITE / "site.css", OUT / "site.css")
shutil.copy(APP / "design/app-icon.svg", OUT / "favicon.svg")
print("\n".join(sorted(str(p.relative_to(OUT)) for p in OUT.rglob("*") if p.is_file())), file=sys.stderr)

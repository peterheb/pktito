/* pktito's window. Setup picks a model and downloads it; then each file
   dropped on the window is transcribed in turn by the Rust side
   (src-tauri/src/main.rs), shown filling in as its chunks finish, and then
   played back against its transcript word by word, searched, copied and
   saved. One file has the window to itself; several at once are a batch,
   a screen of their progress from which each opens and all are saved.
   Finished transcripts stay on the recent list. */
"use strict";

const T = window.__TAURI__;
const invoke = T.core.invoke;
const listen = T.event.listen;

const $ = (id) => document.getElementById(id);

/* The window's own title bar, where it has none (src-tauri/src/main.rs,
   main_window): each screen's header row is the title bar. On macOS the
   traffic lights are at its left end, which the rows make room for (app.css,
   html[data-chrome]) but in full screen, which shows none. On Windows the
   page draws the caption buttons at its right: maximize's glyph is restore's
   while the window is maximized, and all three are dimmed while another
   window is active, as Windows 11's are. */
if (window.__PK_CHROME) {
  const html = document.documentElement, win = T.window.getCurrentWindow();
  html.dataset.chrome = window.__PK_CHROME;
  const state = async () => {
    try {
      if (window.__PK_CHROME === "mac") html.toggleAttribute("data-fullscreen", await win.isFullscreen());
      else {
        const big = await win.isMaximized();
        $("cap-max").innerHTML = big ? "&#xE923;" : "&#xE922;";
        $("cap-max").title = big ? "Restore down" : "Maximize";
        $("cap-max").setAttribute("aria-label", $("cap-max").title);
      }
    } catch {}
  };
  if (window.__PK_CHROME === "win") {
    $("caption").hidden = false;
    $("cap-min").onclick = () => win.minimize();
    $("cap-max").onclick = () => win.toggleMaximize();
    $("cap-close").onclick = () => win.close();
    window.addEventListener("blur", () => html.toggleAttribute("data-inactive", true));
    window.addEventListener("focus", () => html.toggleAttribute("data-inactive", false));
  }
  window.addEventListener("resize", state);
  state();
}
const svg = (body, cls = "pk-ico") => `<svg class="${cls}" viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;
const ICON = {
  music: svg('<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>'),
  film: svg('<rect x="2" y="2" width="20" height="20" rx="2.18"/><line x1="7" y1="2" x2="7" y2="22"/><line x1="17" y1="2" x2="17" y2="22"/>' +
    '<line x1="2" y1="12" x2="22" y2="12"/><line x1="2" y1="7" x2="7" y2="7"/><line x1="2" y1="17" x2="7" y2="17"/>' +
    '<line x1="17" y1="17" x2="22" y2="17"/><line x1="17" y1="7" x2="22" y2="7"/>'),
  file: svg('<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/>' +
    '<line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>'),
  copy: svg('<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>'),
  check: svg('<polyline points="20 6 9 17 4 12"/>'),
  down: svg('<polyline points="6 9 12 15 18 9"/>'),
  up: svg('<polyline points="18 15 12 9 6 15"/>'),
  download: svg('<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>'),
  plus: svg('<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>'),
  alert: svg('<circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>'),
  x: svg('<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>'),
  folder: svg('<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>'),
  back: svg('<line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/>'),
  play: svg('<polygon points="6 3 20 12 6 21 6 3"/>', "pk-ico pk-ico-20 pk-ico-fill"),
  pause: svg('<rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>', "pk-ico pk-ico-20 pk-ico-fill"),
  zap: svg('<polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>', "pk-ico pk-ico-12"),
  external: svg('<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/>' +
    '<line x1="10" y1="14" x2="21" y2="3"/>', "pk-ico pk-ico-12"),
};

let st = null;          // the Rust side's status: the model, the catalog, the platform
let prefs = {};         // how transcripts are saved
let dl = null;          // the model download, as its last event told it
let choice = null;      // the model picked on the setup screen
let recent = [];        // the recent list, newest first
const files = [];       // the files open: one, or a batch, in the order they are transcribed
let batch = false;      // they are a batch, which the batch screen lists
let batchSave = null;   // what Save all chose while some of the batch was still to come
let sel = null;         // the id of the file shown; none for the batch screen
let nextId = 1;
let running = false;    // a transcription is under way
let home = false;       // the start screen is shown, files still under way or not
let about = false;      // About is shown, over the screen that opened it

/* ---------------------------------------------------------- formatting -- */

const two = (n) => String(n).padStart(2, "0");
function hms(s) {
  s = Math.max(0, Math.floor(s));
  return `${two(Math.floor(s / 3600))}:${two(Math.floor(s / 60) % 60)}:${two(s % 60)}`;
}
function stamp(s) {
  const cs = Math.max(0, Math.round(s * 100));
  return `${hms(Math.floor(cs / 100))}.${two(cs % 100)}`;
}
function msTime(s, sep) {
  const ms = Math.max(0, Math.round(s * 1000));
  return `${hms(Math.floor(ms / 1000))}${sep}${String(ms % 1000).padStart(3, "0")}`;
}
/* m:ss, or h:mm:ss from an hour */
function clock(s) {
  s = Math.max(0, Math.round(s));
  const h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60;
  return h ? `${h}:${two(m)}:${two(s % 60)}` : `${m}:${two(s % 60)}`;
}
/* how many seconds of audio a second's work transcribed */
function realTime(audio, secs) {
  return `${(audio / secs).toFixed(1)}x real-time`;
}
function timeLeft(s) {
  if (!(s >= 0) || !isFinite(s)) return "";
  if (s < 50) return `about ${Math.max(5, Math.round(s / 5) * 5)} s left`;
  return `about ${Math.round(s / 60)} min left`;
}
/* "Today", "Yesterday", else the date as the system writes it */
function when(t) {
  const d = new Date(t * 1000), now = new Date();
  const day = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const ago = Math.round((day(now) - day(d)) / 864e5);
  if (ago === 0) return "Today";
  if (ago === 1) return "Yesterday";
  const o = { month: "short", day: "numeric" };
  if (d.getFullYear() !== now.getFullYear()) o.year = "numeric";
  return d.toLocaleDateString(undefined, o);
}
/* A frame rate counted exactly: NTSC's 23.976, 29.97 and 59.94 are 24, 30
   and 60 frames in 1.001 seconds, whatever their rounding says. */
function exactRate(r) {
  for (const n of [24, 30, 48, 60, 120]) if (Math.abs(r - (n * 1000) / 1001) < 0.01) return (n * 1000) / 1001;
  return Math.abs(r - Math.round(r)) < 0.01 ? Math.round(r) : r;
}
/* t as SMPTE timecode at a rate: hh:mm:ss:ff, counting the frames of its
   nominal rate (24 for 23.976, 30 for 29.97), the frame t falls in. At
   29.97 and 59.94 it is drop-frame, hh:mm:ss;ff: the first 2 (or 4) frame
   numbers of every minute but each tenth are skipped, so that the clock
   keeps to real time. */
function timecode(t, rate) {
  const r = exactRate(rate), nominal = Math.round(r);
  const drop = Math.abs(r - 30000 / 1001) < 1e-9 || Math.abs(r - 60000 / 1001) < 1e-9;
  let frames = Math.floor(Math.max(0, t) * r + 1e-6);
  if (drop) {
    const d = nominal === 60 ? 4 : 2, minute = nominal * 60 - d, tens = minute * 10 + d;
    const rem = frames % tens;
    frames += 9 * d * Math.floor(frames / tens) + (rem >= d ? d * Math.floor((rem - d) / minute) : 0);
  }
  const s = Math.floor(frames / nominal);
  return `${two(Math.floor(s / 3600))}:${two(Math.floor(s / 60) % 60)}:${two(s % 60)}${drop ? ";" : ":"}${two(frames % nominal)}`;
}
const rateLabel = (r) => String(Math.round(exactRate(r) * 1000) / 1000);
/* the rate a file's times are counted in: a video's own, unless the one
   chosen is to count a video's too */
const ownRate = (f) => !!f && f.fps > 0 && prefs.video_rate !== false;
const rateOf = (f) => (ownRate(f) ? f.fps : prefs.fps > 0 ? prefs.fps : 24);
/* a time as the strip and the transcript show it: seconds and hundredths, or frames */
const tc = (t, f = current()) => (prefs.frames ? timecode(t, rateOf(f)) : stamp(t));
/* a file's length as the strip shows it */
const lengthOf = (f) => (prefs.frames ? timecode(f.duration, rateOf(f)) : hms(f.duration));

const baseName = (p) => p.split(/[\\/]/).pop();
const stemOf = (p) => baseName(p).replace(/\.[^.]+$/, "");
const extOf = (p) => (baseName(p).match(/\.([^.]+)$/) || ["", ""])[1].toLowerCase();
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const mb = (b) => Math.round(b / 1e6);
const percent = (x) => Math.floor(Math.min(1, Math.max(0, x)) * 100);
const plural = (n, one) => `${n} ${one}${n === 1 ? "" : "s"}`;

/* the wordmark on each screen, which is the way to About there */
const WORDMARK = '<button class="pk-wm" type="button" data-about aria-label="About pktito" title="About pktito">pktito</button>';

const isMac = () => st.platform === "macos";
const isWin = () => st.platform === "windows";
const kbd = (k, shift = false) => (isMac() ? `${shift ? "⇧" : ""}⌘${k}` : `Ctrl+${shift ? "Shift+" : ""}${k}`);
/* A control's tooltip with its shortcut: the window itself shows none,
   so that its words are the transcript's. */
const tip = (label, k, shift = false) => `${label} (${kbd(k, shift)})`;
const revealLabel = () => (isMac() ? "Show in Finder" : isWin() ? "Show in File Explorer" : "Show in folder");
const kindOf = (p) => (st.video.includes(extOf(p)) ? "video" : "audio");
const isMedia = (p) => st.media.includes(extOf(p));

/* --------------------------------------------------------------- views -- */

function viewName() {
  if (!st) return "";
  if (!st.ready || dl) return "setup";
  if (!files.length || home) return "ready";
  return batch && !current() ? "batch" : "work";
}

/* The view in sight: About covers the one beneath, which goes on as ever. */
const shown = () => (about ? "about" : viewName());

function render() {
  const v = viewName();
  for (const id of ["setup", "ready", "batch", "work"]) $(id).hidden = id !== v;
  if (v !== "work") pausePlayback();
  if (v === "setup") renderSetup();
  else if (v === "ready") renderReady();
  else if (v === "batch") renderBatch();
  else if (v === "work") renderWork();
}

async function refresh() {
  st = await invoke("status");
  prefs = st.prefs;
  dl = st.download;
  render();
  pump();
}

const current = () => files.find((f) => f.id === sel) || null;
const waiting = () => files.filter((f) => f.step === "waiting").length;
const busy = (f) => ["reading", "loading", "transcribing"].includes(f.step);
/* waiting its turn or being transcribed */
const live = (f) => f.step === "waiting" || busy(f);

/* --------------------------------------------------------------- model -- */

const inUse = () => st.catalog.find((m) => m.name === st.model);
function modelLabel(long) {
  if (st.model_file) return baseName(st.model_file);
  const m = inUse();
  if (!m) return "Choose a model";
  return long ? `${m.label} · ${m.name}` : `${m.label} model`;
}

/* A download starts with what it already has; its first event follows at once. */
async function startDownload(m) {
  dl = { name: m.name, step: "downloading", got: m.partial, total: m.bytes, speed: 0, host: "", why: "", text: "", detail: "",
    retry: false, trying: false };
  render();
  try {
    await invoke("download_model", { name: m.name });
  } catch (e) {
    dl = null;
    render();
    toastErr(e);
  }
}

async function useOwnFile() {
  try {
    if (!(await invoke("use_model_file"))) return;
    if (dl) await invoke("cancel_download");
    await refresh();
  } catch (e) {
    toastErr(e);
  }
}

/* A language's name in English, the window's language */
const langNames = (() => {
  try {
    return new Intl.DisplayNames(["en"], { type: "language" });
  } catch {
    return null;
  }
})();
const langName = (c) => (langNames && langNames.of(c)) || c;
const ownLang = () => (navigator.language || "en").slice(0, 2).toLowerCase();
/* After a model of several's count, the system's language where it is one
   of them: whether theirs is there is what most who look want to know */
const amongThem = (m) => (m.languages.length > 1 && m.languages.includes(ownLang()) ? `, ${langName(ownLang())} among them` : "");
const langsOf = (m) => m.languages.map((c) => ({ c, n: langName(c) })).sort((a, b) => a.n.localeCompare(b.n));

/* Each model as what it is for and whether it is here: "25 European
   languages", and Downloaded, in green, or its download's size. Its file's
   name is its tooltip, with a model of several's languages. Downloaded has
   no check of its own: the menu's check is the model in use. */
function modelMenu(anchor, keys) {
  const items = st.catalog.map((m) => {
    const on = !st.model_file && st.model === m.name;
    const state = m.have ? `<span class="pk-have">Downloaded</span>` : `${mb(m.bytes)} MB download`;
    const title = m.languages.length > 1 ? `${m.name}: ${langsOf(m).map((x) => x.n).join(", ")}` : m.name;
    return `<button class="pk-mi" type="button" role="menuitemradio" aria-checked="${on}" data-model="${m.name}" title="${esc(title)}">` +
      `<span>${esc(m.label)}</span><small class="pk-sub">${esc(m.about + amongThem(m))} · ${state}</small>${on ? ICON.check : ""}</button>`;
  });
  if (st.model_file) {
    items.push(`<button class="pk-mi" type="button" role="menuitemradio" aria-checked="true" data-model="">` +
      `<span>${esc(baseName(st.model_file))}</span><small title="${esc(st.model_file)}">${esc(st.model_file)}</small>${ICON.check}</button>`);
  }
  // where the models are: about 620 MB each, and the files the CLI takes with -m
  const show = { macos: "Show in Finder", windows: "Show in File Explorer" }[st.platform] || "Show in folder";
  const file = st.model_file ? baseName(st.model_file) : st.model ? `${st.model}.pkf` : "";
  items.push(`<div class="pk-menu-sep" aria-hidden="true"></div>` +
    `<button class="pk-mi" type="button" role="menuitem" data-own="1"><span>Open a model file…</span></button>` +
    `<button class="pk-mi" type="button" role="menuitem" data-reveal="1"${file ? ` title="${esc(file)}"` : ""}><span>${show}</span></button>`);
  openMenu(anchor, keys, items.join(""), "down right", (m) => {
    m.onclick = async (e) => {
      const b = e.target.closest(".pk-mi");
      if (!b) return;
      closeMenu();
      if (b.dataset.own) return useOwnFile();
      if (b.dataset.reveal) return invoke("reveal_model").catch(toastErr);
      const entry = st.catalog.find((x) => x.name === b.dataset.model);
      if (!entry || (!st.model_file && st.model === entry.name)) return;
      if (!entry.have) return startDownload(entry);
      try {
        await invoke("choose_model", { name: entry.name });
        await refresh();
        toast(`Using the ${entry.label} model`);
      } catch (err) {
        toastErr(err);
      }
    };
  });
}

/* --------------------------------------------------------------- setup -- */

let setupKey = "";

function setupMode() {
  if (!dl) return "choose";
  return dl.step === "checking" ? "downloading" : dl.step;
}

function renderSetup() {
  const mode = setupMode();
  const key = mode === "failed" ? `failed ${dl.why} ${dl.retry}` : mode;
  if (key === setupKey) return mode === "choose" ? updateChoose() : updateDownload();
  setupKey = key;
  const body = $("setup-body");
  if (mode === "choose") {
    body.innerHTML = chooseHtml();
    for (const r of body.querySelectorAll('input[name="model"]')) r.onchange = () => { choice = r.value; updateChoose(); };
    for (const b of body.querySelectorAll("[data-langs]")) b.onclick = (e) => {
      // the card's label would choose its model too
      e.preventDefault();
      const box = b.closest(".pk-choice").querySelector(".pk-langs-box");
      box.hidden = !box.hidden;
      b.setAttribute("aria-expanded", String(!box.hidden));
    };
    // in the browser, not in the window
    for (const a of body.querySelectorAll(".pk-langs-note a")) a.onclick = (e) => {
      e.preventDefault();
      invoke("open_link", { url: a.href }).catch((err) => toastErr(err));
    };
    $("go").onclick = go;
    $("have-file").onclick = useOwnFile;
    return updateChoose();
  }
  body.innerHTML = mode === "failed" ? failedHtml() : downloadHtml();
  $("dl-cancel").onclick = () => invoke("cancel_download").catch((e) => toastErr(e));
  if (mode === "failed") {
    $("dl-retry").onclick = () => invoke("download_model", { name: dl.name }).catch((e) => toastErr(e));
    $("dl-file").onclick = useOwnFile;
  } else {
    $("dl-pause").onclick = () => (dl.step === "paused" ? invoke("download_model", { name: dl.name }) : invoke("pause_download"))
      .catch((e) => toastErr(e));
  }
  updateDownload();
}

/* The model that speaks the system's language is picked to begin with:
   the cards and the button say the rest. A model of several's count opens
   their list, three columns down, and under it a link to how accurate it is
   in each. */
function chooseHtml() {
  const fits = st.catalog.find((m) => m.languages.includes(ownLang()));
  if (!st.catalog.some((m) => m.name === choice)) choice = (inUse() || fits || st.catalog[0]).name;
  const cards = st.catalog.map((m) => {
    const many = m.languages.length > 1;
    const about = !many ? esc(m.about)
      : `<button class="pk-linkbtn pk-linkbtn-sm" type="button" data-langs aria-expanded="false">${esc(m.about)}</button>${esc(amongThem(m))}`;
    const list = !many ? "" : `<div class="pk-langs-box" hidden>` +
      `<div class="pk-langs" style="grid-template-rows: repeat(${Math.ceil(m.languages.length / 3)}, auto)">` +
      langsOf(m).map((x) => (x.c === ownLang() ? `<b>${esc(x.n)}</b>` : `<span>${esc(x.n)}</span>`)).join("") + "</div>" +
      (m.accuracy ? `<p class="pk-langs-note">Accuracy varies by language. <a href="${esc(m.accuracy)}">Details${ICON.external}</a></p>` : "") +
      "</div>";
    return `<label class="pk-choice"><input type="radio" name="model" value="${m.name}"${m.name === choice ? " checked" : ""}>` +
      `<span class="pk-name">${esc(m.label)}</span><span class="pk-stamp">${m.have ? "on this computer" : `${mb(m.bytes)} MB`}</span>` +
      `<span class="pk-desc">${esc(m.name)} · ${about}</span>${list}</label>`;
  }).join("");
  return `${WORDMARK}
    <div class="pk-intro"><h1 class="pk-lead">Choose a speech model</h1></div>
    <fieldset class="pk-choices"><legend>Speech model</legend>${cards}</fieldset>
    <div class="pk-foot-col">
      <button class="pk-btn pk-btn-primary pk-btn-lg pk-wide" id="go" type="button"></button>
      <div class="pk-spread">
        <button class="pk-linkbtn" id="have-file" type="button">I already have a model file…</button>
        <span class="pk-small" id="go-note"></span>
      </div>
    </div>`;
}

function updateChoose() {
  const m = st.catalog.find((x) => x.name === choice);
  for (const r of document.querySelectorAll('input[name="model"]')) r.closest(".pk-choice").classList.toggle("is-on", r.value === choice);
  const go = m.have ? `Use the ${m.label} model`
    : m.partial ? `Download the ${m.label} model · ${mb(m.bytes - m.partial)} MB to go`
    : `Download the ${m.label} model · ${mb(m.bytes)} MB`;
  $("go").innerHTML = `${m.have ? ICON.check : ICON.download}${esc(go)}`;
  const w = waiting();
  $("go-note").textContent = w ? `${plural(w, "file")} waiting` : "";
}

async function go() {
  const m = st.catalog.find((x) => x.name === choice);
  if (!m.have) return startDownload(m);
  try {
    await invoke("choose_model", { name: m.name });
    await refresh();
  } catch (e) {
    toastErr(e);
  }
}

function downloadHtml() {
  const m = st.catalog.find((x) => x.name === dl.name) || { label: dl.name };
  return `${WORDMARK}
    <div class="pk-intro"><h1 class="pk-lead">Downloading the ${esc(m.label)} model</h1>
      <p class="pk-text">${esc(dl.name)} from <span id="dl-host"></span></p></div>
    <div class="pk-dl">
      <div class="pk-spread"><span class="pk-figure" id="dl-pct"></span><span class="pk-stamp" id="dl-nums"></span></div>
      <div class="pk-bar" id="dl-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-label="Model download"><i></i></div>
    </div>
    <div class="pk-foot-row">
      <button class="pk-btn" id="dl-pause" type="button"></button>
      <button class="pk-btn pk-btn-ghost" id="dl-cancel" type="button">Cancel</button>
      <span class="pk-small" id="dl-next"></span>
    </div>`;
}

/* What stopped it in a few words, what it has kept, and the particulars
   folded away */
function failedHtml() {
  const stopped = dl.got ? `Stopped at ${mb(dl.got)} of ${mb(dl.total)} MB` : "Nothing downloaded yet";
  return `${WORDMARK}
    <div class="pk-intro"><h1 class="pk-lead">${esc(dl.why)}</h1></div>
    <div class="pk-stopped">
      <div class="pk-bar is-paused" id="dl-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-label="Model download, stopped"><i></i></div>
      <div class="pk-spread"><span class="pk-small pk-err">${ICON.alert}${stopped}</span>
        ${dl.retry ? `<span class="pk-small" id="dl-auto"></span>` : ""}</div>
    </div>
    <details class="pk-details"><summary class="pk-ui">${ICON.down}Details</summary><p class="pk-stamp">${esc(dl.detail)}</p></details>
    <div class="pk-foot-row">
      <button class="pk-btn pk-btn-primary pk-btn-lg" id="dl-retry" type="button">Try again</button>
      <button class="pk-btn pk-btn-lg" id="dl-file" type="button">Use a model file…</button>
      <button class="pk-btn pk-btn-ghost pk-btn-lg" id="dl-cancel" type="button">Cancel</button>
    </div>`;
}

function updateDownload() {
  const mode = setupMode();
  const p = dl.total ? dl.got / dl.total : 0;
  const bar = $("dl-bar");
  bar.firstElementChild.style.width = `${(p * 100).toFixed(1)}%`;
  bar.setAttribute("aria-valuenow", percent(p));
  bar.classList.toggle("is-paused", mode !== "downloading");
  if (mode === "failed") {
    $("dl-retry").disabled = dl.trying;
    $("dl-retry").textContent = dl.trying ? "Trying…" : "Try again";
    if ($("dl-auto")) $("dl-auto").textContent = dl.trying ? "" : "Retries by itself";
    return;
  }
  $("dl-host").textContent = dl.host || "models.pktito.net";
  $("dl-pct").textContent = `${percent(dl.step === "checking" ? 1 : p)}%`;
  let nums = `${mb(dl.got)} of ${mb(dl.total)} MB`;
  if (dl.step === "checking") nums = "Checking";
  else if (dl.step === "paused") nums += " · paused";
  else if (dl.speed > 0) nums += ` · ${(dl.speed / 1e6).toFixed(1)} MB/s · ${timeLeft((dl.total - dl.got) / dl.speed)}`;
  $("dl-nums").textContent = nums;
  const pause = $("dl-pause");
  pause.textContent = dl.step === "paused" ? "Resume" : "Pause";
  pause.classList.toggle("pk-btn-primary", dl.step === "paused");
  pause.disabled = dl.step === "checking";
  const w = waiting();
  $("dl-next").textContent = w ? `${plural(w, "file")} waiting` : "";
}

listen("download", async (e) => {
  const s = e.payload;
  if (s.step === "done" || s.step === "cancelled") {
    dl = null;
    setupKey = "";
    await refresh();
    const m = st.catalog.find((x) => x.name === s.name);
    if (s.step === "done") toast(`${m ? m.label : s.name} model ready`);
    return;
  }
  dl = s;
  if (viewName() === "setup") renderSetup();
});

/* --------------------------------------------------------------- ready -- */

/* What is still open when the window went back to the start heads its
   recent list and leads back to it: a lone file under way, as the batch
   screen shows a file, or a batch as one row. */
function renderReady() {
  $("ready-model").innerHTML = `${esc(modelLabel(false))}${ICON.down}`;
  $("choose").innerHTML = `${ICON.plus}Choose files…`;
  $("choose").title = tip("Choose files", "O");
  drawWave($("idle-wave"), null, 0);
  $("recent").hidden = !recent.length && !files.length;
  $("ready").classList.toggle("has-recent", !$("recent").hidden);
  $("clear-recent").hidden = !recent.length;
  // each with the × that takes it off the list, over its date under the
  // pointer; from the keyboard, Delete
  $("recent-list").innerHTML = (batch ? batchRowHtml() : files.map(liveHtml).join("")) + recent.map((r) =>
    `<li class="pk-qi"><button class="pk-row" type="button" data-recent="${esc(r.id)}" title="${esc(r.path)}" aria-keyshortcuts="Delete">` +
    `${ICON.file}<span class="pk-name">${esc(r.name)}</span><span class="pk-stamp">${hms(r.duration)}</span>` +
    `<span class="pk-small pk-when">${esc(when(r.finished))}</span></button><button class="pk-x" type="button" tabindex="-1" ` +
    `data-forget="${esc(r.id)}" aria-label="Remove ${esc(r.name)} from Recent" title="Remove from Recent; the file stays">${ICON.x}</button></li>`).join("");
}

const liveHtml = (f) => `<li><button class="pk-row" type="button" data-live="${f.id}" title="${esc(f.path)}">${rowBody(f)}</button></li>`;

/* A batch on the start screen, under way or done since: how far it has
   got, all its files counted, and their length once every one is read. */
function batchRowHtml() {
  const n = files.length, done = files.filter((f) => f.step === "done").length, over = files.filter((f) => !live(f)).length;
  const f = files.find((x) => x.step === "transcribing");
  const length = files.every((x) => x.duration != null) ? hms(files.reduce((t, x) => t + x.duration, 0)) : "";
  const status = files.some(live)
    ? `<span class="pk-bar pk-mini"><i style="width:${((over + (f ? f.done : 0)) / n * 100).toFixed(1)}%"></i></span>` +
      `<span class="pk-status pk-num">Transcribing ${Math.min(n, over + 1)} of ${n}</span>`
    : done === n ? `<span class="pk-status pk-ok">${ICON.check}Done</span>` : `<span class="pk-status pk-num">${done} of ${n} done</span>`;
  return `<li><button class="pk-row" type="button" data-batch title="${esc(files.map((x) => x.name).join("\n"))}">${ICON.folder}` +
    `<span class="pk-name">${plural(n, "file")}</span><span class="pk-stamp">${length}</span>${status}</button></li>`;
}

/* the width the recent list's scrollbar takes from it, for app.css to give
   back: Windows' does; macOS's float over the list and take none */
new ResizeObserver(() => {
  const ul = $("recent-list");
  ul.style.setProperty("--sb", `${ul.offsetWidth - ul.clientWidth}px`);
}).observe($("recent-list"));

function updateLive(f) {
  const at = batch ? "[data-batch]" : `[data-live="${f.id}"]`, row = $("recent-list").querySelector(at);
  if (!row) return renderReady();
  const focused = document.activeElement === row;
  row.parentNode.outerHTML = batch ? batchRowHtml() : liveHtml(f);
  if (focused) $("recent-list").querySelector(at).focus();
}

async function openRecent(id) {
  const there = files.find((f) => f.recentId === id);
  if (there) {
    sel = there.id;
    home = false;
    return render();
  }
  try {
    const t = await invoke("open_recent", { id });
    // beside a batch it is no part of it: back from it is the start
    const f = Object.assign(newFile(t.path), { name: t.name, step: "done", done: 1, duration: t.duration, peaks: t.peaks,
      audio: t.audio, segments: t.segments, took: t.took, transcribed: t.transcribed || 0, model: t.model, recentId: t.id,
      fps: t.fps || 0, lone: batch });
    files.push(f);
    sel = f.id;
    home = false;
    render();
    if (!f.audio) reloadAudio(f);
  } catch (e) {
    toastErr(e);
  }
}

/* The cache let its audio go: read it again from the original, if it is
   there. Until then, or if it cannot be, the play button says why, and
   the transcript reads as ever. */
async function reloadAudio(f) {
  f.audioWhy = "Reading the audio…";
  if (current() === f) setPlayIcon();
  try {
    f.audio = await invoke("reload_audio", { id: f.recentId });
    f.audioWhy = "";
  } catch (e) {
    f.audioWhy = String(e);
  }
  if (current() === f) {
    mainKey = "";
    showMain();
  }
}

/* A transcript off the recent list, and its audio from the cache; the
   file it came from stays where it is. */
async function forget(id) {
  try {
    await invoke("forget_recent", { id });
    const i = recent.findIndex((r) => r.id === id);
    recent = recent.filter((r) => r.id !== id);
    renderReady();
    // the keyboard on the row that took its place
    const rows = $("recent-list").querySelectorAll("[data-recent]");
    if (rows.length) rows[Math.min(Math.max(0, i), rows.length - 1)].focus({ preventScroll: true });
  } catch (e) {
    toastErr(e);
  }
}

$("recent-list").addEventListener("keydown", (e) => {
  const b = e.target.closest("[data-recent]");
  if (!b || (e.key !== "Delete" && e.key !== "Backspace")) return;
  e.preventDefault();
  forget(b.dataset.recent);
});

$("recent-list").onclick = (e) => {
  const x = e.target.closest("[data-forget]");
  if (x) return forget(x.dataset.forget);
  const b = e.target.closest("[data-recent], [data-live], [data-batch]");
  if (!b) return;
  if (b.dataset.recent) return openRecent(b.dataset.recent);
  sel = b.dataset.live ? Number(b.dataset.live) : null;
  home = false;
  render();
};
/* Clearing the list asks first, in the window: the system's dialog makes
   its first button the default, so Enter or Space there cleared the list
   (the Windows test, 2026-10-22). Here Cancel is the default, Esc and a
   click outside are Cancel too, and Clear wants its own click, or Tab and
   Enter. */
let answer = null;

function askClear() {
  $("ask").hidden = false;
  $("ask-no").focus({ preventScroll: true });
  return new Promise((r) => (answer = r));
}

/* false if it was not open */
function closeAsk(yes = false) {
  if ($("ask").hidden) return false;
  $("ask").hidden = true;
  $("clear-recent").focus({ preventScroll: true });
  answer(yes);
  return true;
}

$("ask-yes").onclick = () => closeAsk(true);
$("ask-no").onclick = () => closeAsk(false);
$("ask").addEventListener("click", (e) => {
  if (e.target === $("ask")) closeAsk(false);
});
/* the keyboard stays on its two buttons */
$("ask").addEventListener("keydown", (e) => {
  if (e.key !== "Tab") return;
  e.preventDefault();
  (document.activeElement === $("ask-no") ? $("ask-yes") : $("ask-no")).focus();
});
$("clear-recent").onclick = async () => {
  if (!(await askClear())) return;
  try {
    await invoke("clear_recent");
    recent = [];
    render();
  } catch (e) {
    toastErr(e);
  }
};
$("choose").onclick = () => chooseFiles();
$("ready-model").onclick = (e) => modelMenu(e.currentTarget, !e.detail);

/* --------------------------------------------------------------- files -- */

function newFile(path) {
  return { id: nextId++, path, name: baseName(path), kind: kindOf(path), step: "waiting", done: 0, eta: -1, duration: null,
    peaks: null, audio: null, audioWhy: "", pos: 0, segments: [], cues: null, fps: 0, took: 0, transcribed: 0, model: "", recentId: null, message: "",
    detail: "" };
}

async function chooseFiles() {
  if (!st) return;
  try {
    await addPaths(await invoke("pick_files"));
  } catch (e) {
    toastErr(e);
  }
}

/* Files dropped or chosen, from any screen, if they are audio or video
   and there: a name typed into the Open dialog can name nothing. With
   nothing under way they take the window, what was open closing (a
   finished transcript is on the recent list): one is shown, and several
   are a batch, its screen shown. Any that come while files are under way
   join them as a batch, and so do any added on a batch's screen or one of
   its files, done or not: added there, one alone replaced the batch, Save
   all and all (the Windows test, 2026-10-22). */
async function addPaths(paths) {
  // files dropped or picked while the window asks: Cancel, and on to them
  closeAsk();
  const media = paths.filter(isMedia);
  let here = media;
  try {
    if (media.length) here = await invoke("existing", { paths: media });
  } catch {
    // the transcription says so, of one that is not there
  }
  if (here.length) {
    const v = viewName(), into = batch && (v === "batch" || (v === "work" && !current().lone));
    if (!into && !files.some(live)) closeFiles();
    for (const f of files.filter((x) => x.lone)) dropFile(f);
    for (const p of here) files.push(newFile(p));
    batch = files.length > 1;
    sel = batch ? null : files[0].id;
    home = false;
    mainKey = "";
    closeAbout();
  }
  const gone = media.filter((p) => !here.includes(p)).map(baseName), other = paths.filter((p) => !isMedia(p)).map(baseName);
  const say = [gone.length && `Not found: ${gone.join(", ")}`, other.length && `Not audio or video: ${other.join(", ")}`];
  if (gone.length || other.length) toast(say.filter(Boolean).join(" · "), { err: true });
  render();
  pump();
}

async function pump() {
  if (running || viewName() === "setup") return;
  const f = files.find((x) => x.step === "waiting");
  if (!f) return;
  running = true;
  f.step = "reading";
  update(f);
  try {
    await invoke("transcribe", { id: f.id, path: f.path });
  } catch (e) {
    f.step = "error";
    f.message = String(e);
    update(f);
  }
  running = false;
  pump();
}

listen("job", (e) => {
  const j = e.payload;
  const f = files.find((x) => x.id === j.id);
  if (!f) return;
  f.step = j.step;
  if (j.duration) f.duration = j.duration;
  if (j.peaks.length) f.peaks = j.peaks;
  if (j.audio) f.audio = j.audio;
  if (j.fps) f.fps = j.fps;
  if (j.step === "transcribing") {
    f.done = j.done;
    f.eta = j.eta;
    f.transcribed = j.transcribed;
  }
  f.segments.push(...j.segments);
  if (j.step === "error") Object.assign(f, { message: j.message, detail: j.detail });
  if (j.step === "done") {
    Object.assign(f, { done: 1, took: j.took, transcribed: j.transcribed, model: j.model, recentId: j.recent });
    finished(f);
  }
  update(f, j.segments);
  // nothing of the batch still to come: what Save all chose for the rest is done
  if (!files.some(live)) batchSave = null;
});

async function finished(f) {
  // what Save all chose for the rest of the batch, as it was when f finished
  const plan = batch && !f.lone ? batchSave : null;
  try {
    recent = await invoke("recent_list");
  } catch {
    // the list shows what it had
  }
  // a lone file finished while the start screen was shown is on the recent list now
  if (home && !batch) dropFile(f);
  if (viewName() === "ready") renderReady();
  if (plan && f.segments.length) await saveTo([f], plan.kind, plan.dir);
}

/* Show what changed in f where it shows. */
function update(f, segs = []) {
  const v = viewName();
  if (v === "setup") return renderSetup();
  if (v === "ready") return updateLive(f);
  if (v === "batch") return updateRow(f);
  if (v === "work" && f.id === sel) showMain(segs);
}

/* f closed, and out of the player if it is there */
function dropFile(f) {
  const i = files.indexOf(f);
  if (i < 0) return;
  files.splice(i, 1);
  if (playId === f.id) {
    pausePlayback();
    player.removeAttribute("src");
    playId = null;
  }
  if (sel === f.id) sel = files.length ? files[Math.min(i, files.length - 1)].id : null;
}

/* f off the batch, from its row; the keyboard, if it was on the list, on
   the row that took its place. The batch's last file closes it. */
function removeFile(f) {
  if (busy(f)) return toast("Stop it first", { err: true });
  const i = files.indexOf(f), keys = !!document.activeElement.closest("#queue");
  dropFile(f);
  if (!files.length) closeFiles();
  mainKey = "";
  render();
  const rows = $("queue").querySelectorAll("[data-id]");
  if (keys && rows.length) rows[Math.min(i, rows.length - 1)].focus({ preventScroll: true });
}

/* Every file closed, and the batch with them: their transcripts are on the
   recent list. */
function closeFiles() {
  for (const f of files.slice()) dropFile(f);
  batch = false;
  batchSave = null;
}

/* Back one screen: from a file of a batch to the batch, from a file opened
   beside it to the start, and from the batch or a lone file to the start. */
function back() {
  const f = viewName() === "work" ? current() : null;
  if (f && batch && !f.lone) {
    closeMenu();
    sel = null;
    mainKey = "";
    render();
    // the keyboard back on the file's row
    const row = $("queue").querySelector(`[data-id="${f.id}"]`);
    if (row) row.focus();
    return;
  }
  if (f && f.lone) dropFile(f);
  goHome();
}

/* Back to the start. A lone file finished, stopped or failed is closed, a
   finished transcript staying on the recent list; one waiting or being
   transcribed goes on, and heads the list there. A batch goes on there as
   one row while any of it is to come, and after, until it is left done. */
function goHome() {
  const v = viewName();
  if (v !== "work" && v !== "batch") return;
  closeMenu();
  if (!batch) for (const f of files.filter((x) => !live(x))) dropFile(f);
  else if (v === "batch" && !files.some(live)) closeFiles();
  home = true;
  sel = null;
  mainKey = "";
  render();
}

function again(f) {
  Object.assign(f, { step: "waiting", done: 0, eta: -1, peaks: null, audio: null, audioWhy: "", pos: 0, segments: [], cues: null,
    took: 0, transcribed: 0, recentId: null, message: "", detail: "" });
  mainKey = "";
  render();
  pump();
}

/* ---------------------------------------------------------------- work -- */

function renderWork() {
  if (!current()) sel = files[0].id;
  showMain();
}

/* A file's row on the batch screen: its name and how far it has got. The
   start screen draws the same for a lone file still under way. */
function rowBody(f) {
  let status;
  if (f.step === "waiting") status = `<span class="pk-status">Waiting</span>`;
  else if (f.step === "reading") status = `<span class="pk-status">Reading the audio</span>`;
  else if (f.step === "loading") status = `<span class="pk-status">Loading the model</span>`;
  else if (f.step === "transcribing") {
    status = `<span class="pk-bar pk-mini"><i style="width:${(f.done * 100).toFixed(1)}%"></i></span>` +
      `<span class="pk-status pk-num">Transcribing ${percent(f.done)}%${f.eta > 0 ? ` · ${timeLeft(f.eta)}` : ""}</span>`;
  } else if (f.step === "done") status = `<span class="pk-status pk-ok">${ICON.check}Done</span>`;
  else if (f.step === "stopped") status = `<span class="pk-status">Stopped at ${percent(f.done)}%</span>`;
  else status = `<span class="pk-status pk-err"${f.detail ? ` title="${esc(f.detail)}"` : ""}>${esc(f.message || "Couldn't transcribe")}</span>`;
  return `${f.kind === "video" ? ICON.film : ICON.music}<span class="pk-name">${esc(f.name)}</span>` +
    `<span class="pk-stamp">${f.duration == null ? "" : hms(f.duration)}</span>${status}`;
}

/* --------------------------------------------------------------- batch -- */

/* A row of the batch, which opens its file, and the × that shows over it
   under the pointer, except while it is transcribed; from the keyboard,
   Delete. */
function rowHtml(f) {
  return `<li${busy(f) ? "" : ' class="pk-qi"'}><button class="pk-row" type="button" data-id="${f.id}" aria-keyshortcuts="Delete" title="${esc(f.path)}">` +
    `${rowBody(f)}</button>` + (busy(f) ? "" : `<button class="pk-x" type="button" tabindex="-1" data-x="${f.id}" ` +
      `aria-label="Take ${esc(f.name)} off the list" title="Take it off the list">${ICON.x}</button>`) + "</li>";
}

function renderBatch() {
  const at = document.activeElement.closest("#queue [data-id]");
  $("batch-title").textContent = plural(files.length, "file");
  $("queue").innerHTML = files.map(rowHtml).join("");
  const row = at && $("queue").querySelector(`[data-id="${at.dataset.id}"]`);
  if (row) row.focus({ preventScroll: true });
  batchHead();
}

/* One row anew, in place: progress comes every chunk, and a list redrawn
   under the pointer would lose clicks. */
function updateRow(f) {
  const row = $("queue").querySelector(`[data-id="${f.id}"]`);
  if (!row) return renderBatch();
  const focused = document.activeElement === row;
  row.parentNode.outerHTML = rowHtml(f);
  if (focused) $("queue").querySelector(`[data-id="${f.id}"]`).focus();
  batchHead();
}

/* How far the batch has got, and what Save all does with the rest as it
   finishes; Stop and the spinner while any of it is to come. */
function batchHead() {
  const going = files.some(live), p = going && batchSave;
  const saving = p ? ` · saving the rest as they finish: ${p.kind === "folder" ? `all five formats into ${baseName(p.dir)}` : `.${p.kind} next to each original`}` : "";
  $("tally").textContent = `${files.filter((f) => f.step === "done").length} of ${files.length} done${saving}`;
  $("batch-spin").hidden = $("batch-stop").hidden = !going;
  $("save-all").disabled = !files.some((f) => live(f) || (f.step === "done" && f.segments.length));
}

$("queue").addEventListener("click", (e) => {
  const x = e.target.closest("[data-x]");
  if (x) return removeFile(files.find((f) => f.id === Number(x.dataset.x)));
  const row = e.target.closest("[data-id]");
  if (!row) return;
  closeMenu();
  sel = Number(row.dataset.id);
  render();
});
/* the arrow keys from row to row; Delete takes one off */
$("queue").addEventListener("keydown", (e) => {
  const row = e.target.closest("[data-id]");
  if (!row) return;
  const rows = [...$("queue").querySelectorAll("[data-id]")], i = rows.indexOf(row);
  if (e.key === "ArrowDown" || e.key === "ArrowUp") rows[Math.min(rows.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)))].focus();
  else if (e.key === "Delete" || e.key === "Backspace") removeFile(files.find((f) => f.id === Number(row.dataset.id)));
  else return;
  e.preventDefault();
});
/* The file under way stops where it is, and those waiting come off the
   list; what is done stays, to be saved. */
$("batch-stop").onclick = () => {
  for (const f of files.filter((x) => x.step === "waiting")) dropFile(f);
  const f = files.find(busy);
  if (f) invoke("stop", { id: f.id }).catch(() => {});
  if (!files.length) closeFiles();
  render();
};
$("add").onclick = () => chooseFiles();
$("back").onclick = () => back();
$("batch-back").onclick = () => goHome();

/* The selected file: waiting, running, its transcript to review, or why it failed. */
function modeOf(f) {
  if (f.step === "waiting") return "waiting";
  if (busy(f)) return "running";
  return f.step === "error" ? "error" : "review";
}

/* Where the file is and what made its transcript, for its name's tooltip:
   the window keeps both out of sight. */
const fileTip = (f) => (f.model ? `${f.path}\n${f.model} · took ${clock(f.took)}` : f.path);
const playable = (f) => !!f.audio && !f.audioWhy;

let mainKey = "";

function showMain(segs = []) {
  const f = current();
  if (!f) return;
  const mode = modeOf(f);
  const key = `${f.id} ${mode} ${f.step}`;
  if (key !== mainKey) return buildMain(f, mode, key);
  if (mode === "running") {
    updateRunning(f);
    if (segs.length) appendSegs(f, segs);
  }
}

function buildMain(f, mode, key) {
  const tr = $("transcript");
  // The same file in a new mode keeps the reader's place, except that a
  // transcript just finished starts at its top for a reader who was
  // following its end as it came.
  const [was, wasMode] = mainKey.split(" ");
  const atEnd = tr.scrollHeight - tr.scrollTop - tr.clientHeight < 48;
  const top = was !== String(f.id) || (wasMode === "running" && mode === "review" && atEnd) ? 0 : tr.scrollTop;
  mainKey = key;
  pausePlayback();
  $("title").textContent = f.name;
  $("title").title = fileTip(f);
  $("find").value = "";
  query = "";
  hits = [];
  hitTimes = [];
  findCount();
  $("stop").disabled = false;
  updateHead(f, mode);
  const review = mode === "review";
  $("strip-wrap").hidden = mode === "error";
  $("transport").hidden = $("time").hidden = !review;
  $("prog").hidden = review;
  $("dur").textContent = f.duration == null ? "" : lengthOf(f);
  const wave = $("wave");
  if (review) {
    wave.removeAttribute("role");
    $("wavebtn").setAttribute("aria-label", "Jump to a point in the audio");
  } else {
    wave.setAttribute("role", "progressbar");
    wave.setAttribute("aria-label", "Transcribed so far");
    wave.setAttribute("aria-valuemin", "0");
    wave.setAttribute("aria-valuemax", "100");
    $("wavebtn").removeAttribute("aria-label");
  }
  $("wavebtn").disabled = !review || !playable(f);
  $("views").hidden = !review || !f.segments.length;
  for (const b of $("views").children) b.setAttribute("aria-checked", String((b.dataset.view === "captions") === !!prefs.captions));
  // its cues, the first time it is shown as captions
  if (review && prefs.captions && !f.cues && f.segments.length) {
    cuesOf(f).then(() => {
      if (current() === f && prefs.captions) rebuild();
    }, (e) => toastErr(e));
  }
  // a transcript whose audio is gone: its waveform dims with the play button
  wave.classList.toggle("is-off", review && !playable(f));
  renderTranscript(f, mode);
  tr.scrollTop = top;
  drawMainWave();
  if (review) {
    loadAudio(f);
    showTime(f.pos);
  } else if (mode === "running") updateRunning(f);
}

function updateHead(f, mode) {
  const review = mode === "review", has = f.segments.length > 0;
  $("find-box").hidden = !review || !has;
  $("copy").hidden = !(review || mode === "running");
  // its icon, which everyone reads, and its words for tooltip and reader
  $("copy").innerHTML = ICON.copy;
  $("copy").title = !review ? "Copy so far" : prefs.captions ? "Copy captions (SRT)" : "Copy text";
  $("copy").setAttribute("aria-label", $("copy").title);
  $("copy").disabled = !has;
  // at work: from the file's start until its last words, before its first too
  $("spin").hidden = mode !== "running";
  // saving is what a finished transcript is for
  $("save-as").classList.add("pk-btn-primary");
  $("save-as").hidden = !review;
  $("save-as").innerHTML = `Save as${ICON.down}`;
  $("save-as").disabled = !has;
  // not for a file gone from its place (added again from where it is now,
  // it is a new row) or one that is not media: neither can go better
  $("again").hidden = !(f.step === "stopped" || (f.step === "error" && !["Not found", "Not audio or video"].includes(f.message)));
  $("again").title = f.step === "stopped" ? `Stopped at ${percent(f.done)}%` : "";
  $("stop").hidden = mode !== "running";
}

function updateRunning(f) {
  const going = f.step === "transcribing";
  $("pct").textContent = `${percent(going ? f.done : 0)}%`;
  // under it the step, and once the first audio is done, the pace so far
  $("pace").textContent = f.step === "reading" ? "Reading the audio" : f.step === "loading" ? "Loading the model"
    : going && f.done > 0 && f.transcribed > 0 ? realTime(f.done * f.duration, f.transcribed) : "";
  $("pct").title = going && f.eta > 0 ? timeLeft(f.eta) : "";
  $("dur").textContent = f.duration == null ? "" : lengthOf(f);
  const wave = $("wave");
  if (wave._peaks !== f.peaks) drawMainWave();
  else setHead(wave, Math.floor((going ? f.done : 0) * wave._n));
  wave.setAttribute("aria-valuenow", percent(going ? f.done : 0));
}

/* ---------------------------------------------------------- transcript -- */

/* A segment, or a cue; its time plays from there once the transcript is
   finished. A cue shows its end too, its lines as they will read, and a
   mark if it asks more than 20 characters a second of its reader. */
function segHtml(s, i, filling) {
  const cue = s.endMs !== undefined;
  const fast = cue && s.flags & 1 ?
    `<span class="pk-fast" title="${(s.chars / (s.end - s.start)).toFixed(1)} characters a second, over the 20 a reader keeps up with">${ICON.zap}</span>` : "";
  const start = tc(s.start);
  return `<div class="pk-seg${cue ? " pk-cue" : ""}" data-i="${i}"><div class="pk-seg-times"><button class="pk-seg-t" type="button" tabindex="-1" ` +
    `aria-label="Play from ${start}"${filling ? "" : ' title="Play from here"'}>${start}</button>` +
    (cue ? `<span class="pk-seg-end">${tc(s.end)}${fast}</span>` : "") + `</div><p>${bodyHtml(s, i)}</p></div>`;
}

/* Row i's text: each timed word in a span of its own, for the playhead,
   and with a search its matches marked, in the words' spans and between
   them. A search's marks once took the spans' place, and no word lit while
   one was open (Peter, 2026-10-22). A match across words is a mark in each
   piece: the first carries its place in the text (data-at), every piece its
   number (data-hit), so that the current one lights whole. */
function bodyHtml(s, i) {
  const text = s.text, words = [], found = [];
  let at = 0;
  (s.words || []).forEach((w, k) => {
    const j = text.indexOf(w.text, at);
    if (j < 0) return;
    words.push([j, j + w.text.length, k]);
    at = j + w.text.length;
  });
  if (query) {
    // a cue's line break reads as the space it stands for
    const flat = text.replace(/\n/g, " ");
    let low = flat.toLowerCase(), q = query.toLowerCase();
    // a lowercase that changes the length would put the marks in the wrong places
    if (low.length !== text.length) [low, q] = [flat, query];
    for (let from = 0, j; (j = low.indexOf(q, from)) >= 0; from = j + q.length) found.push([j, j + q.length]);
  }
  // the text cut wherever a word or a match starts or ends
  const cuts = [...new Set([0, text.length, ...words.flatMap((w) => [w[0], w[1]]), ...found.flat()])].sort((a, b) => a - b);
  let out = "", wi = 0, hi = 0, open = -1;
  for (let c = 0; c + 1 < cuts.length; c++) {
    const a = cuts[c], b = cuts[c + 1];
    while (wi < words.length && words[wi][1] <= a) wi++;
    while (hi < found.length && found[hi][1] <= a) hi++;
    const word = wi < words.length && words[wi][0] <= a ? words[wi][2] : -1;
    if (word !== open) {
      out += (open >= 0 ? "</span>" : "") + (word >= 0 ? `<span data-w="${word}">` : "");
      open = word;
    }
    const piece = esc(text.slice(a, b));
    out += hi < found.length && found[hi][0] <= a
      ? `<mark class="pk-hit" data-hit="${i}.${hi}"${found[hi][0] === a ? ` data-at="${a}"` : ""}>${piece}</mark>` : piece;
  }
  return out + (open >= 0 ? "</span>" : "");
}

function renderTranscript(f, mode) {
  const tr = $("transcript");
  tr.classList.toggle("is-live", mode === "running");
  tr.innerHTML = rowsOf(f).map((s, i) => segHtml(s, i, mode === "running")).join("") + (mode === "review" ? statsHtml(f) : "");
  curSeg = curWord = null;
  if (mode === "running") markLive();
  let msg = "", err = false;
  if (mode === "waiting") msg = "Waiting";
  else if (mode === "error") {
    msg = f.message || "Couldn't transcribe";
    err = true;
  } else if (mode === "review" && !f.segments.length) msg = f.step === "stopped" ? "Stopped before any speech" : "No speech in this file";
  $("msg").textContent = msg;
  $("msg").title = err ? f.detail : "";
  $("msg").hidden = !msg;
  $("msg").classList.toggle("pk-err", err);
}

/* What a transcript cost, after its last row and quiet: the transcription's
   time and its pace. A transcript from before that was kept says what it
   took in all. */
function statsHtml(f) {
  if (!f.took || !f.segments.length) return "";
  const secs = (x) => (x < 10 ? `${x.toFixed(2)}s` : x < 60 ? `${x.toFixed(1)}s` : clock(x));
  const line = f.transcribed > 0
    ? `Transcribed in ${secs(f.transcribed)}, ${realTime(f.duration, f.transcribed)}`
    : `Took ${secs(f.took)}`;
  return `<p class="pk-stats">${esc(line)}</p>`;
}

/* While transcribing, the newest segment is current and its last word is
   the one just decoded. */
function markLive() {
  const tr = $("transcript");
  for (const el of tr.querySelectorAll(".is-current, .pk-now")) el.classList.remove("is-current", "pk-now");
  const last = tr.lastElementChild;
  if (!last) return;
  last.classList.add("is-current");
  const words = last.querySelectorAll("[data-w]");
  if (words.length) words[words.length - 1].classList.add("pk-now");
}

function appendSegs(f, segs) {
  const tr = $("transcript");
  const atEnd = tr.scrollHeight - tr.scrollTop - tr.clientHeight < 48;
  const base = f.segments.length - segs.length;
  tr.insertAdjacentHTML("beforeend", segs.map((s, k) => segHtml(s, base + k, true)).join(""));
  markLive();
  if (atEnd) tr.scrollTop = tr.scrollHeight;
  updateHead(f, "running");
}

$("transcript").addEventListener("click", (e) => {
  const t = e.target.closest(".pk-seg-t");
  const f = current();
  if (!t || !f || modeOf(f) !== "review") return;
  seek(rowsOf(f)[Number(t.closest(".pk-seg").dataset.i)].start, true);
});

/* ------------------------------------------------------------ waveform -- */

/* A bar and the gap after it, 2 px and 1: fine enough to find a word by.
   At the frame's 8 px, a bar of an 800 px strip held 3 s of a 5-minute
   file; at 3 it holds 1.1 s. */
const BAR = 3;
/* the bars across el: the last needs no gap after it */
const barsIn = (el) => Math.max(1, Math.floor((el.clientWidth + 1) / BAR));

/* The outline's own range: its 10th percentile the strip's floor and its
   99.5th its top. The outline is each slice's loudness over the 48 dB
   below the loudest (src-tauri/src/audio.rs), and narration as even as
   most is within a few dB of its top throughout: drawn on all 48 dB its
   bars were a solid block, its pauses lost (Peter's file, 2026-10-21).
   Kept on the array, which its file keeps. */
function rangeOf(peaks) {
  if (!peaks._range) {
    const s = Float64Array.from(peaks).sort();
    const lo = s[Math.floor(s.length * 0.1)], hi = s[Math.min(s.length - 1, Math.floor(s.length * 0.995))];
    peaks._range = [lo, Math.max(hi, lo + 16)];
  }
  return peaks._range;
}

/* Bars across el from the outline (0-255 a slice), the playhead at bar k;
   no outline draws the empty strip. A bar is the mean of the slices under
   it: their loudest hid every pause shorter than a bar. */
function drawWave(el, peaks, k) {
  const n = barsIn(el);
  const top = el.clientHeight || 64;
  const [lo, hi] = peaks && peaks.length ? rangeOf(peaks) : [0, 255];
  const h = [];
  for (let i = 0; i < n; i++) {
    let v = 0;
    if (peaks && peaks.length) {
      const a = Math.floor((i * peaks.length) / n), b = Math.max(a + 1, Math.floor(((i + 1) * peaks.length) / n));
      for (let j = a; j < b; j++) v += peaks[j];
      v = Math.min(1, Math.max(0, (v / (b - a) - lo) / (hi - lo)));
    }
    // the playhead alone reaches the top
    h.push(3 + Math.round(v * (top - 7)));
  }
  Object.assign(el, { _peaks: peaks, _n: n, _h: h, _k: -1 });
  el.classList.toggle("is-idle", !peaks);
  el.innerHTML = h.map((v) => `<i style="height:${v}px"></i>`).join("");
  setHead(el, k);
}

function setHead(el, k) {
  if (!el._n) return;
  k = Math.max(0, Math.min(el._n - 1, k || 0));
  if (k === el._k) return;
  const bars = el.children;
  const [a, b] = el._k < 0 ? [0, el._n - 1] : [Math.min(el._k, k), Math.max(el._k, k)];
  for (let i = a; i <= b; i++) {
    bars[i].className = i < k ? "on" : "";
    bars[i].style.height = `${el._h[i]}px`;
  }
  bars[k].className = "head";
  bars[k].style.height = `${el.clientHeight || 64}px`;
  el._k = k;
}

function drawMainWave() {
  const f = current();
  if (!f || viewName() !== "work") return;
  const mode = modeOf(f), wave = $("wave");
  if (mode === "error") return;
  const n = barsIn(wave);
  const at = mode === "review" ? (f.duration ? f.pos / f.duration : 0) : f.step === "transcribing" ? f.done : 0;
  drawWave(wave, mode === "waiting" ? null : f.peaks, Math.floor(at * n));
}

/* The drop's strip: a made-up phrase, louder in the middle */
function drawDecor(el) {
  const n = barsIn(el);
  let seed = 11;
  const rnd = () => (seed = (seed * 9301 + 49297) % 233280) / 233280;
  el.innerHTML = Array.from({ length: n }, (_, i) =>
    `<i style="height:${Math.round(6 + Math.sin((Math.PI * (i + 0.5)) / n) * (14 + rnd() * 50))}px"></i>`).join("");
}

/* Each strip redrawn when its own width changes: the window resized, or
   its screen shown again. */
const resized = new ResizeObserver((entries) => {
  for (const e of entries) {
    if (e.target.id === "wave" && viewName() === "work") drawMainWave();
    else if (e.target.id === "idle-wave" && viewName() === "ready") drawWave($("idle-wave"), null, 0);
  }
});
for (const el of [$("wave"), $("idle-wave")]) resized.observe(el);

/* ------------------------------------------------------------ playback -- */

const player = $("audio");
let playId = null;    // the file the player holds
let playing = false;
let curSeg = null, curWord = null;

/* Playback's speed, kept between runs (prefs.speed). The player keeps the
   voice's pitch at any of them (preservesPitch, the web views' default), and
   the playhead, the underline and the waveform follow the recording's own
   time, which the speed leaves as it is. */
const SPEEDS = [0.75, 1, 1.25, 1.5, 1.75, 2];
const speed = () => (SPEEDS.includes(prefs.speed) ? prefs.speed : 1);

function showSpeed() {
  const s = speed();
  // the default too, which a new file's load sets the speed back to
  player.defaultPlaybackRate = player.playbackRate = s;
  player.preservesPitch = true;
  $("speed").textContent = `${s}×`;
  $("speed").classList.toggle("is-on", s !== 1);
  $("speed").setAttribute("aria-label", `Playback speed, ${s} times`);
  for (const c of $("menu").querySelectorAll("[data-speed]")) {
    c.classList.toggle("is-on", Number(c.dataset.speed) === s);
    c.setAttribute("aria-pressed", String(Number(c.dataset.speed) === s));
  }
}

function setSpeed(s) {
  if (s === speed()) return;
  prefs.speed = s;
  showSpeed();
  setPref("speed", s);
}

const stepSpeed = (d) => setSpeed(SPEEDS[Math.max(0, Math.min(SPEEDS.length - 1, SPEEDS.indexOf(speed()) + d))]);

/* The speeds as chips, then Slower and Faster with their keys: a step
   keeps the menu open, for the next one */
function speedMenu(anchor, keys) {
  const chip = (x) => `<button class="pk-rate" type="button" data-speed="${x}">${x}×</button>`;
  const html = `<div class="pk-rates" role="group" aria-label="Playback speed">${SPEEDS.map(chip).join("")}</div>` +
    `<div class="pk-menu-sep" aria-hidden="true"></div>` +
    `<button class="pk-mi" type="button" role="menuitem" data-step="-1"><span>Slower</span><span class="pk-key">&lt;</span></button>` +
    `<button class="pk-mi" type="button" role="menuitem" data-step="1"><span>Faster</span><span class="pk-key">&gt;</span></button>`;
  openMenu(anchor, keys, html, "down", (m) => {
    m.onclick = (e) => {
      const c = e.target.closest("[data-speed]"), b = e.target.closest("[data-step]");
      if (c) {
        setSpeed(Number(c.dataset.speed));
        closeMenu();
      } else if (b) stepSpeed(Number(b.dataset.step));
    };
  });
  showSpeed();
}

function loadAudio(f) {
  setPlayIcon();
  if (playId === f.id && player.getAttribute("src")) return;
  playId = f.id;
  if (f.audio) player.src = T.core.convertFileSrc(f.audio);
  else player.removeAttribute("src");
}

/* The play button for the file shown; when it cannot play, its tooltip
   says why. */
function setPlayIcon() {
  const f = current(), what = playing ? "Pause" : "Play";
  $("play").innerHTML = playing ? ICON.pause : ICON.play;
  $("play").setAttribute("aria-label", what);
  $("play").title = (f && f.audioWhy) || `${what} (Space)`;
  $("play").disabled = $("speed").disabled = !f || !playable(f);
}

function togglePlay() {
  const f = current();
  if (!f || modeOf(f) !== "review" || !playable(f) || playId !== f.id) return;
  if (!player.paused) return pausePlayback();
  if (f.pos >= f.duration - 0.05) f.pos = 0;
  playFrom(f.pos);
}

/* Play from t, once the player is there. Asked to seek and play at once,
   WebKit on macOS 27 starts the player at its old place and seeks it while
   it plays; with the old place's bytes still on their way, its GPU process
   deadlocked and the window hung (the Mac, 2026-10-23: a seek to 1187 s,
   then play). The seek finishes first, then the play. */
let playAfterSeek = false;
function playFrom(t) {
  const start = () => player.play().catch((e) => toast("Can't play", { err: true, detail: e.message }));
  if (Math.abs(player.currentTime - t) < 0.01 && !player.seeking) return start();
  player.currentTime = t;
  if (playAfterSeek) return;
  playAfterSeek = true;
  player.addEventListener("seeked", () => {
    if (playAfterSeek) start();
    playAfterSeek = false;
  }, { once: true });
}

/* The player stops and its file keeps its place, at once: the pause event
   comes later, maybe after another file has taken the player. */
function pausePlayback() {
  playAfterSeek = false;
  if (player.paused) return;
  const f = files.find((x) => x.id === playId);
  if (f) f.pos = player.currentTime;
  playing = false;
  player.pause();
  setPlayIcon();
}

function seek(t, play) {
  const f = current();
  if (!f || !playable(f)) return;
  f.pos = Math.max(0, Math.min(f.duration, t));
  showTime(f.pos, true);
  if (play && player.paused) playFrom(f.pos);
  else player.currentTime = f.pos;
}

player.addEventListener("play", () => {
  playing = true;
  setPlayIcon();
  requestAnimationFrame(tick);
});
player.addEventListener("pause", () => {
  playing = false;
  setPlayIcon();
});
player.addEventListener("ended", () => {
  playing = false;
  setPlayIcon();
  const f = files.find((x) => x.id === playId);
  if (!f) return;
  f.pos = f.duration;
  if (current() === f) showTime(f.pos);
});
/* a window out of sight gets no animation frames; the player's own few
   updates a second keep the place there */
player.addEventListener("timeupdate", () => {
  const f = current();
  if (playing && f && playId === f.id) {
    f.pos = player.currentTime;
    showTime(f.pos, "smooth");
  }
});
player.addEventListener("error", () => {
  const f = files.find((x) => x.id === playId);
  if (!f || !f.audio) return;
  f.audioWhy = "Can't play";
  if (current() !== f) return;
  setPlayIcon();
  $("wavebtn").disabled = true;
  $("wave").classList.add("is-off");
});

function tick() {
  if (!playing) return;
  const f = current();
  if (f && playId === f.id) {
    f.pos = player.currentTime;
    showTime(f.pos, "smooth");
  }
  requestAnimationFrame(tick);
}

function inSight(el) {
  const r = el.getBoundingClientRect(), box = $("transcript").getBoundingClientRect();
  return r.bottom > box.top && r.top < box.bottom;
}

/* last index whose start is at or before t, or -1 */
function at(list, t) {
  let lo = 0, hi = list.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (list[mid].start <= t) lo = mid + 1;
    else hi = mid;
  }
  return lo - 1;
}

/* The playhead at t: the time, the waveform, and the segment and word
   under it, brought into sight when follow says how: "smooth" while it
   plays, at once after a jump. */
function showTime(t, follow = false) {
  const f = current();
  if (!f) return;
  $("time").textContent = tc(t, f);
  const wave = $("wave");
  if (wave._n && f.duration) setHead(wave, Math.floor((t / f.duration) * wave._n));
  let seg = null, word = null;
  if (t > 0 || playing) {
    const rows = rowsOf(f), i = at(rows, t);
    seg = i >= 0 ? $("transcript").children[i] : null;
    if (seg) {
      const k = at(rows[i].words || [], t);
      word = k >= 0 ? seg.querySelector(`[data-w="${k}"]`) : null;
    }
  }
  if (seg !== curSeg) {
    // while it plays, only a reader still with the playhead is taken along
    const along = follow && (follow !== "smooth" || !curSeg || inSight(curSeg));
    if (curSeg) curSeg.classList.remove("is-current");
    if (seg) {
      seg.classList.add("is-current");
      // in the middle, the words to come in sight as well as those gone
      if (along) seg.scrollIntoView({ block: "center", behavior: follow === "smooth" ? "smooth" : "auto" });
    }
    curSeg = seg;
  }
  if (word !== curWord) {
    if (curWord) curWord.classList.remove("pk-now");
    if (word) word.classList.add("pk-now");
    curWord = word;
  }
}

/* The strip's time is where its format is chosen: seconds and hundredths,
   or frames. A video's frames are counted at its own rate, an audio file's
   at the rate picked here, which a video can be told to keep to as well:
   an editor's timeline has one rate, whatever its clips were shot at. */
const RATES = [23.976, 24, 25, 29.97, 30, 50, 59.94, 60];
function timeMenu(anchor, keys) {
  const f = current();
  if (!f) return;
  const now = f.pos || 0, rate = rateOf(f), own = ownRate(f);
  const choice = (frames, label, sub) => `<button class="pk-mi" type="button" role="menuitemradio" aria-checked="${!!prefs.frames === frames}" ` +
    `data-frames="${frames ? 1 : 0}"><span>${label}</span><small>${sub}</small>${!!prefs.frames === frames ? ICON.check : ""}</button>`;
  const chip = (value, label, on) => `<button class="pk-rate${on ? " is-on" : ""}" type="button" data-rate="${value}" aria-pressed="${on}">${label}</button>`;
  const html = choice(false, "Seconds", stamp(now)) +
    choice(true, "Frames", `${timecode(now, rate)} at ${rateLabel(rate)} fps${own ? ", the video's" : ""}`) +
    `<div class="pk-rates" role="group" aria-label="Frames a second">` +
    (f.fps > 0 ? chip("video", `Video · ${rateLabel(f.fps)}`, own) : "") +
    RATES.map((r) => chip(r, r, !own && Math.abs(r - rate) < 0.001)).join("") + "</div>";
  openMenu(anchor, keys, html, "down wide", (m) => {
    m.onclick = async (e) => {
      const b = e.target.closest("[data-frames]"), r = e.target.closest("[data-rate]");
      if (!b && !r) return;
      if (r && r.dataset.rate === "video") prefs.video_rate = true;
      else if (r) {
        prefs.fps = Number(r.dataset.rate);
        // picked over a video's own rate, it is the rate for every video
        if (f.fps > 0) prefs.video_rate = false;
      }
      // a rate picked is a rate to count in
      await setPref("frames", r ? true : b.dataset.frames === "1");
      closeMenu();
      retime();
    };
  });
}

/* the times drawn again in the format chosen, playback, the reader's place
   and the match they were on as they were */
function retime() {
  const f = current();
  if (!f || modeOf(f) !== "review") return;
  $("dur").textContent = f.duration == null ? "" : lengthOf(f);
  const tr = $("transcript"), top = tr.scrollTop, on = hitAt;
  if (query) {
    runFind();
    if (on >= 0 && on < hits.length) goHit(on);
    tr.scrollTop = top;
  } else {
    renderTranscript(f, "review");
    tr.scrollTop = top;
    showTime(f.pos);
  }
}

$("time").onclick = (e) => timeMenu(e.currentTarget, !e.detail);
$("play").onclick = () => togglePlay();
$("speed").onclick = (e) => speedMenu(e.currentTarget, !e.detail);
$("wavebtn").addEventListener("click", (e) => {
  const f = current();
  // Enter or Space on it is not a place on it
  if (!f || !e.detail) return;
  const r = $("wave").getBoundingClientRect();
  seek(((e.clientX - r.left) / r.width) * f.duration, false);
});
$("wavebtn").addEventListener("keydown", (e) => {
  const f = current();
  if (!f) return;
  const to = { ArrowLeft: f.pos - 5, ArrowRight: f.pos + 5, Home: 0, End: f.duration }[e.key];
  if (to === undefined) return;
  e.preventDefault();
  seek(to, false);
});

/* ---------------------------------------------------------------- find -- */

let query = "", hits = [], hitTimes = [], hitAt = -1;

/* text with each match marked, and where in text it starts */
/* The word of s at a character of its text, found as bodyHtml finds them:
   its index, or -1 before the first */
function wordAt(s, offset) {
  let at = 0, found = -1;
  (s.words || []).forEach((w, k) => {
    const j = s.text.indexOf(w.text, at);
    if (j < 0) return;
    if (j <= offset) found = k;
    at = j + w.text.length;
  });
  return found;
}

function runFind() {
  const f = current();
  if (!f || modeOf(f) !== "review") return;
  query = $("find").value.trim();
  const tr = $("transcript"), top = tr.scrollTop, rows = rowsOf(f);
  tr.innerHTML = rows.map((s, i) => segHtml(s, i, false)).join("") + statsHtml(f);
  tr.scrollTop = top;
  curSeg = curWord = null;
  showTime(f.pos);
  // each match once, by its first piece
  hits = [...tr.querySelectorAll(".pk-hit[data-at]")];
  // each match at its word's time, for its tick under the waveform
  hitTimes = hits.map((h) => {
    const s = rows[Number(h.closest(".pk-seg").dataset.i)], k = wordAt(s, Number(h.dataset.at));
    return k >= 0 ? s.words[k].start : s.start;
  });
  // the match the playhead last passed, so that a search typed as it plays
  // or rests mid-recording starts where the listener is; the first when it
  // has passed none
  hitAt = -1;
  if (hits.length) {
    let k = 0;
    hitTimes.forEach((t, j) => { if (t <= f.pos) k = j; });
    goHit(k);
  }
  findCount();
}

function stepHit(d) {
  if (hits.length) goHit((hitAt + d + hits.length) % hits.length);
}

/* every piece of match k */
const hitPieces = (k) => $("transcript").querySelectorAll(`[data-hit="${hits[k].dataset.hit}"]`);

function goHit(k) {
  if (hitAt >= 0) for (const el of hitPieces(hitAt)) el.classList.remove("is-on");
  hitAt = k;
  for (const el of hitPieces(hitAt)) el.classList.add("is-on");
  hits[hitAt].scrollIntoView({ block: "center" });
  findCount();
}

/* the count, previous and next, and the ticks */
function findCount() {
  $("find-n").textContent = query ? `${hits.length ? hitAt + 1 : 0}/${hits.length}` : "";
  $("find-box").classList.toggle("has-query", !!query);
  $("find-prev").disabled = $("find-next").disabled = hits.length < 2;
  const f = current(), dur = f && f.duration;
  $("ticks").innerHTML = dur ? hitTimes.map((t, k) =>
    `<i data-k="${k}"${k === hitAt ? ' class="is-on"' : ""} style="left:${((t / dur) * 100).toFixed(3)}%"></i>`).join("") : "";
}

function clearFind() {
  $("find").value = "";
  runFind();
}

function openFind() {
  const f = current();
  if (shown() !== "work" || !f || $("find-box").hidden) return;
  $("find").focus();
  $("find").select();
}

$("find").addEventListener("input", runFind);
$("find-prev").onclick = (e) => { e.preventDefault(); stepHit(-1); };
$("find-next").onclick = (e) => { e.preventDefault(); stepHit(1); };
/* a tick: its match, and the playhead there */
$("ticks").addEventListener("click", (e) => {
  const k = e.target.closest("[data-k]");
  if (!k) return;
  goHit(Number(k.dataset.k));
  seek(hitTimes[hitAt], false);
});
$("find").addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    stepHit(e.shiftKey ? -1 : 1);
  } else if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    clearFind();
    $("find").blur();
  }
});

/* ------------------------------------------------------------ captions -- */

/* A finished transcript as its subtitle track: the engine's pk_captions
   (the captions command) cuts its words into cues of one or two lines of
   42 characters, as the CLI's --srt does; English's rules for a model of
   English alone. Asked for once a transcript, the first time something
   needs them: the view, or a .srt, .vtt or .json. */
async function cuesOf(f) {
  if (f.cues) return f.cues;
  const w = f.segments.flatMap((s) => s.words || []);
  const m = st.catalog.find((x) => x.name === f.model);
  // the English model's words are English; the multilingual model's say
  // which they are (the engine's pk_language)
  const english = !!m && m.languages.length === 1 && m.languages[0] === "en" ? 1 : -1;
  const raw = await invoke("captions", { words: w.map((x) => ({ text: x.text, start: x.start, end: x.end })), english,
    duration: f.duration || 0 });
  // each cue a row of the transcript: its text with its line break, its
  // words, its times; one a pause cut in mid-sentence ends in an ellipsis
  // (pktito's PK_CUE_PAUSE, which its line has room for)
  f.cues = raw.map((c) => {
    const ws = w.slice(c.first, c.first + c.n);
    return { start: c.start / 1000, end: c.end / 1000, startMs: c.start, endMs: c.end, chars: c.chars, flags: c.flags, words: ws,
      text: ws.map((x, k) => (k ? (c.first + k === c.line2 ? "\n" : " ") : "") + x.text).join("") + (c.flags & 8 ? "\u2026" : "") };
  });
  return f.cues;
}

/* what the transcript shows: its cues, once a finished one is shown as
   captions and they have come, else its sentences */
const rowsOf = (f) => (prefs.captions && f.cues && modeOf(f) === "review" ? f.cues : f.segments);

/* The transcript drawn again in the view chosen, a search kept, and the
   playhead's row in sight: the other view's place means nothing in this
   one, and with its row out of sight playback no longer took the reader
   along (the Windows test, 2026-10-22). */
function rebuild() {
  const f = current();
  if (!f) return;
  if (modeOf(f) !== "review") {
    mainKey = "";
    return showMain();
  }
  // the transcript alone: through showMain it paused playback and dropped
  // the search (Peter, 2026-10-22)
  for (const b of $("views").children) b.setAttribute("aria-checked", String((b.dataset.view === "captions") === !!prefs.captions));
  updateHead(f, "review");
  const on = hitAt;
  if (query) {
    runFind();
    if (on >= 0 && on < hits.length) goHit(on);
  } else {
    renderTranscript(f, "review");
    showTime(f.pos);
  }
  if (curSeg) curSeg.scrollIntoView({ block: "center" });
}

$("views").onclick = async (e) => {
  const b = e.target.closest("[data-view]"), f = current();
  if (!b || !f) return;
  const on = b.dataset.view === "captions";
  if (on === !!prefs.captions) return;
  setPref("captions", on);
  try {
    if (on) await cuesOf(f);
  } catch (err) {
    toastErr(err);
  }
  if (current() === f) rebuild();
};

/* -------------------------------------------------------- copy and save -- */

const FORMATS = [
  { ext: "txt", label: () => (prefs.times ? "Text with times" : "Text"), key: ["S", false] },
  { ext: "srt", label: () => "Subtitles", key: ["S", true] },
  { ext: "vtt", label: () => "Web subtitles" },
  { ext: "md", label: () => (prefs.times ? "Markdown with times" : "Markdown") },
  { ext: "json", label: () => "JSON with word timings" },
];

/* A file's text in a format, the bytes pktito-cli writes for it too
   (src-tauri/src/format.rs): times before each sentence of .txt and .md
   as the Save menu has them, unless told */
const textOf = (f, ext, times = !!prefs.times) =>
  invoke("text_of", { ext, name: f.name, duration: f.duration || 0, model: f.model || null, segments: f.segments, times });

/* What the view shows: a finished transcript's captions as an .srt's text,
   numbered cues and their times, the form subtitle tools take; or its
   sentences, without their times. */
async function copyText() {
  const f = current();
  if (!f || !f.segments.length) return;
  const cues = !!prefs.captions && modeOf(f) === "review";
  try {
    await navigator.clipboard.writeText(await textOf(f, cues ? "srt" : "txt", false));
    toast(cues ? "Copied captions" : "Copied");
  } catch (e) {
    toast("Couldn't copy", { err: true, detail: String(e) });
  }
}

async function save(ext) {
  // a batch's Save as is Save all, in that format
  if (shown() === "batch") return saveAll(ext);
  const f = current();
  if (shown() !== "work" || !f || modeOf(f) !== "review" || !f.segments.length) return;
  closeMenu();
  try {
    const r = await invoke("save_text", { original: f.path, ext, text: await textOf(f, ext), nextTo: prefs.next_to_original });
    if (r) toast(`Saved ${baseName(r.path)}`, { reveal: r.path });
  } catch (e) {
    toastErr(e);
  }
}

/* Every transcript of the batch: those done now, and any still to come
   each as it finishes (batchSave, finished). "folder" is all five formats
   into one folder, asked for once; a format, that file beside each
   original. */
async function saveAll(kind) {
  closeMenu();
  let dir = null;
  if (kind === "folder") {
    try {
      dir = await invoke("pick_folder");
    } catch (e) {
      return toastErr(e);
    }
    if (!dir) return;
  }
  batchSave = files.some(live) ? { kind, dir } : null;
  batchHead();
  const ready = files.filter((f) => f.step === "done" && f.segments.length);
  if (ready.length) await saveTo(ready, kind, dir);
}

async function saveTo(list, kind, dir) {
  const exts = kind === "folder" ? FORMATS.map((x) => x.ext) : [kind];
  try {
    const out = await invoke("save_many", { dir,
      files: await Promise.all(list.flatMap((f) => exts.map(async (ext) => ({ original: f.path, ext, text: await textOf(f, ext) })))) });
    if (out.length) toast(out.length === 1 ? `Saved ${baseName(out[0])}` : `Saved ${plural(out.length, "file")}`, { reveal: out[0] });
  } catch (e) {
    toastErr(e);
  }
}

async function setPref(k, v) {
  prefs[k] = v;
  try {
    await invoke("set_prefs", { prefs });
  } catch (e) {
    toastErr(e);
  }
}

const checkHtml = (pref, text, more = "") =>
  `<label class="pk-check"><input type="checkbox" data-pref="${pref}"${prefs[pref] ? " checked" : ""}>` +
  `<span>${text}${more ? `<small>${more}</small>` : ""}</span></label>`;

function saveMenu(anchor, keys) {
  const stem = stemOf(current().path);
  const html = FORMATS.map((x) => `<button class="pk-mi" type="button" role="menuitem" data-save="${x.ext}"><span>${x.label()}</span>` +
    `<small>${esc(stem)}.${x.ext}</small>${x.key ? `<span class="pk-kbd">${kbd(...x.key)}</span>` : ""}</button>`).join("") +
    `<div class="pk-menu-sep" aria-hidden="true"></div>` +
    checkHtml("next_to_original", "Save next to the original") + checkHtml("times", "Timestamps in .txt and .md");
  openMenu(anchor, keys, html, "down right", (m) => {
    m.onclick = (e) => {
      const b = e.target.closest("[data-save]");
      if (b) save(b.dataset.save);
    };
    m.onchange = (e) => {
      const c = e.target.closest("[data-pref]");
      if (!c) return;
      setPref(c.dataset.pref, c.checked);
      for (const b of m.querySelectorAll("[data-save]")) b.firstElementChild.textContent = FORMATS.find((x) => x.ext === b.dataset.save).label();
    };
  });
}

/* each choice with how many it saves now, and how many as they finish */
function saveAllMenu(anchor, keys) {
  const n = files.filter((f) => f.step === "done" && f.segments.length).length, later = files.filter(live).length;
  const count = (ext) => `${n} x .${ext}${later ? `, ${later} more as they finish` : ""}`;
  const html = `<button class="pk-mi" type="button" role="menuitem" data-all="txt"><span>Text, next to each original</span><small>${count("txt")}</small></button>` +
    `<button class="pk-mi" type="button" role="menuitem" data-all="srt"><span>Subtitles, next to each original</span><small>${count("srt")}</small></button>` +
    `<button class="pk-mi" type="button" role="menuitem" data-all="folder"><span>All of them into one folder…</span><small>.txt .srt .vtt .md .json</small></button>`;
  openMenu(anchor, keys, html, "down right", (m) => {
    m.onclick = (e) => {
      const b = e.target.closest("[data-all]");
      if (b) saveAll(b.dataset.all);
    };
  });
}

$("copy").onclick = () => copyText();
$("save-as").onclick = (e) => saveMenu(e.currentTarget, !e.detail);
$("save-all").onclick = (e) => saveAllMenu(e.currentTarget, !e.detail);
$("stop").onclick = () => {
  const f = current();
  if (!f) return;
  $("stop").disabled = true;
  invoke("stop", { id: f.id }).catch(() => {});
};
$("again").onclick = () => current() && again(current());

/* --------------------------------------------------------------- menus -- */

let menuFor = null;

/* One menu at a time, by its button: below it, or above for "up"; its
   right edge on the button's for "right"; "wide" for a row of choices that
   should not wrap. Opened from the keyboard, its first item takes the
   focus. */
function openMenu(anchor, keys, html, place, wire) {
  if (menuFor === anchor) return closeMenu();
  closeMenu();
  const m = $("menu");
  m.innerHTML = html;
  m.classList.toggle("is-wide", place.includes("wide"));
  m.hidden = false;
  menuFor = anchor;
  anchor.setAttribute("aria-expanded", "true");
  wire(m);
  const r = anchor.getBoundingClientRect(), w = m.offsetWidth, h = m.offsetHeight;
  const left = place.includes("right") ? r.right - w : r.left;
  let top = place.includes("up") ? r.top - h - 6 : r.bottom + 6;
  if (top + h > innerHeight - 12) top = r.top - h - 6;
  m.style.left = `${Math.max(12, Math.min(innerWidth - w - 12, left))}px`;
  m.style.top = `${Math.max(12, top)}px`;
  const first = m.querySelector("button, input");
  if (first && keys) first.focus({ preventScroll: true });
}

/* The keyboard's place, if it was in the menu, goes back to its button. */
function closeMenu() {
  if (!menuFor) return false;
  const was = $("menu").contains(document.activeElement);
  $("menu").hidden = true;
  menuFor.setAttribute("aria-expanded", "false");
  if (was) menuFor.focus({ preventScroll: true });
  menuFor = null;
  return true;
}

document.addEventListener("pointerdown", (e) => {
  if (menuFor && !$("menu").contains(e.target) && !menuFor.contains(e.target)) closeMenu();
}, true);
$("menu").addEventListener("keydown", (e) => {
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  const items = [...$("menu").querySelectorAll("button, input")];
  const i = items.indexOf(document.activeElement);
  items[(i + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length].focus();
  e.preventDefault();
});
window.addEventListener("blur", () => closeMenu());

/* --------------------------------------------------------------- toast -- */

let toastTimer = 0;

/* A command's error, "what happened: what said so", as the Rust side
   writes them */
function toastErr(e) {
  const s = String(e), i = s.indexOf(": ");
  toast(i < 0 ? s : s.slice(0, i), { err: true, detail: i < 0 ? "" : s.slice(i + 2) });
}

/* A few words; what said so, if anything, in the tooltip */
function toast(text, { reveal = null, err = false, detail = "" } = {}) {
  const t = $("toast");
  t.innerHTML = `${err ? ICON.alert : ICON.check}<span></span>` +
    (reveal ? `<button class="pk-linkbtn" type="button">${revealLabel()}</button>` : "");
  t.querySelector("span").textContent = text;
  t.querySelector("span").title = detail || text;
  if (reveal) t.querySelector("button").onclick = () => invoke("reveal", { path: reveal }).catch((e) => toastErr(e));
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { t.hidden = true; }, reveal || err ? 6000 : 3500);
}

/* --------------------------------------------------------------- about -- */

let aboutFrom = null;   // what had the keyboard when About opened, which gets it back
let mascot;             // undefined until About first opens; null without WebGL 2
let mascotFiles = null;

/* One of the window's scripts, the first time something needs it, so that
   the window loads what it shows */
const scripts = {};
function loadScript(src) {
  scripts[src] = scripts[src] || new Promise((ok, no) => {
    const s = document.createElement("script");
    s.src = src;
    s.onload = ok;
    s.onerror = no;
    document.head.appendChild(s);
  });
  return scripts[src];
}

/* The mascot's model and viewer, the first time About opens: 22 KB and
   7 KB of the app's assets, compressed. */
function loadMascot() {
  mascotFiles = mascotFiles || loadScript("mascot-model.js").then(() => loadScript("mascot.js"))
    .then(() => { mascot = drawMascot($("about-stage"), $("mascot")); })
    .catch(() => { mascot = null; });
  return mascotFiles;
}

async function openAbout() {
  if (!st || about) return;
  closeMenu();
  pausePlayback();
  aboutFrom = document.activeElement;
  about = true;
  // the engine's code for this processor by its name; all of "neon-dp (ARMv8.2: SDOT)" in the tooltip
  $("about-version").textContent = `Version ${st.version} · ${st.backend.split(" (")[0]}`;
  $("about-version").title = st.backend;
  document.body.classList.add("is-about");
  $("about").hidden = false;
  $("about-back").focus({ preventScroll: true });
  await loadMascot();
  $("about").classList.toggle("no-stage", !mascot);
  if (about && mascot) mascot.show();
}

/* Back to the screen beneath, as it is now; false if About was not up */
function closeAbout() {
  if (!about) return false;
  about = false;
  closeNotices();
  if (mascot) mascot.hide();
  document.body.classList.remove("is-about");
  $("about").hidden = true;
  if (aboutFrom && aboutFrom.offsetParent) aboutFrom.focus({ preventScroll: true });
  aboutFrom = null;
  return true;
}

document.addEventListener("click", (e) => {
  if (e.target.closest("[data-about]")) openAbout();
});
$("about-back").onclick = () => closeAbout();
/* its links in the browser, not in the window */
$("about").addEventListener("click", (e) => {
  const a = e.target.closest("a[href]");
  if (!a) return;
  e.preventDefault();
  invoke("open_link", { url: a.href }).catch((err) => toastErr(err));
});
$("about").addEventListener("auxclick", (e) => {
  if (e.target.closest("a[href]")) e.preventDefault();
});

/* Open source licenses, under About's Everything else: what pktito is
   built from beside its own code and the models About names, each with its
   copyright and license, and the licenses' texts. notices.js (scripts/notices.py) has every system's;
   this one's are listed. It loads the first time the list opens: 20 KB
   compressed. */
function noticesHtml() {
  const bit = NOTICES.systems[st.platform] || 0;
  const here = (x) => !bit || !x[4] || (x[4] & bit) !== 0;
  const extras = NOTICES.extras.filter(here), crates = NOTICES.crates.filter(here);
  const keys = [...new Set([...extras, ...crates].flatMap((x) => x[5]))];
  // a crate's name is code, and set as code
  const row = (name, lic, notice, code = false) => `<li><span class="pk-nn${code ? " pk-code" : ""}">${esc(name)}</span>` +
    `<span class="pk-nl">${esc(lic)}</span><span class="pk-nc">${esc(notice)}</span></li>`;
  const list = (xs) => `<ul class="pk-nlist">${xs.join("")}</ul>`;
  const system = { macos: "macOS", windows: "Windows", linux: "Linux" }[st.platform] || st.platform;
  return `<p class="pk-small">This build of pktito includes the following open-source software and libraries:</p>
    <h3>Typefaces and icons</h3>
    ${list(extras.filter((e) => e[0] === "type").map((e) => row(e[1], e[3], e[2])))}
    <h3>Libraries (${esc(system)})</h3>
    ${list(extras.filter((e) => e[0] === "code").map((e) => row(e[1], e[3], e[2]))
      .concat(crates.map((c) => row(`${c[0]} ${c[1]}`, c[2], c[3], true))))}
    <h3>License texts</h3>
    ${keys.map((k) => `<h4>${esc(NOTICES.texts[k].title)}</h4><pre>${esc(NOTICES.texts[k].text)}</pre>`).join("")}`;
}

async function openNotices() {
  if (!about || !$("notices").hidden) return;
  try {
    await loadScript("notices.js");
  } catch (e) {
    return toast("Couldn't load the list", { err: true, detail: String(e) });
  }
  if (!about) return;
  const body = $("notices-body");
  if (!body.childElementCount) body.innerHTML = noticesHtml();
  body.scrollTop = 0;
  $("notices").hidden = false;
  body.focus({ preventScroll: true });
  // the mascot rests behind it, home, and draws nothing: it glanced at the
  // pointer moving over the list (the Windows test, 2026-10-21)
  if (mascot) mascot.hide();
}

/* false if the list was not open */
function closeNotices() {
  if ($("notices").hidden) return false;
  $("notices").hidden = true;
  $("notices-open").focus({ preventScroll: true });
  if (about && mascot) mascot.show();
  return true;
}

$("notices-open").onclick = () => openNotices();
$("notices-close").onclick = () => closeNotices();
$("notices").addEventListener("click", (e) => {
  if (e.target === $("notices")) closeNotices();
});
/* the keyboard stays in the list: its two places, the close button and the text */
$("notices").addEventListener("keydown", (e) => {
  if (e.key !== "Tab") return;
  e.preventDefault();
  (document.activeElement === $("notices-close") ? $("notices-body") : $("notices-close")).focus();
});

/* ------------------------------------------------------- drag and drop -- */

function showDrop(paths) {
  const ok = paths.filter(isMedia);
  $("drop-line").textContent = ok.length ? `Drop to transcribe ${plural(ok.length, "file")}` : "Not audio or video";
  const names = (ok.length ? ok : paths).map(baseName);
  const chips = names.slice(0, 6).map((n) => `<li>${esc(n)}</li>`);
  if (names.length > 6) chips.push(`<li>and ${names.length - 6} more</li>`);
  $("drop-names").innerHTML = chips.join("");
  $("drop").hidden = false;
  drawDecor($("drop-wave"));
}

const webview = T.webview && T.webview.getCurrentWebview ? T.webview.getCurrentWebview() : null;
if (webview && webview.onDragDropEvent) {
  webview.onDragDropEvent((e) => {
    const p = e.payload;
    if (!st) return;
    if (p.type === "enter") showDrop(p.paths);
    else if (p.type === "drop") {
      $("drop").hidden = true;
      addPaths(p.paths);
    } else if (p.type === "leave") $("drop").hidden = true;
  });
}

/* ------------------------------------------------------------ keyboard -- */

const typing = (el) => el && (el.isContentEditable || el.tagName === "TEXTAREA" ||
  (el.tagName === "INPUT" && !["checkbox", "radio", "button"].includes(el.type)));

/* No page menu: WebView2's and WebKit's (back, reload, print, save the
   page) are no part of the app. Text fields and selected text keep theirs,
   for cut, copy and paste, which on Windows is all it holds (main.rs,
   tame_webview). On Windows a right-click where the window drags, its
   header rows and bands, opens the window's menu, as on a title bar (the
   Windows test, 2026-10-22). */
document.addEventListener("contextmenu", (e) => {
  if (e.target.closest("input, textarea") || String(getSelection())) return;
  e.preventDefault();
  if (window.__PK_CHROME === "win" && e.target.closest("[data-tauri-drag-region]") && !e.target.closest("button, a, label"))
    invoke("window_menu", { atCursor: true }).catch(() => {});
});

document.addEventListener("keydown", (e) => {
  /* The browser's own keys, which WebView2 acts on unless the page does:
     reload (F5, Ctrl+R), print, view source, find again (F3, Ctrl+G), its
     tools, back and forward. A reload loses the transcript open, and none
     of them is anything here. Alt+Space is the window's menu on Windows. */
  const key = e.key.toLowerCase(), ctl = e.ctrlKey || e.metaKey;
  if (key === "f5" || key === "f3" || key === "f12" || key === "browserback" || key === "browserforward" ||
      (ctl && !e.altKey && !e.shiftKey && ["r", "p", "u", "g"].includes(key)) || (ctl && e.shiftKey && ["r", "i", "j", "c"].includes(key)) ||
      (st && !isMac() && e.altKey && !ctl && !typing(e.target) && (key === "arrowleft" || key === "arrowright"))) {
    e.preventDefault();
    return;
  }
  if (window.__PK_CHROME === "win" && e.altKey && !ctl && e.code === "Space") {
    e.preventDefault();
    invoke("window_menu", { atCursor: false }).catch(() => {});
    return;
  }
  if (!st) return;
  // Esc backs out one step: a question, a menu, About, a search, then the
  // file, then its batch
  if (e.key === "Escape") {
    e.preventDefault();
    if (closeAsk() || closeMenu() || closeNotices() || closeAbout()) return;
    if (query && viewName() === "work") clearFind();
    else back();
    return;
  }
  /* Only macOS has a menu bar, outside the window, and its shortcuts work
     there. On Windows and Linux the window has no menu bar, so the page
     takes them. */
  if (!isMac() && e.ctrlKey && !e.altKey && !e.metaKey) {
    const k = e.key.toLowerCase();
    if (k === "o" && !e.shiftKey) chooseFiles();
    else if (k === "s") save(e.shiftKey ? "srt" : "txt");
    else if (k === "f" && !e.shiftKey) openFind();
    else return;
    e.preventDefault();
    return;
  }
  /* < and > step playback's speed, as on YouTube: the keys point the way,
     and [ and ] take AltGr on German, French and Spanish keyboards. AltGr
     comes as Ctrl+Alt on Windows, so only a Ctrl or Cmd shortcut is left
     alone. */
  if ((e.key === "<" || e.key === ">") && !e.metaKey && !(e.ctrlKey && !e.altKey) && !typing(e.target) && shown() === "work") {
    const f = current();
    if (f && modeOf(f) === "review" && playable(f)) {
      e.preventDefault();
      stepSpeed(e.key === ">" ? 1 : -1);
    }
    return;
  }
  if (e.key === " " && !menuFor && !typing(e.target) && shown() === "work") {
    const f = current();
    if (f && modeOf(f) === "review" && playable(f)) {
      e.preventDefault();
      if (!e.repeat) togglePlay();
    }
  }
});

/* The mouse's back button goes back as the header's arrow does, closing
   first a question, a menu or About. WebKit and WebView2 both give it as
   button 3; the page has no history of its own for it to go back in. */
document.addEventListener("mouseup", (e) => {
  if (e.button !== 3 || !st) return;
  e.preventDefault();
  if (closeAsk() || closeMenu() || closeNotices() || closeAbout()) return;
  back();
});

listen("menu", (e) => {
  const id = e.payload;
  // a command from the menu bar while the window asks: Cancel, and on to it
  closeAsk();
  if (id === "add") chooseFiles();
  else if (id === "about") openAbout();
  else if (id === "find") openFind();
  else if (id === "install-cli") installCli();
  else if (id.startsWith("save-")) save(id.slice(5));
});

/* pktito-cli on macOS's PATH (main.rs, install_cli): what happened, in a
   toast; nothing when the password was not given */
async function installCli() {
  try {
    const said = await invoke("install_cli");
    if (said) toast(said);
  } catch (e) {
    toastErr(e);
  }
}

/* --------------------------------------------------------------- start -- */

async function start() {
  st = await invoke("status");
  prefs = st.prefs;
  dl = st.download;
  for (const b of [$("back"), $("batch-back"), $("about-back")]) {
    b.innerHTML = ICON.back;
    b.title = "Back (Esc)";
  }
  $("add").innerHTML = `${ICON.plus}Add files`;
  $("add").title = tip("Add files", "O");
  $("save-all").innerHTML = `${ICON.download}Save all${ICON.down}`;
  $("time").title = "Seconds or frames";
  $("speed").title = "Playback speed (< slower, > faster)";
  showSpeed();
  $("notices-close").innerHTML = ICON.x;
  $("notices-close").title = "Close (Esc)";
  // the question's buttons where the system's own dialogs put Cancel: first
  // on macOS, last on Windows
  if (isMac()) $("ask-btns").prepend($("ask-no"));
  $("find-box").title = tip("Find", "F");
  $("find-prev").innerHTML = ICON.up;
  $("find-prev").title = "Previous (Shift+Enter)";
  $("find-next").innerHTML = ICON.down;
  $("find-next").title = "Next (Enter)";
  try {
    recent = await invoke("recent_list");
  } catch {
    recent = [];
  }
  render();
  const given = await invoke("initial_files");
  if (given.length) await openPaths(given);
}

/* Files the system opens with the app, at start or while it runs (main.rs,
   hand_over): a .pkf, double-clicked, the model in use, the last if more
   than one; the media queued, as dropped */
async function openPaths(paths) {
  const models = paths.filter((p) => extOf(p) === "pkf"), media = paths.filter((p) => extOf(p) !== "pkf");
  if (models.length) await openModel(models[models.length - 1]);
  if (media.length) addPaths(media);
}
listen("open", (e) => openPaths(e.payload));

/* A model file opened from outside the window, checked by loading it, as
   Open a model file does, then in use and the start screen showing it. One
   pktito cannot load leaves the model in use as it was, and the toast says
   why (main.rs, use_file). */
async function openModel(path) {
  try {
    const name = await invoke("open_model", { path });
    if (dl) await invoke("cancel_download");
    await refresh();
    closeAbout();
    goHome();
    toast(`Using ${name}`);
  } catch (e) {
    toastErr(e);
  }
}

start().catch((e) => {
  document.body.textContent = `pktito could not start: ${e}`;
});

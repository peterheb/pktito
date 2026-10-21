/* pktito's window: a queue of files, each extracted and transcribed in turn
   by the Rust side (src-tauri/src/main.rs), and the selected file's
   transcript, which fills in as its segments finish. The language chosen
   picks the model, downloaded the first time a file needs it. */
"use strict";

const T = window.__TAURI__;
const invoke = T.core.invoke;
const listen = T.event.listen;

const $ = (id) => document.getElementById(id);
const svg = (body, sw = 2) =>
  `<svg class="pk-ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="${sw}" ` +
  `stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;
const ICON = {
  music: svg('<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>'),
  film: svg('<rect x="2" y="2" width="20" height="20" rx="2.18"/><line x1="7" y1="2" x2="7" y2="22"/>' +
    '<line x1="17" y1="2" x2="17" y2="22"/><line x1="2" y1="12" x2="22" y2="12"/>'),
  copy: svg('<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>'),
  check: svg('<polyline points="20 6 9 17 4 12"/>', 2.4),
};

let status = null;
const files = [];      // {id, path, name, kind, step, pct, duration, segments, message}
let nextId = 1;
let selected = null;
let busy = false;

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
function srtTime(s) {
  const ms = Math.max(0, Math.round(s * 1000));
  return `${hms(Math.floor(ms / 1000))},${String(ms % 1000).padStart(3, "0")}`;
}
const baseName = (p) => p.split(/[\\/]/).pop();
const stem = (p) => baseName(p).replace(/\.[^.]+$/, "");
const extOf = (p) => (baseName(p).match(/\.([^.]+)$/) || ["", ""])[1].toLowerCase();
const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const mb = (b) => Math.round(b / 1e6);

/* --------------------------------------------------------------- views -- */

function show(view) {
  for (const v of ["empty", "work"]) $(v).hidden = v !== view;
  const slot = $(view === "work" ? "side-model" : "empty-model");
  if ($("model-box").parentNode !== slot) slot.appendChild($("model-box"));
}

async function refresh() {
  status = await invoke("status");
  if (!status.model && !status.model_file) {
    // the first run: the first model that speaks the system's language, else English
    const lang = (navigator.language || "en").slice(0, 2).toLowerCase();
    const m = status.catalog.find((x) => x.languages.includes(lang)) || status.catalog[0];
    await invoke("choose_model", { name: m.name });
    status = await invoke("status");
  }
  renderModel();
  render();
  if (!started) {
    started = true;
    const given = await invoke("initial_files");
    if (given.length) addPaths(given);
  }
}
let started = false;

/* --------------------------------------------------------------- model -- */

function renderModel() {
  const sel = $("model");
  sel.innerHTML = "";
  for (const m of status.catalog) sel.add(new Option(m.label, m.name));
  if (status.model_file) sel.add(new Option(baseName(status.model_file), "file"));
  sel.add(new Option("A model file on this computer…", "pick"));
  sel.value = status.model_file ? "file" : status.model;
  modelLine();
}

/* Under the choice, where its model stands: on this computer, downloaded
   with the first file, or downloading now. */
function modelLine(text = "", err = false) {
  const m = status.catalog.find((x) => x.name === status.model);
  const d = files.find((f) => f.step === "downloading");
  if (!text) {
    if (d) text = `Downloading ${d.message}`;
    else if (status.model_file) text = status.model_file;
    else if (m && m.have) text = `${m.name}, on this computer`;
    else if (m) text = `${m.name}: ${mb(m.bytes)} MB, downloaded with the first file`;
  }
  const line = $("model-line");
  line.textContent = text;
  line.title = status.model_file || status.backend;
  line.classList.toggle("err", err);
}

$("model").onchange = async () => {
  const v = $("model").value;
  try {
    if (v === "pick") await invoke("use_model_file");
    else if (v !== "file") await invoke("choose_model", { name: v });
    status = await invoke("status");
    renderModel();
  } catch (e) {
    renderModel();
    modelLine(String(e), true);
  }
};

/* --------------------------------------------------------- adding files -- */

async function chooseFiles() {
  if (!status) return;
  addPaths(await invoke("pick_files"));
}

function addPaths(paths) {
  const refused = [];
  let first = null;
  for (const p of paths) {
    if (!status.media.includes(extOf(p))) {
      refused.push(baseName(p));
      continue;
    }
    const f = { id: nextId++, path: p, name: baseName(p), kind: status.video.includes(extOf(p)) ? "video" : "audio",
      step: "waiting", pct: 0, duration: null, segments: [], message: "" };
    files.push(f);
    first = first || f;
  }
  if (first && (selected === null || !files.some((f) => f.id === selected))) selected = first.id;
  render();
  if (refused.length) note(`pktito reads audio and video files. Not added: ${refused.join(", ")}`, true);
  pump();
}

/* ---------------------------------------------------- transcribing them -- */

async function pump() {
  if (busy) return;
  const f = files.find((x) => x.step === "waiting");
  if (!f) return;
  busy = true;
  try {
    await invoke("transcribe", { id: f.id, path: f.path });
  } catch (e) {
    f.step = "error";
    f.message = String(e);
  }
  busy = false;
  render();
  pump();
}

listen("job", async (e) => {
  const j = e.payload;
  const f = files.find((x) => x.id === j.id);
  if (!f) return;
  const was = f.step;
  f.step = j.step;
  f.pct = j.pct;
  if (j.duration) f.duration = j.duration;
  f.message = j.message;
  if (j.segments.length) {
    f.segments.push(...j.segments);
    if (selected === f.id) appendSegments(j.segments);
  }
  renderQueue();
  if (selected === f.id) renderHead();
  if (was === "downloading" && j.step !== "downloading") {
    status = await invoke("status");
    modelLine();
  } else if (j.step === "downloading") modelLine();
});

/* ----------------------------------------------------------- rendering -- */

function render() {
  if (!status) return;
  if (!files.length) {
    selected = null;
    return show("empty");
  }
  show("work");
  renderQueue();
  renderMain();
}

function rowHtml(f) {
  const sel = f.id === selected;
  let tail = "";
  if (f.step === "waiting") tail = `<span class="pk-status">Waiting</span>`;
  else if (f.step === "extracting")
    tail = `<span class="pk-bar is-indeterminate" role="progressbar" aria-label="Extracting audio"><i></i></span>` +
      `<span class="pk-status">Extracting audio</span>`;
  else if (f.step === "downloading" || f.step === "transcribing") {
    const what = f.step === "downloading" ? "Downloading the model" : "Transcribing";
    tail = `<span class="pk-bar" role="progressbar" aria-label="${what}" aria-valuenow="${f.pct}" aria-valuemin="0" ` +
      `aria-valuemax="100" style="--p:${f.pct}%"><i></i></span><span class="pk-status pk-num">${what} ${f.pct}%</span>`;
  }
  else if (f.step === "done") tail = `<span class="pk-status ok">${ICON.check}Done</span>`;
  else tail = `<span class="pk-status err">${esc(f.message || "This file could not be transcribed")}</span>`;
  return `<li class="pk-file" role="option" aria-selected="${sel}" tabindex="${sel ? 0 : -1}" data-id="${f.id}" ` +
    `title="${esc(f.path)}">${f.kind === "video" ? ICON.film : ICON.music}<span class="pk-name">${esc(f.name)}</span>` +
    `<span class="pk-stamp">${f.duration === null ? "" : hms(f.duration)}</span>${tail}</li>`;
}

function renderQueue() {
  const focused = document.activeElement && document.activeElement.closest(".pk-file");
  $("queue").innerHTML = files.map(rowHtml).join("");
  if (focused) {
    const row = $("queue").querySelector(`[data-id="${selected}"]`);
    if (row) row.focus();
  }
}

function current() {
  return files.find((f) => f.id === selected) || null;
}

function renderHead() {
  const f = current();
  $("title").textContent = f ? f.name : "";
  const has = !!(f && f.segments.length);
  for (const id of ["copy", "save-txt", "save-srt"]) $(id).disabled = !has;
  if (!$("copy").dataset.copied) $("copy").innerHTML = `${ICON.copy}Copy text`;
  let line = "", err = false;
  if (f) {
    if (f.step === "waiting") line = "Waiting for the files before it.";
    else if (f.step === "extracting") line = "Extracting audio";
    else if (f.step === "downloading") line = `Downloading ${f.message}. It downloads once; then pktito works offline.`;
    else if (f.step === "transcribing" && !f.segments.length) line = `Transcribing ${f.pct}%`;
    else if (f.step === "done" && !f.segments.length) line = "No speech in this file.";
    else if (f.step === "error") { line = f.message; err = true; }
  }
  note(line, err);
}

function renderMain() {
  renderHead();
  const f = current();
  $("transcript").innerHTML = "";
  if (f) appendSegments(f.segments);
}

function appendSegments(segs) {
  const frag = document.createDocumentFragment();
  for (const s of segs) {
    const row = document.createElement("div");
    row.className = "pk-seg";
    const st = document.createElement("span");
    st.className = "pk-stamp";
    st.textContent = stamp(s.start);
    st.title = "Copy this time";
    st.onclick = () => navigator.clipboard.writeText(st.textContent);
    const p = document.createElement("p");
    p.textContent = s.text;
    row.append(st, p);
    frag.appendChild(row);
  }
  $("transcript").appendChild(frag);
}

function note(text, err = false) {
  $("note").textContent = text || "";
  $("note").classList.toggle("err", err);
}

/* ------------------------------------------------------------- actions -- */

function plainText(f) {
  return f.segments.map((s) => s.text).join("\n") + "\n";
}

function srtText(f) {
  return f.segments.map((s, i) => `${i + 1}\n${srtTime(s.start)} --> ${srtTime(s.end)}\n${s.text}\n`).join("\n");
}

$("copy").onclick = async () => {
  const f = current();
  if (!f || !f.segments.length) return;
  await navigator.clipboard.writeText(plainText(f));
  const b = $("copy");
  b.dataset.copied = "1";
  b.innerHTML = `${ICON.check}Copied`;
  setTimeout(() => { delete b.dataset.copied; renderHead(); }, 1500);
};

async function save(ext) {
  const f = current();
  if (!f || !f.segments.length) return;
  try {
    const path = await invoke("save_text", { name: stem(f.path), ext, text: ext === "srt" ? srtText(f) : plainText(f) });
    if (path) note(`Saved ${baseName(path)}`);
  } catch (e) {
    note(String(e), true);
  }
}
$("save-txt").onclick = () => save("txt");
$("save-srt").onclick = () => save("srt");
$("drop-big").onclick = chooseFiles;
$("add-small").onclick = chooseFiles;

$("queue").addEventListener("click", (e) => {
  const row = e.target.closest(".pk-file");
  if (!row) return;
  selected = Number(row.dataset.id);
  renderQueue();
  renderMain();
  row.focus();
});

$("queue").addEventListener("keydown", (e) => {
  const i = files.findIndex((f) => f.id === selected);
  if (i < 0) return;
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    const j = Math.min(files.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)));
    selected = files[j].id;
    $("queue").classList.add("by-keys");
    e.preventDefault();
  } else if (e.key === "Delete" || e.key === "Backspace") {
    const f = files[i];
    if (["extracting", "downloading", "transcribing"].includes(f.step)) return note("This file is being transcribed. Remove it when it is done.");
    files.splice(i, 1);
    selected = files.length ? files[Math.min(i, files.length - 1)].id : null;
    e.preventDefault();
  } else return;
  render();
  const row = $("queue").querySelector(`[data-id="${selected}"]`);
  if (row) row.focus();
});
$("queue").addEventListener("pointermove", () => $("queue").classList.remove("by-keys"));

/* On Windows the File menu's shortcuts never reach the menu while the page
   has the keyboard, which is nearly always: Ctrl+O and Ctrl+S did nothing
   in the Windows test (2026-10-21). So there the page takes them. */
if (navigator.userAgent.includes("Windows")) {
  document.addEventListener("keydown", (e) => {
    if (!e.ctrlKey || e.altKey || e.metaKey) return;
    const k = e.key.toLowerCase();
    if (k === "o" && !e.shiftKey) chooseFiles();
    else if (k === "s") save(e.shiftKey ? "srt" : "txt");
    else return;
    e.preventDefault();
  });
}

/* -------------------------------------------------------- drag and drop -- */

const DEFAULT_LINE = $("drop-line").textContent, DEFAULT_HINT = $("drop-hint").textContent;
function dragOver(paths) {
  const n = paths ? paths.length : 0;
  for (const id of ["drop-big", "add-small"]) $(id).classList.add("is-over");
  if (n) {
    $("drop-line").textContent = `Release to add ${n} file${n === 1 ? "" : "s"}`;
    $("drop-hint").textContent = paths.map(baseName).join(", ");
    $("add-line").textContent = `Release to add ${n} file${n === 1 ? "" : "s"}`;
  }
}
function dragEnd() {
  for (const id of ["drop-big", "add-small"]) $(id).classList.remove("is-over");
  $("drop-line").textContent = DEFAULT_LINE;
  $("drop-hint").textContent = DEFAULT_HINT;
  $("add-line").textContent = "Add files";
}
const webview = T.webview && T.webview.getCurrentWebview ? T.webview.getCurrentWebview() : null;
if (webview && webview.onDragDropEvent) {
  webview.onDragDropEvent((e) => {
    const p = e.payload;
    if (!status) return;
    if (p.type === "enter") dragOver(p.paths);
    else if (p.type === "drop") { dragEnd(); addPaths(p.paths); }
    else if (p.type === "leave") dragEnd();
  });
}

/* ---------------------------------------------------------------- menu -- */

listen("menu", (e) => {
  if (e.payload === "add") chooseFiles();
  else if (e.payload === "save-txt") save("txt");
  else if (e.payload === "save-srt") save("srt");
});

refresh().catch((e) => {
  document.body.textContent = `pktito could not start: ${e}`;
});

#!/usr/bin/env python3
"""ui/notices.js: what pktito is built from besides its own code and the two
models About names, and the licenses each is used under, for the list that
About's "Everything else" opens.

    python3 scripts/notices.py           # after Cargo.lock changes
    python3 scripts/notices.py --check   # before a release: is ui/notices.js current?

Cargo says which crates each system's build links: the normal dependencies,
walked from the app, so that build scripts, what only they use, and
dev-dependencies stay out. A crate that offers a choice is taken under the
plainest of its licenses, MIT before Apache-2.0 before the rest (the order
of cargo-about's accepted list). Its copyright lines come from its own
license files, else from its authors. The MIT and Apache-2.0 texts are kept
once; any other text a crate carries, at its root or beside code copied into
it, is kept as it is. The typefaces, the icons, Rust's standard library and
the WebView2 loader are listed by hand in EXTRAS, their texts in
scripts/licenses.
"""
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LIC = os.path.join(ROOT, "scripts", "licenses")

# each system the app ships for: its bit in a crate's mask, and its targets
SYSTEMS = {
    "macos": (1, ["aarch64-apple-darwin", "x86_64-apple-darwin"]),
    "windows": (2, ["x86_64-pc-windows-gnu", "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"]),
    "linux": (4, ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]),
}

PREFER = ["MIT", "Apache-2.0", "Zlib", "ISC", "BSD-3-Clause", "BSD-2-Clause", "Unicode-3.0", "BSL-1.0",
          "MPL-2.0", "CDLA-Permissive-2.0", "0BSD", "MIT-0", "Unlicense", "CC0-1.0"]

TITLES = {"MIT": "MIT License", "Apache-2.0": "Apache License 2.0", "MPL-2.0": "Mozilla Public License 2.0",
          "BSD-3-Clause": "BSD 3-Clause License", "BSD-2-Clause": "BSD 2-Clause License", "ISC": "ISC License",
          "Zlib": "zlib License", "Unicode-3.0": "Unicode License v3", "BSL-1.0": "Boost Software License 1.0",
          "CDLA-Permissive-2.0": "Community Data License Agreement, Permissive 2.0"}
# the texts in scripts/licenses that a crate's own may be the same as
CANONICAL = ["MIT", "Apache-2.0", "MPL-2.0", "BSD-3-Clause"]

# license files of code that the build leaves out: libdbus-sys's copy of
# libdbus is built only with its "vendored" feature, which is off
LEFT_OUT = {"libdbus-sys": "vendor/"}

# by hand: [group ("type" or "code"), name, notice, license, systems (0 for
# all), text keys], the texts in scripts/licenses or the app's own files
EXTRAS = [
    ["type", "Archivo", "Copyright 2020 The Archivo Project Authors", "OFL-1.1", 0, ["ui/fonts/Archivo-OFL.txt"]],
    ["type", "Commit Mono", "Copyright (c) 2023 Eigil Nikolajsen", "OFL-1.1", 0, ["ui/fonts/CommitMono-OFL.txt"]],
    # the window's icons are drawn after Feather's
    ["type", "Feather icons", "Copyright (c) 2013-2023 Cole Bemis", "MIT", 0, ["MIT"]],
    ["code", "The Rust standard library", "by the Rust Project Developers", "MIT", 0, ["MIT"]],
    # linked into the app by webview2-com-sys (statically with MSVC, beside
    # it as WebView2Loader.dll with MinGW), from Microsoft's WebView2 SDK
    ["code", "The WebView2 loader", "Copyright (C) Microsoft Corporation", "BSD-3-Clause", 2, ["scripts/licenses/WebView2-SDK.txt"]],
]

FILE = re.compile(r"^(licen[cs]e|copying|copyright|unlicense|notice)", re.I)
# a line that is a copyright notice, not one of a license's sentences about
# them, nor its list item (c)
COPYLINE = re.compile(r"^(copyright\b(?!\s*[,;:]|\s*(notice|owner|holders?|and|license|statement|laws?|protection|assignment)\b)"
                      r"|(\(c\)|©)\s*\d{4})", re.I)
PLACEHOLDER = re.compile(r"\[yyyy\]|\{yyyy\}|<year>|\[name of copyright owner\]|<copyright holders?>|<owner>", re.I)


def read(path):
    with open(path, encoding="utf-8", errors="replace") as fh:
        return fh.read().replace("\r\n", "\n").replace("\r", "\n")


def bare(line):
    return line.strip().strip("*#/>`-").strip()


def words(t):
    """t's words, lowercased, without its copyright lines."""
    kept = "\n".join(l for l in t.splitlines() if not COPYLINE.match(bare(l)))
    # one MPL-2.0 text links mozilla.org by https, another by http
    return [w if w != "https" else "http" for w in re.findall(r"[a-z0-9]+", kept.lower())]


def copyrights(t):
    return [re.sub(r"\s+", " ", bare(l)) for l in t.splitlines() if COPYLINE.match(bare(l)) and not PLACEHOLDER.search(l)]


def split_terms(t):
    """An Apache-2.0 text's terms, and what follows them."""
    i = t.upper().find("END OF TERMS AND CONDITIONS")
    return (t, "") if i < 0 else (t[:i], t[i + len("END OF TERMS AND CONDITIONS"):])


MIT_W = words(read(os.path.join(LIC, "MIT.txt")))
_terms, _appendix = split_terms(read(os.path.join(LIC, "Apache-2.0.txt")))
APACHE_W, APPENDIX_W = words(_terms), words(_appendix)


def mit(t):
    w = words(t)
    i = w.index("permission") if "permission" in w else -1
    return i >= 0 and set(w[:i]) <= {"the", "mit", "license", "expat"} and w[i:] == MIT_W


def apache(t):
    """None if t is not the Apache-2.0 text; else what it adds after the
    license's terms and its appendix: "" for nothing."""
    terms, after = split_terms(t)
    w = words(terms)
    if "apache" not in w or w[w.index("apache"):] != APACHE_W[APACHE_W.index("apache"):]:
        return None
    rest = words(after)
    if rest[:len(APPENDIX_W)] == APPENDIX_W:
        # the appendix, perhaps filled in: past its last words, what is added
        m = re.search(r"limitations\s+under\s+the\s+license\.?", after, re.I)
        return after[m.end():].strip() if m else ""
    return after.strip() if rest else ""


# what a text says it is, by its own words; 0BSD opens as ISC does, so first
KINDS = [
    ("0BSD", r"permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted\.\s+the software is provided"),
    ("ISC", r"permission to use, copy, modify, and/or distribute this software for any purpose with or without fee"),
    ("Zlib", r"provided ['‘]as-is['’], without any express or implied warranty"),
    ("BSD-3-Clause", r"neither the name of .{0,120} may be used to endorse or promote"),
    ("BSD-2-Clause", r"redistribution and use in source and binary forms"),
    ("Unicode-3.0", r"unicode license|unicode, inc\.|unicode data files"),
    ("MPL-2.0", r"mozilla public license"),
    ("CDLA-Permissive-2.0", r"community data license agreement"),
    ("BSL-1.0", r"boost software license"),
    ("Unlicense", r"free and unencumbered software released into the public domain"),
    ("MIT", r"permission is hereby granted, free of charge"),
    ("Apache-2.0", r"apache license"),
]


def both(t):
    """t holds the MIT and the Apache-2.0 texts together, as a crate offering
    either may keep them in one file."""
    w, m = " ".join(words(t)), " ".join(MIT_W)
    return m in w and " ".join(APACHE_W[APACHE_W.index("apache"):]) in w


def kind(t):
    """("MIT" or "Apache-2.0", True) for their standard texts, (an id,
    False) for another license's, (None, False) for a note about them."""
    if mit(t) or both(t):
        return "MIT", True
    extra = apache(t)
    if extra is not None:
        return "Apache-2.0", extra == ""
    # as a text says it, without the comment marks some write it in
    flat = re.sub(r"\s+", " ", " ".join(bare(l) for l in t.lower().splitlines()))
    for k, pat in KINDS:
        if re.search(pat, flat):
            return k, False
    return None, False


def choose(expr):
    """The licenses an SPDX expression has us follow: the plainest side of
    each OR (or the old "/"), every side of an AND."""
    toks = re.findall(r"\(|\)|/|[^\s()/]+", expr)
    pos = 0

    def take():
        nonlocal pos
        pos += 1
        return toks[pos - 1]

    def cost(s):
        return sorted(PREFER.index(x) if x in PREFER else 99 for x in s)

    def any_of():
        alts = [all_of()]
        while pos < len(toks) and toks[pos] in ("OR", "/"):
            take()
            alts.append(all_of())
        return min(alts, key=cost)

    def all_of():
        s = one()
        while pos < len(toks) and toks[pos] == "AND":
            take()
            s = s | one()
        return s

    def one():
        t = take()
        if t == "(":
            s = any_of()
            take()
            return s
        if pos < len(toks) and toks[pos] == "WITH":
            take()
            take()
        return frozenset([t])

    return any_of()


def linked(target):
    """The packages the app links for target, by id."""
    out = subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
                         cwd=os.path.join(ROOT, "src-tauri"), capture_output=True, text=True, check=True).stdout
    m = json.loads(out)
    pk = {p["id"]: p for p in m["packages"]}
    nodes = {n["id"]: n for n in m["resolve"]["nodes"]}
    root = m["resolve"]["root"]
    seen, todo = set(), [root]
    while todo:
        i = todo.pop()
        if i not in seen:
            seen.add(i)
            todo += [d["pkg"] for d in nodes[i]["deps"] if any(k["kind"] is None for k in d["dep_kinds"])]
    seen.discard(root)
    return {i: pk[i] for i in seen}


def license_files(p):
    root = os.path.dirname(p["manifest_path"])
    skip = LEFT_OUT.get(p["name"])
    found = []
    for dp, dn, fn in os.walk(root):
        dn[:] = sorted(d for d in dn if d not in (".git", "target"))
        for x in sorted(fn):
            rel = os.path.relpath(os.path.join(dp, x), root).replace(os.sep, "/")
            if FILE.match(x) and not re.search(r"\.(rs|c|h|toml|json|spdx)$", x, re.I) and not (skip and rel.startswith(skip)):
                found.append((rel, read(os.path.join(dp, x))))
    return found


def authors(p):
    names = [re.sub(r"\s*<[^>]*>", "", a).strip() for a in p.get("authors") or []]
    if any(names):
        return ", ".join(n for n in names if n)
    # no names given: where they are to be found
    home = (p.get("repository") or p.get("homepage") or "").removesuffix(".git")
    return f"the {p['name']} authors" + (f", {home}" if home else "")


def check(data):
    """0 if ui/notices.js holds data, else 1, after naming the crates added,
    removed or changed, and any hand-listed part or text: 0.9.2 shipped
    without the three crates ureq's SOCKS proxy brought in the day after the
    list was last written."""
    path = os.path.join(ROOT, "ui", "notices.js")
    js = read(path)
    old = json.loads(js[js.index("const NOTICES = ") + len("const NOTICES = "):].rstrip().rstrip(";"))
    new = json.loads(json.dumps(data, ensure_ascii=False))
    if old == new:
        print(f"{path}: current, {len(new['crates'])} crates")
        return 0

    def differ(what, a, b):
        for k in sorted(b.keys() - a.keys()):
            print(f"  {what} added: {k}")
        for k in sorted(a.keys() - b.keys()):
            print(f"  {what} removed: {k}")
        for k in sorted(a.keys() & b.keys()):
            if a[k] != b[k]:
                print(f"  {what} changed: {k}")

    differ("crate", {f"{c[0]} {c[1]}": c for c in old["crates"]}, {f"{c[0]} {c[1]}": c for c in new["crates"]})
    differ("part", {e[1]: e for e in old["extras"]}, {e[1]: e for e in new["extras"]})
    differ("text", old["texts"], new["texts"])
    differ("system", old["systems"], new["systems"])
    print(f"{path} is out of date: run python3 scripts/notices.py and commit it")
    return 1


def main():
    crates, texts = {}, {}

    def keep(key, title, text):
        texts.setdefault(key, {"title": title, "text": text.strip() + "\n", "by": []})
        return key

    def sig(text):
        return " ".join(words(text)) + "|" + " ".join(copyrights(text))

    # one key per distinct text, however many crates carry it
    seen_text = {}
    for k in CANONICAL:
        t = read(os.path.join(LIC, k + ".txt"))
        seen_text[sig(t)] = keep(k, TITLES[k], t)

    def keep_own(p, rel, text, lic):
        s = sig(text)
        if s not in seen_text:
            seen_text[s] = keep(f"{p['name']}/{rel}", "", text)
            texts[seen_text[s]]["lic"] = lic
            texts[seen_text[s]]["at"] = rel
        k = seen_text[s]
        if k not in CANONICAL:
            texts[k]["by"].append(p["name"])
        return k

    for bit, targets in SYSTEMS.values():
        for t in targets:
            for i, p in linked(t).items():
                crates.setdefault(i, [p, 0])[1] |= bit

    out = []
    unknown = []
    for p, mask in sorted(crates.values(), key=lambda c: (c[0]["name"].lower(), c[0]["version"])):
        expr = p.get("license") or ""
        chosen = choose(expr) if expr else frozenset()
        files = license_files(p)
        keys, lines = [], []
        for lic in sorted(chosen, key=lambda x: PREFER.index(x) if x in PREFER else 99):
            if lic not in PREFER:
                unknown.append(f"{p['name']} {p['version']}: {lic}")
            got = False
            for rel, t in files:
                if "/" in rel:
                    continue
                k, standard = kind(t)
                if k != lic:
                    continue
                got = True
                if standard:
                    lines += copyrights(split_terms(t)[1] if lic == "Apache-2.0" else t)
                elif lic == "Apache-2.0":
                    # the standard text, and what the crate adds after it,
                    # whose own notices it carries
                    keys.append(keep_own(p, rel, apache(t), "+Apache-2.0"))
                else:
                    keys.append(keep_own(p, rel, t, lic))
            if lic in ("MIT", "Apache-2.0"):
                keys.insert(0, lic)
            elif not got:
                path = os.path.join(LIC, lic + ".txt")
                if not os.path.exists(path):
                    sys.exit(f"{p['name']} {p['version']}: no {lic} text of its own, and none in scripts/licenses")
                keys.append(keep(lic, TITLES.get(lic, lic), read(path)))
        for rel, t in files:
            if "/" not in rel and kind(t)[0] is None:
                lines += copyrights(t)
                # a note a crate of several licenses keeps beside them, as
                # ring's LICENSE says which of its code is under which
                if " AND " in expr:
                    keys.append(keep_own(p, rel, t, ""))
        # code copied into the crate from elsewhere, under its own terms; of
        # MIT and Apache-2.0 offered side by side, MIT
        dirs = {}
        for rel, t in files:
            if "/" in rel:
                dirs.setdefault(rel.rsplit("/", 1)[0], []).append((rel, t))
        for d, fs in dirs.items():
            kinds = [kind(t) for _, t in fs]
            has_mit = ("MIT", True) in kinds
            for (rel, t), (k, standard) in zip(fs, kinds):
                if standard and k == "Apache-2.0" and has_mit:
                    continue
                if standard:
                    lines += copyrights(split_terms(t)[1] if k == "Apache-2.0" else t)
                    keys.append(k)
                else:
                    keys.append(keep_own(p, rel, t, k or ""))
        if not lines:
            # the notices in the crate's own texts, past any it adds to Apache-2.0's
            for k in keys:
                if k in texts and k not in CANONICAL and texts[k].get("lic") != "+Apache-2.0":
                    lines += copyrights(texts[k]["text"])
        lines = list(dict.fromkeys(lines))
        notice = "; ".join(lines) if lines else f"by {authors(p)}"
        if "MPL-2.0" in chosen:
            notice += f". Its source: https://crates.io/crates/{p['name']}/{p['version']}"
        shown = " and ".join(sorted(chosen, key=lambda x: PREFER.index(x) if x in PREFER else 99)) or "unstated"
        out.append([p["name"], p["version"], shown, notice, mask, list(dict.fromkeys(keys))])

    extras = []
    for group, name, notice, lic, mask, ks in EXTRAS:
        kk = []
        for k in ks:
            if k in texts:
                kk.append(k)
            else:
                kk.append(keep(k, f"{name.split(',')[0]}: {os.path.basename(k)}", read(os.path.join(ROOT, k))))
        extras.append([group, name, notice, lic, mask, kk])

    used = {k for c in out + extras for k in c[-1]}
    for k, v in texts.items():
        if v["by"]:
            names = list(dict.fromkeys(v["by"]))
            if v["lic"] == "+Apache-2.0":
                v["title"] = f"{names[0]}: {v['at']}, past the Apache License 2.0 it begins with"
            elif "/" in v.get("at", ""):
                v["title"] = f"{names[0]}: {v['at']}" + (f", {TITLES.get(v['lic'], v['lic'])}" if v["lic"] else "")
            elif v["lic"]:
                v["title"] = f"{TITLES.get(v['lic'], v['lic'])}: {', '.join(names)}"
            else:
                v["title"] = f"{names[0]}: {v['at']}"
        for x in ("by", "lic", "at"):
            v.pop(x, None)
    data = {"systems": {s: b for s, (b, _) in SYSTEMS.items()}, "extras": extras, "crates": out,
            "texts": {k: v for k, v in texts.items() if k in used}}
    if "--check" in sys.argv[1:]:
        sys.exit(check(data))
    js = ("/* What pktito is built from, besides its own code and the models About\n"
          "   names, and the licenses each is used under: from scripts/notices.py. */\n"
          "const NOTICES = " + json.dumps(data, ensure_ascii=False, separators=(",", ":")) + ";\n")
    path = os.path.join(ROOT, "ui", "notices.js")
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(js)
    per = {s: sum(1 for c in out if c[4] & b) for s, (b, _) in SYSTEMS.items()}
    print(f"{path}: {len(js) / 1024:.0f} KB, {len(out)} crates ({', '.join(f'{s} {n}' for s, n in per.items())}), "
          f"{len(data['texts'])} texts")
    for u in unknown:
        print("  a license not in PREFER, check it:", u)


if __name__ == "__main__":
    main()

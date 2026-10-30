#!/usr/bin/env python3
"""Turn downloaded transcripts (PDF or episode web page) into plain text.

usage: python3 -I refs_to_text.py SOURCES_JSON DOWNLOADS_JSON BASE_DIR OUT_DIR
"""
import html
import json
import os
import re
import subprocess
import sys

END_MARKERS = ["</article>", "<footer", "Explore More", "Back to top", "id=\"footer",
               "Subscribe to Podcast", "From corals to coastal science"]


def pdf_text(path):
    return subprocess.run(["pdftotext", "-enc", "UTF-8", path, "-"],
                          capture_output=True, text=True, check=True).stdout


def page_text(path):
    s = open(path, encoding="utf-8", errors="replace").read()
    m = re.search(r">\s*Transcript\s*<", s)
    if not m:
        return None
    s = s[m.end():]
    s = s[s.find(">") + 1:]  # rest of the heading's closing tag
    s = re.sub(r"(?s)<!--.*?-->", "", s)
    cut = min([i for i in (s.find(e) for e in END_MARKERS) if i >= 0] or [len(s)])
    s = s[:cut]
    s = re.sub(r"(?is)<(script|style|figcaption)[^>]*>.*?</\1>", "", s)
    s = re.sub(r"(?i)<br\s*/?>", "\n", s)
    s = re.sub(r"(?i)</(p|h\d|li|div)>", "\n\n", s)
    s = re.sub(r"<[^>]+>", "", s)
    s = html.unescape(s).replace("\xa0", " ")
    lines = [re.sub(r"[ \t]+", " ", ln).strip() for ln in s.splitlines()]
    text = re.sub(r"\n{3,}", "\n\n", "\n".join(lines)).strip()
    # the page source wraps lines inside paragraphs; rejoin each paragraph
    paras = [" ".join(p.split("\n")) for p in text.split("\n\n")]
    return "\n\n".join(p for p in paras if p) + "\n"


def main():
    sources_path, downloads_path, base, out = sys.argv[1:5]
    sources = {s["id"]: s for s in json.load(open(sources_path))}
    os.makedirs(os.path.join(out, "refs"), exist_ok=True)
    for d in json.load(open(downloads_path))["downloads"]:
        if not d.get("transcript_path"):
            continue
        src = sources[d["id"]]
        path = os.path.join(base, d["transcript_path"])
        text = pdf_text(path) if path.lower().endswith(".pdf") else page_text(path)
        if not text or len(text) < 500:
            print(f"WARN {d['id']}: no transcript text found")
            continue
        head = (f"# Reference transcript: {src['title']}\n"
                f"# Source: {src['transcript']}\n"
                f"# Covers the whole recording ({d['duration_s'] / 60:.0f} min), not just the clip(s); "
                f"see manifest.json for clip offsets. Text as published, not aligned to audio.\n\n")
        with open(os.path.join(out, "refs", d["id"] + ".txt"), "w", encoding="utf-8") as f:
            f.write(head + text)
        print(f"ok   {d['id']:28s} {len(text.split()):6d} words")


if __name__ == "__main__":
    main()

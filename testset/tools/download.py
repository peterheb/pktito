#!/usr/bin/env python3
"""Download every source (and its transcript) listed in sources.json.

Each download goes into its own directory: raw/<id>/ for audio, refs/<id>/ for
transcripts. Writes downloads.json with size, SHA-256 and duration per source.

usage: python3 -I download.py SOURCES_JSON OUT_DIR
"""
import concurrent.futures as cf
import hashlib
import json
import os
import subprocess
import sys
import urllib.parse

UA = "Mozilla/5.0 (pktito-testset; +https://pktito.net)"


def fname_for(url, fallback):
    name = os.path.basename(urllib.parse.urlparse(url).path.rstrip("/"))
    if not name or "." not in name:
        name = fallback
    return name


def curl(url, dest):
    tmp = dest + ".part"
    cmd = ["curl", "-sS", "-L", "--fail", "--retry", "4", "--retry-delay", "3",
           "--max-time", "900", "-A", UA, "-o", tmp, url]
    subprocess.run(cmd, check=True)
    os.replace(tmp, dest)


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def probe_duration(path):
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration",
         "-of", "default=nw=1:nk=1", path],
        capture_output=True, text=True, check=True).stdout.strip()
    return float(out)


def fetch(src, out):
    rec = {"id": src["id"]}
    adir = os.path.join(out, "raw", src["id"])
    os.makedirs(adir, exist_ok=True)
    apath = os.path.join(adir, fname_for(src["url"], "audio.mp3"))
    if not os.path.exists(apath):
        curl(src["url"], apath)
    rec["audio_path"] = os.path.relpath(apath, out)
    rec["audio_bytes"] = os.path.getsize(apath)
    rec["audio_sha256"] = sha256(apath)
    rec["duration_s"] = round(probe_duration(apath), 3)
    if src.get("transcript"):
        tdir = os.path.join(out, "refs", src["id"])
        os.makedirs(tdir, exist_ok=True)
        tpath = os.path.join(tdir, fname_for(src["transcript"], "page.html"))
        if not os.path.exists(tpath):
            try:
                curl(src["transcript"], tpath)
            except subprocess.CalledProcessError as e:
                rec["transcript_error"] = str(e)
                tpath = None
        if tpath:
            rec["transcript_path"] = os.path.relpath(tpath, out)
    return rec


def main():
    sources = json.load(open(sys.argv[1]))
    out = sys.argv[2]
    results, errors = {}, {}
    with cf.ThreadPoolExecutor(max_workers=4) as ex:
        futs = {ex.submit(fetch, s, out): s["id"] for s in sources}
        for fut in cf.as_completed(futs):
            sid = futs[fut]
            try:
                r = fut.result()
                results[sid] = r
                print(f"ok   {sid:28s} {r['duration_s']:8.1f}s {r['audio_bytes']/1e6:7.1f} MB"
                      + ("  transcript ERR" if 'transcript_error' in r else ""), flush=True)
            except Exception as e:  # report and keep going
                errors[sid] = str(e)
                print(f"FAIL {sid}: {e}", flush=True)
    ordered = [results[s["id"]] for s in sources if s["id"] in results]
    with open(os.path.join(out, "downloads.json"), "w") as f:
        json.dump({"downloads": ordered, "errors": errors}, f, indent=2)
    print(f"{len(ordered)} downloaded, {len(errors)} failed")


if __name__ == "__main__":
    main()

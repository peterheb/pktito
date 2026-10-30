#!/usr/bin/env python3
"""Rebuild the pktito speech test set from manifest.json.

Downloads each source recording from its original public URL (1.06 GB in
all), checks its SHA-256, and cuts every clip with the same ffmpeg command
the set was built with. Needs Python 3.8+ and ffmpeg on the PATH; nothing else.

    python3 testset/fetch.py                      # into testset/clips, downloads in testset/sources
    python3 testset/fetch.py --out ~/pktito-test  # somewhere else
    python3 testset/fetch.py --only court-qa interview_cdc-eid-cre_1

--only takes styles (court-qa, press-qa, interview, monologue, historical,
scripted), source ids or clip ids. Each clip's decoded PCM is compared with
the manifest's pcm_sha256: "same samples" means pktito will see exactly the
audio the set was built with; "different samples" usually means a different
ffmpeg version decoded or resampled the source slightly differently.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

UA = "Mozilla/5.0 (pktito-testset fetch.py)"


def ffmpeg_cut_args(src_path, start, end, out_path, tags):
    args = ["ffmpeg", "-v", "error", "-nostdin", "-y", "-i", str(src_path),
            "-ss", f"{start:.3f}", "-to", f"{end:.3f}",
            "-map", "0:a:0", "-map_metadata", "-1",
            "-ac", "1", "-ar", "16000", "-sample_fmt", "s16",
            "-c:a", "flac", "-compression_level", "8"]
    for k, v in tags.items():
        args += ["-metadata", f"{k}={v}"]
    return args + [str(out_path)]


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def pcm_sha256(path):
    raw = subprocess.run(["ffmpeg", "-v", "error", "-nostdin", "-i", str(path), "-f", "s16le", "-"],
                         capture_output=True, check=True).stdout
    return hashlib.sha256(raw).hexdigest(), len(raw) // 2


def download(url, dest, size_hint=None, tries=4):
    dest.parent.mkdir(parents=True, exist_ok=True)
    part = dest.with_name(dest.name + ".part")
    for attempt in range(1, tries + 1):
        try:
            have = part.stat().st_size if part.exists() else 0
            req = urllib.request.Request(url, headers={"User-Agent": UA})
            if have:
                req.add_header("Range", f"bytes={have}-")
            with urllib.request.urlopen(req, timeout=60) as r:
                mode = "ab" if have and r.status == 206 else "wb"
                with open(part, mode) as f:
                    shutil.copyfileobj(r, f, 1 << 20)
            part.replace(dest)
            return
        except Exception as e:  # network hiccup: wait and resume
            if attempt == tries:
                raise
            wait = 2 ** attempt
            print(f"    retry in {wait}s ({e})", flush=True)
            time.sleep(wait)


def selected(clip, only):
    if not only:
        return True
    return bool({clip["style"], clip["source_id"], clip["clip_id"]} & only)


def main():
    here = Path(__file__).resolve().parent
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--manifest", default=str(here / "manifest.json"))
    ap.add_argument("--out", default=str(here))
    ap.add_argument("--only", nargs="*", default=[], help="styles, source ids or clip ids")
    ap.add_argument("--sources", default=None, help="where to keep downloaded recordings (default OUT/sources)")
    args = ap.parse_args()

    if not shutil.which("ffmpeg"):
        sys.exit("ffmpeg is not on the PATH")
    manifest = json.load(open(args.manifest, encoding="utf-8"))
    out = Path(args.out).expanduser()
    src_dir = Path(args.sources).expanduser() if args.sources else out / "sources"
    only = set(args.only)
    clips = [c for c in manifest["clips"] if selected(c, only)]
    if not clips:
        sys.exit("nothing matches --only")

    # one download per source recording
    paths, problems = {}, []
    for sid in dict.fromkeys(c["source_id"] for c in clips):
        c = next(c for c in clips if c["source_id"] == sid)
        dest = src_dir / sid / c["source_file"]
        if dest.exists() and dest.stat().st_size == c["source_bytes"]:
            print(f"have {sid}")
        else:
            print(f"get  {sid}  ({c['source_bytes'] / 1e6:.0f} MB)  {c['source_url']}", flush=True)
            download(c["source_url"], dest)
        digest = sha256_file(dest)
        if digest != c["source_sha256"]:
            problems.append(f"{sid}: source file changed since the set was built (SHA-256 differs); "
                            f"check its clips' timing")
        paths[sid] = dest

    same = diff = 0
    for c in clips:
        target = out / c["file"]
        target.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(ffmpeg_cut_args(paths[c["source_id"]], c["start_s"], c["end_s"], target,
                                       c["flac_tags"]), check=True)
        digest, nsamp = pcm_sha256(target)
        if digest == c["pcm_sha256"]:
            same += 1
            status = "same samples"
        else:
            diff += 1
            status = "different samples"
            if abs(nsamp - c["samples"]) > 160:
                problems.append(f"{c['clip_id']}: {nsamp / 16000:.2f}s, expected {c['duration_s']:.2f}s")
        print(f"cut  {c['clip_id']:44s} {nsamp / 16000:7.2f}s  {status}", flush=True)

    print(f"\n{len(clips)} clips in {out / 'clips'}: {same} same samples, {diff} different")
    for p in problems:
        print("WARNING:", p)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())

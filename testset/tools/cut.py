#!/usr/bin/env python3
"""Cut the selected windows into 16 kHz mono FLAC clips and write the manifest.

usage: python3 -I cut.py SOURCES_JSON DOWNLOADS_JSON SELECTION_JSON BASE_DIR OUT_DIR
"""
import csv
import hashlib
import json
import os
import subprocess
import sys

FIELDS = ["clip_id", "file", "style", "language", "duration_s", "family", "title", "recorded",
          "domain", "audio", "speakers", "seen_risk", "start_s", "end_s", "source_url",
          "source_page", "transcript_url", "rights", "attribution", "speech_ratio",
          "longest_gap_s", "pauses_per_min", "notes"]


def ffmpeg_cut_args(src_path, start, end, out_path, tags):
    """The one place the clip encoding is defined (fetch.py repeats it verbatim)."""
    args = ["ffmpeg", "-v", "error", "-nostdin", "-y", "-i", src_path,
            "-ss", f"{start:.3f}", "-to", f"{end:.3f}",
            "-map", "0:a:0", "-map_metadata", "-1",
            "-ac", "1", "-ar", "16000", "-sample_fmt", "s16",
            "-c:a", "flac", "-compression_level", "8"]
    for k, v in tags.items():
        args += ["-metadata", f"{k}={v}"]
    return args + [out_path]


def pcm_sha256(path):
    raw = subprocess.run(["ffmpeg", "-v", "error", "-nostdin", "-i", path, "-f", "s16le", "-"],
                         capture_output=True, check=True).stdout
    return hashlib.sha256(raw).hexdigest(), len(raw) // 2


def main():
    sources_path, downloads_path, selection_path, base, out = sys.argv[1:6]
    sources = json.load(open(sources_path))
    dl = {d["id"]: d for d in json.load(open(downloads_path))["downloads"]}
    sel = json.load(open(selection_path))
    os.makedirs(os.path.join(out, "clips"), exist_ok=True)
    clips = []
    for s in sources:
        d = dl[s["id"]]
        for k, c in enumerate(sel[s["id"]]["clips"], 1):
            clip_id = f"{s['style']}_{s['id']}_{k}"
            rel = f"clips/{clip_id}.flac"
            tags = {
                "title": f"{s['title']} [{c['start_s']:.3f}-{c['end_s']:.3f}s]",
                "artist": s["publisher"],
                "date": s["recorded"],
                "comment": f"Excerpt of {s['url']} | {s['rights']}",
            }
            subprocess.run(ffmpeg_cut_args(os.path.join(base, d["audio_path"]), c["start_s"],
                                           c["end_s"], os.path.join(out, rel), tags), check=True)
            h, nsamp = pcm_sha256(os.path.join(out, rel))
            transcript_file = None
            if d.get("transcript_path"):
                transcript_file = "refs/" + s["id"] + ".txt"
            clips.append({
                "clip_id": clip_id, "file": rel, "style": s["style"], "language": s["language"],
                "duration_s": round(nsamp / 16000, 3), "samples": nsamp,
                "family": s["family"], "source_id": s["id"], "title": s["title"],
                "publisher": s["publisher"], "recorded": s["recorded"], "domain": s["domain"],
                "audio": s["audio"], "speakers": s["speakers"], "seen_risk": s["seen_risk"],
                "start_s": c["start_s"], "end_s": c["end_s"],
                "source_url": s["url"], "source_page": s["page"],
                "source_file": os.path.basename(d["audio_path"]),
                "source_sha256": d["audio_sha256"], "source_bytes": d["audio_bytes"],
                "source_duration_s": d["duration_s"],
                "transcript_url": s.get("transcript"), "transcript_file": transcript_file,
                "rights": s["rights"], "attribution": s["attribution"],
                "speech_ratio": c["speech_ratio"], "longest_gap_s": c["longest_gap_s"],
                "pauses_per_min": round(c["pauses_per_min"], 1),
                "pcm_sha256": h, "flac_tags": tags, "notes": s.get("notes", ""),
            })
            print(f"cut {clip_id:44s} {nsamp / 16000:7.2f}s", flush=True)
    manifest = {
        "name": "pktito varied speech test set",
        "version": 1,
        "created": "2026-10-29",
        "audio_format": "FLAC, 16 kHz, mono, 16-bit",
        "clip_count": len(clips),
        "total_duration_s": round(sum(c["duration_s"] for c in clips), 1),
        "encoding": "ffmpeg -i SOURCE -ss START -to END -map 0:a:0 -map_metadata -1 -ac 1 -ar 16000 "
                    "-sample_fmt s16 -c:a flac -compression_level 8 (plus title/artist/date/comment tags)",
        "clips": clips,
    }
    with open(os.path.join(out, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2, ensure_ascii=False)
    with open(os.path.join(out, "manifest.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=FIELDS, extrasaction="ignore")
        w.writeheader()
        for c in clips:
            w.writerow(c)
    print(f"{len(clips)} clips, {manifest['total_duration_s'] / 60:.1f} min")


if __name__ == "__main__":
    main()

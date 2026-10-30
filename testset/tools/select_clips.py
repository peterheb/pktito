#!/usr/bin/env python3
"""Pick clip windows in each source: spread across the episode, edges on pauses.

Runs Silero VAD over every source (cached), splits each source's usable span
(after skipping intros/outros) into N equal parts, and in each part picks the
window that starts and ends in a pause, is close to the target length, and has
the most speech and no long gaps (music beds and dead air show up as gaps).

usage: python3 -I select_clips.py SOURCES_JSON DOWNLOADS_JSON BASE_DIR VAD_ONNX CACHE_DIR OUT_JSON [ID ...]
"""
import concurrent.futures as cf
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np  # noqa: E402
import vadlib  # noqa: E402

EDGE_PAD = 0.35   # max silence kept before the first / after the last word
SNAP = 30.0       # how far the end may move from start + target length


def analyze(args):
    sid, path, model, cache = args
    npy = os.path.join(cache, sid + ".probs.npy")
    dbf = os.path.join(cache, sid + ".db.npy")
    if os.path.exists(npy) and os.path.exists(dbf):
        return sid, (np.load(npy), np.load(dbf))
    x = vadlib.decode_16k(path)
    p = np.load(npy) if os.path.exists(npy) else vadlib.Silero(model).probs(x)
    db = vadlib.frame_db(x)[:len(p)]
    np.save(npy, p)
    np.save(dbf, db)
    return sid, (p, db)


def choose(src, dur, pdb):
    p, db = pdb
    mask = vadlib.speech_mask(p, src.get("vad_on", 0.5), src.get("vad_off", 0.35))
    starts = src.get("starts_s") or []
    ps = vadlib.pauses(mask, 0.3)
    lo = src["skip_head_s"]
    hi = dur - src["skip_tail_s"]
    target = src["clip_s"]
    n = src["clips"]
    part = (hi - lo) / n
    picks = []
    for k in range(n):
        a, b = lo + k * part, lo + (k + 1) * part
        best = None
        for (p0, p1) in ps:
            plen = p1 - p0
            want = starts[k] if k < len(starts) else None
            if want is not None:
                if abs(p1 - want) > 20.0 and not (want <= 0.5 and p0 <= 0.5):
                    continue
            elif not (a <= p1 <= b):
                continue
            s = max(p0, p1 - EDGE_PAD)
            if s + target - SNAP > hi:
                continue
            best_end = None
            for (q0, q1) in ps:
                if q0 < s + target - SNAP or q0 > s + target + SNAP:
                    continue
                e = min(q1, q0 + EDGE_PAD)
                if e > hi:
                    continue
                qlen = q1 - q0
                cost = abs(e - (s + target)) / 10.0 - min(qlen, 1.5)
                if best_end is None or cost < best_end[0]:
                    best_end = (cost, e, qlen)
            if best_end is None:
                continue
            _, e, qlen = best_end
            st = vadlib.window_stats(mask, s, e, db)
            score = (st["speech_ratio"]
                     - 0.04 * max(0.0, st["longest_gap_s"] - 3.0)
                     - 0.5 * abs((e - s) - target) / target
                     - 0.02 * max(0.0, st["floor_db"] + 22.0)
                     + 0.03 * min(plen, 1.5) + 0.03 * min(qlen, 1.5))
            if want is not None:  # a start asked for: the pause nearest it wins
                score = -abs(s - want)
            if best is None or score > best["score"]:
                best = {"start_s": round(s, 3), "end_s": round(e, 3),
                        "duration_s": round(e - s, 3), "score": round(score, 4),
                        "start_pause_s": round(plen, 2), "end_pause_s": round(qlen, 2),
                        **{k2: round(v, 3) for k2, v in st.items()}}
        if best is None:  # no pause-aligned window: fall back to raw span
            s, e = a, min(a + target, hi)
            best = {"start_s": round(s, 3), "end_s": round(e, 3), "duration_s": round(e - s, 3),
                    "score": None, "start_pause_s": 0, "end_pause_s": 0,
                    **{k2: round(v, 3) for k2, v in vadlib.window_stats(mask, s, e, db).items()},
                    "note": "fallback window, not pause-aligned"}
        picks.append(best)
    return picks, float(mask.mean())


def main():
    sources_path, downloads_path, base, model, cache, out = sys.argv[1:7]
    only = set(sys.argv[7:])
    os.makedirs(cache, exist_ok=True)
    sources = [s for s in json.load(open(sources_path)) if not only or s["id"] in only]
    dl = {d["id"]: d for d in json.load(open(downloads_path))["downloads"]}
    jobs = [(s["id"], os.path.join(base, dl[s["id"]]["audio_path"]), model, cache) for s in sources]
    probs = {}
    with cf.ProcessPoolExecutor(max_workers=2) as ex:
        for sid, p in ex.map(analyze, jobs):
            probs[sid] = p
            print(f"vad  {sid:28s} {len(p) * vadlib.FRAME_S:8.1f}s", flush=True)
    result = json.load(open(out)) if os.path.exists(out) else {}
    for s in sources:
        picks, overall = choose(s, dl[s["id"]]["duration_s"], probs[s["id"]])
        result[s["id"]] = {"overall_speech_ratio": round(overall, 3), "clips": picks}
        for c in picks:
            print(f"pick {s['id']:28s} {c['start_s']:8.1f}-{c['end_s']:8.1f} ({c['duration_s']:5.1f}s) "
                  f"speech {c['speech_ratio']:.2f} gap {c['longest_gap_s']:4.1f}s "
                  f"pauses/min {c['pauses_per_min']:4.1f} floor {c['floor_db']:6.1f}dB")
    json.dump(result, open(out, "w"), indent=2)


if __name__ == "__main__":
    main()

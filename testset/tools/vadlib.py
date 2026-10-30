"""Silero VAD (v5 ONNX) in plain numpy, plus pause/segment helpers.

Mirrors the official OnnxWrapper: 512-sample chunks at 16 kHz, each prefixed
with the previous chunk's last 64 samples, recurrent state carried across.
"""
import subprocess

import numpy as np

SR = 16000
CHUNK = 512
CTX = 64
FRAME_S = CHUNK / SR  # 0.032 s


def decode_16k(path, start=None, end=None):
    """Decode any audio file to 16 kHz mono float32 with ffmpeg."""
    cmd = ["ffmpeg", "-v", "error", "-nostdin", "-i", path]
    if start is not None:
        cmd += ["-ss", f"{start:.3f}"]
    if end is not None:
        cmd += ["-to", f"{end:.3f}"]
    cmd += ["-ac", "1", "-ar", str(SR), "-f", "f32le", "-"]
    raw = subprocess.run(cmd, capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.float32)


class Silero:
    def __init__(self, model_path, threads=1):
        import onnxruntime as ort
        opts = ort.SessionOptions()
        opts.intra_op_num_threads = threads
        opts.inter_op_num_threads = 1
        self.sess = ort.InferenceSession(model_path, sess_options=opts,
                                         providers=["CPUExecutionProvider"])

    def probs(self, x):
        n = len(x) // CHUNK
        state = np.zeros((2, 1, 128), dtype=np.float32)
        ctx = np.zeros((1, CTX), dtype=np.float32)
        sr = np.array(SR, dtype=np.int64)
        out = np.empty(n, dtype=np.float32)
        run = self.sess.run
        for i in range(n):
            chunk = x[i * CHUNK:(i + 1) * CHUNK].reshape(1, CHUNK)
            inp = np.concatenate([ctx, chunk], axis=1)
            p, state = run(None, {"input": inp, "state": state, "sr": sr})
            out[i] = p[0, 0]
            ctx = inp[:, -CTX:]
        return out


def speech_mask(p, on=0.5, off=0.35):
    """Hysteresis: speech starts at p >= on and ends when p < off."""
    m = np.zeros(len(p), dtype=bool)
    state = False
    for i, v in enumerate(p):
        if state:
            if v < off:
                state = False
        elif v >= on:
            state = True
        m[i] = state
    return m


def runs(mask, value):
    """Return (start_frame, end_frame) runs where mask == value (end exclusive)."""
    d = np.diff(np.concatenate([[False], mask == value, [False]]).astype(np.int8))
    starts = np.flatnonzero(d == 1)
    ends = np.flatnonzero(d == -1)
    return list(zip(starts, ends))


def pauses(mask, min_s=0.3):
    """Non-speech runs at least min_s long, as (start_s, end_s)."""
    min_f = int(round(min_s / FRAME_S))
    return [(a * FRAME_S, b * FRAME_S) for a, b in runs(mask, False) if b - a >= min_f]


def frame_db(x):
    """RMS level per VAD frame, dBFS."""
    n = len(x) // CHUNK
    fr = x[:n * CHUNK].reshape(n, CHUNK)
    return (20 * np.log10(np.sqrt((fr.astype(np.float64) ** 2).mean(1)) + 1e-9)).astype(np.float32)


def floor_db(mask, db):
    """Level of the gaps relative to the speech: near -40 dB for clean speech,
    -20 dB or higher when music or loud noise keeps playing under the pauses."""
    if mask.all() or not mask.any():
        return 0.0
    return float(np.median(db[~mask]) - np.median(db[mask]))


def window_stats(mask, s, e, db=None):
    a, b = int(s / FRAME_S), int(e / FRAME_S)
    w = mask[a:b]
    if len(w) == 0:
        return {"speech_ratio": 0.0, "longest_gap_s": e - s, "pauses_per_min": 0.0, "floor_db": 0.0}
    gaps = [(y - x) * FRAME_S for x, y in runs(w, False)]
    n_p = sum(1 for g in gaps if g >= 0.3)
    return {
        "speech_ratio": float(w.mean()),
        "longest_gap_s": float(max(gaps) if gaps else 0.0),
        "pauses_per_min": n_p / ((e - s) / 60.0),
        "floor_db": floor_db(w, db[a:b]) if db is not None else 0.0,
    }

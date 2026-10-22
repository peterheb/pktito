#!/usr/bin/env python3
"""
ui/mascot-model.js, the voxel model of the mascot on the About view: a budgie
("little parakeet") in headphones, with a desk microphone, a speech bubble
whose waveform ui/mascot.js animates, and a transcript card.

    python3 scripts/mascot.py              # needs numpy
    python3 scripts/mascot.py --no-headphones

The model is built from signed-distance shapes sampled on a 1-unit grid, painted
with a hand-picked palette (ramps with ordered dithering, as a voxel artist
would by hand), and written as runs of palette indices.

Colours were sampled from the reference illustrations. Everything is in "voxel
units": 1 unit = 1 voxel, y is up, +z points at the camera in the default pose,
the ground plane is y = 0.
"""
import argparse
import json
import math
import os
import sys

import numpy as np

# --------------------------------------------------------------------------- grid
ORIGIN = np.array([-76.0, -4.0, -74.0])  # world coords of the min corner of cell (0,0,0)
DIMS = (176, 140, 124)


def region(lo, hi):
    lo = np.asarray(lo, float)
    hi = np.asarray(hi, float)
    i0 = np.clip(np.floor(lo - ORIGIN).astype(int), 0, None)
    i1 = np.minimum(np.ceil(hi - ORIGIN).astype(int), DIMS)
    sl = tuple(slice(int(a), int(b)) for a, b in zip(i0, i1))
    xs = ORIGIN[0] + np.arange(i0[0], i1[0]) + 0.5
    ys = ORIGIN[1] + np.arange(i0[1], i1[1]) + 0.5
    zs = ORIGIN[2] + np.arange(i0[2], i1[2]) + 0.5
    X, Y, Z = np.meshgrid(xs, ys, zs, indexing="ij")
    return sl, X, Y, Z


class Chunk:
    """A voxel grid of palette indices (0 = empty)."""

    def __init__(self):
        self.g = np.zeros(DIMS, np.uint8)

    def add(self, lo, hi, inside, color):
        sl, X, Y, Z = region(lo, hi)
        m = inside(X, Y, Z)
        if not m.any():
            return
        c = color(X[m], Y[m], Z[m])
        sub = self.g[sl]
        sub[m] = c

    def carve(self, lo, hi, inside):
        sl, X, Y, Z = region(lo, hi)
        m = inside(X, Y, Z)
        sub = self.g[sl]
        sub[m] = 0

    def recolor(self, lo, hi, fn):
        """fn(X, Y, Z, current) -> new colour array (same shape); 0 keeps empty cells empty."""
        sl, X, Y, Z = region(lo, hi)
        sub = self.g[sl]
        new = fn(X, Y, Z, sub)
        sub[:] = np.where(sub > 0, new, 0).astype(np.uint8)


# ------------------------------------------------------------------------ palette
class Palette:
    def __init__(self):
        self.cols = [(0, 0, 0)]
        self.map = {}

    def c(self, hexstr):
        h = hexstr.lstrip("#").upper()
        if h not in self.map:
            self.cols.append(tuple(int(h[i:i + 2], 16) for i in (0, 2, 4)))
            self.map[h] = len(self.cols) - 1
        return self.map[h]

    def ramp(self, *hexes):
        return np.array([self.c(h) for h in hexes], dtype=np.uint8)


PAL = Palette()


def hash01(X, Y, Z, seed=0):
    xi = np.floor(X).astype(np.int64).astype(np.uint64)
    yi = np.floor(Y).astype(np.int64).astype(np.uint64)
    zi = np.floor(Z).astype(np.int64).astype(np.uint64)
    with np.errstate(over="ignore"):
        h = xi * np.uint64(374761393) + yi * np.uint64(668265263) + zi * np.uint64(2147483647)
        h = h + np.uint64(seed * 1013904223 + 12345)
        h = (h ^ (h >> np.uint64(13))) * np.uint64(1274126177)
        h = h ^ (h >> np.uint64(16))
    return (h & np.uint64(0xFFFF)).astype(np.float64) / 65535.0


def pick(ramp, t, X=None, Y=None, Z=None, seed=0, dither=0.8):
    """Pick from a colour ramp with t in 0..1, optionally ordered-dithered by cell position."""
    n = len(ramp)
    f = np.clip(t, 0, 1) * (n - 1)
    if X is not None:
        f = f + (hash01(X, Y, Z, seed) - 0.5) * dither
    return ramp[np.clip(np.rint(f).astype(int), 0, n - 1)]


# ------------------------------------------------------------------ shape helpers
def sd_ell(x, y, z, c, r):
    k = np.sqrt(((x - c[0]) / r[0]) ** 2 + ((y - c[1]) / r[1]) ** 2 + ((z - c[2]) / r[2]) ** 2)
    return (k - 1.0) * min(r)


def smin(a, b, k):
    h = np.clip(0.5 + 0.5 * (b - a) / k, 0.0, 1.0)
    return b * (1 - h) + a * h - k * h * (1 - h)


def sd_capsule(x, y, z, a, b, r):
    a = np.asarray(a, float)
    b = np.asarray(b, float)
    ab = b - a
    t = ((x - a[0]) * ab[0] + (y - a[1]) * ab[1] + (z - a[2]) * ab[2]) / np.dot(ab, ab)
    t = np.clip(t, 0, 1)
    return np.sqrt((x - a[0] - t * ab[0]) ** 2 + (y - a[1] - t * ab[1]) ** 2 + (z - a[2] - t * ab[2]) ** 2) - r


def sd_rrect(x, y, cx, cy, hw, hh, r):
    qx = np.abs(x - cx) - hw + r
    qy = np.abs(y - cy) - hh + r
    return np.minimum(np.maximum(qx, qy), 0) + np.hypot(np.maximum(qx, 0), np.maximum(qy, 0)) - r


def in_tri(x, y, A, B, C):
    def s(p, q):
        return (x - q[0]) * (p[1] - q[1]) - (p[0] - q[0]) * (y - q[1])

    d1, d2, d3 = s(A, B), s(B, C), s(C, A)
    neg = (d1 < 0) | (d2 < 0) | (d3 < 0)
    pos = (d1 > 0) | (d2 > 0) | (d3 > 0)
    return ~(neg & pos)


def norm(v):
    v = np.asarray(v, float)
    return v / np.linalg.norm(v)


# --------------------------------------------------------------------- the budgie
YAW = math.radians(32)            # the bird faces toward screen-right in the default pose
HC = np.array([0.5, 88.0, 3.0])   # head centre (bird-local)
HEAD_R = (28.0, 22.5, 25.0)
HP, HR = math.radians(9), math.radians(-5)  # head pitch up / roll
_f = np.array([0, math.sin(HP), math.cos(HP)])
_u = np.array([0, math.cos(HP), -math.sin(HP)])
_r = np.array([1.0, 0, 0])
HR_AX = _r * math.cos(HR) + _u * math.sin(HR)
HU_AX = _u * math.cos(HR) - _r * math.sin(HR)
HF_AX = _f


def to_local(X, Y, Z):
    c, s = math.cos(YAW), math.sin(YAW)
    return c * X - s * Z, Y, s * X + c * Z


def head_q(xl, yl, zl):
    dx, dy, dz = xl - HC[0], yl - HC[1], zl - HC[2]
    qx = dx * HR_AX[0] + dy * HR_AX[1] + dz * HR_AX[2]
    qy = dx * HU_AX[0] + dy * HU_AX[1] + dz * HU_AX[2]
    qz = dx * HF_AX[0] + dy * HF_AX[1] + dz * HF_AX[2]
    return qx, qy, qz


def hq(X, Y, Z):
    return head_q(*to_local(X, Y, Z))


BIRD_LO = (-50, 0, -62)
BIRD_HI = (50, 118, 54)

# ramps -------------------------------------------------------------------------
RAMP_HEAD = PAL.ramp("#F3DF55", "#F9E964", "#FEF27C", "#FEF68E", "#FDF7A4")
RAMP_BODY = PAL.ramp("#86B943", "#97C84B", "#AAD24F", "#BEDD55", "#D0E75D", "#E0EE6C")
RAMP_BEAK = PAL.ramp("#EE9A1E", "#F5A624", "#FDB033", "#FFBE52")
RAMP_BEAK_LOW = PAL.ramp("#E48C16", "#F09A20", "#F7A62A")
RAMP_CHEEK = PAL.ramp("#2A5CB5", "#326CC1", "#3872C6", "#4A85D6")
RAMP_EYE = PAL.ramp("#120C0C", "#1F1514", "#2C1F1F", "#3A2A27")
RAMP_COVERT = PAL.ramp("#EBE456", "#F2EC5A", "#F6F178", "#FAF59A")
RAMP_COVERT_EDGE = PAL.ramp("#2F7D44", "#3F8F3A", "#55A23E", "#2E8B6A")
RAMP_FLIGHT = PAL.ramp("#0A4F9E", "#0C63BF", "#117EBB", "#2A9DB8", "#3DB1B6")
RAMP_FLIGHT_DARK = PAL.ramp("#08418A", "#0A4F9E")
RAMP_TAIL = PAL.ramp("#0B5CA8", "#117EBB", "#1F93C4", "#2FA6B8")
RAMP_FAR_WING = PAL.ramp("#C9E24E", "#8CCB4A", "#3FA39A", "#2A7AAC", "#1F66B5", "#144F9A")
RAMP_FEET = PAL.ramp("#E6806A", "#EE9178", "#F4A890", "#F8BCA6")
C_WHITE = PAL.c("#FFFFFF")
C_RING = PAL.c("#FFFBEA")
C_SPOT = PAL.c("#1D3B66")
C_BROW = PAL.c("#5E4B28")
C_STRIPE = PAL.c("#5B5326")
C_CERE = PAL.c("#5B8FD9")
C_CERE_D = PAL.c("#3F6FBB")
C_NOSTRIL = PAL.c("#2D4F8F")
C_MOUTH = PAL.c("#F0786A")
C_MOUTH_D = PAL.c("#C9514E")
C_TONGUE = PAL.c("#F58A7B")
C_HL2 = PAL.c("#F4E8E8")


def sp(az, el, R=HEAD_R):
    a, e = math.radians(az), math.radians(el)
    return np.array([R[0] * math.sin(a) * math.cos(e), R[1] * math.sin(e), R[2] * math.cos(a) * math.cos(e)])


def tangent(p, R=HEAD_R):
    n = norm(np.array([p[0] / R[0] ** 2, p[1] / R[1] ** 2, p[2] / R[2] ** 2]))
    u = norm(np.cross([0, 1, 0], n))
    v = np.cross(n, u)
    return n, u, v


def sd_body(xl, yl, zl):
    body = sd_ell(xl, yl, zl, (0, 38.5, -3), (29, 35, 27))
    chest = sd_ell(xl, yl, zl, (0, 62, 12), (19, 15, 15))
    return smin(body, chest, 6.0)


def build_bird(ch, headphones=True):
    # ---- torso + head (smooth union) ----
    def torso_inside(X, Y, Z):
        xl, yl, zl = to_local(X, Y, Z)
        qx, qy, qz = head_q(xl, yl, zl)
        dh = sd_ell(qx, qy, qz, (0, 0, 0), HEAD_R)
        return smin(sd_body(xl, yl, zl), dh, 8.0) < 0

    def torso_color(X, Y, Z):
        xl, yl, zl = to_local(X, Y, Z)
        qx, qy, qz = head_q(xl, yl, zl)
        dh = sd_ell(qx, qy, qz, (0, 0, 0), HEAD_R)
        db = sd_body(xl, yl, zl)
        in_head = dh < db + 2.0
        ch_ = pick(RAMP_HEAD, (qy + 20) / 40.0, X, Y, Z, seed=1, dither=0.9)
        tb = np.clip((yl - 6) / 66.0, 0, 1) - 0.14 * (zl < -8)
        cb = pick(RAMP_BODY, tb, X, Y, Z, seed=2, dither=0.9)
        return np.where(in_head, ch_, cb).astype(np.uint8)

    ch.add(BIRD_LO, BIRD_HI, torso_inside, torso_color)

    # ---- tail & wings (blades) ----
    tail = Blade((-3, 22, -22), (-40, 5, -52), (0, 1, 0.05),
                 ([0, 0.3, 1], [9, 8.6, 5.6]), ([0, 1], [3.8, 2.4]), 4, 4)
    near = Blade((-24, 62, 8), (-31, 12, -31), (-1, 0.2, 0.1),
                 ([0, 0.2, 0.45, 0.75, 1], [8, 14, 15.5, 11, 4.5]), ([0, 0.4, 1], [5, 6.5, 3.2]), 6, 6)
    # raised to the headphone, 40 long: at 30 it read as a stub beside the
    # folded wing's 64 once the bird was turned (renders, 2026-10-21)
    far = Blade((23, 54, 2), (46, 86, -6), (1, 0, 0.3),
                ([0, 0.3, 0.6, 1], [9, 14, 13, 5]), ([0, 0.5, 1], [5, 6, 3]), 6, 5)

    def blade_inside(b):
        def fn(X, Y, Z):
            xl, yl, zl = to_local(X, Y, Z)
            return b.inside(xl, yl, zl)[0]
        return fn

    def tail_color(X, Y, Z):
        xl, yl, zl = to_local(X, Y, Z)
        along, cw, cn = tail.coords(xl, yl, zl)
        t = along / tail.L
        wv = np.abs(cw) / 9.0
        base = pick(RAMP_TAIL, 0.15 + wv * 0.85 + 0.12 * (cn > 0), X, Y, Z, seed=5, dither=0.7)
        ridge = (np.abs(cw) % 3.4) < 0.7
        dark = pick(RAMP_FLIGHT_DARK, np.full_like(t, 0.3), X, Y, Z, seed=6)
        out = np.where(ridge & (t > 0.1), dark, base)
        return np.where(t > 0.9, dark, out).astype(np.uint8)

    ch.add(BIRD_LO, BIRD_HI, blade_inside(tail), tail_color)

    def near_color(X, Y, Z):
        xl, yl, zl = to_local(X, Y, Z)
        along, cw, cn = near.coords(xl, yl, zl)
        t = along / near.L
        r = np.floor(along / 5.6)
        us = along - r * 5.6
        vs = cw + (r % 2) * 3.4
        cell = np.floor(vs / 6.8)
        vc = vs - cell * 6.8 - 3.4
        dd = np.hypot(us, vc * 0.95)
        edge = (dd > 4.1) & (dd < 5.6)
        cov_in = pick(RAMP_COVERT, 0.35 + 0.5 * np.clip(1 - t * 2, 0, 1), X, Y, Z, seed=7, dither=0.8)
        cov_ed = pick(RAMP_COVERT_EDGE, np.clip(t * 2.2, 0, 1), X, Y, Z, seed=8, dither=0.6)
        coverts = np.where(edge, cov_ed, cov_in)
        tt = np.clip((t - 0.38) / 0.62, 0, 1)
        base = pick(RAMP_FLIGHT, 0.15 + 0.55 * tt + 0.3 * np.clip((cw + 10) / 20, 0, 1), X, Y, Z, seed=9, dither=0.8)
        ridge = ((cw + 40.0) % 4.4) < 0.9
        dark = pick(RAMP_FLIGHT_DARK, np.full_like(t, 0.5), X, Y, Z, seed=10)
        flight = np.where(ridge, dark, base)
        return np.where(t < 0.40, coverts, flight).astype(np.uint8)

    ch.add(BIRD_LO, BIRD_HI, blade_inside(near), near_color)

    def far_color(X, Y, Z):
        xl, yl, zl = to_local(X, Y, Z)
        along, cw, cn = far.coords(xl, yl, zl)
        t = along / far.L
        base = pick(RAMP_FAR_WING, 0.05 + 0.95 * np.clip(t, 0, 1) ** 0.9, X, Y, Z, seed=11, dither=0.9)
        ridge = ((cw + 40.0) % 4.2) < 0.8
        dark = pick(RAMP_FLIGHT_DARK, np.full_like(t, 0.5), X, Y, Z, seed=12)
        return np.where(ridge & (t > 0.35), dark, base).astype(np.uint8)

    ch.add(BIRD_LO, BIRD_HI, blade_inside(far), far_color)

    # ---- feet ----
    def feet_inside(X, Y, Z):
        xl, yl, zl = to_local(X, Y, Z)
        d = np.full_like(xl, 99.0)
        for s in (-1, 1):
            d = np.minimum(d, sd_capsule(xl, yl, zl, (s * 10, 1.5, 9), (s * 10, 11, 8), 2.8))
            d = np.minimum(d, sd_ell(xl, yl, zl, (s * 10, 2.2, 12), (4.4, 2.4, 5.2)))
            for ang in (-30, 0, 30):
                a = math.radians(ang)
                d = np.minimum(d, sd_capsule(xl, yl, zl, (s * 10, 2.0, 13),
                                             (s * 10 + 9.5 * math.sin(a), 1.9, 13 + 9.5 * math.cos(a)), 1.9))
            d = np.minimum(d, sd_capsule(xl, yl, zl, (s * 10, 2.0, 8), (s * 10, 1.9, 3.5), 1.7))
        return d < 0

    def feet_color(X, Y, Z):
        return pick(RAMP_FEET, Y / 5.0, X, Y, Z, seed=13, dither=0.6)

    ch.add(BIRD_LO, BIRD_HI, feet_inside, feet_color)

    # ---- beak ----
    def beak_d(qx, qy, qz):
        up = smin(sd_ell(qx, qy, qz, (0, -7.5, 25.5), (7.3, 6.2, 7.6)),
                  sd_ell(qx, qy, qz, (0, -10.8, 31), (4.4, 4.4, 4.0)), 2.0)
        low = sd_ell(qx, qy, qz, (0, -15.6, 25.5), (5.4, 3.4, 5.8))
        return up, low

    def beak_inside(X, Y, Z):
        qx, qy, qz = hq(X, Y, Z)
        up, low = beak_d(qx, qy, qz)
        mouth = sd_ell(qx, qy, qz, (0, -12.0, 29.5), (3.4, 1.7, 5.2))
        tongue = sd_ell(qx, qy, qz, (0, -13.4, 28.0), (2.3, 1.0, 3.2))
        return (np.minimum(up, low) < 0) & ((mouth > 0) | (tongue < 0))

    def beak_color(X, Y, Z):
        qx, qy, qz = hq(X, Y, Z)
        up, low = beak_d(qx, qy, qz)
        mouth = sd_ell(qx, qy, qz, (0, -12.0, 29.5), (3.4, 1.7, 5.2))
        tongue = sd_ell(qx, qy, qz, (0, -13.4, 28.0), (2.3, 1.0, 3.2))
        top = pick(RAMP_BEAK, (qy + 14) / 12.0, X, Y, Z, seed=14, dither=0.7)
        bot = pick(RAMP_BEAK_LOW, (qy + 19) / 8.0, X, Y, Z, seed=15, dither=0.6)
        c = np.where(low < up, bot, top)
        wall = (mouth < 1.1) & (qz > 22)
        c = np.where(wall, np.where(mouth < 0.35, C_MOUTH_D, C_MOUTH), c)
        c = np.where(tongue < 0.2, C_TONGUE, c)
        return c.astype(np.uint8)

    ch.add(BIRD_LO, BIRD_HI, beak_inside, beak_color)

    # ---- cere ----
    def cere_inside(X, Y, Z):
        qx, qy, qz = hq(X, Y, Z)
        return sd_ell(qx, qy, qz, (0, -2.4, 22.8), (6.0, 3.8, 4.2)) < 0

    def cere_color(X, Y, Z):
        qx, qy, qz = hq(X, Y, Z)
        nos = np.zeros(qx.shape, bool)
        for s in (-1, 1):
            nos |= (np.hypot(qx - s * 2.5, qy + 2.0) < 1.05) & (qz > 24.5)
        c = np.where(qy > -2.4, C_CERE, C_CERE_D)
        return np.where(nos, C_NOSTRIL, c).astype(np.uint8)

    ch.add(BIRD_LO, BIRD_HI, cere_inside, cere_color)

    # ---- face paint (eyes, cheeks, throat spots, head barring) ----
    def paint_face(X, Y, Z, cur):
        xl, yl, zl = to_local(X, Y, Z)
        qx, qy, qz = head_q(xl, yl, zl)
        dh = sd_ell(qx, qy, qz, (0, 0, 0), HEAD_R)
        near_s = (dh > -3.4) & (dh < db_slack(xl, yl, zl, dh))
        out = cur.copy()

        # head barring on the crown / nape (sides and back)
        az = np.degrees(np.arctan2(qx, qz))
        el = np.degrees(np.arcsin(np.clip(qy / np.maximum(np.sqrt(qx ** 2 + qy ** 2 + qz ** 2), 1e-6), -1, 1)))
        phase = (qy + 2.0 * np.sin(np.radians(az) * 2.4) + 0.12 * qz) / 3.4
        stripe = ((phase % 1.0) < 0.27) & (el > 14) & (np.abs(az) > 72) & near_s
        out = np.where(stripe, C_STRIPE, out)

        q = np.stack([qx, qy, qz], axis=-1)

        def frame_coords(center):
            n, u, v = tangent(center)
            d = q - center
            return d @ u, d @ v, d @ n

        for s in (-1, 1):
            # cheek patch
            cc = sp(s * 64, -19)
            du, dv, dn = frame_coords(cc)
            ang = math.radians(20 * s)
            eu = du * math.cos(ang) - dv * math.sin(ang)
            ev = du * math.sin(ang) + dv * math.cos(ang)
            patch = ((eu / 5.0) ** 2 + (ev / 7.6) ** 2 < 1) & near_s & (np.abs(dn) < 6)
            out = np.where(patch, pick(RAMP_CHEEK, 0.5 + 0.35 * np.clip(ev / 7.6, -1, 1), X, Y, Z, seed=16, dither=0.8), out)

            # throat spots
            for (a_, e_, r_) in ((50, -40, 1.8), (43, -51, 1.9), (31, -60, 1.7)):
                sc = sp(s * a_, e_)
                du, dv, dn = frame_coords(sc)
                spot = (np.hypot(du, dv) < r_) & near_s & (np.abs(dn) < 5)
                out = np.where(spot, C_SPOT, out)

            # eye
            ec = sp(s * 41, 4)
            du, dv, dn = frame_coords(ec)
            rr = np.hypot(du, dv)
            ok = near_s & (np.abs(dn) < 6)
            brow = ok & (np.abs(du) < 5.4) & (np.abs(dv - (9.4 - 0.09 * du ** 2 + 0.16 * du * (-s))) < 0.85)
            out = np.where(brow, C_BROW, out)
            out = np.where(ok & (rr < 6.7), C_RING, out)
            tiris = np.clip(0.45 - dv * 0.05 + du * 0.03, 0, 1)
            out = np.where(ok & (rr < 5.5), pick(RAMP_EYE, tiris, X, Y, Z, seed=17, dither=0.3), out)
            h1 = ((du + 2.0) ** 2 + (dv - 2.4) ** 2) < 2.3 ** 2
            h2 = ((du - 2.3) ** 2 + (dv + 2.6) ** 2) < 1.0 ** 2
            out = np.where(ok & (rr < 5.5) & h1, C_WHITE, out)
            out = np.where(ok & (rr < 5.5) & h2 & ~h1, C_HL2, out)
        return out

    def db_slack(xl, yl, zl, dh):
        # restrict face paint to voxels that belong to the head side of the head/body blend
        db = sd_body(xl, yl, zl)
        return np.where(dh < db + 2.0, 1.2, -99.0)

    ch.recolor(BIRD_LO, BIRD_HI, paint_face)

    # ---- headphones ----
    if headphones:
        build_headphones(ch)


class Blade:
    """A flat, tapered, leaf-like solid running from S to T (used for wings and tail)."""

    def __init__(self, S, T, n_hint, rw, rt, r_end0=5, r_end1=4):
        S = np.array(S, float)
        T = np.array(T, float)
        a = T - S
        self.L = float(np.linalg.norm(a))
        a = a / self.L
        n = np.array(n_hint, float)
        n = n - a * np.dot(n, a)
        n = n / np.linalg.norm(n)
        self.S, self.a, self.n, self.w = S, a, n, np.cross(a, n)
        self.rw, self.rt, self.r0, self.r1 = rw, rt, r_end0, r_end1

    def coords(self, xl, yl, zl):
        dx, dy, dz = xl - self.S[0], yl - self.S[1], zl - self.S[2]
        along = dx * self.a[0] + dy * self.a[1] + dz * self.a[2]
        cw = dx * self.w[0] + dy * self.w[1] + dz * self.w[2]
        cn = dx * self.n[0] + dy * self.n[1] + dz * self.n[2]
        return along, cw, cn

    def inside(self, xl, yl, zl):
        along, cw, cn = self.coords(xl, yl, zl)
        t = np.clip(along / self.L, 0, 1)
        rw = np.interp(t, *self.rw)
        rt = np.interp(t, *self.rt)
        ax = np.where(along < 0, along / self.r0, np.where(along > self.L, (along - self.L) / self.r1, 0.0))
        f = (cw / rw) ** 2 + (cn / rt) ** 2 + ax ** 2
        return f < 1.0, along / self.L, cw, cn


# ------------------------------------------------------------------ headphones
RAMP_BAND = PAL.ramp("#2E63B8", "#3D7BD6", "#5B93E6")
C_CUP_RIM = PAL.c("#C9D1DE")
C_CUP = PAL.c("#F2F5FA")
C_CUP_IN = PAL.c("#E3E9F5")
C_CUP_RING = PAL.c("#3D8BFF")
C_PAD = PAL.c("#2F3441")
C_GLYPH = PAL.c("#2F7BE0")

FEATHER = [
    ".....XX",
    "....XXX",
    "...XXXX",
    "..XXXX.",
    ".XXXX..",
    ".XXX...",
    "XXX....",
    "X......",
]


def build_headphones(ch):
    A, B, ZOFF = 31.5, 25.6, -4.5  # band ellipse radii (x,y) and z offset in head space
    CUPX, CUPY, CUPZ = 32.0, -3.0, -6.0

    def band(qx, qy, qz):
        phi = np.clip(np.arctan2(qy / B, qx / A), 0, math.pi)
        px, py = A * np.cos(phi), B * np.sin(phi)
        rxy = np.hypot(qx - px, qy - py)
        return (rxy / 2.1) ** 2 + ((qz - ZOFF) / 3.6) ** 2, rxy

    def band_inside(X, Y, Z):
        qx, qy, qz = hq(X, Y, Z)
        f, _ = band(qx, qy, qz)
        return f < 1

    def band_color(X, Y, Z):
        qx, qy, qz = hq(X, Y, Z)
        return pick(RAMP_BAND, 0.25 + qy / 34.0, X, Y, Z, seed=20, dither=0.6)

    ch.add(BIRD_LO, BIRD_HI, band_inside, band_color)

    def cup_geom(qx, qy, qz, s):
        ax = s * qx - CUPX
        rr = np.hypot(qy - CUPY, qz - CUPZ)
        return ax, rr

    def cup_inside(X, Y, Z):
        qx, qy, qz = hq(X, Y, Z)
        m = np.zeros(qx.shape, bool)
        for s in (-1, 1):
            ax, rr = cup_geom(qx, qy, qz, s)
            # rounded cylinder body (outer cup) + pad toward the head
            dx = np.maximum(np.abs(ax - 0.4) - 3.0, 0)
            dr = np.maximum(rr - 8.8, 0)
            body = np.hypot(dx, dr) < 1.5
            pad = (ax > -5.6) & (ax < -2.4) & (rr < 9.0)
            m |= body | pad
        return m

    def cup_color(X, Y, Z):
        qx, qy, qz = hq(X, Y, Z)
        out = np.full(qx.shape, C_CUP, np.uint8)
        for s in (-1, 1):
            ax, rr = cup_geom(qx, qy, qz, s)
            sel = (ax > -6) & (rr < 11)
            c = np.full(qx.shape, C_CUP, np.uint8)
            c = np.where(rr > 8.3, C_CUP_RIM, c)
            c = np.where((rr > 5.9) & (rr < 7.4) & (ax > 1.4), C_CUP_RING, c)
            c = np.where((rr < 5.4) & (ax > 1.4), C_CUP_IN, c)
            # feather glyph on the outer face
            row = np.floor(-(qy - CUPY) + 4.0).astype(int)
            col = np.floor((qz - CUPZ) * -s + 3.5).astype(int)
            g = np.zeros(qx.shape, bool)
            for r_, line in enumerate(FEATHER):
                for c_, chx in enumerate(line):
                    if chx == "X":
                        g |= (row == r_) & (col == c_)
            c = np.where(g & (ax > 1.4) & (rr < 5.4), C_GLYPH, c)
            c = np.where(ax < -2.2, C_PAD, c)
            out = np.where(sel & (ax * 1.0 > -6), c, out)
        return out.astype(np.uint8)

    ch.add(BIRD_LO, BIRD_HI, cup_inside, cup_color)


# ------------------------------------------------------------------- the props
MX, MZ = 34.0, 20.0  # microphone centre
RAMP_MIC = PAL.ramp("#3E4458", "#4C5367", "#5A627A")
C_MIC_DARK = PAL.c("#2B303D")
C_MESH_L = PAL.c("#9AA1B7")
C_MESH_D = PAL.c("#767D93")
C_MIC_RING = PAL.c("#3A9BFF")
C_MIC_CORE = PAL.c("#242A38")
RAMP_BASE = PAL.ramp("#3A4052", "#4F566A", "#656C7F")
RAMP_YOKE = PAL.ramp("#2F3646", "#373F51", "#454D61")


def build_mic(ch):
    lo, hi = (MX - 20, 0, MZ - 20), (MX + 20, 56, MZ + 20)
    CY = 39.5  # capsule centre

    def base_in(X, Y, Z):
        rr = np.hypot(X - MX, Z - MZ)
        dy = np.maximum(np.abs(Y - 1.7) - 1.7 + 1.3, 0)
        dr = np.maximum(rr - 11 + 1.3, 0)
        return np.hypot(dy, dr) < 1.3

    ch.add(lo, hi, base_in, lambda X, Y, Z: pick(RAMP_BASE, (Y - 0.5) / 3.0 * 0.5 + 0.5 * (np.hypot(X - MX, Z - MZ) < 8), X, Y, Z, seed=30, dither=0.5))

    ch.add(lo, hi, lambda X, Y, Z: (np.hypot(X - MX, Z - MZ) < 2.4) & (Y > 3) & (Y < 28),
           lambda X, Y, Z: pick(RAMP_YOKE, 0.5 + (X - MX) / 6.0, X, Y, Z, seed=31, dither=0.4))

    R_U = 13.4

    def yoke_in(X, Y, Z):
        dx, dy = X - MX, Y - CY
        ang = np.arctan2(dy, dx)  # lower half: angle in (-pi, 0)
        rr = np.hypot(dx, dy)
        arc = (dy < 0) & (np.abs(rr - R_U) < 1.9) & (np.abs(Z - MZ) < 1.4)
        arm = (dy >= 0) & (dy < 0.5 + 0) & (np.abs(np.abs(dx) - R_U) < 1.9) & (np.abs(Z - MZ) < 1.4)
        knob = (np.hypot(np.abs(dx) - 12.6, np.hypot(dy, Z - MZ)) < 3.0)
        return arc | arm | knob

    ch.add(lo, hi, yoke_in, lambda X, Y, Z: pick(RAMP_YOKE, 0.4 + 0.5 * (Z - MZ) / 3.0, X, Y, Z, seed=32, dither=0.4))

    L, R, RZ = 3.0, 10.5, 9.6

    def cap_d(X, Y, Z):
        rho = np.sqrt(((X - MX) / R) ** 2 + ((Z - MZ) / RZ) ** 2) * 10.0
        dy = np.maximum(np.abs(Y - CY) - L, 0)
        return np.hypot(rho, dy) - 10.0

    def cap_color(X, Y, Z):
        d = cap_d(X, Y, Z)
        top = Y > CY + 1.5
        dots = ((np.floor(X) + np.floor(Y) + np.floor(Z)) % 2) == 0
        mesh = np.where(dots, C_MESH_D, C_MESH_L)
        body = pick(RAMP_MIC, 0.5 + (X - MX) / 28.0, X, Y, Z, seed=33, dither=0.5)
        band = (np.abs(Y - (CY + 1.5)) < 0.6)
        c = np.where(top, mesh, body)
        c = np.where(band, C_MIC_DARK, c)
        # blue ring on the front face
        rho = np.hypot(X - MX, Y - (CY - 6.5))
        front = (Z > MZ + 4.0) & (d > -2.6)
        c = np.where(front & (rho > 3.0) & (rho < 4.7), C_MIC_RING, c)
        c = np.where(front & (rho <= 3.0), C_MIC_CORE, c)
        return c.astype(np.uint8)

    ch.add(lo, hi, lambda X, Y, Z: cap_d(X, Y, Z) < 0, cap_color)


C_CARD_B = PAL.c("#D3DDEF")
RAMP_CARD = PAL.ramp("#EBF0F9", "#F2F6FC", "#F8FAFE", "#FCFDFF")
C_LINE_B = PAL.c("#4590EF")
C_LINE_G = PAL.c("#BDC9DC")


def build_card(ch):
    CX, CY, HW, HH = 69.0, 63.0, 23.0, 20.0
    lo, hi = (CX - 28, CY - 24, -9), (CX + 28, CY + 24, 1)

    def inside(X, Y, Z):
        return (sd_rrect(X, Y, CX, CY, HW, HH, 6.5) < 0) & (Z > -6) & (Z < -2)

    def color(X, Y, Z):
        d = sd_rrect(X, Y, CX, CY, HW, HH, 6.5)
        c = pick(RAMP_CARD, (Y - (CY - HH)) / (2 * HH), X, Y, Z, seed=40, dither=0.4)
        return np.where(d > -1.5, C_CARD_B, c).astype(np.uint8)

    ch.add(lo, hi, inside, color)

    lines = ((73.6, 30.7, C_LINE_B), (66.4, 33.7, C_LINE_G), (59.3, 29.0, C_LINE_G), (52.3, 24.7, C_LINE_G))
    for (yc, length, col) in lines:
        x0 = 52.0

        def lin(X, Y, Z, yc=yc, length=length):
            return (sd_rrect(X, Y, x0 + length / 2, yc, length / 2, 1.5, 1.5) < 0) & (Z > -7) & (Z < -1)

        ch.add(lo, hi, lin, lambda X, Y, Z, col=col: np.full(X.shape, col, np.uint8))


RAMP_BUB = PAL.ramp("#D3E8FB", "#DCEDFC", "#E6F3FE", "#EEF7FE")
C_BUB_B = PAL.c("#BFD9F4")
BUB_CX, BUB_CY, BUB_HW, BUB_HH = 62.0, 108.0, 29.0, 19.0
BAR_COLORS = ["#12C8FF", "#14BEFD", "#1AB0FB", "#209FF6", "#2A93F0", "#2A8CEE", "#2582EA", "#2276E2",
              "#2068D6", "#1D5BC4", "#1A4FB0"]
BAR_BASE_H = [4, 7, 11, 18, 13, 26, 17, 10, 14, 8, 5]


def build_bubble(ch):
    lo, hi = (BUB_CX - 33, BUB_CY - 28, -9), (BUB_CX + 33, BUB_CY + 22, 1)
    A, B, C = (43.5, 91.5), (55.0, 91.5), (39.0, 82.5)

    def sd2(X, Y):
        return sd_rrect(X, Y, BUB_CX, BUB_CY, BUB_HW, BUB_HH, 14.0)

    def inside(X, Y, Z):
        body = sd2(X, Y) < 0
        tail = in_tri(X, Y, A, B, C)
        return (body | tail) & (Z > -6) & (Z < -2)

    def color(X, Y, Z):
        d = sd2(X, Y)
        c = pick(RAMP_BUB, (Y - (BUB_CY - BUB_HH)) / (2 * BUB_HH), X, Y, Z, seed=50, dither=0.4)
        return np.where((d > -1.4) & (d < 0.5), C_BUB_B, c).astype(np.uint8)

    ch.add(lo, hi, inside, color)


# ------------------------------------------------------------------------ export
def runs(arr):
    """The cells in order, z fastest, as [count, colour, count, colour, ...]: as
    plain numbers they compress to 26 KB, against 31 KB as base64 varints
    (brotli at quality 9, Tauri's for a release build's assets, 2026-10-21)."""
    flat = arr.reshape(-1)
    change = np.nonzero(np.diff(flat))[0] + 1
    starts = np.concatenate(([0], change))
    ends = np.concatenate((change, [flat.size]))
    out = np.empty(2 * len(starts), np.int64)
    out[0::2] = ends - starts
    out[1::2] = flat[starts]
    return out.tolist()


def hide_interior(g):
    """Voxels buried under six solid neighbours are never drawn; give them one flat colour so RLE stays small."""
    m = g > 0
    p = np.pad(m, 1)
    buried = (m & p[:-2, 1:-1, 1:-1] & p[2:, 1:-1, 1:-1] & p[1:-1, :-2, 1:-1] & p[1:-1, 2:, 1:-1]
              & p[1:-1, 1:-1, :-2] & p[1:-1, 1:-1, 2:])
    out = g.copy()
    out[buried] = 1
    return out


def export_chunk(ch, pad=(0, 0, 0), force_lo=None, force_hi=None):
    ch.g = hide_interior(ch.g)
    nz = np.argwhere(ch.g > 0)
    lo = nz.min(axis=0)
    hi = nz.max(axis=0) + 1
    if force_lo is not None:
        lo = np.minimum(lo, np.floor(np.array(force_lo) - ORIGIN).astype(int))
    if force_hi is not None:
        hi = np.maximum(hi, np.ceil(np.array(force_hi) - ORIGIN).astype(int))
    lo = np.clip(lo - np.array(pad), 0, None)
    hi = np.minimum(hi + np.array(pad), DIMS)
    sub = np.ascontiguousarray(ch.g[lo[0]:hi[0], lo[1]:hi[1], lo[2]:hi[2]])
    return {
        "o": [float(ORIGIN[i] + lo[i]) for i in range(3)],
        "d": [int(hi[i] - lo[i]) for i in range(3)],
        "runs": runs(sub),
    }, int((sub > 0).sum())


def generate(headphones=True):
    bird, props, bubble = Chunk(), Chunk(), Chunk()
    build_bird(bird, headphones=headphones)
    build_mic(props)
    build_card(props)
    build_bubble(bubble)

    # waveform bar spec (the viewer stamps these into the bubble chunk each animation frame)
    bars = {
        "x0": [41 + 4 * i for i in range(len(BAR_BASE_H))],
        "w": 3,
        "yc": BUB_CY,
        "z0": -7,
        "z1": -1,
        "colors": [PAL.c(h) for h in BAR_COLORS],
        "base": BAR_BASE_H,
        "max": 28,
    }

    chunks, voxels = {}, 0
    for name, ch, kw in (("bird", bird, {}), ("props", props, {}),
                         ("bubble", bubble, {"force_lo": (BUB_CX - 33, BUB_CY - 28, -8), "force_hi": (BUB_CX + 33, BUB_CY + 22, 0)})):
        chunks[name], n = export_chunk(ch, **kw)
        voxels += n
    allnz = np.argwhere((bird.g | props.g | bubble.g) > 0)
    mn, mx = allnz.min(axis=0) + ORIGIN, allnz.max(axis=0) + 1 + ORIGIN
    pivot = ((mn + mx) / 2).tolist()
    pts = allnz + ORIGIN + 0.5
    rxz = float(np.max(np.hypot(pts[:, 0] - pivot[0], pts[:, 2] - pivot[2])))
    data = {
        "palette": [list(c) for c in PAL.cols],
        "chunks": chunks,
        "bars": bars,
        "pivot": pivot,
        "rxz": round(rxz, 3),
        "ground": {"bird": [-4.0, -2.0, 62.0, 40.0], "mic": [MX, MZ, 17.0, 14.0]},
    }
    return data, voxels


def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.join(root, "ui", "mascot-model.js"))
    ap.add_argument("--no-headphones", action="store_true")
    args = ap.parse_args()
    data, voxels = generate(headphones=not args.no_headphones)
    js = ("/* The mascot's voxels, from scripts/mascot.py: each chunk's cells as runs of\n"
          "   palette indices, for ui/mascot.js. */\n"
          "const MASCOT = " + json.dumps(data, separators=(",", ":")) + ";\n")
    with open(args.out, "w") as fh:
        fh.write(js)
    print(f"{args.out}: {len(js) / 1024:.0f} KB, {voxels} voxels, {len(data['palette']) - 1} colours")


if __name__ == "__main__":
    sys.exit(main())

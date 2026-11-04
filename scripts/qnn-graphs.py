#!/usr/bin/env python3
"""The NPU's graphs in DIR held to the catalog (src-tauri/src/model.rs, each
Entry's qnn) before they go to models.pktito.net: each file's size and
SHA-256 the catalog's, and its header as the qnn branch's core/qnn.c writes
it (the vendored copy's PK_QNN_GRAPH, 616 bytes, little-endian) saying what
the catalog says the file is - the chip it lists the graph under, and the
model: the key the engine makes from the slim file the app downloads with
the graph (in DIR too, and held to the catalog as well), its checkpoint and
conversion, the whole length (3072 feature frames, PK_QNN_FRAMES) and every
layer. A phone whose chip has a graph downloads it without trying it first,
so a file under the wrong chip or model is a model that fails there for
everyone.

    scripts/qnn-graphs.py DIR
"""

import hashlib
import os
import re
import struct
import sys

here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# the graph the vendored engine reads (vendor/pktito-qnn/core/qnn.c)
MAGIC = re.search(
    r'#define PK_QNN_GRAPH "([^"]+)"',
    open(
        os.path.join(here, "vendor", "pktito-qnn", "core", "qnn.c"), encoding="utf-8"
    ).read(),
).group(1)
# magic, key, chip, made, built, names[6][16], id[6], rank[6], dims[6][4], bytes
HEAD = struct.Struct("<32s176s32s64s64s96s6I6I24IQ")
FRAMES = 3072  # the length the engine asks for (core/qnn.h, PK_QNN_FRAMES)
PART = r'file: "([^"]+)",\s*bytes: ([\d_]+),\s*sha256: "([0-9a-f]{64})"'


def catalog():
    """each entry's name, its slim file and its graphs, as (chip, file, bytes, sha256)"""
    src = open(
        os.path.join(here, "src-tauri", "src", "model.rs"), encoding="utf-8"
    ).read()
    out = []
    for e in re.split(r"\n    Entry \{", src)[1:]:
        name = re.search(r'name: "([^"]+)"', e).group(1)
        slim = re.search(r"slim: Part \{\s*" + PART, e).groups()
        graphs = re.findall(r'Graph \{\s*chip: "([^"]+)",\s*part: Part \{\s*' + PART, e)
        out.append((name, slim, graphs))
    return out


def text(b):
    """a NUL-padded field"""
    return b.split(b"\0", 1)[0].decode("utf-8", "replace")


def held(path, size, sha256):
    """why the file at path isn't the catalog's, or None"""
    if not os.path.exists(path):
        return "not in DIR"
    n = os.path.getsize(path)
    if n != int(size.replace("_", "")):
        return f"{n} bytes, the catalog's {size}"
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while b := f.read(1 << 24):
            h.update(b)
    return (
        None
        if h.hexdigest() == sha256
        else f"SHA-256 {h.hexdigest()}, not the catalog's"
    )


def pkf(path):
    """a .pkf's params and metadata (the qnn branch's core/pkf.h)"""
    with open(path, "rb") as f:
        b = f.read(256)
        (n_params,) = struct.unpack_from("<I", b, 104)
        (params_off,) = struct.unpack_from("<Q", b, 112)
        meta_off, meta_size = struct.unpack_from("<QQ", b, 144)
        f.seek(params_off)
        p = f.read(40 * n_params)
        f.seek(meta_off)
        m = f.read(meta_size).split(b"\0")
    params = {}
    for i in range(n_params):
        key, kind, v = struct.unpack_from("<32sI4s", p, 40 * i)
        params[text(key)] = struct.unpack(
            "<f" if kind == 2 else "<i" if kind == 1 else "<I", v
        )[0]
    return params, {k.decode(): v.decode() for k, v in zip(m[0::2], m[1::2])}


def graph(path, chip, params, meta):
    """why the graph at path isn't what the catalog lists it as (None where
    it is), and what made it"""
    with open(path, "rb") as f:
        h = HEAD.unpack(f.read(HEAD.size))
    magic, key, got, made, built = (text(b) for b in h[:5])
    rank, dims, n = h[12:18], h[18:42], h[42]
    makers = f"{made}, QNN {built}"
    t = FRAMES
    for _ in range(3):  # the encoder's three halvings of the length
        t = (t + 1) // 2
    want = f"{meta['source_sha256']} {meta['converter']} {FRAMES} {params['n_layers']}"
    mel, out = list(dims[0:4]), list(dims[20:24])
    if magic != MAGIC:
        return f"not {MAGIC}: {magic!r}", None
    if got != chip:
        return f"compiled for the chip {got}, listed under {chip}", makers
    if key != want:
        return f"the graph of {key!r}, where the slim file's is {want!r}", makers
    if n != os.path.getsize(path) - HEAD.size:
        return f"a binary of {n} bytes in {os.path.getsize(path) - HEAD.size}", makers
    if rank[0] != 4 or mel != [1, FRAMES, params["n_mels"], 1]:
        return f"features in of {mel[: rank[0]]}", makers
    if rank[5] != 2 or out[:2] != [t, params["d_model"]]:
        return f"an encoding out of {out[: rank[5]]}", makers
    return None, makers


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: " + __doc__.strip().splitlines()[-1].strip())
    d = sys.argv[1]
    bad = 0
    for name, (slim, size, sha), graphs in catalog():
        why = held(os.path.join(d, slim), size, sha)
        print(f"{slim}: {why}" if why else f"{slim}: the catalog's")
        if why:
            bad += 1 + len(graphs)
            continue
        params, meta = pkf(os.path.join(d, slim))
        for chip, file, size, sha in graphs:
            path = os.path.join(d, file)
            why, makers = held(path, size, sha), None
            if not why:
                why, makers = graph(path, chip, params, meta)
            said = why or f"the catalog's, {name} for {chip}"
            print(f"{file}: {said}" + (f" ({makers})" if makers else ""))
            bad += why is not None
    if bad:
        sys.exit(f"{bad} of the files not what the catalog says")


if __name__ == "__main__":
    main()

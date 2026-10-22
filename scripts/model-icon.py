#!/usr/bin/env python3
"""src-tauri/icons/model.ico and model.icns: the icon of pktito's model files
(.pkf), which the installer gives them on Windows (src-tauri/windows/hooks.nsh)
and the app's Info.plist on macOS.

    python3 scripts/model-icon.py      # on macOS, after design/model-icon*.svg change

Each size is rendered by `cargo tauri icon` from the drawing made for it:
design/model-icon.svg from 128 px up, and below that the versions that keep
their edges on whole pixels. The .ico holds the sizes Explorer asks for at
100 to 200 % and its large views, each a PNG; iconutil makes the .icns.
"""
import os
import struct
import subprocess
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DESIGN = os.path.join(ROOT, "design")
ICONS = os.path.join(ROOT, "src-tauri", "icons")

# each size and the drawing it comes from
SOURCE = {16: "model-icon-16.svg", 20: "model-icon-24.svg", 24: "model-icon-24.svg",
          32: "model-icon-32.svg", 40: "model-icon-32.svg", 48: "model-icon-64.svg",
          64: "model-icon-64.svg", 128: "model-icon.svg", 256: "model-icon.svg",
          512: "model-icon.svg", 1024: "model-icon.svg"}
ICO = [16, 20, 24, 32, 40, 48, 64, 256]
# the .iconset's names: each size at 1x, and twice it at 2x
ICNS = [(16, 16, ""), (16, 32, "@2x"), (32, 32, ""), (32, 64, "@2x"), (128, 128, ""), (128, 256, "@2x"),
        (256, 256, ""), (256, 512, "@2x"), (512, 512, ""), (512, 1024, "@2x")]


def render(tmp):
    png = {}
    for svg in sorted(set(SOURCE.values())):
        sizes = [s for s, f in SOURCE.items() if f == svg]
        out = os.path.join(tmp, svg)
        subprocess.run(["cargo", "tauri", "icon", os.path.join(DESIGN, svg), "-o", out,
                        "--png", ",".join(map(str, sizes))], cwd=ROOT, check=True, capture_output=True)
        for s in sizes:
            with open(os.path.join(out, f"{s}x{s}.png"), "rb") as f:
                png[s] = f.read()
    return png


def write_ico(path, png):
    head = struct.pack("<HHH", 0, 1, len(ICO))
    at = len(head) + 16 * len(ICO)
    entries, data = b"", b""
    for s in ICO:
        # a width or height of 256 is written as 0
        entries += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(png[s]), at + len(data))
        data += png[s]
    with open(path, "wb") as f:
        f.write(head + entries + data)


def main():
    with tempfile.TemporaryDirectory() as tmp:
        png = render(tmp)
        write_ico(os.path.join(ICONS, "model.ico"), png)
        iconset = os.path.join(tmp, "model.iconset")
        os.mkdir(iconset)
        for s, px, at in ICNS:
            with open(os.path.join(iconset, f"icon_{s}x{s}{at}.png"), "wb") as f:
                f.write(png[px])
        subprocess.run(["iconutil", "-c", "icns", iconset, "-o", os.path.join(ICONS, "model.icns")], check=True)
    print("wrote src-tauri/icons/model.ico and model.icns")


if __name__ == "__main__":
    main()

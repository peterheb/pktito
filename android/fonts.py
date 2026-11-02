#!/usr/bin/env python3
"""The Android app's fonts: ui/fonts' woff2 files, the design system's
(design/tokens.json), as TTF in app/src/main/res/font, which Android's font
resources take and woff2 they do not. The glyphs are the same; only the
container changes. Their licences, the SIL OFL's, go with them, into the
app's assets. Needs fontTools and brotli (pip install fonttools brotli);
the TTFs are checked in, so a build does not."""
import glob, json, os, re, shutil
from fontTools.ttLib import TTFont

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(here)
out = os.path.join(here, "app", "src", "main", "res", "font")
os.makedirs(out, exist_ok=True)
for f in json.load(open(os.path.join(root, "design", "tokens.json")))["type"]["fonts"]:
    stem = os.path.splitext(os.path.basename(f["file"]))[0]
    # a resource's name: lower case, digits and underscores (scripts/tokens.py's font_res)
    name = re.sub(r"(?<=[a-z])(?=[A-Z])", "_", stem).replace("-", "_").lower()
    # the head table's date the woff2's, so that a run makes the same bytes
    font = TTFont(os.path.join(root, "ui", f["file"]), recalcTimestamp=False)
    font.flavor = None
    font.save(os.path.join(out, name + ".ttf"))
    print(f"{f['file']} -> res/font/{name}.ttf")
for ofl in sorted(glob.glob(os.path.join(root, "ui", "fonts", "*-OFL.txt"))):
    dst = os.path.join(here, "app", "src", "main", "assets", "licenses")
    os.makedirs(dst, exist_ok=True)
    shutil.copy(ofl, dst)
    print(f"fonts/{os.path.basename(ofl)} -> assets/licenses/")

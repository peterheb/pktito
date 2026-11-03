#!/usr/bin/env python3
"""ui/fonts/Archivo-{400,500,700}.woff2: Archivo, the design system's sans
(design/tokens.json), made from its variable font in Google's fonts
repository: an instance at each weight the type styles use, at its normal
width, holding Google Fonts' latin and latin-ext ranges and the font's
combining marks. Those have every
letter of the Latin-script languages the multilingual model speaks, 21 of
its 25 (model.rs); Archivo has no Cyrillic or Greek, which the system's
fonts draw, a word at a time. The files were Google Fonts' latin range
alone, 230 characters, which drew a Polish or Czech transcript's ł or ř
from another font in the middle of a word.

android/fonts.py makes the Android app's TTFs from these, and
scripts/tokens.py copies those into the iOS app.

    scripts/archivo.py          # needs fontTools and brotli (pip install fonttools brotli)
"""

import hashlib
import io
import os
import urllib.request

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

# Archivo 2.001, as google/fonts added it (its last change to the file)
URL = (
    "https://github.com/google/fonts/raw/6c70c829f09ea345d3590406693220ea35c6553f/"
    "ofl/archivo/Archivo%5Bwdth%2Cwght%5D.ttf"
)
SHA256 = "0e094a7d3c7c4c25cf1310c4b30014f1dae9332220b1c2c88f4fa996f0b05053"
WEIGHTS = (400, 500, 700)
# Google Fonts' latin and latin-ext ranges for Archivo (fonts.googleapis.com/css2)
LATIN = (
    "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, "
    "U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD"
)
LATIN_EXT = (
    "U+0100-02BA, U+02BD-02C5, U+02C7-02CC, U+02CE-02D7, U+02DD-02FF, U+0304, U+0308, "
    "U+0329, U+1D00-1DBF, U+1E00-1E9F, U+1EF2-1EFF, U+2020, U+20A0-20AB, U+20AD-20C4, "
    "U+2113, U+2C60-2C7F, U+A720-A7FF"
)
# and every combining mark the font has: a name iOS and macOS hand over
# decomposed, as "Café" an e and a combining acute, draws its accents from it
MARKS = "U+0300-036F"

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def unicodes(ranges):
    out = []
    for r in ranges.split(","):
        a, _, b = r.strip().removeprefix("U+").partition("-")
        out += range(int(a, 16), int(b or a, 16) + 1)
    return out


def main():
    data = urllib.request.urlopen(URL).read()
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise SystemExit(f"{URL}: not the file pinned here")
    for w in WEIGHTS:
        # the head table's date the source's, so that a run makes the same bytes
        font = TTFont(io.BytesIO(data), recalcTimestamp=False)
        font = instancer.instantiateVariableFont(
            font, {"wght": w, "wdth": 100}, updateFontNames=True
        )
        opts = subset.Options()
        opts.layout_features = ["*"]
        opts.name_IDs = ["*"]
        opts.drop_tables += ["DSIG"]
        sub = subset.Subsetter(opts)
        sub.populate(unicodes=unicodes(LATIN) + unicodes(LATIN_EXT) + unicodes(MARKS))
        sub.subset(font)
        font.flavor = "woff2"
        path = os.path.join(root, "ui", "fonts", f"Archivo-{w}.woff2")
        font.save(path)
        print(
            f"ui/fonts/Archivo-{w}.woff2: {len(font.getBestCmap())} characters, "
            f"{os.path.getsize(path) / 1024:.1f} KB"
        )


if __name__ == "__main__":
    main()

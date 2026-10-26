#!/usr/bin/env python3
"""The Android app's icon, from design/app-icon.svg as the desktop's icon set
is: an adaptive icon, the SVG's rounded square as its background in plume
and its four bars and the beak as its foreground, which the launcher cuts
to its own shape; a monochrome layer, the same bars and beak, for a
launcher that tints icons to the wallpaper (themed icons, Android 13); and
the 512 px square that Play's listing asks for, full bleed, which Play
rounds itself.

    python3 android/icon.py     # after design/app-icon.svg changes

The SVG's 1024 units are the 72 dp a launcher shows of an adaptive icon's
108 (18 dp each side are for its motion): the farthest corner of a bar is
21 dp from the middle, well inside the 33 dp circle every shape keeps.
Needs Pillow for the PNG; the files are checked in, so a build does not.
"""
import os
import xml.etree.ElementTree as ET

from PIL import Image, ImageDraw

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(here)
res = os.path.join(here, "app", "src", "main", "res")
SVG = "{http://www.w3.org/2000/svg}"

svg = ET.parse(os.path.join(root, "design", "app-icon.svg")).getroot()
ground, *marks = [r.attrib for r in svg.iter(SVG + "rect")]
size = float(svg.attrib["viewBox"].split()[2])


def vector(name, what, fill=None):
    paths = "\n".join(
        f'        <path android:fillColor="{fill or m["fill"]}" '
        f'android:pathData="M{m["x"]},{m["y"]}h{m["width"]}v{m["height"]}h-{m["width"]}z" />'
        for m in marks
    )
    with open(os.path.join(res, "drawable", name + ".xml"), "w") as fh:
        fh.write(f"""<?xml version="1.0" encoding="utf-8"?>
<!-- {what}: design/app-icon.svg's bars and beak, its {size:g} units in the
     72 dp a launcher shows (android/icon.py) -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <group
        android:scaleX="{72 / size:g}"
        android:scaleY="{72 / size:g}"
        android:translateX="18"
        android:translateY="18">
{paths}
    </group>
</vector>
""")


vector("ic_launcher_foreground", "The launcher icon's foreground")
# a tint's alpha is what a themed icon takes; its colour is the launcher's
vector("ic_launcher_monochrome", "The themed icon", fill="#FF000000")
os.makedirs(os.path.join(res, "mipmap-anydpi"), exist_ok=True)
with open(os.path.join(res, "mipmap-anydpi", "ic_launcher.xml"), "w") as fh:
    fh.write("""<?xml version="1.0" encoding="utf-8"?>
<!-- design/app-icon.svg as an adaptive icon (android/icon.py) -->
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/icon_background" />
    <foreground android:drawable="@drawable/ic_launcher_foreground" />
    <monochrome android:drawable="@drawable/ic_launcher_monochrome" />
</adaptive-icon>
""")
with open(os.path.join(res, "values", "icon.xml"), "w") as fh:
    fh.write(f"""<?xml version="1.0" encoding="utf-8"?>
<!-- the launcher icon's background: design/app-icon.svg's (android/icon.py) -->
<resources>
    <color name="icon_background">{ground["fill"]}</color>
</resources>
""")

# Play's listing: 512 px, square, the SVG drawn on whole pixels
k = 512 / size
im = Image.new("RGB", (512, 512), ground["fill"])
d = ImageDraw.Draw(im)
for m in marks:
    x, y = float(m["x"]) * k, float(m["y"]) * k
    d.rectangle([round(x), round(y), round(x + float(m["width"]) * k) - 1, round(y + float(m["height"]) * k) - 1], fill=m["fill"])
os.makedirs(os.path.join(here, "play"), exist_ok=True)
im.save(os.path.join(here, "play", "icon-512.png"), optimize=True)
print("res/mipmap-anydpi/ic_launcher.xml, res/drawable/ic_launcher_{foreground,monochrome}.xml, res/values/icon.xml, play/icon-512.png")

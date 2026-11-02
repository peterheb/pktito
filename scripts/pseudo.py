#!/usr/bin/env python3
"""The apps' words in a pseudo-language, to see whether their screens hold a
language less compact than English, up to a limit (Peter, 2026-10-31):
each string as long as its English length makes it, x2 to 10 characters,
x1.6 to 20, x1.4 to 50, x1.3 past (German's "Copy" is "Kopieren"; its
long sentences run about a third longer than English); its letters the
Latin Extended ones the multilingual model's languages write, so that a
letter a font lacks shows; in brackets, so that a string cut short shows,
and so does one with none, hard-coded or put together from pieces.
Placeholders, markup and plural forms stay as they are. For debug builds
only:

- ui/locales/en-XA.json, which a debug build's window takes for English
  when run with PKTITO_PSEUDO=1 (src-tauri/src/strings.rs)
- android/app/src/debug/res/values-en-rXA/strings.xml, the Android app's
  words in the en-XA locale, which only a debug build has

The iOS app's come from Android's, as its English does (scripts/strings-ios.py).

    scripts/pseudo.py
"""
import json
import math
import os
import re

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# a-z's twins from Polish, Czech, Hungarian, Romanian and Lithuanian, and
# with them accents above capitals (Ő Ű Ř) and below (Ą Ę Ș)
ACCENT = str.maketrans("acdeilnorstuyzACDEILNORSTUYZ", "ąčďęįłńőřșțűýżĄČĎĘĮŁŃŐŘȘȚŰÝŻ")

# what stays as it is: markup, entities and placeholders, Android's
# (%1$s, %d, %%, its backslash escapes) and the window's ({name})
PROTECT = re.compile(r"(<[^>]+>|&[A-Za-z0-9#]+;|%(?:\d+\$)?[sdf]|%%|\\u[0-9A-Fa-f]{4}|\\.|\{\w+\})")


def factor(n):
    return 2.0 if n <= 10 else 1.6 if n <= 20 else 1.4 if n <= 50 else 1.3


def padding(n):
    """n characters to lengthen a string by: a space, then dots in fives a
    space apart, which a line may break between"""
    if n <= 1:
        return "·" * max(n, 0)
    s = (" " + " ".join(["·····"] * (n // 6 + 1)))[:n]
    return s[:-1] + "·" if s.endswith(" ") else s


def pseudo(s):
    """s accented, bracketed, and as long as its length's factor makes it;
    the app's name as it is, which a translation keeps"""
    if s == "pktito":
        return s
    parts = PROTECT.split(s)
    text = sum(len(p) for p in parts[::2])
    if not text:
        return s
    body = "".join(p if i % 2 else p.translate(ACCENT) for i, p in enumerate(parts))
    return "[" + body + padding(math.ceil(text * factor(text)) - text - 2) + "]"


def desktop():
    src = os.path.join(root, "ui", "locales", "en.json")
    words = json.load(open(src, encoding="utf-8"))
    out = {k: (pseudo(v) if isinstance(v, str) else {q: pseudo(f) for q, f in v.items()}) for k, v in words.items()}
    dst = os.path.join(root, "ui", "locales", "en-XA.json")
    with open(dst, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=2)
        f.write("\n")
    print(f"ui/locales/en-XA.json: {len(out)} keys")


def android():
    src = os.path.join(root, "android", "app", "src", "main", "res", "values", "strings.xml")
    text = open(src, encoding="utf-8").read()

    def body(raw):
        # a CDATA string's markup inside it
        m = re.fullmatch(r"(\s*<!\[CDATA\[)(.*)(\]\]>\s*)", raw, re.S)
        return m.group(1) + pseudo(m.group(2)) + m.group(3) if m else pseudo(raw)

    out, n = [], 0
    for m in re.finditer(r'<string name="([^"]+)"([^>]*)>(.*?)</string>|<plurals name="([^"]+)">(.*?)</plurals>', text, re.S):
        if m.group(1):
            if 'translatable="false"' in m.group(2):
                continue
            out.append(f'    <string name="{m.group(1)}"{m.group(2)}>{body(m.group(3))}</string>')
        else:
            items = re.sub(r'(<item quantity="[^"]+">)(.*?)(</item>)', lambda i: i.group(1) + body(i.group(2)) + i.group(3),
                           m.group(5), flags=re.S)
            out.append(f'    <plurals name="{m.group(4)}">{items}</plurals>')
        n += 1
    dst = os.path.join(root, "android", "app", "src", "debug", "res", "values-en-rXA", "strings.xml")
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(dst, "w", encoding="utf-8") as f:
        f.write('<?xml version="1.0" encoding="utf-8"?>\n'
                "<!-- The Android app's words in scripts/pseudo.py's pseudo-language, for debug builds: made by it, not edited -->\n"
                "<resources>\n" + "\n".join(out) + "\n</resources>\n")
    print(f"android/app/src/debug/res/values-en-rXA/strings.xml: {n} strings")


if __name__ == "__main__":
    desktop()
    android()

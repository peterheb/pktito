#!/usr/bin/env python3
"""ios/App/Localizable.xcstrings, the iOS app's words, from the Android app's
(android/app/src/main/res/values/strings.xml), so that the two say the same
things in the same words and a translation of one is most of the other's:
each string by its Android name, with the comment above it for whoever
translates it; Android's arguments (%1$s, %1$d) as iOS's (%1$@, %1$lld); its
one plural as the catalog's plural; and its HTML (the strings in CDATA, as
the desktop's) as the Markdown SwiftUI sets, links, bold and italics. Then
the iOS app's own words (ios/strings.xml), in the same form. With
--pseudo, the pseudo-language's (scripts/pseudo.py) instead, as
ios/App/Pseudo/en-XA.json, which only a debug build has.

    scripts/strings-ios.py [--pseudo]
"""
import html
import json
import os
import re
import sys
import xml.etree.ElementTree as ET

here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# Android's words, then the iOS app's own (ios/strings.xml), in the same form
sources = [os.path.join(here, "android", "app", "src", "main", "res", "values", "strings.xml"),
           os.path.join(here, "ios", "strings.xml")]


def unescape(s):
    """Android's escapes in a string resource: \\' \\" \\n \\@ \\?"""
    s = re.sub(r"\\n", "\n", s)
    return re.sub(r"\\([\\'\"@?])", r"\1", s)


def args(s):
    """Android's arguments as Foundation's: a string %@, a number %lld"""
    s = re.sub(r"%(\d+\$)?s", lambda m: "%" + (m.group(1) or "") + "@", s)
    return re.sub(r"%(\d+\$)?d", lambda m: "%" + (m.group(1) or "") + "lld", s)


def markdown(s):
    """the HTML the desktop's strings hold, as Markdown; its entities as
    the characters they stand for (&nbsp; a no-break space)"""
    s = re.sub(r"<br\s*/?>", "\n", s)
    s = re.sub(r'<a href="([^"]+)">(.*?)</a>', r"[\2](\1)", s)
    s = re.sub(r"</?b>", "**", s)
    s = re.sub(r"</?i>", "*", s)
    if re.search(r"<[a-z/][^>]*>", s):
        raise ValueError("HTML Markdown can't say: " + s)
    return html.unescape(s)


def unit(v):
    return {"stringUnit": {"state": "translated", "value": v}}


def read(srcs):
    """Each string of srcs, (path, transform) pairs, by name: the comment
    before it, and its iOS value, or its plural's forms; transform takes
    each string's Android text first (the pseudo-language's, for one)."""
    found = {}
    for src, transform in srcs:
        # comments are not in ElementTree's tree: each string's is the last
        # one before it in the file
        text = open(src, encoding="utf-8").read()
        comment_before = {}
        last = None
        for m in re.finditer(r"<!--(.*?)-->|<(string|plurals) name=\"([^\"]+)\"", text, re.S):
            if m.group(1) is not None:
                last = " ".join(m.group(1).split())
            else:
                if last:
                    comment_before[m.group(3)] = last
                last = None
        for el in ET.fromstring(text):
            name = el.get("name")
            if name in found:
                raise ValueError(name + ": both Android's and iOS's")
            if el.tag == "string":
                at = text.index(f'name="{name}"')
                cdata = "<![CDATA[" in text[at:text.index("</string>", at)]
                v = args(unescape(transform(el.text or "")))
                found[name] = (comment_before.get(name), markdown(v) if cdata else v)
            elif el.tag == "plurals":
                found[name] = (comment_before.get(name),
                               {i.get("quantity"): args(unescape(transform(i.text or ""))) for i in el})
    return found


if "--pseudo" in sys.argv[1:]:
    # the pseudo-language (scripts/pseudo.py) for a debug build launched
    # with -pseudo: Android's, which pseudo.py writes, and the iOS app's own
    # words made into it here (UI/Controls.swift, L)
    sys.path.insert(0, os.path.join(here, "scripts"))
    from pseudo import pseudo
    found = read([(os.path.join(here, "android", "app", "src", "debug", "res", "values-en-rXA", "strings.xml"), lambda t: t),
                  (sources[1], pseudo)])
    dst = os.path.join(here, "ios", "App", "Pseudo", "en-XA.json")
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(dst, "w", encoding="utf-8") as f:
        json.dump({k: v for k, (_, v) in sorted(found.items())}, f, ensure_ascii=False, indent=2)
        f.write("\n")
    print("ios/App/Pseudo/en-XA.json:", len(found), "strings")
    sys.exit()

strings = {}
for name, (comment, v) in read([(src, lambda t: t) for src in sources]).items():
    entry = {"extractionState": "manual"}
    if comment:
        entry["comment"] = comment
    if isinstance(v, dict):
        entry["localizations"] = {"en": {"variations": {"plural": {q: unit(f) for q, f in v.items()}}}}
    else:
        entry["localizations"] = {"en": unit(v)}
    strings[name] = entry

out = {"sourceLanguage": "en", "strings": dict(sorted(strings.items())), "version": "1.0"}
dst = os.path.join(here, "ios", "App", "Localizable.xcstrings")
with open(dst, "w", encoding="utf-8") as f:
    json.dump(out, f, ensure_ascii=False, indent=2)
    f.write("\n")
print("ios/App/Localizable.xcstrings:", len(strings), "strings from Android's strings.xml and ios/strings.xml")

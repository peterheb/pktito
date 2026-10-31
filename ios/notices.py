#!/usr/bin/env python3
"""App/notices.json: what the iOS app is built from besides its own code and
the two models About names, and the licenses each is used under, for the
list that About's "Everything else" opens, as android/notices.py makes the
Android app's and scripts/notices.py the desktop's.

    ios/notices.py              # after Cargo.lock or a dependency changes
    ios/notices.py --check      # before a release: 1 if it is stale

The Rust crates are the ones Cargo says the iOS library links, taken as
scripts/notices.py takes the desktop's. The rest of the app is its own Swift
and Apple's frameworks, which iOS carries. The typefaces, the icons and
Rust's standard library are the desktop's, listed by hand there.
"""
import json
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(here)
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(root, "scripts"))
import notices as desk  # noqa: E402

IOS = {"ios": (8, ["aarch64-apple-ios", "aarch64-apple-ios-sim"])}
BY_HAND = ["Archivo", "Commit Mono", "Feather icons", "The Rust standard library"]
PATH = os.path.join(here, "App", "notices.json")


def check(out):
    """1, saying what differs, if notices.json is not what the build is made
    of now, as scripts/notices.py --check says of ui/notices.js; else 0."""
    with open(PATH, encoding="utf-8") as fh:
        had = json.load(fh)
    found = []
    for part in ("type", "libraries"):
        old, new = ({r[0]: r for r in x[part]} for x in (had, out))
        found += [f"{part[:-1] if part == 'type' else 'library'} {what}: {n}" for what, names in
                  (("added", new.keys() - old.keys()), ("removed", old.keys() - new.keys()),
                   ("changed", {n for n in old.keys() & new.keys() if old[n] != new[n]})) for n in sorted(names)]
    old, new = had["texts"], out["texts"]
    found += [f"text {what}: {k}" for what, keys in
              (("added", new.keys() - old.keys()), ("removed", old.keys() - new.keys()),
               ("changed", {k for k in old.keys() & new.keys() if old[k] != new[k]})) for k in sorted(keys)]
    for f in found:
        print(f)
    print(f"{os.path.relpath(PATH, root)}: " + ("stale, run ios/notices.py" if found else "current"))
    return 1 if found else 0


def main():
    data, unknown = desk.notices(IOS, [e for e in desk.EXTRAS if e[1] in BY_HAND])
    out = {
        # [name, license, notice, the keys of its texts]
        "type": [[e[1], e[3], e[2], e[5]] for e in data["extras"] if e[0] == "type"],
        # the same and whether its name is code: by hand, then Cargo's
        "libraries": [[e[1], e[3], e[2], e[5], False] for e in data["extras"] if e[0] == "code"]
        + [[f"{c[0]} {c[1]}", c[2], c[3], c[5], True] for c in data["crates"]],
        "texts": {},
    }
    used = [k for row in out["type"] + out["libraries"] for k in row[3]]
    out["texts"] = {k: data["texts"][k] for k in dict.fromkeys(used)}
    if "--check" in sys.argv[1:]:
        sys.exit(check(out))
    js = json.dumps(out, ensure_ascii=False, separators=(",", ":")) + "\n"
    with open(PATH, "w", encoding="utf-8") as fh:
        fh.write(js)
    print(f"{os.path.relpath(PATH, root)}: {len(js.encode()) / 1024:.0f} KB, {len(data['crates'])} crates, "
          f"{len(out['texts'])} texts")
    for u in unknown:
        print("  a license not in PREFER, check it:", u)


if __name__ == "__main__":
    main()

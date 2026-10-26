#!/usr/bin/env python3
"""app/src/main/assets/notices.json: what the Android app is built from
besides its own code and the two models About names, and the licenses each
is used under, for the list that About's "Everything else" opens, as
scripts/notices.py makes ui/notices.js for the desktop's.

    android/notices.py      # after Cargo.lock or a dependency changes

The Rust crates are the ones Cargo says the Android library links, taken as
scripts/notices.py takes the desktop's. The Kotlin side is what Gradle packages
into a release build (app/build.gradle.kts, the task "packaged"): each
module that resolves to a file, its license from its POM (or its parent's)
and the license files it carries, its copyright from those files, else its
POM's developers. The typefaces, the icons and Rust's standard library are
the desktop's, listed by hand there. Gradle needs what a build does:
JAVA_HOME and ANDROID_HOME, set as for building the app.
"""
import glob
import io
import json
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
import zipfile

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(here)
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(root, "scripts"))
import notices as desk  # noqa: E402

ANDROID = {"android": (8, ["aarch64-linux-android", "x86_64-linux-android"])}
BY_HAND = ["Archivo", "Commit Mono", "Feather icons", "The Rust standard library"]
# a POM's name for a license, as SPDX's
LICENSES = {
    "The Apache Software License, Version 2.0": "Apache-2.0",
    "The Apache License, Version 2.0": "Apache-2.0",
    "Apache License, Version 2.0": "Apache-2.0",
    "Apache-2.0": "Apache-2.0",
}
GRADLE = os.path.expanduser("~/.gradle/caches/modules-2/files-2.1")
POM = {"m": "http://maven.apache.org/POM/4.0.0"}


def pom(group, module, version):
    found = glob.glob(os.path.join(GRADLE, group, module, version, "*", f"{module}-{version}.pom"))
    if not found:
        sys.exit(f"{group}:{module}:{version}: no POM in Gradle's cache")
    return ET.parse(found[0]).getroot()


def texts_of(e, path):
    return [x.text.strip() for x in e.findall(path, POM) if x.text and x.text.strip()]


def licenses(group, module, version):
    """A module's licenses by SPDX id, from its POM or the nearest parent's
    that names them, and its developers or organization."""
    p = pom(group, module, version)
    names = texts_of(p, "m:licenses/m:license/m:name")
    who = texts_of(p, "m:developers/m:developer/m:name") or texts_of(p, "m:organization/m:name")
    parent = p.find("m:parent", POM)
    if not names and parent is not None:
        names, up = licenses(*(texts_of(parent, f"m:{k}")[0] for k in ("groupId", "artifactId", "version")))
        who = who or up
    return names, who


def license_files(path):
    """The license and notice files in a module's .jar or .aar, and in an
    .aar's classes.jar, each (its path, its text)."""
    z = zipfile.ZipFile(path)
    found = [(n, z) for n in z.namelist()]
    if path.endswith(".aar") and "classes.jar" in z.namelist():
        inner = zipfile.ZipFile(io.BytesIO(z.read("classes.jar")))
        found += [(n, inner) for n in inner.namelist()]
    return [(n, zz.read(n).decode("utf-8", "replace").replace("\r\n", "\n"))
            for n, zz in found if desk.FILE.match(n.rsplit("/", 1)[-1]) and not n.endswith("/")]


def packaged():
    """What Gradle packages into a release build: group:module:version, file."""
    subprocess.run(["./gradlew", "-q", ":app:packaged"], cwd=here, check=True)
    with open(os.path.join(here, "app", "build", "notices", "packaged.txt"), encoding="utf-8") as fh:
        return [line.rstrip("\n").split("\t") for line in fh if line.strip()]


def main():
    data, unknown = desk.notices(ANDROID, [e for e in desk.EXTRAS if e[1] in BY_HAND])
    texts = data["texts"]
    maven = []
    for coord, path in packaged():
        group, module, version = coord.split(":")
        names, who = licenses(group, module, version)
        ids = [LICENSES.get(n) for n in names]
        if not ids or None in ids:
            sys.exit(f"{coord}: licensed as {names or 'nothing its POM says'}; add it to LICENSES or by hand")
        keys, lines = list(dict.fromkeys(ids)), []
        for rel, t in license_files(path):
            k, standard = desk.kind(t)
            if standard and k in keys:
                lines += desk.copyrights(desk.split_terms(t)[1] if k == "Apache-2.0" else t)
            else:
                # a text of its own, as it is
                key = f"{coord}/{rel}"
                texts.setdefault(key, {"title": f"{group}:{module}: {rel}", "text": t.strip() + "\n"})
                keys.append(key)
        for k in ids:
            if k not in texts:
                texts[k] = {"title": desk.TITLES[k], "text": desk.read(os.path.join(desk.LIC, k + ".txt")).strip() + "\n"}
        lines = list(dict.fromkeys(lines))
        notice = "; ".join(lines) if lines else "by " + ", ".join(who or [f"the {module} authors"])
        maven.append([f"{group}:{module} {version}", " and ".join(dict.fromkeys(ids)), notice, keys, True])

    out = {
        # [name, license, notice, the keys of its texts]
        "type": [[e[1], e[3], e[2], e[5]] for e in data["extras"] if e[0] == "type"],
        # the same and whether its name is code: by hand, then Gradle's, then Cargo's
        "libraries": [[e[1], e[3], e[2], e[5], False] for e in data["extras"] if e[0] == "code"]
        + maven + [[f"{c[0]} {c[1]}", c[2], c[3], c[5], True] for c in data["crates"]],
        "texts": {},
    }
    used = [k for row in out["type"] + out["libraries"] for k in row[3]]
    out["texts"] = {k: texts[k] for k in dict.fromkeys(used)}
    path = os.path.join(here, "app", "src", "main", "assets", "notices.json")
    js = json.dumps(out, ensure_ascii=False, separators=(",", ":")) + "\n"
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(js)
    print(f"{path}: {len(js.encode()) / 1024:.0f} KB, {len(maven)} Maven modules, {len(data['crates'])} crates, "
          f"{len(out['texts'])} texts")
    for u in unknown:
        print("  a license not in PREFER, check it:", u)


if __name__ == "__main__":
    main()

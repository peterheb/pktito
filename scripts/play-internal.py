#!/usr/bin/env python3
"""Uploads an Android App Bundle to Google Play's internal testing track and
releases it there to the testers, through the Android Publisher API:

    python3 scripts/play-internal.py APP.aab [--notes NOTES.txt]

One edit holds the whole change: the bundle is uploaded into it, the
internal track is set to that bundle's version code, completed (every
internal tester gets it), and the edit is committed; until the commit Play
shows nothing, and a failure on the way leaves the track as it was. The
notes, if given, are the release's "What's new" in en-US, 500 characters
at most.

The service account's key is read from PLAY_KEY_FILE (by default
~/.config/pktito/play-upload.json), by its path only: it reaches no command
line or log. The account is meant to have Play Console's "Release apps to
testing tracks" for this app and nothing more; this script names only the
internal track, and production stays a person's step. Needs google-auth and
requests (pip install google-auth requests); a 403 right after the account
is granted access is Play's delay in passing it on, which can take a day.
"""

import argparse
import os
import sys

from google.auth.transport.requests import AuthorizedSession
from google.oauth2 import service_account

PACKAGE = "net.pktito.app"
TRACK = "internal"
SCOPE = "https://www.googleapis.com/auth/androidpublisher"
API = f"https://androidpublisher.googleapis.com/androidpublisher/v3/applications/{PACKAGE}"
UPLOAD = f"https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications/{PACKAGE}"


def call(session, method, url, **kw):
    r = session.request(method, url, timeout=600, **kw)
    if not r.ok:
        sys.exit(
            f"play-internal: {method} {url.split(PACKAGE)[-1] or '/'}: {r.status_code} {r.text[:800]}"
        )
    return r.json() if r.content else {}


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("aab")
    ap.add_argument("--notes", help="a text file: the release's What's new (en-US)")
    a = ap.parse_args()

    key = os.environ.get("PLAY_KEY_FILE") or os.path.expanduser(
        "~/.config/pktito/play-upload.json"
    )
    creds = service_account.Credentials.from_service_account_file(key, scopes=[SCOPE])
    s = AuthorizedSession(creds)

    notes = None
    if a.notes:
        with open(a.notes, encoding="utf-8") as f:
            notes = f.read().strip()
        if len(notes) > 500:
            sys.exit(
                f"play-internal: {a.notes}: {len(notes)} characters, Play takes 500"
            )

    edit = call(s, "POST", f"{API}/edits")["id"]
    with open(a.aab, "rb") as f:
        bundle = call(
            s,
            "POST",
            f"{UPLOAD}/edits/{edit}/bundles?uploadType=media",
            data=f,
            headers={"Content-Type": "application/octet-stream"},
        )
    code = bundle["versionCode"]
    print(f"uploaded {a.aab}: versionCode {code}, sha256 {bundle.get('sha256')}")

    release = {"versionCodes": [str(code)], "status": "completed"}
    if notes:
        release["releaseNotes"] = [{"language": "en-US", "text": notes}]
    call(
        s,
        "PUT",
        f"{API}/edits/{edit}/tracks/{TRACK}",
        json={"track": TRACK, "releases": [release]},
    )
    call(s, "POST", f"{API}/edits/{edit}:commit")
    print(f"{TRACK}: versionCode {code} released to its testers")


if __name__ == "__main__":
    main()

# pktito.net

The site: four pages and a 404 that `build.py` writes into `public/`, in the
design system the app uses (`design/tokens.json`, `design/bundle.css`,
`ui/fonts`), served by a Cloudflare Worker (`worker.js`) that also hands out
the installers from R2 and counts their downloads.

```sh
python3 site/build.py           # site/public
cd site && npx wrangler@4 dev   # http://localhost:8787
cd site && npx wrangler@4 deploy
```

The first deploy needs the bucket and the database made, and the
database's id in `wrangler.jsonc`:

```sh
cd site
npx wrangler@4 login
npx wrangler@4 r2 bucket create pktito-releases
npx wrangler@4 d1 create pktito-stats    # prints the id
npx wrangler@4 d1 migrations apply pktito-stats --remote
```

`python3 site/build.py --artifact DIR` writes a preview with relative links
instead, as the claude.ai artifact of it is published.

## Installers

The download page links to `/download/macos`, `/download/windows`,
`/download/linux-deb` and `/download/linux-appimage`, which stay put from
release to release. The Worker redirects each to the file that
`latest.json`, in the R2 bucket `pktito-releases`, names for it, and serves
`/download/<version>/<file>` from `<version>/<file>` there, resumable and
cached for a year. A system `latest.json` does not name, as before the
first release, goes to `/soon`.

Posting a release is its files first, then `latest.json`:

```sh
npx wrangler@4 r2 object put pktito-releases/0.1.0/pktito_0.1.0_aarch64.dmg --file pktito_0.1.0_aarch64.dmg --remote
npx wrangler@4 r2 object put pktito-releases/latest.json --file latest.json --content-type application/json --remote
```

```json
{
  "version": "0.1.0",
  "files": {
    "macos": "pktito_0.1.0_aarch64.dmg",
    "windows": "pktito_0.1.0_x64-setup.exe",
    "linux-deb": "pktito_0.1.0_amd64.deb",
    "linux-appimage": "pktito_0.1.0_amd64.AppImage"
  }
}
```

## Download counts

A download of an installer that starts adds one to its file's count for
the day (UTC), in the D1 database `pktito-stats`: the counts and nothing
else, no address and nothing that tells one visitor from another. A
resumed download, a HEAD, a probe of a few bytes and a crawler's fetch
are not counted.

```sh
npx wrangler@4 d1 execute pktito-stats --remote --command \
  "SELECT version, file, SUM(n) AS downloads FROM downloads GROUP BY version, file"
```

## Models

models.pktito.net is bunny.net and needs no Worker: a storage zone holding
the tree `scripts/models-tree.sh` lays out, behind a pull zone with
models.pktito.net as its hostname (a CNAME in Cloudflare's DNS, not
proxied, so that bunny can issue its certificate). Each file is named by
its contents, so the pull zone can keep it forever. The app resumes a
download with a range request, which the pull zone answers.

`scripts/models-upload.sh` uploads them to the storage zone; bunny checks
each against its SHA-256, and the script checks what the zone then holds.
The zone's password goes in a file, never on a command line:

```sh
scripts/models-upload.sh storage.bunnycdn.com ZONE ../pktito/models/*.pkf
```

## Screenshots

One standard shot of the app, for the home page: LibriVox's _Alice's
Adventures in Wonderland_, chapter 1 (`wonderland_ch_01_64kb.mp3`,
archive.org's `alice_in_wonderland_librivox`), transcribed by the
multilingual model, the window at its first size, scrolled past the
recording's LibriVox notice so that "Alice's Adventures in Wonderland by
Lewis Carroll." is the first row. Six of it, light and dark on each system,
in `site/screenshots/`; a visitor sees their own system's, a phone its
nearest (Android the Windows one, iPhone the Mac's), Linux the Linux one.

| System  | Taken on              | How                                            | Files                               |
| ------- | --------------------- | ---------------------------------------------- | ----------------------------------- |
| macOS   | the Mac               | `scripts/screenshots-mac.sh`                   | `mac-light.png`, `mac-dark.png`     |
| Windows | the 9950X, x64        | not yet: the same shot, the window's own frame | `win-light.png`, `win-dark.png`     |
| Linux   | pi5a's desktop, labwc | not yet: the same shot, under its title bar    | `linux-light.png`, `linux-dark.png` |

Why the multilingual model and the MP3: the English model runs the notice,
the title and "Chapter one" together as one row, so no row is the title;
and the first 16 rows of the MP3 are the same through macOS's decoder and
ffmpeg, Linux's (2026-10-23, 0.8.5). Windows' Media Foundation is still to
be compared.

The Mac's script quits pktito, sets its settings for the shot and puts them
back, opens the chapter (downloaded once and checked), scrolls three rows
by simulated trackpad pixels, and captures the window with macOS's shadow in
light and then dark, the system's appearance put back after. It needs
Screen Recording and Accessibility for the terminal it runs in. Retake it
after any change to the window's look; if the row spacing changes, its
`SCROLL` (three one-line rows, 43 px each) changes with it. The Mac's
display is 2560×1440 at 1×, so its shots are 1× (792×912 with the shadow); a
Retina display would give 2×.

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
{"version": "0.1.0", "files": {"macos": "pktito_0.1.0_aarch64.dmg",
  "windows": "pktito_0.1.0_x64-setup.exe", "linux-deb": "pktito_0.1.0_amd64.deb",
  "linux-appimage": "pktito_0.1.0_amd64.AppImage"}}
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

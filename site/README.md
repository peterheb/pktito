# pktito.net

The site: four pages and a 404 that `build.py` writes into `public/`, in the
design system the app uses (`design/tokens.json`, `design/bundle.css`,
`ui/fonts`), served by a Cloudflare Worker (`worker.js`) that also hands out
the installers from R2.

```sh
python3 site/build.py           # site/public
cd site && npx wrangler@4 dev   # http://localhost:8787
cd site && npx wrangler@4 deploy
```

`python3 site/build.py --artifact DIR` writes a preview with relative links
instead, as the claude.ai artifact of it is published.

## Installers

The download page links to `/download/macos`, `/download/windows`,
`/download/linux-deb` and `/download/linux-appimage`, which stay put from
release to release. The Worker redirects each to the file that
`latest.json`, in the R2 bucket `pktito-releases`, names for it, and serves
`/download/<version>/<file>` from `<version>/<file>` there, resumable and
cached for a year. A system `latest.json` does not name gets the 404 page.

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

## Models

models.pktito.net is separate and needs no Worker: the tree
`scripts/models-tree.sh` lays out, on any static host that sends
`Cache-Control: public, max-age=31536000, immutable`; an R2 bucket with
models.pktito.net as its custom domain is one. The files are over 600 MB,
so upload them with an S3 client that sends a file in parts, such as rclone.

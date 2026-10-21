#!/bin/bash
# Uploads .pkf files to the bunny.net storage zone behind models.pktito.net,
# each under the first twelve digits of its SHA-256, as models-tree.sh lays
# them out and the app asks for them (src-tauri/src/model.rs, download_url):
#
#     scripts/models-upload.sh STORAGE_HOST ZONE MODEL.pkf...
#
# STORAGE_HOST is the zone's storage endpoint (storage.bunnycdn.com, or
# ny.storage.bunnycdn.com and the like: FTP & API Access in the dashboard),
# or a whole base URL, as for a test server.
# The zone's password is read from BUNNY_STORAGE_KEY_FILE (by default
# ~/.config/pktito/bunny-storage.key) and handed to curl on its standard
# input, so it reaches no command line. bunny refuses an upload whose
# bytes do not hash to the Checksum sent with them; once one is in, the
# zone's listing is read back for its size and SHA-256.
set -euo pipefail
[ $# -ge 3 ] || { echo "usage: $0 STORAGE_HOST ZONE MODEL.pkf..." >&2; exit 2; }
host=$1 zone=$2
shift 2
case $host in *://*) base=$host ;; *) base=https://$host ;; esac
keyfile=${BUNNY_STORAGE_KEY_FILE:-$HOME/.config/pktito/bunny-storage.key}
key=$(tr -d ' \r\n' < "$keyfile")
sha() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -c1-64; }
auth() { printf 'header = "AccessKey: %s"\n' "$key"; }
for f in "$@"; do
    h=$(sha "$f")
    name=$(basename "$f")
    size=$(wc -c < "$f" | tr -d ' ')
    url="$base/$zone/${h:0:12}/$name"
    t0=$(date +%s)
    { auth; printf 'header = "Checksum: %s"\n' "$(tr a-f A-F <<< "$h")"; } |
        curl --fail-with-body -sS -K - -T "$f" -H "Content-Type: application/octet-stream" "$url" > /dev/null
    # the listing of its folder: [{"ObjectName": ..., "Length": ..., "Checksum": ...}]
    listed=$(auth | curl --fail-with-body -sS -K - "$base/$zone/${h:0:12}/")
    python3 -c '
import json, sys
name, size, h, listed = sys.argv[1], int(sys.argv[2]), sys.argv[3], json.loads(sys.argv[4])
o = next((o for o in listed if o["ObjectName"] == name), None)
held = (o["Length"], (o.get("Checksum") or "").lower()) if o else None
if held != (size, h):
    sys.exit(name + ": the zone holds " + str(held) + ", not " + str(size) + " bytes with SHA-256 " + h)
' "$name" "$size" "$h" "$listed"
    printf '%s/%s  %s bytes, SHA-256 checked, %ss\n' "${h:0:12}" "$name" "$size" "$(( $(date +%s) - t0 ))"
done

#!/bin/bash
# Lays out .pkf files as models.pktito.net serves them, each under the first
# twelve digits of its SHA-256 (src-tauri/src/model.rs, download_url):
#
#     scripts/models-tree.sh DIR/*.pkf OUT
#
# OUT is then what to upload, or to serve while testing:
#
#     (cd OUT && python3 -m http.server 8765) &
#     PKTITO_MODEL_BASE=http://127.0.0.1:8765/ cargo tauri dev
#
# Every file is named by its contents, so the server can tell a CDN to keep
# each one forever (Cache-Control: public, max-age=31536000, immutable).
set -euo pipefail
[ $# -ge 2 ] || { echo "usage: $0 MODEL.pkf... OUT" >&2; exit 2; }
out=${!#}
sha() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -c1-64; }
for f in "${@:1:$#-1}"; do
    h=$(sha "$f")
    mkdir -p "$out/${h:0:12}"
    ln -f "$f" "$out/${h:0:12}/$(basename "$f")" 2>/dev/null || cp "$f" "$out/${h:0:12}/"
    printf '%s  %s/%s  %s bytes\n' "$h" "${h:0:12}" "$(basename "$f")" "$(wc -c < "$f" | tr -d ' ')"
done

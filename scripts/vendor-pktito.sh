#!/bin/sh
# Copy pktito's library sources (the C engine, no CLI, no tools) into
# vendor/pktito, and record which commit they came from. The app builds
# from this copy alone; run this again to take a newer pktito.
#
#   scripts/vendor-pktito.sh [PKTITO_CHECKOUT]
#
# The checkout defaults to ../libpktito, then ../pkito.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
src=${1:-}
if [ -z "$src" ]; then
  for d in "$here/../libpktito" "$here/../pkito"; do
    [ -f "$d/core/pktito.h" ] && src=$d && break
  done
fi
[ -n "$src" ] && [ -f "$src/core/pktito.h" ] || { echo "no pktito checkout found; pass its path" >&2; exit 1; }
src=$(cd "$src" && pwd)
dst="$here/vendor/pktito"
rm -rf "$dst"
mkdir -p "$dst/core" "$dst/backend"
cp "$src"/core/*.c "$src"/core/*.h "$dst/core/"
rm -f "$dst/core/cli.c"
cp "$src/backend/kernels.h" "$src/backend/kernels.c" "$dst/backend/"
for b in "$src"/backend/*/; do
  name=$(basename "$b")
  mkdir -p "$dst/backend/$name"
  cp "$b"*.c "$dst/backend/$name/"
done
cp "$src/LICENSE" "$dst/LICENSE"
commit=$(git -C "$src" rev-parse --short HEAD 2>/dev/null || echo unknown)
dirty=$(git -C "$src" status --porcelain -- core backend 2>/dev/null | head -1)
printf 'pktito %s%s, copied %s from %s\n' "$commit" "${dirty:+ (with uncommitted changes)}" "$(date -u +%Y-%m-%d)" "$src" > "$dst/VENDORED"
echo "vendored pktito $commit into vendor/pktito"

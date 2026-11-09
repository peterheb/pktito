#!/bin/sh
# Copy pktito's library sources (the C engine and its NPU encoders, no CLI,
# no tools) into vendor/pktito, with models.json, the catalog of model files
# that build.rs makes the app's editions from, and tests/editions.tsv, what
# each known device runs, which the app's tests hold editions.rs to, with
# tests/editions-rules.json and .tsv, the made-up catalog that holds the
# rules for preview and too-new editions. Record which commit they came
# from. The app builds from this copy alone; run this
# again to take a newer pktito.
#
#   scripts/vendor-pktito.sh [PKTITO_CHECKOUT [DEST]]
#
# The checkout defaults to ../libpktito, then ../pktito; DEST to
# vendor/pktito.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
src=${1:-}
if [ -z "$src" ]; then
  for d in "$here/../libpktito" "$here/../pktito"; do
    [ -f "$d/core/pktito.h" ] && src=$d && break
  done
fi
[ -n "$src" ] && [ -f "$src/core/pktito.h" ] || { echo "no pktito checkout found; pass its path" >&2; exit 1; }
src=$(cd "$src" && pwd)
dst="$here/${2:-vendor/pktito}"
rm -rf "$dst"
mkdir -p "$dst/core" "$dst/backend" "$dst/npu" "$dst/tests"
cp "$src"/core/*.c "$src"/core/*.h "$dst/core/"
# pktito-c's own, not the library's: its command line, and its reading of
# models.json (the app's is editions.rs)
rm -f "$dst/core/cli.c" "$dst/core/catalog.c" "$dst/core/editions.c" "$dst/core/editions.h"
# the encoders on the Neural Engine (ane.m, through Core ML) and on a
# Snapdragon's NPU (qnn.c, through QNN), which build.rs compiles only where
# each runs
cp "$src"/npu/*.h "$src"/npu/*.c "$src"/npu/*.m "$dst/npu/"
cp "$src/backend/kernels.h" "$src/backend/kernels.c" "$dst/backend/"
for b in "$src"/backend/*/; do
  name=$(basename "$b")
  mkdir -p "$dst/backend/$name"
  cp "$b"*.c "$dst/backend/$name/"
done
cp "$src/models.json" "$dst/models.json"
cp "$src/tests/editions.tsv" "$src/tests/editions-rules.json" "$src/tests/editions-rules.tsv" "$dst/tests/"
cp "$src/LICENSE" "$dst/LICENSE"
commit=$(git -C "$src" rev-parse --short HEAD 2>/dev/null || echo unknown)
dirty=$(git -C "$src" status --porcelain -- core backend npu models.json tests/editions.tsv tests/editions-rules.json tests/editions-rules.tsv 2>/dev/null | head -1)
printf 'pktito %s%s, copied %s from %s\n' "$commit" "${dirty:+ (with uncommitted changes)}" "$(date -u +%Y-%m-%d)" "$src" > "$dst/VENDORED"
echo "vendored pktito $commit into ${2:-vendor/pktito}"

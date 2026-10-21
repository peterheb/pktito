#!/bin/bash
# Runs the app over a directory of media (scripts/media-matrix.sh makes
# one) and scores each transcript against the source recording's:
#
#     scripts/media-check.sh APP MODEL SPEECH.wav DIR
#
# One line a file, tab-separated: the file; ok, or what the app said; the
# seconds of audio decoded; and the word error rate against the source's
# transcript, in percent. PKTITO_DECODER picks the decoder as in the app.
set -uo pipefail
[ $# -eq 4 ] || { echo "usage: $0 APP MODEL SPEECH.wav DIR" >&2; exit 2; }
app=$1 model=$2 src=$3 dir=$4
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# a transcript's words, one a line, in lower case, without punctuation
words() { sed -E 's/^ *[0-9.]+  //' | tr '[:upper:]' '[:lower:]' | tr -cs "[:alnum:]'" '\n' | sed '/^$/d'; }

# the word error rate of the words in $2 against those in $1: the fewest
# substitutions, insertions and deletions, over the reference's length
wer() {
    # n and m start at 0 so that an empty transcript's d[m] is d[0]: a
    # variable never set is "" as an array index, and d[""], read as 0,
    # once scored a file that gave no words at all as 0%
    awk 'BEGIN { n = 0; m = 0 } NR == FNR { r[++n] = $0; next } { h[++m] = $0 }
         END {
             for (j = 0; j <= m; j++) d[j] = j
             for (i = 1; i <= n; i++) {
                 diag = d[0]; d[0] = i
                 for (j = 1; j <= m; j++) {
                     up = d[j]; c = diag + (r[i] != h[j])
                     if (up + 1 < c) c = up + 1
                     if (d[j - 1] + 1 < c) c = d[j - 1] + 1
                     d[j] = c; diag = up
                 }
             }
             printf "%.1f", n ? 100 * d[m] / n : 0
         }' "$1" "$2"
}

"$app" --transcribe "$src" --model "$model" 2>/dev/null | words > "$tmp/ref"
[ -s "$tmp/ref" ] || { echo "$0: the app could not transcribe $src" >&2; exit 1; }
for f in "$dir"/*; do
    name=$(basename "$f")
    if "$app" --transcribe "$f" --model "$model" > "$tmp/out" 2> "$tmp/err"; then
        secs=$(sed -nE 's/.* ([0-9.]+) s of audio.*/\1/p' "$tmp/err")
        words < "$tmp/out" > "$tmp/hyp"
        printf '%s\tok\t%s\t%s\n' "$name" "$secs" "$(wer "$tmp/ref" "$tmp/hyp")"
    else
        printf '%s\t%s\t\t\n' "$name" "$(tail -n 1 "$tmp/err" | sed 's/^pktito: //')"
    fi
done

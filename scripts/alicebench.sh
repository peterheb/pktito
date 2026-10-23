#!/bin/sh
# AliceBench v1: how fast a computer transcribes, as one number. LibriVox's
# Alice's Adventures in Wonderland (archive.org, alice_in_wonderland_librivox,
# public domain), its twelve chapters, 2:58:51 in all, timed by pktito-cli
# bench with the English model (parakeet-tdt-0.6b-v2); the result is the
# audio's length over the time it took.
#
#     scripts/alicebench.sh [DIR]
#
# DIR keeps the chapters (default ~/.cache/alicebench-v1); any missing, or
# not the bytes below, are downloaded, 200 MB in all. PKTITO_CLI names the
# pktito-cli to time, else the one on the PATH, else the Mac app's.
#
# The chapters are FLAC, 16 kHz mono, made from LibriVox's 64 kbps MP3s with
#     ffmpeg -i wonderland_ch_NN_64kb.mp3 -map 0:a -map_metadata -1 \
#         -fflags +bitexact -flags:a +bitexact -ac 1 -ar 16000 \
#         -sample_fmt s16 -c:a flac wonderland_ch_NN.flac
# and not the MP3s themselves: each OS's MP3 decoder gives other samples
# (macOS's and ffmpeg's, chapter 11: 22 ms apart in length, 5 words of
# 1937 different), where a FLAC reads the same through all of them (macOS,
# Windows' Media Foundation on Arm64 and ffmpeg: the same transcript, byte
# for byte, 2026-10-23). So each chapter's transcript is also checked
# against the one every machine gives, which says this pktito computes
# what the reference engine does; the line below each is the SHA-256 of
# the file and of its transcript (pktito-cli bench --json's).
set -eu

base=https://models.pktito.net/alicebench/v1
chapters='
wonderland_ch_01.flac 8ac53c4d56166846351d58f3f6a26e603f10e0b1ce6e7a3b22f106e0a754c4c7 420b1701aa22b14ae761476d9394889dbb32307becd4efd389cebfbffa923128
wonderland_ch_02.flac 5210f2d89843a94a42555228406ca75672ff1ebb4731c0d07b75c5125cc2f4dc 9b9c182116e0c1e302298cf35a6f95d568ee28ef37d8c6aa2500a4445816da05
wonderland_ch_03.flac 3a7bc0fd543b47daa9eccaef002be75716c5fd59cae9ea1390247b3eb1895808 502be4285a107012dfcd86373e6623aa01ba0f842f2b189c8c4e794b07fc8a29
wonderland_ch_04.flac b0cc8de92e8875d4ce85a3f647aa77e17faea65de6ca490d7fff756bba228e8a 2eb60b8d7451ffaf2edec1153bc251051f3b1348cc6e7fd3fef2b64ed0231354
wonderland_ch_05.flac dc2d1865e72aef0023da36bfdaedb824313611961306e2c409592636f7127ca6 3fbcb15dbb16a47449618d283e2e1de0b96477ee32279fd3b179e33b535a0643
wonderland_ch_06.flac 56248ff5002a7d20fd149bc1ccc2f2117cc330d2940d2fb79f707a28f8e18347 f0170d1b5162a1a7efb6492e991eb17782753d8785357ab2bb9fc0f0fe93e1bb
wonderland_ch_07.flac a75e21ed3ad05f9b2d12cb4208577ae8e4f8872892ec4478446542ab1b7009a9 4c5aff13c3eafa3c080d1ca38bf48ed02ff474989c94ef0ad78a0e1b5cb9d9a4
wonderland_ch_08.flac c89cd244315f2ffa5cbcf6509891d7e510d30092e9bfaf69d4d09bb803f3970f d0f40e98ee30f34a4e1efb66f7608c4b74199d4f50e25ada12f3c0fde6fb1548
wonderland_ch_09.flac d8c0a03d1d3482d26244484e9c4a5361bb50e80cf2e1b3b11939fe16c164bb14 8d4180ce60b8c69b368a29953adf7fb5bd6f8b0972ec7c8498ab5717b786b8b5
wonderland_ch_10.flac 721f1aa64865cb18082a88c66ff542cdcdd0628f794c4b35fbbc839a837858bc a480673ea85f2728d719c92eddecacce14757b7c073aaa95f946714734ff3ac3
wonderland_ch_11.flac 09d0a325f913069b53ed23f02dd8f17f76027518c0a0b89f1e14b10f5ec13d69 a770e46c49181c3511d2863201cb3507b9ff6dc149576fcf5dbd5cd43658c7fc
wonderland_ch_12.flac c7a4a9d9cd8fd1aae1d4abbc05289920074dfa9f845650e4a5773e959c997852 9a2d63b4ff85f2df4ed5daa9f01853df7e3f692259b02b01a223f80ae49c2624
'
dir=${1:-$HOME/.cache/alicebench-v1}

cli=${PKTITO_CLI:-$(command -v pktito-cli || true)}
[ -n "$cli" ] || cli=/Applications/pktito.app/Contents/MacOS/pktito-cli
[ -x "$cli" ] || { echo "alicebench: no pktito-cli; set PKTITO_CLI to one" >&2; exit 2; }

if command -v sha256sum >/dev/null; then sha() { sha256sum "$1" | cut -d' ' -f1; }
else sha() { shasum -a 256 "$1" | cut -d' ' -f1; }; fi

mkdir -p "$dir"
files=
echo "$chapters" | while read -r name want _; do
    [ -n "$name" ] || continue
    f=$dir/$name
    [ -f "$f" ] && [ "$(sha "$f")" = "$want" ] && continue
    echo "alicebench: downloading $name" >&2
    curl -fsSL -o "$f.part" "$base/$name"
    [ "$(sha "$f.part")" = "$want" ] || { echo "alicebench: $name isn't AliceBench v1's" >&2; exit 1; }
    mv "$f.part" "$f"
done
for name in $(echo "$chapters" | cut -d' ' -f1); do files="$files $dir/$name"; done

"$cli" download parakeet-tdt-0.6b-v2 >/dev/null
out=$dir/result.json
# shellcheck disable=SC2086
"$cli" bench -m parakeet-tdt-0.6b-v2 --json $files > "$out"

# each chapter's speed and whether its transcript is the reference's, from
# bench's JSON (one key a line), against the list above; then the whole's
echo "$chapters" | awk '
    NR == FNR { if ($1 != "") ref[$1] = $3; next }
    function val(s) { sub(/^[^:]*: */, "", s); sub(/,$/, "", s); gsub(/"/, "", s); return s }
    /"file":/ { file = val($0) }
    /"x_real_time":/ && file != "" { x = val($0) }
    /"transcript_sha256":/ { sha = val($0) }
    /^    }/ { ok = sha == ref[file] ? "" : "  transcript differs"; if (!ok) same++; else differ++
               printf "%7.1fx  %s%s\n", x, file, ok; got++; file = "" }
    /^  "backend":/ { backend = val($0) } /^  "model":/ { model = val($0) }
    /^  "pktito_cli":/ { version = val($0) } /^  "threads":/ { threads = val($0) }
    /^  "x_real_time":/ { total = val($0) }
    END {
        printf "AliceBench v1: %.1fx real time (pktito-cli %s, %s, %s, %s threads)\n", total, version, model, backend, threads
        if (got != 12) { print "  only " got " of the 12 chapters were timed"; exit 1 }
        if (differ) { print "  " differ " of the 12 transcripts differ from the reference: not what pktito should compute"; exit 1 }
        print "  all 12 transcripts are the reference'"'"'s"
    }' - "$out"

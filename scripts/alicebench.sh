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
# not the bytes below, are downloaded: 200 MB of FLAC, or on Linux 330 MB of
# WAV, which pktito-cli reads there without ffmpeg (ALICEBENCH_FORMAT=flac
# or wav chooses). PKTITO_CLI names the pktito-cli to time, else the one on
# the PATH, else the Mac app's.
#
# The model is AliceBench's own copy, in DIR/models, apart from the app's
# models folder, which the app may change (from 0.9.7 it deletes the whole
# .pkf once the Neural Engine or an NPU has run from the slim one,
# docs/EDITIONS.md): the whole .pkf everywhere, and on a Mac also the Neural
# Engine's slim file and encoder, each downloaded once and again only when
# the catalog's changes (pktito-cli download --to). A Mac times the Neural
# Engine's, as the app runs; ALICEBENCH_CPU=1 times the CPU's there. A
# pktito-cli before 0.9.7 has no --to, and uses the app's models as before.
#
# The chapters are FLAC, 16 kHz mono, made from LibriVox's 64 kbps MP3s with
#     ffmpeg -i wonderland_ch_NN_64kb.mp3 -map 0:a -map_metadata -1 \
#         -fflags +bitexact -flags:a +bitexact -ac 1 -ar 16000 \
#         -sample_fmt s16 -c:a flac -frame_size 4096 wonderland_ch_NN.flac
# and not the MP3s themselves: each OS's MP3 decoder gives other samples
# (macOS's and ffmpeg's, chapter 11: 22 ms apart in length, 5 words of
# 1937 different), where a FLAC reads the same through all of them (macOS,
# Windows' Media Foundation on Arm64 and ffmpeg: the same transcript, byte
# for byte, 2026-10-23). The WAVs hold the FLACs' samples, unchanged
#     ffmpeg -i wonderland_ch_NN.flac -map 0:a -map_metadata -1 \
#         -fflags +bitexact -flags:a +bitexact -c:a pcm_s16le wonderland_ch_NN.wav
# and give the same transcripts (Linux arm64 with no ffmpeg, 2026-10-23).
# So each chapter's transcript is also scored against the reference
# (scripts/alicebench/ref, pktito-cli bench --ref): identical, or by its
# word error rate and its character edits, the text alone, its split into
# sentences aside. An engine that isn't pktito's CPU engine bit for bit,
# the Neural Engine's (pktito's npu/ane.m), differs from it by a comma here and
# a word's spelling there, and the score says how much, so that a row of
# it is measured, not failed (Peter, 2026-11-02). The line below each
# chapter is the SHA-256 of the FLAC and of the WAV.
#
# -frame_size 4096: without it ffmpeg's FLAC encoder fixes its block size
# from the first audio the resampler hands it, which made 399 and 419
# samples here and 18 for chapter 2, whose MP3 is 22.05 kHz; macOS 26
# would not decode those 18-sample blocks (AVAssetReader: 'fmt?', on a Mac
# mini M4, 2026-10-25), and they made the file 33.0 MB for 13.0. The samples,
# and so the WAVs and transcripts, are the same either way.
set -eu

base=https://models.pktito.net/alicebench/v1
chapters='
wonderland_ch_01 ad3326f63b5371090294c38a80467dcd288bc309b9def8547c550429844b6a31 b2ded69ef8b4c45bc19198b84fc8f7f809d8aab72b7453b34c9f55bcdc268e4c
wonderland_ch_02 7507934bbf636c7ef5a3089eba6f5aec3848528ac1150b987ad80840472c3f97 0cd6126925dc85a961b8b13ffc5fe4138e76264022bd35aa07596d47266db375
wonderland_ch_03 919e8f75289091fce57dde9c0e1947132ad42e88095b8921696d0a3fccd0e50d 7d9fe8a5421bb67dca0a0d4f09d4ee6f53ffda8531c75e757b9d90ea2e95d577
wonderland_ch_04 f575db797db9a0db3af94f1a560b191fcd485e9cc212c19da4ebdf9f2af8e20e 7b597ccc74a5de7d3b773acf738cf81c3f5e3f3d7438e6240b8f132018c488ea
wonderland_ch_05 57d3ec5801fd93782f8d5e6f193a45f11f06ffb5e8358c88b9aecf5bbb2780b5 3b0005e4e815566b2f91a72eb6aa0709fb59a1da370b4c039246652566f7aa98
wonderland_ch_06 cffb9569b7e875e2588bcd92c0fcc38bdb15be232595d54a57c25ca5fcc6133f ab2792a680b87d713bc89e6b38332ac7c4af16c1c257fa0922845c65f60444ad
wonderland_ch_07 59fbba17032e80ae188837a4f77b399fa97817597bf57925952f28e3cd7c731a 69e487aa4ccdff941cc87eb357186e465c35526a173487d3827b72f950cc2687
wonderland_ch_08 e47086d07d2f7e73488550531b2eb4c7e20c7761e806a68f0f544a5dc8d51d5a 8fe1aad714433fee0651c38f6ee568ba9cc8ffa5a4f2c0d9a645ad5edd570492
wonderland_ch_09 a0ec98e0bcb2cc84502faaffdcede67d309f27cc846d8ec18b00160c4c322f0e b78f98e64fb213b2864c1ba280872dfda8b13b8ee8405bb3211669e4616ed439
wonderland_ch_10 463975d3aaedb928618ef1b16bb4ede8640c4a75d0d92b209fcc9c8bd6832aa5 53216cf90987d9262e1d3cb125a576d080cabfbe799dce4ad940a65b7baf7cda
wonderland_ch_11 6ef258f362737f4976e06713624dde9aff46a8a72728a5ecf2c70ef907384f59 80241c5a802b31922fa932bace59d5d37a25934d61eedf384a965d8952f3a2e6
wonderland_ch_12 922986f0c8f45fe6465daf990b832d664f24b8fdc159c45082ea449abac64ff9 b5260f9949cea728bd45c19f9c3a622982af3615c0a9ece99775383639735312
'
dir=${1:-$HOME/.cache/alicebench-v1}

cli=${PKTITO_CLI:-$(command -v pktito-cli || true)}
[ -n "$cli" ] || cli=/Applications/pktito.app/Contents/MacOS/pktito-cli
[ -x "$cli" ] || { echo "alicebench: no pktito-cli; set PKTITO_CLI to one" >&2; exit 2; }

if command -v sha256sum >/dev/null; then sha() { sha256sum "$1" | cut -d' ' -f1; }
else sha() { shasum -a 256 "$1" | cut -d' ' -f1; }; fi

format=${ALICEBENCH_FORMAT:-$([ "$(uname)" = Linux ] && echo wav || echo flac)}
case $format in flac|wav) ;; *) echo "alicebench: ALICEBENCH_FORMAT is flac or wav" >&2; exit 2 ;; esac

mkdir -p "$dir"
files=
echo "$chapters" | while read -r stem flac wav _; do
    [ -n "$stem" ] || continue
    name=$stem.$format
    if [ "$format" = wav ]; then want=$wav; else want=$flac; fi
    f=$dir/$name
    [ -f "$f" ] && [ "$(sha "$f")" = "$want" ] && continue
    echo "alicebench: downloading $name" >&2
    curl -fsSL -o "$f.part" "$base/$name"
    [ "$(sha "$f.part")" = "$want" ] || { echo "alicebench: $name isn't AliceBench v1's" >&2; exit 1; }
    mv "$f.part" "$f"
done
for stem in $(echo "$chapters" | cut -d' ' -f1); do files="$files $dir/$stem.$format"; done

name=parakeet-tdt-0.6b-v2
models=$dir/models
mac=$([ "$(uname)" = Darwin ] && echo 1 || true)
cpu=$([ -n "$mac" ] && [ "${ALICEBENCH_CPU:-}" = 1 ] && echo --cpu || true)
"$cli" download --cpu --to "$models" $name >/dev/null 2>&1 || true
if [ -f "$models/$name.pkf" ]; then
    model=$models/$name.pkf
    if [ -n "$mac" ]; then
        "$cli" download --to "$models" $name >/dev/null
        [ -n "$cpu" ] || model=$models/$name-slim.pkf
    fi
else
    "$cli" download $name >/dev/null
    model=$name
fi
ref=$(cd "$(dirname "$0")" && pwd)/alicebench/ref
out=$dir/result.json
# shellcheck disable=SC2086
"$cli" bench -m "$model" $cpu --json --ref "$ref" $files > "$out"

# each chapter's speed and its transcript's score, from bench's JSON (one
# key a line; a file's keys at 6 spaces, its score's at 8, the whole
# score's at 4); then the whole's
awk '
    function val(s) { sub(/^[^:]*: */, "", s); sub(/,$/, "", s); gsub(/"/, "", s); return s }
    /^      "file":/ { file = val($0); same = ""; wer = ""; edits = "" }
    /^      "x_real_time":/ { x = val($0) }
    /^        "identical":/ { same = val($0) }
    /^        "wer":/ { wer = val($0) }
    /^        "char_edits":/ { edits = val($0) }
    /^    }/ && file != "" {
        said = same == "true" ? "identical" : same == "false" ? sprintf("WER %.3f%%, %d character edit%s", wer, edits, (edits == 1 ? "" : "s")) : "no reference"
        printf "%7.1fx  %s  %s\n", x, file, said; got++; if (same == "") missing++; file = "" }
    /^    "identical":/ { identical = val($0) } /^    "wer":/ { twer = val($0) }
    /^    "word_errors":/ { terr = val($0) } /^    "words":/ { twords = val($0) }
    /^    "char_edits":/ { tedits = val($0) }
    /^  "backend":/ { backend = val($0) } /^  "model":/ { model = val($0) }
    /^  "pktito_cli":/ { version = val($0) } /^  "threads":/ { threads = val($0) }
    /^  "compiler":/ { compiler = val($0) }
    /^  "x_real_time":/ { total = val($0) }
    END {
        printf "AliceBench v1: %.1fx real time (pktito-cli %s, %s, %s, %s threads%s)\n", total, version, model, backend, threads, (compiler != "" ? ", " compiler : "")
        if (got != 12) { print "  only " got " of the 12 chapters were timed"; exit 1 }
        if (missing) { print "  " missing " of the 12 chapters had no reference in alicebench/ref"; exit 1 }
        if (identical == 12) print "  all 12 transcripts identical to the reference"
        else printf "  %d of 12 transcripts identical to the reference; WER %.3f%% (%d of %d words), %d character edit%s\n", identical, twer, terr, twords, tedits, (tedits == 1 ? "" : "s")
    }' "$out"

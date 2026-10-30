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
# So each chapter's transcript is also checked against the one every
# machine gives, which says this pktito computes what the reference engine
# does; the line below each is the SHA-256 of the FLAC, of the WAV and of
# its transcript (pktito-cli bench --json's), and for chapters 1, 2, 7, 8,
# 9, 10 and 12 the transcript every build before 0.9.3 gives: 0.9.3
# transcribes again the speech the English model wrote no words for
# (vendor/pktito/core/transcribe.c, RETRY_SPEECH_S), which gives
# those chapters back the LibriVox notices, credits and titles it drops at
# their starts, as NeMo does, and chapter 1 its "End of chapter one".
# Either passes, and one of an earlier version is named as such.
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
wonderland_ch_01 ad3326f63b5371090294c38a80467dcd288bc309b9def8547c550429844b6a31 b2ded69ef8b4c45bc19198b84fc8f7f809d8aab72b7453b34c9f55bcdc268e4c c34389351b253d8ab1e0abe533988e9412c29e0f5a80a2be241fb8caa8455a03 420b1701aa22b14ae761476d9394889dbb32307becd4efd389cebfbffa923128
wonderland_ch_02 7507934bbf636c7ef5a3089eba6f5aec3848528ac1150b987ad80840472c3f97 0cd6126925dc85a961b8b13ffc5fe4138e76264022bd35aa07596d47266db375 1f96d97333a405864ae95f1c746a0e75fdd1c70187aa48ad10e190b94d8bb2db 9b9c182116e0c1e302298cf35a6f95d568ee28ef37d8c6aa2500a4445816da05
wonderland_ch_03 919e8f75289091fce57dde9c0e1947132ad42e88095b8921696d0a3fccd0e50d 7d9fe8a5421bb67dca0a0d4f09d4ee6f53ffda8531c75e757b9d90ea2e95d577 502be4285a107012dfcd86373e6623aa01ba0f842f2b189c8c4e794b07fc8a29
wonderland_ch_04 f575db797db9a0db3af94f1a560b191fcd485e9cc212c19da4ebdf9f2af8e20e 7b597ccc74a5de7d3b773acf738cf81c3f5e3f3d7438e6240b8f132018c488ea 2eb60b8d7451ffaf2edec1153bc251051f3b1348cc6e7fd3fef2b64ed0231354
wonderland_ch_05 57d3ec5801fd93782f8d5e6f193a45f11f06ffb5e8358c88b9aecf5bbb2780b5 3b0005e4e815566b2f91a72eb6aa0709fb59a1da370b4c039246652566f7aa98 3fbcb15dbb16a47449618d283e2e1de0b96477ee32279fd3b179e33b535a0643
wonderland_ch_06 cffb9569b7e875e2588bcd92c0fcc38bdb15be232595d54a57c25ca5fcc6133f ab2792a680b87d713bc89e6b38332ac7c4af16c1c257fa0922845c65f60444ad f0170d1b5162a1a7efb6492e991eb17782753d8785357ab2bb9fc0f0fe93e1bb
wonderland_ch_07 59fbba17032e80ae188837a4f77b399fa97817597bf57925952f28e3cd7c731a 69e487aa4ccdff941cc87eb357186e465c35526a173487d3827b72f950cc2687 96cea8833c13920b98e50cde05d91bb13b95c9defe410672cdde24f6f32e1367 4c5aff13c3eafa3c080d1ca38bf48ed02ff474989c94ef0ad78a0e1b5cb9d9a4
wonderland_ch_08 e47086d07d2f7e73488550531b2eb4c7e20c7761e806a68f0f544a5dc8d51d5a 8fe1aad714433fee0651c38f6ee568ba9cc8ffa5a4f2c0d9a645ad5edd570492 33991e6198d387c036fea19857250ba60f23dcf3a7ecfd4d70a0cfb6a5891eb3 d0f40e98ee30f34a4e1efb66f7608c4b74199d4f50e25ada12f3c0fde6fb1548
wonderland_ch_09 a0ec98e0bcb2cc84502faaffdcede67d309f27cc846d8ec18b00160c4c322f0e b78f98e64fb213b2864c1ba280872dfda8b13b8ee8405bb3211669e4616ed439 36b66f11f802c89afab765f8a35525088599082bba06d121aa516fa21394bd7b 8d4180ce60b8c69b368a29953adf7fb5bd6f8b0972ec7c8498ab5717b786b8b5
wonderland_ch_10 463975d3aaedb928618ef1b16bb4ede8640c4a75d0d92b209fcc9c8bd6832aa5 53216cf90987d9262e1d3cb125a576d080cabfbe799dce4ad940a65b7baf7cda b32863dc3e50239df146ae48f8930b8eb662c75c5414e00ce50ba6a6219819ac a480673ea85f2728d719c92eddecacce14757b7c073aaa95f946714734ff3ac3
wonderland_ch_11 6ef258f362737f4976e06713624dde9aff46a8a72728a5ecf2c70ef907384f59 80241c5a802b31922fa932bace59d5d37a25934d61eedf384a965d8952f3a2e6 a770e46c49181c3511d2863201cb3507b9ff6dc149576fcf5dbd5cd43658c7fc
wonderland_ch_12 922986f0c8f45fe6465daf990b832d664f24b8fdc159c45082ea449abac64ff9 b5260f9949cea728bd45c19f9c3a622982af3615c0a9ece99775383639735312 050f58d7462d540e82f0fa6752ad7431b2ea3765d8d60de76e6abf398a35ba83 9a2d63b4ff85f2df4ed5daa9f01853df7e3f692259b02b01a223f80ae49c2624
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

"$cli" download parakeet-tdt-0.6b-v2 >/dev/null
out=$dir/result.json
# shellcheck disable=SC2086
"$cli" bench -m parakeet-tdt-0.6b-v2 --json $files > "$out"

# each chapter's speed and whether its transcript is the reference's, from
# bench's JSON (one key a line), against the list above; then the whole's
echo "$chapters" | awk '
    NR == FNR { if ($1 != "") { ref[$1 ".flac"] = ref[$1 ".wav"] = $4; was[$1 ".flac"] = was[$1 ".wav"] = $5 }; next }
    function val(s) { sub(/^[^:]*: */, "", s); sub(/,$/, "", s); gsub(/"/, "", s); return s }
    /"file":/ { file = val($0) }
    /"x_real_time":/ && file != "" { x = val($0) }
    /"transcript_sha256":/ { sha = val($0) }
    /^    }/ { ok = sha == ref[file] ? "" : sha == was[file] && sha != "" ? "  as before 0.9.3" : "  transcript differs"
               if (ok == "  transcript differs") differ++; else { same++; if (ok) earlier++ }
               printf "%7.1fx  %s%s\n", x, file, ok; got++; file = "" }
    /^  "backend":/ { backend = val($0) } /^  "model":/ { model = val($0) }
    /^  "pktito_cli":/ { version = val($0) } /^  "threads":/ { threads = val($0) }
    /^  "compiler":/ { compiler = val($0) }
    /^  "x_real_time":/ { total = val($0) }
    END {
        printf "AliceBench v1: %.1fx real time (pktito-cli %s, %s, %s, %s threads%s)\n", total, version, model, backend, threads, (compiler != "" ? ", " compiler : "")
        if (got != 12) { print "  only " got " of the 12 chapters were timed"; exit 1 }
        if (differ) { print "  " differ " of the 12 transcripts differ from the reference: not what pktito should compute"; exit 1 }
        print "  all 12 transcripts are the reference'"'"'s" (earlier ? ", " earlier " as before 0.9.3" : "")
    }' - "$out"

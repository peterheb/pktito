#!/bin/bash
# One recording of speech in the containers and codecs people have, for
# checking that each platform's decoder reads them (scripts/media-check.sh
# runs the app over the lot):
#
#     scripts/media-matrix.sh SPEECH.wav DIR
#
# ffmpeg makes them, and on macOS afconvert adds HE-AAC, which ffmpeg cannot
# encode. (macOS decodes AMR and xHE-AAC but has no encoder for them.) Each
# holds the whole recording, so its transcript can be scored against the
# recording's own.
set -euo pipefail
[ $# -eq 2 ] || { echo "usage: $0 SPEECH.wav DIR" >&2; exit 2; }
src=$1 dir=$2
mkdir -p "$dir"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

ff() { ffmpeg -nostdin -hide_banner -loglevel error -y "$@"; }
export SVT_LOG=1   # SVT-AV1 reports to the terminal past ffmpeg's -loglevel

# the masters: stereo at 44.1, 48 and 96 kHz, and 5.1 as films carry
# dialogue - the voice in the centre, a little of it at the front, quiet
# noise around, nothing in the LFE
m44=$tmp/m44.wav m48=$tmp/m48.wav m96=$tmp/m96.wav m51=$tmp/m51.wav
ff -i "$src" -ac 2 -ar 44100 "$m44"
ff -i "$src" -ac 2 -ar 48000 "$m48"
ff -i "$src" -ac 2 -ar 96000 -c:a pcm_s24le "$m96"
len=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$src")
ff -i "$m48" -filter_complex \
    "[0:a]pan=mono|c0=c0,asplit=3[c][l][r];[l]volume=0.3[fl];[r]volume=0.3[fr];aevalsrc=0:s=48000:d=$len[lfe];
     anoisesrc=color=pink:amplitude=0.03:sample_rate=48000:d=$len,asplit=2[sl][sr];
     [fl][fr][c][lfe][sl][sr]join=inputs=6:channel_layout=5.1(side)[a]" \
    -map "[a]" "$m51"

audio() { # NAME OPTIONS...
    local name=$1; shift
    ff "$@" "$dir/$name"
}
video() { # NAME MASTER OPTIONS...: a small moving picture over the master's audio
    local name=$1 master=$2; shift 2
    ff -f lavfi -i "testsrc2=size=320x180:rate=10" -i "$master" -map 0:v -map 1:a -shortest "$@" "$dir/$name"
}
x264=(-c:v libx264 -preset veryfast -pix_fmt yuv420p)

# uncompressed and lossless
audio wav-pcm16-44k.wav            -i "$m44" -c:a pcm_s16le
audio wav-pcm24-48k.wav            -i "$m48" -c:a pcm_s24le
audio wav-float32-48k.wav          -i "$m48" -c:a pcm_f32le
audio wav-ulaw-8k-mono.wav         -i "$src" -ar 8000 -c:a pcm_mulaw            # a phone line's
audio wav-ima-adpcm-22k-mono.wav   -i "$src" -ar 22050 -c:a adpcm_ima_wav       # an old dictaphone's
audio aiff-pcm16-44k.aiff          -i "$m44" -c:a pcm_s16be
audio flac-16bit-44k.flac          -i "$m44" -c:a flac
audio flac-24bit-96k.flac          -i "$m96" -c:a flac
audio m4a-alac-44k.m4a             -i "$m44" -c:a alac
audio caf-alac-48k.caf             -i "$m48" -c:a alac

# lossy
audio mp3-cbr-128k-44k.mp3         -i "$m44" -c:a libmp3lame -b:a 128k
audio mp3-vbr-22k-mono.mp3         -i "$src" -ar 22050 -c:a libmp3lame -q:a 7  # a podcast's
audio m4a-aac-lc-44k.m4a           -i "$m44" -c:a aac -b:a 128k
audio aac-adts-44k.aac             -i "$m44" -c:a aac -b:a 128k
audio ogg-vorbis-48k.ogg           -i "$m48" -c:a libvorbis -q:a 4
audio opus-voice-mono.opus         -i "$src" -c:a libopus -b:a 24k -application voip  # a voice message
audio ogg-opus-voice-mono.ogg      -i "$src" -c:a libopus -b:a 24k -application voip  # the same, as WhatsApp names it
audio webm-opus-48k.webm           -i "$m48" -c:a libopus -b:a 96k              # a browser's recording
audio wma-v2-44k.wma               -i "$m44" -c:a wmav2 -b:a 128k
audio ac3-5.1-48k.ac3              -i "$m51" -c:a ac3 -b:a 448k
if command -v afconvert >/dev/null; then
    afconvert -f m4af -d aach -b 48000 "$m48" "$dir/m4a-he-aac-48k.m4a"         # a 24 kHz core the decoder doubles
    afconvert -f m4af -d aacp -b 24000 "$m48" "$dir/m4a-he-aac-v2-48k.m4a"      # and parametric stereo
fi

# video: the audio is what is read, but around a picture in the container
video mp4-h264-aac.mp4             "$m48" "${x264[@]}" -c:a aac -b:a 128k
video mov-h264-aac.mov             "$m48" "${x264[@]}" -c:a aac -b:a 128k
video mov-hevc-aac.mov             "$m48" -c:v libx265 -preset ultrafast -tag:v hvc1 -c:a aac -b:a 128k -x265-params log-level=error  # a phone's
video mov-prores-pcm24.mov         "$m48" -c:v prores -c:a pcm_s24le            # an editor's
video mkv-h264-aac.mkv             "$m48" "${x264[@]}" -c:a aac -b:a 128k
video mkv-h264-ac3-5.1.mkv         "$m51" "${x264[@]}" -c:a ac3 -b:a 448k       # a film's
video mkv-hevc-eac3-5.1.mkv        "$m51" -c:v libx265 -preset ultrafast -c:a eac3 -b:a 384k -x265-params log-level=error
video mkv-h264-dts-5.1.mkv         "$m51" "${x264[@]}" -c:a dca -strict -2
video webm-vp9-opus.webm           "$m48" -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -c:a libopus -b:a 96k  # a video site's
video webm-vp8-vorbis.webm         "$m48" -c:v libvpx -deadline realtime -cpu-used 8 -c:a libvorbis
video webm-av1-opus.webm           "$m48" -c:v libsvtav1 -preset 12 -svtav1-params log=0 -c:a libopus -b:a 96k
video avi-mpeg4-mp3.avi            "$m44" -c:v mpeg4 -q:v 5 -c:a libmp3lame -b:a 128k
video avi-mjpeg-pcm16.avi          "$m44" -c:v mjpeg -q:v 5 -c:a pcm_s16le -ar 22050 -ac 1  # a still camera's
video wmv-wmv2-wmav2.wmv           "$m44" -c:v wmv2 -c:a wmav2 -b:a 128k
video flv-h264-aac.flv             "$m44" "${x264[@]}" -c:a aac -b:a 128k
video ts-h264-aac.ts               "$m48" "${x264[@]}" -c:a aac -b:a 128k       # a broadcast's
video mts-h264-ac3.mts             "$m48" "${x264[@]}" -c:a ac3 -b:a 256k -f mpegts -mpegts_m2ts_mode 1  # a camcorder's
video mpg-mpeg2-mp2.mpg            "$m48" -c:v mpeg2video -c:a mp2 -b:a 192k -f vob  # a DVD's
video 3gp-h264-aac.3gp             "$m44" "${x264[@]}" -c:a aac -b:a 48k -ac 1 -ar 22050
video ogv-theora-vorbis.ogv        "$m48" -c:v libtheora -q:v 5 -c:a libvorbis

# around the edges
for two in mkv-two-audio-tracks.mkv mp4-two-audio-tracks.mp4; do               # the first track is the one read
    ff -f lavfi -i "testsrc2=size=320x180:rate=10" -i "$m48" -f lavfi -i "sine=frequency=440:sample_rate=48000" \
        -map 0:v -map 1:a -map 2:a -shortest "${x264[@]}" -c:a aac -b:a 128k "$dir/$two"
done
audio "café – voix 日本語.m4a"      -i "$m44" -c:a aac -b:a 128k                # a name past ASCII
ff -f lavfi -i "testsrc2=size=320x180:rate=10" -t 10 "${x264[@]}" "$dir/mp4-video-only.mp4"  # nothing to hear
audio m4a-silence-20s.m4a          -f lavfi -i "anullsrc=r=44100:cl=stereo" -t 20 -c:a aac   # nothing said
cp "$dir/mp3-cbr-128k-44k.mp3" "$dir/mp3-named-m4a.m4a"                         # the wrong extension
ff -i "$m48" -c:a aac -b:a 128k -movflags +faststart "$tmp/whole.m4a"
head -c $(( $(wc -c < "$tmp/whole.m4a") / 2 )) "$tmp/whole.m4a" > "$dir/m4a-cut-short.m4a"  # a download cut off halfway
printf 'not a recording\n' > "$dir/not-media.mp3"                              # a file only named like one

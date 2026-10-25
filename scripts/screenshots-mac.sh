#!/bin/bash
# The site's standard screenshot, the Mac's two: the window at its first
# size, LibriVox's Alice's Adventures in Wonderland, chapter 1, transcribed
# by the multilingual model and scrolled past the recording's LibriVox
# notice, so that "Alice's Adventures in Wonderland by Lewis Carroll." is
# the first row; once in light and once in dark, each with macOS's window
# shadow, as ⌘⇧4 Space takes it.
#
#     scripts/screenshots-mac.sh [OUT]       (default site/screenshots)
#
# writes OUT/mac-light.png and OUT/mac-dark.png. Why these choices:
#
# - The multilingual model, not English: English runs the notice, the
#   title and "Chapter one" together as one row from 0:05.60, so no row
#   is the title; the multilingual model ends each at its full stop
#   (2026-10-23, 0.8.5).
# - The MP3 as LibriVox publishes it, which reads as a real file's name:
#   macOS's decoder and ffmpeg (Linux's) give the same first 16 rows of it,
#   so the Mac's and Linux's shots show the same words. The AliceBench FLAC
#   is resampled by ffmpeg and differs in two of those rows.
# - Scrolled by SCROLL pixels: the three notice rows, one line each, 43 px
#   apiece at the window's first size (app.css's row spacing, 2026-10-23).
#   If that spacing changes, measure a one-line row's pitch again and set
#   SCROLL to three of them.
#
# It needs this terminal to have Screen Recording (the capture) and
# Accessibility (the scroll) in System Settings → Privacy & Security. It
# quits pktito, sets its settings for the shot (the model, no saved window
# size, and playback, the view and the times at their defaults: 1x, full
# volume, sentences, seconds) and puts them back after, and puts the
# system's appearance back as it was. The cursor is moved over the window to scroll it and back after.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$here/site/screenshots}
app=${PKTITO_APP:-$here/src-tauri/target/release/bundle/macos/pktito.app}
data="$HOME/Library/Application Support/net.pktito.app"
SCROLL=129
mp3_url=https://archive.org/download/alice_in_wonderland_librivox/wonderland_ch_01_64kb.mp3
mp3_sha=fca40613cd198eb2b3e3b36e087b6cbaf22be8d143951d744670973a3c26d0d1
cache="$HOME/.cache/pktito-screenshots"
mp3="$cache/wonderland_ch_01_64kb.mp3"

[ -d "$app" ] || { echo "no $app: cargo tauri build --bundles app first, or set PKTITO_APP" >&2; exit 2; }
[ -f "$data/models/parakeet-tdt-0.6b-v3.pkf" ] || { echo "the multilingual model isn't downloaded: pktito-cli download v3" >&2; exit 2; }
mkdir -p "$out" "$cache"
if [ ! -f "$mp3" ] || [ "$(shasum -a 256 "$mp3" | cut -c1-64)" != "$mp3_sha" ]; then
    curl -fsSL -o "$mp3.part" "$mp3_url"
    [ "$(shasum -a 256 "$mp3.part" | cut -c1-64)" = "$mp3_sha" ] || { echo "the chapter isn't LibriVox's" >&2; exit 1; }
    mv "$mp3.part" "$mp3"
fi

tmp=$(mktemp -d)
cp "$data/settings.json" "$tmp/settings.json"
dark_was=$(osascript -e 'tell application "System Events" to tell appearance preferences to get dark mode')
quit_app() {
    osascript -e 'tell application id "net.pktito.app" to quit' 2>/dev/null || true
    for _ in $(seq 50); do pgrep -qf 'MacOS/pktito-app$' || return 0; sleep 0.1; done
}
restore() {
    quit_app
    cp "$tmp/settings.json" "$data/settings.json"
    osascript -e "tell application \"System Events\" to tell appearance preferences to set dark mode to $dark_was"
    rm -rf "$tmp"
}
trap restore EXIT

# the window: where, and its number, for the capture and the scroll
cat > "$tmp/window.swift" <<'SWIFT'
import CoreGraphics
import Foundation
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
guard let w = list.first(where: { ($0[kCGWindowOwnerName as String] as? String) == "pktito" && ($0[kCGWindowLayer as String] as? Int) == 0 }),
      let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { exit(1) }
let args = CommandLine.arguments
if args.count > 1 {
    // scroll the transcript by args[1] pixels, the cursor over its middle, then back
    let at = CGPoint(x: b["X"]! + b["Width"]! / 2, y: b["Y"]! + b["Height"]! * 0.7)
    let was = CGEvent(source: nil)!.location
    CGWarpMouseCursorPosition(at)
    usleep(200_000)
    var left = Int32(args[1])!
    while left > 0 {
        let step = min(left, 10)
        CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: -step, wheel2: 0, wheel3: 0)!.post(tap: .cghidEventTap)
        left -= step
        usleep(16_000)
    }
    usleep(300_000)
    CGWarpMouseCursorPosition(was)
} else {
    print(w[kCGWindowNumber as String]!)
}
SWIFT
swiftc -O -o "$tmp/window" "$tmp/window.swift" 2>/dev/null

quit_app
python3 - "$data/settings.json" <<'PY'
import json, sys
p = sys.argv[1]
s = json.load(open(p))
s.update(model="parakeet-tdt-0.6b-v3", model_file=None)
s.setdefault("prefs", {}).update(speed=1.0, volume=1.0, captions=False, frames=False)
s.pop("window", None)
json.dump(s, open(p, "w"), indent=2)
PY
started=$(date +%s)
open -a "$app" "$mp3"
# done once the recent list's newest is this chapter, finished since
for _ in $(seq 120); do
    sleep 1
    python3 - "$data/recent/index.json" "$started" <<'PY' && break
import json, sys
items = json.load(open(sys.argv[1]))
newest = items[0] if items else {}
sys.exit(0 if newest.get("name") == "wonderland_ch_01_64kb.mp3" and newest.get("finished", 0) >= int(sys.argv[2]) else 1)
PY
done
osascript -e 'tell application id "net.pktito.app" to activate'
sleep 1
"$tmp/window" "$SCROLL"
id=$("$tmp/window")
for look in light dark; do
    osascript -e "tell application \"System Events\" to tell appearance preferences to set dark mode to $([ $look = dark ] && echo true || echo false)"
    osascript -e 'tell application id "net.pktito.app" to activate'
    # the theme's change, and an overlay scrollbar's fading
    sleep 2
    screencapture -x -l"$id" "$out/mac-$look.png"
    echo "$out/mac-$look.png"
done

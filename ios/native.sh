#!/bin/sh
# The app's native library: the engine, pktito's ane fork (vendor/pktito-ane,
# whose encoder runs on the Neural Engine), and the Rust core the desktop
# app shares (src-tauri's library; src/ios.rs is its C side), cross-built for
# iOS devices and the Simulator into build/PktitoCore.xcframework, with
# Core/include's header and module map, which the Xcode project links and
# Swift imports as PktitoCore.
#
#   ios/native.sh [debug]
#
# Rust needs the targets once:
# rustup target add aarch64-apple-ios aarch64-apple-ios-sim
set -eu
here=$(cd "$(dirname "$0")" && pwd)
profile=${1:-release}
export IPHONEOS_DEPLOYMENT_TARGET=18.0  # the app's (pktito.xcodeproj)
args=""
for t in aarch64-apple-ios aarch64-apple-ios-sim; do
  # a static library keeps its symbols for the app's link, whatever the
  # release profile strips from the desktop's binaries
  cargo rustc --manifest-path "$here/../src-tauri/Cargo.toml" --lib --crate-type staticlib \
    --target $t $([ "$profile" = release ] && echo --release) -- -C strip=none
  args="$args -library $here/../src-tauri/target/$t/$profile/libpktito_app.a -headers $here/Core/include"
done
rm -rf "$here/build/PktitoCore.xcframework"
xcodebuild -create-xcframework $args -output "$here/build/PktitoCore.xcframework" >/dev/null
echo "ios/native.sh: build/PktitoCore.xcframework ($profile)"

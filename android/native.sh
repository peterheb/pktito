#!/bin/sh
# The app's native library, libpktito_app.so: the engine and the Rust core
# the desktop app shares (src-tauri's library; src/android.rs is its JNI
# side), cross-built with the NDK's clang for each ABI the app ships, into
# app/src/main/jniLibs, where Gradle packages it.
#
#   NDK=/path/to/android-ndk android/native.sh [debug]
#
# ANDROID_NDK_HOME serves for NDK. Rust needs the targets once:
# rustup target add aarch64-linux-android x86_64-linux-android
#
# With QNN_SDK naming Qualcomm's AI Runtime SDK, the engine runs its
# encoder on a Snapdragon's NPU too (src-tauri/build.rs), and Qualcomm's
# libraries for it go with the app, 46 MB: beside the Arm64 one QNN's HTP
# backend and, for each NPU the catalog has graphs for (src-tauri/src/
# model.rs, Graph), its stub, the Arm side of the call to the NPU; and its
# skel, the NPU's own, into app/src/main/assets/qnn. Android never loads a
# skel: the NPU's loader reads it from where ADSP_LIBRARY_PATH says, and the
# app copies it out of its assets for that (Editions.libs). QAIRT 2.51's
# skels are aligned to 4 KB pages, and Play refuses an app for Android 15
# or later with any library under 16 KB in lib/ (LiteRT's skels, 2026:
# google-ai-edge/litert-samples issue 370; DenisovAV/flutter_gemma 529).
# Not the compiler, libQnnHtpPrepare.so (88 MB), as the graphs come
# compiled on a PC, nor libQnnHtpV73.so and its like, which a compiled
# graph runs without (the Googlebook, 2026-11-03). With them goes the SDK's
# notice file for the open-source code in them, as the SDK has it, into
# app/src/main/assets/qnn with the SDK's version, for About's licenses: QTI's
# license (its "Third Party Notices") leaves those licenses to the app, and
# the file unaltered.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
NDK=${NDK:-${ANDROID_NDK_HOME:-}}
[ -n "$NDK" ] || { echo "native.sh: NDK=/path/to/android-ndk" >&2; exit 1; }
API=29                                  # the app's minSdk (app/build.gradle.kts)
bin=$(echo "$NDK"/toolchains/llvm/prebuilt/*/bin)
profile=${1:-release}
for abi in arm64-v8a x86_64; do
  case $abi in
    arm64-v8a) t=aarch64-linux-android ;;
    x86_64) t=x86_64-linux-android ;;
  esac
  v=$(echo $t | tr - _)                 # the cc crate's spelling of the target
  V=$(echo $v | tr a-z A-Z)             # and cargo's
  env "CC_$v=$bin/$t$API-clang" "AR_$v=$bin/llvm-ar" "CARGO_TARGET_${V}_LINKER=$bin/$t$API-clang" \
    cargo rustc --manifest-path "$here/../src-tauri/Cargo.toml" --lib --crate-type cdylib \
      --target $t $([ "$profile" = release ] && echo --release)
  mkdir -p "$here/app/src/main/jniLibs/$abi"
  cp "$here/../src-tauri/target/$t/$profile/libpktito_app.so" "$here/app/src/main/jniLibs/$abi/"
done
lib=$here/app/src/main/jniLibs/arm64-v8a
qnn=$here/app/src/main/assets/qnn
rm -f "$lib"/libQnn*.so
rm -rf "$qnn"
if [ -n "${QNN_SDK:-}" ]; then
  mkdir -p "$qnn"
  cp "$QNN_SDK/lib/aarch64-android/libQnnHtp.so" "$lib/"
  for v in 73 75 79; do
    cp "$QNN_SDK/lib/aarch64-android/libQnnHtpV${v}Stub.so" "$lib/"
    cp "$QNN_SDK/lib/hexagon-v$v/unsigned/libQnnHtpV${v}Skel.so" "$qnn/"
  done
  cp "$QNN_SDK/QNN_NOTICE.txt" "$qnn/"
  sed -n 's/^version: *//p' "$QNN_SDK/sdk.yaml" > "$qnn/VERSION"
fi

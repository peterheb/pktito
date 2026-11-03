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
# libraries for it go beside the Arm64 one: QNN's HTP backend, and for each
# NPU the catalog has graphs for (src-tauri/src/model.rs, Graph), its stub,
# the Arm side of the call to the NPU, and its skel, the NPU's own, which it
# loads from the app's native library folder (ADSP_LIBRARY_PATH): 46 MB.
# Not the compiler, libQnnHtpPrepare.so (88 MB), as the graphs come
# compiled on a PC, nor libQnnHtpV73.so and its like, which a compiled
# graph runs without (the Googlebook, 2026-11-03).
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
rm -f "$lib"/libQnn*.so
if [ -n "${QNN_SDK:-}" ]; then
  cp "$QNN_SDK/lib/aarch64-android/libQnnHtp.so" "$lib/"
  for v in 73 75 79; do
    cp "$QNN_SDK/lib/aarch64-android/libQnnHtpV${v}Stub.so" "$QNN_SDK/lib/hexagon-v$v/unsigned/libQnnHtpV${v}Skel.so" "$lib/"
  done
fi

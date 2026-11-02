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

// The Android app: Kotlin for the screen, the files and the service that
// keeps a transcription running, and the desktop's Rust core with the
// engine as one native library (native.sh, src-tauri/src/android.rs).
plugins {
    id("com.android.application") version "9.4.1" apply false
    // AGP builds Kotlin itself; Compose's compiler is Kotlin's plugin
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21" apply false
}

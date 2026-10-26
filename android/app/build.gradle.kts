plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "net.pktito.app"
    compileSdk = 37

    defaultConfig {
        applicationId = "net.pktito.app"
        // Android 10: a foreground service's type, and the NDK's media
        // key native/decode_android.c sets (a decoder's PCM encoding,
        // Android 9), with no older path beside them; native.sh builds for
        // the same
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.9.2"
        // the two native.sh builds the library for: without this AndroidX's
        // graphics-path library (Compose's) brought armeabi-v7a and x86 too,
        // and a 32-bit phone that Play offered the app to would have had
        // no libpktito_app.so for it
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    buildTypes {
        // R8's shrinking and its rules (proguard-rules.pro); signed by
        // Play's upload key, which is not here
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
    }
    // the screens drawn on the JVM (src/test, Shots), with the app's fonts
    testOptions {
        unitTests.isIncludeAndroidResources = true
        // Robolectric sets up its file descriptors through the JDK's internals
        unitTests.all { it.jvmArgs("--add-exports=java.base/jdk.internal.access=ALL-UNNAMED") }
    }
}

// what the release build packages from Maven, each module and its file, for
// notices.py's list of licenses: what resolves to a file, so neither a
// BOM nor a multiplatform module's metadata, whose code is its -android's
tasks.register("packaged") {
    val modules = configurations.named("releaseRuntimeClasspath")
        .flatMap { it.incoming.artifacts.resolvedArtifacts }
        .map { all -> all.map { "${it.id.componentIdentifier.displayName}\t${it.file}" }.distinct().sorted() }
    val out = layout.buildDirectory.file("notices/packaged.txt")
    inputs.property("modules", modules)
    outputs.file(out)
    doLast { out.get().asFile.writeText(modules.get().joinToString("\n", postfix = "\n")) }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.09.00"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.core:core-ktx:1.19.1")
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.robolectric:robolectric:4.17")
    testImplementation("androidx.compose.ui:ui-test-junit4")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}

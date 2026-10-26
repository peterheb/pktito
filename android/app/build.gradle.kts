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
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.09.00"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.core:core-ktx:1.19.1")
}

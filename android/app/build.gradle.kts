plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

// Release signing is opt-in and never inferred: a keystore given through the environment is
// used as-is; without one, release builds are signed with the standard *debug* key so CI
// artifacts install. scripts/build-apk.sh labels the result accordingly, and a build signed
// with the debug key must never be published as a supported release.
val releaseKeystore: String? = System.getenv("CARBONHV_KEYSTORE")
val releaseKeystorePassword: String? = System.getenv("CARBONHV_KEYSTORE_PASSWORD")
val releaseKeyAlias: String = System.getenv("CARBONHV_KEY_ALIAS") ?: "carbonhv"
val releaseKeyPassword: String? = System.getenv("CARBONHV_KEY_PASSWORD") ?: releaseKeystorePassword
val releaseSigningConfigured: Boolean =
    releaseKeystore != null && releaseKeystorePassword != null

android {
    namespace = "dev.carbonhv"
    compileSdk = 36

    defaultConfig {
        applicationId = "dev.carbonhv"
        minSdk = 30
        targetSdk = 36
        // Both come from the release tag in CI (scripts/build-apk.sh --tag v1.2.3).
        versionCode = (System.getenv("CARBONHV_VERSION_CODE") ?: "1").toInt()
        versionName = System.getenv("CARBONHV_VERSION") ?: "0.1.0"

        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    signingConfigs {
        if (releaseSigningConfigured) {
            create("release") {
                storeFile = file(releaseKeystore!!)
                storePassword = releaseKeystorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        release {
            // R8 is left off on purpose: no shrinking rules have been validated against the
            // Compose UI, and a release build that crashes is worse than a larger APK. Turn it
            // on together with instrumented tests, not before.
            isMinifyEnabled = false
            isShrinkResources = false
            // The debug key keeps CI artifacts installable; see the note above.
            signingConfig =
                signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
        }
    }

    buildFeatures {
        aidl = true
        compose = true
    }

    sourceSets {
        named("main") {
            aidl.srcDir("../../host/android/aidl/ui")
            jniLibs.srcDir("../emu/src/main/jniLibs")
        }
    }

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
        // QEMU must be an extracted .so: untrusted apps may execute only from nativeLibraryDir.
        jniLibs.useLegacyPackaging = true
    }
}

dependencies {
    implementation(project(":android:domain"))

    implementation(platform("androidx.compose:compose-bom:2026.03.01"))
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.core:core-ktx:1.19.1")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3:1.4.0")
    implementation("androidx.compose.material:material-icons-core:1.7.8")
    debugImplementation("androidx.compose.ui:ui-tooling")

    testImplementation("junit:junit:4.13.2")
}

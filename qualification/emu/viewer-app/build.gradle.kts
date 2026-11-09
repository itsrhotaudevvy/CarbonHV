// Qualification viewer for the emulated engine: QEMU in the app sandbox, guest frames on the
// screen. Never shipped; framework-only; uninstall after use.
plugins {
    id("com.android.application")
}

android {
    namespace = "dev.carbonhv.emuviewer"
    compileSdk = 36

    defaultConfig {
        applicationId = "dev.carbonhv.emuviewer"
        minSdk = 30
        // Same targetSdk as CarbonHV, so the W^X and exec rules under test are the real ones.
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    packaging {
        // Extract lib*.so into nativeLibraryDir: the only place an untrusted app may exec from
        // (plan V7). The executables are staged by build-native.sh.
        jniLibs.useLegacyPackaging = true
    }
}

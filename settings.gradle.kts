pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "CarbonHV"

include(":android:domain")
include(":android:host-contract")
include(":android:app")

// Gate 3 qualification probe for the emulated engine (never shipped; see qualification/emu/).
include(":qualification:emu:probe-app")

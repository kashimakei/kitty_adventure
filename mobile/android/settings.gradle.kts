// The Android app around the Rust game: Java glue from miniquad, the game's native library
// (built by cargo-ndk, see app/build.gradle.kts) and the art and sounds staged in ../assets.
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

rootProject.name = "KittyAdventure"
include(":app")

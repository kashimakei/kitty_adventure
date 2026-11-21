import javax.inject.Inject

plugins {
    id("com.android.application")
}

/** The Rust crate (mobile/): its cdylib, libkitty_mobile.so, is the whole game. */
val rustCrateDir: File = rootDir.parentFile

/** Android ABIs to build the game for (gradle.properties; -PrustAbis=arm64-v8a for a quick build). */
val rustAbis: List<String> = providers.gradleProperty("rustAbis").get()
    .split(",").map { it.trim() }.filter { it.isNotEmpty() }

/** Android 8.0; cargo-ndk links the game against the same API level. */
val minApi = 26

android {
    namespace = "com.kashimakei.kittyadventure"
    compileSdk = 35
    buildToolsVersion = "35.0.0"
    // The NDK cargo-ndk builds with (r25 is the oldest current Rust supports). AGP also uses it to
    // strip the libraries; pinning it stops AGP looking for its own default NDK.
    ndkVersion = "25.2.9519653"

    defaultConfig {
        applicationId = "com.kashimakei.kittyadventure"
        minSdk = minApi
        targetSdk = 35
        versionCode = 1
        versionName = "0.2.0"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // The game opens its files by path from the asset root ("kitty/anchors.json",
    // "sounds/meow.wav"); build.sh stages them from ../assets into mobile/assets.
    sourceSets["main"].assets.srcDir(rustCrateDir.resolve("assets"))

    androidResources {
        // The sounds are compressed already, so store them as they are.
        noCompress += listOf("ogg", "wav")
    }
}

base {
    archivesName = "kitty-adventure"
}

/**
 * Builds libkitty_mobile.so with cargo-ndk for each ABI into a jniLibs directory that AGP
 * packages. Cargo tracks the Rust sources (including ../Engines) itself, so the task always runs
 * and leaves deciding what to rebuild to cargo.
 */
abstract class CargoNdkBuild : DefaultTask() {
    @get:Input
    abstract val abis: ListProperty<String>

    @get:Input
    abstract val apiLevel: Property<Int>

    @get:Internal
    abstract val crateDir: DirectoryProperty

    @get:Internal
    abstract val ndkDir: DirectoryProperty

    @get:OutputDirectory
    abstract val jniLibsDir: DirectoryProperty

    @get:Inject
    abstract val execOps: ExecOperations

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun build() {
        // Start empty so ABIs dropped from the list don't linger from an earlier build.
        val out = jniLibsDir.get().asFile
        out.deleteRecursively()
        out.mkdirs()
        // Android Studio may not have ~/.cargo/bin on its PATH.
        val cargo = File(System.getProperty("user.home"), ".cargo/bin/cargo")
            .takeIf { it.canExecute() }?.path ?: "cargo"
        // Android 15+ phones may use 16 KB memory pages, which need 16 KB aligned ELF segments;
        // NDK r25 links with 4 KB alignment by default.
        val rustFlags = listOfNotNull(System.getenv("RUSTFLAGS"), "-C link-arg=-Wl,-z,max-page-size=16384")
            .filter { it.isNotBlank() }.joinToString(" ")
        execOps.exec {
            workingDir = crateDir.get().asFile
            environment("ANDROID_NDK_HOME", ndkDir.get().asFile.path)
            environment("RUSTFLAGS", rustFlags)
            commandLine(cargo, "ndk", "--platform", apiLevel.get(), "--output-dir", out)
            args(abis.get().flatMap { listOf("--target", it) })
            args("build", "--release", "--lib")
        }
    }
}

/** Fails early, with the fix, when the game's art and sounds haven't been staged. */
val checkGameAssets by tasks.registering {
    val anchors = rustCrateDir.resolve("assets/kitty/anchors.json")
    doLast {
        check(anchors.isFile) {
            "Game assets are missing from ${anchors.parentFile.parent}: run ../build.sh assets (or ../build.sh android) first"
        }
    }
}

tasks.named("preBuild") {
    dependsOn(checkGameAssets)
}

androidComponents {
    onVariants { variant ->
        val cargoNdk = tasks.register<CargoNdkBuild>("cargoNdk${variant.name.replaceFirstChar { it.uppercase() }}") {
            group = "build"
            description = "Builds the game's native library for $rustAbis with cargo-ndk."
            abis = rustAbis
            apiLevel = minApi
            crateDir.set(rustCrateDir)
            ndkDir = androidComponents.sdkComponents.ndkDirectory
        }
        // Registers the libraries as generated jniLibs, so merge<Variant>JniLibFolders runs the
        // cargo build first.
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoNdk, CargoNdkBuild::jniLibsDir)
    }
}

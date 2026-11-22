import javax.inject.Inject

plugins {
    id("com.android.application")
}

/** The Rust crate (mobile/): its cdylib, libkitty_mobile.so, is the whole game. */
val rustCrateDir: File = rootDir.parentFile

/** The game library's name, as MainActivity loads it with System.loadLibrary. */
val rustLibrary = "kitty_mobile"

/** Android ABIs to build the game for (gradle.properties; -PrustAbis=arm64-v8a for a quick build). */
val rustAbis: List<String> = providers.gradleProperty("rustAbis").get()
    .split(",").map { it.trim() }.filter { it.isNotEmpty() }

/** Android 8.0; cargo-ndk links the game against the same API level. */
val minApi = 26

/**
 * The NDK cargo-ndk builds with: r25 is the oldest current Rust supports. AGP also uses it to
 * strip the libraries, and pinning it stops AGP looking for (and downloading) its own default.
 */
val ndkVersionUsed = "25.2.9519653"

/**
 * The debug signing key. It's the game's own, beside AGP's ~/.android/debug.keystore rather than
 * that file, which other tools can replace without AGP's AndroidDebugKey alias (cargo-quad-apk
 * did). One per user, not per checkout, so debug APKs from any clone or worktree install over each
 * other. Made on the first build.
 */
val debugKeystore: File = File(System.getProperty("user.home"), ".android/kitty-adventure-debug.keystore")
val debugKeyAlias = "AndroidDebugKey"
val debugKeyPassword = "android"

android {
    namespace = "com.kashimakei.kittyadventure"
    compileSdk = 35
    buildToolsVersion = "35.0.0"
    ndkVersion = ndkVersionUsed

    defaultConfig {
        applicationId = "com.kashimakei.kittyadventure"
        minSdk = minApi
        targetSdk = 35
        versionCode = 5
        versionName = "0.3.0"
    }

    signingConfigs {
        getByName("debug") {
            storeFile = debugKeystore
            storePassword = debugKeyPassword
            keyAlias = debugKeyAlias
            keyPassword = debugKeyPassword
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // The game opens its files by path from the asset root ("kitty/anchors.json",
    // "sounds/meow.wav"); build.sh stages them from ../assets into mobile/assets.
    sourceSets["main"].assets.srcDir(rustCrateDir.resolve("assets"))

    androidResources {
        // Keep the sounds uncompressed (aapt2's default for these types anyway), so the
        // AssetManager reads them straight out of the APK without inflating them.
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

    @get:Input
    abstract val library: Property<String>

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
        // cargo-ndk copies every cdylib the build makes, including dependencies that are cdylibs
        // too (lewton, ../Engines' Python module). Only the game, and libc++ if it ever needs it,
        // belongs in the APK.
        val game = "lib${library.get()}.so"
        val keep = setOf(game, "libc++_shared.so")
        out.walkTopDown().filter { it.isFile && it.name !in keep }.forEach { it.delete() }
        for (abi in abis.get()) {
            check(out.resolve("$abi/$game").isFile) { "cargo-ndk didn't build $game for $abi" }
        }
    }
}

/** Makes the debug keystore with the JDK's keytool, the way AGP makes ~/.android/debug.keystore. */
abstract class DebugKeystore : DefaultTask() {
    @get:Internal
    abstract val keystore: RegularFileProperty

    @get:Input
    abstract val alias: Property<String>

    @get:Input
    abstract val password: Property<String>

    @get:Inject
    abstract val execOps: ExecOperations

    init {
        onlyIf { !keystore.get().asFile.exists() }
    }

    @TaskAction
    fun create() {
        keystore.get().asFile.parentFile.mkdirs()
        val keytool = File(System.getProperty("java.home"), "bin/keytool")
        execOps.exec {
            commandLine(
                keytool, "-genkeypair", "-noprompt",
                "-keystore", keystore.get().asFile, "-storetype", "PKCS12",
                "-storepass", password.get(), "-keypass", password.get(), "-alias", alias.get(),
                "-keyalg", "RSA", "-keysize", "2048", "-validity", "10950",
                "-dname", "CN=Android Debug,O=Android,C=US",
            )
        }
    }
}

val debugKeystoreTask = tasks.register<DebugKeystore>("debugKeystore") {
    description = "Makes the debug signing keystore if it doesn't exist."
    keystore.set(debugKeystore)
    alias = debugKeyAlias
    password = debugKeyPassword
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
    dependsOn(checkGameAssets, debugKeystoreTask)
}

androidComponents {
    onVariants { variant ->
        val cargoNdk = tasks.register<CargoNdkBuild>("cargoNdk${variant.name.replaceFirstChar { it.uppercase() }}") {
            group = "build"
            description = "Builds the game's native library for $rustAbis with cargo-ndk."
            abis = rustAbis
            apiLevel = minApi
            library = rustLibrary
            crateDir.set(rustCrateDir)
            // AGP leaves sdkComponents.ndkDirectory unset here, so find the pinned NDK in the SDK.
            ndkDir = androidComponents.sdkComponents.sdkDirectory.map { it.dir("ndk/$ndkVersionUsed") }
        }
        // Registers the libraries as generated jniLibs, so merge<Variant>JniLibFolders runs the
        // cargo build first.
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoNdk, CargoNdkBuild::jniLibsDir)
    }
}

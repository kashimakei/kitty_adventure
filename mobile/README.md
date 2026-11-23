# Kitty Adventure for Android, web and desktop

Kitty Adventure: The Treetop Rescue, in Rust with [macroquad](https://macroquad.rs). A storm has
scattered Kitty's five friends across Whisker Wood. A map climbs through five regions of three
levels each (fixed layouts, picked from the map as they unlock); each level is a stretch of forest
three screens wide where she climbs bendy branches, dodges squirrels, dogs, crows, eagles and
snakes, and reaches a clue to her friend, or at a region's end the friend. Every level keeps its
best score and up to three stars, and rescuing the last friend opens an endless climb.

The same code runs on desktop, in the browser (wasm) and on Android. The branches use the same
physics as the pygame original in the repository root (`Engines/`, built here without its Python
bindings), and the art and sounds come from `../assets`.

Controls: on a phone, slide the bar at the bottom left to walk and run (the further, the faster),
hold the right half to crouch and let go to pounce (a tap hops, or climbs up when hanging), and
swipe down there to let go of a branch or drop through it; the II button or Back pauses. On a keyboard,
arrows or A/D to run (Shift walks), hold and release Space/Up/W to pounce, Down/S to let go or
drop through, Esc/P to pause. Branches bounce: hold through a landing and let go as the ring at
Kitty's feet glows blue to go higher, but bounce too high and the end of the branch snaps off.

## Building

```bash
./build.sh desktop    # run it in a window
./build.sh web        # build dist/; serve it with: python3 -m http.server -d dist 8000
./build.sh android    # build a debug APK: android/app/build/outputs/apk/debug/kitty-adventure-debug.apk
./build.sh install    # build it, install it on the phone connected over adb and start it
./build.sh assets     # just stage the art and sounds
cargo test --release  # levels are climbable, Kitty's physics, touch gestures, enemies, saves
```

Every target first stages the art and sounds the game uses from `../assets` into `assets/`
(git-ignored), which the desktop build reads from the working directory, the web build copies
into `dist/`, and the Android build packages at the APK's asset root.

`ABIS=arm64-v8a ./build.sh android` builds for 64-bit ARM phones only, which is quicker. By default
the APK also has `armeabi-v7a` (older 32-bit phones) and `x86_64` (the emulator).

## Prerequisites

- Rust via [rustup](https://rustup.rs). For the web build: `rustup target add wasm32-unknown-unknown`.
- For Android:
  - JDK 17.
  - The Android SDK in `~/Android/Sdk` (or set `ANDROID_HOME`) with `platforms;android-35`,
    `build-tools;35.0.0`, `platform-tools` and `ndk;25.2.9519653`. With the command-line tools:
    `sdkmanager "platforms;android-35" "build-tools;35.0.0" "platform-tools" "ndk;25.2.9519653"`
    (it asks you to accept Google's SDK licence).
  - `cargo install cargo-ndk` and
    `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`.

  You don't need Gradle itself: `android/gradlew` fetches Gradle 8.11.1 and the Android Gradle
  plugin on the first build.

## Installing on a phone

1. On the phone, turn on Developer options (tap Settings > About phone > Build number seven
   times), then USB debugging in Settings > System > Developer options.
2. Connect it by USB and accept the debugging prompt on the phone.
3. Run `./build.sh install`.

Without adb, copy the APK to the phone and open it there (allow installing unknown apps when
asked).

The APK is signed with a debug key that the first build makes in
`~/.android/kitty-adventure-debug.keystore`, so builds from any checkout install over each other.
Keep that file: an APK signed with a different key won't install over the app until you uninstall
it. The earlier spike (`com.kashimakei.kitty_mobile`) is a separate app; remove it with
`adb uninstall com.kashimakei.kitty_mobile`.

## How the Android app is built

`android/` is a plain Gradle project (Android Gradle plugin 8.7) for Android 8.0+ (API 26),
targeting API 35. You can open it in Android Studio after running `./build.sh assets` once.

- **The game** is this crate's `cdylib`, `libkitty_mobile.so`. The `cargoNdkDebug` task in
  `android/app/build.gradle.kts` builds it in release mode with cargo-ndk for each ABI (the
  `rustAbis` Gradle property, from `ABIS` in `build.sh`) into `app/build/generated/jniLibs`. It
  links with 16 KB page alignment, which phones with 16 KB memory pages (Android 15+) need.
- **The Java glue** is miniquad's own: `MainActivity` (package `com.kashimakei.kittyadventure`) is
  miniquad 0.4.11's `java/MainActivity.java` template filled in the way `cargo-quad-apk` does,
  plus immersive fullscreen, camera-cutout handling, `pixelsPerMm()` (so the Rust side can size
  touch gestures in millimetres) and a Back-gesture exclusion on the bottom 200 dp of each side
  edge, where thumbs slide and hold. `QuadNative` must stay in package `quad_native`, since the
  Rust side exports `Java_quad_1native_QuadNative_*`. When updating miniquad, diff its `java/`
  folder against these two files.
- **The assets** come straight from `mobile/assets`.
- **The launcher icons** are made from `../assets/kitty.png` by `android/make_icons.py` (needs
  Pillow); the PNGs are committed.

## Layout

```
mobile/
  build.sh          build and run targets (above)
  Cargo.toml        the kitty_mobile crate: a library (for Android) and the `kitty` binary
  src/
    lib.rs          screens (title, story, map, levels, pause, try again, level clear, clues,
                    endless) and the fixed 60 Hz loop
    campaign.rs     the five regions and fifteen levels: seeds, tuning, critters, clues, lines
    flow.rs         the rules of progress: unlocks, stars, bests, the level-clear tally
    map.rs          the scrolling map of Whisker Wood
    look.rs         each region's colours (sky, scenery, leaves, wood, ground)
    world.rs        one level: trees, Kitty, enemies, items, collisions, drawing
    level.rs        level generation from a LevelSpec, and the endless climb's ramp
    kitty.rs        Kitty: walking, the charge jump, bouncing, hanging, dropping, animation
    limb.rs tree.rs branches (wrapping the Engines/ physics) and trunks
    enemies/        squirrel, snake, crow, dog, eagle
    camera.rs       the 600x1200 playfield letterboxed to any screen, and the camera
    sprites.rs      sprite frames, anchors and pixel-accurate hit masks from assets/
    input/          keyboard, mouse and touch turned into controls; touch.rs has the play
                    gestures (slider, hold and release, swipe down) and menu.rs the menu ones
                    (taps, drags, flings, keys) as pure logic the tests drive
    ui.rs           buttons for the menus and cards
    platform.rs     Android hooks over JNI (screen density, going to the background)
    save.rs         progress as JSON (save.json, or localStorage on the web), merged on write
    hud.rs story.rs audio.rs items.rs effects.rs parallax.rs rng.rs consts.rs
  tests/            physics.rs (Kitty's jumps on the branches, snapping limbs and forks, and
                    that every level can be climbed with single leaps), climbable.rs (a search
                    over the physics that every campaign level can be climbed at its seed, and
                    endless levels on many seeds, even with the path's sturdy branches snapped
                    short and its thin ones snapped off; pick_campaign_seeds, ignored, picks
                    seeds after a level's tuning changes), touch.rs and menu.rs (gestures),
                    bops.rs, spawner.rs, squirrel.rs
  web/              index.html and macroquad's JS loader for the web build
  android/          the Gradle project
    app/build.gradle.kts            app settings and the cargo-ndk build
    app/src/main/AndroidManifest.xml
    app/src/main/java/              MainActivity and QuadNative (miniquad's Java glue)
    app/src/main/res/               launcher icons, app name, theme
    make_icons.py                   regenerates the launcher icons
```

iOS isn't planned for now: macroquad supports it, but building needs a Mac with Xcode and an
Apple developer account.

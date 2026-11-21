# macroquad spike

A small Rust/macroquad build of Kitty Adventure, to find out what a port that runs on
desktop, web, Android and iOS from one codebase would take before committing to it.

It has one level's worth of the hard parts: two trees with bendy branches from the same
`Engines/` physics the pygame game uses, Kitty with walk/sit/jump/fall/land animations from
`assets/kitty/`, touch buttons alongside the keyboard, a fixed 60 Hz physics step, and the
600×1200 playfield scaled to fit any screen.

## Running it

```bash
./build.sh desktop        # opens a window
./build.sh web            # builds dist/; serve with: python3 -m http.server -d dist 8000
cargo test --release      # Kitty lands on a branch, bends it, and it springs back
```

`build.sh` stages the frames from `../assets/kitty/` into `assets/` (git-ignored) first. The
web build needs `rustup target add wasm32-unknown-unknown` once.

Controls: arrows/A/D to walk and Space/Up to jump, or the on-screen buttons.

## What the spike showed

**Works**
- `Engines/` builds without Python: the PyO3 bindings are now behind a `python` feature (on by
  default, so `maturin develop` and the pygame game are unchanged). The macroquad build uses
  the same `Branch` code directly.
- The sprite pipeline carries over as-is: the PNG frames and `anchors.json` from
  `tools/slice_sprites.py` load straight into Rust.
- Web: an 857 KB wasm file (300 KB gzipped). It ran at 60 fps in the browser, with scaling,
  touch/mouse buttons, jumping, landing and walking all working.
- The native desktop build compiles cleanly; the physics test passes.

**Problems found and fixed**
- With a fixed timestep, a tap on a frame that ran no physics step was dropped (common at
  60 Hz, where frame times hover just under 1/60 s). Taps are now queued for the next step.
- Quick taps could fall between frames; jumping now uses tap events instead of polling.
- A 330 px branch sagged 73 px under Kitty and curled back past its own tip. The spike uses
  the game's 170–220 px branch sizes; branch bend grows with the cube of its length.

**To do in a real port**
- Loading: 40 separate frame images plus a 3.4 MB background PNG take a few seconds on the
  web. Pack frames into one atlas, ship a smaller background, and show a loading screen.
- `sample_count: 4` logs WebGL warnings in the browser; use no multisampling on web.

## Android

```bash
./build.sh android    # -> target/android-artifacts/release/apk/kitty_mobile.apk
~/Android/Sdk/platform-tools/adb install -r target/android-artifacts/release/apk/kitty_mobile.apk
```

This builds a 4.8 MB APK for Android 8.0+ on 64- and 32-bit ARM phones, signed with a debug key.

**Tested on a Pixel 9 Pro (Android 17):** it runs at the screen's 120 Hz. Touch input, jumping
onto and walking along branches, and scaling to the phone's taller-than-1:2 screen all work.
Physics steps at a fixed 60 Hz, so on 120 Hz screens every physics state is drawn twice; a real
port should blend between steps for the smoothest motion.
One-time setup (the `sdkmanager` step asks you to accept Google's SDK licence):

```bash
sudo apt install openjdk-17-jdk-headless
# Android command-line tools unpacked to ~/Android/Sdk/cmdline-tools/latest, then:
~/Android/Sdk/cmdline-tools/latest/bin/sdkmanager "platform-tools" "platforms;android-30" "build-tools;30.0.3" "ndk;25.2.9519653"
cargo install cargo-quad-apk
rustup target add aarch64-linux-android armv7-linux-androideabi
```

**What it took:** macroquad's packager, `cargo-quad-apk` 0.1.4, is old. It needs build-tools
30.0.3 (the last with `dx`), a v3 `Cargo.lock` (its bundled Cargo can't read v4), and the
shims in `setup_android.sh` to work with NDK r25 (which current Rust requires) and a current JDK.
`build.sh android` runs that script for you. It also names the native library it loads after
the last part of the app ID, so the ID has to end in `kitty_mobile`.

**Before a Play Store release:** this pipeline targets API 30 and produces APKs. Google Play
now wants a recent target API and App Bundles (AAB), which `cargo-quad-apk` can't make. A
real release would build the same Rust library and wrap it in a small Gradle project instead.
That's a known pattern, but it's extra setup to budget for.

## iOS

Not planned for now: Android comes first. macroquad supports iOS, but building it needs a Mac
with Xcode (or a cloud Mac) and an Apple developer account.

## Size of a full port

The pygame game is about 2,000 lines of Python (not counting the physics sandbox). This
spike's ~470 lines of Rust cover the frame loop, input, scaling, sprites, one character's
movement and the branch integration. Still to port: level generation and the 3-screen camera,
Kitty's full state machine (idle fidgets, hanging, climbing), dogs, eagles, damage masks and
the body hurtbox, breakable branches, wind, HUD and minimap, sound, splash and game-over
screens. Expect roughly 2,500–3,000 lines of Rust in total.

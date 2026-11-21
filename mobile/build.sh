#!/usr/bin/env bash
# Build Kitty Adventure.
#   ./build.sh desktop   run it in a window
#   ./build.sh web       build dist/ (serve it with: python3 -m http.server -d dist 8000)
#   ./build.sh android   build a debug APK with Gradle (needs the Android SDK, NDK and cargo-ndk;
#                        see README.md)
#   ./build.sh install   build the debug APK, install it on the phone connected over adb and start it
#   ./build.sh assets    just stage the art and sounds from ../assets
# ABIS=arm64-v8a ./build.sh android   builds for 64-bit ARM phones only, which is quicker.
set -euo pipefail
cd "$(dirname "$0")"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
APK=android/app/build/outputs/apk/debug/kitty-adventure-debug.apk
APP_ID=com.kashimakei.kittyadventure

stage_assets() {
    rm -rf assets
    mkdir -p assets/sounds
    for character in kitty dog eagle squirrel snake crow friends items foliage; do
        mkdir -p "assets/$character"
        cp ../assets/"$character"/*.png ../assets/"$character"/anchors.json "assets/$character/"
    done
    cp ../assets/Background_lvl1.png assets/background.png
    mkdir -p assets/parallax
    cp ../assets/parallax/*.png assets/parallax/
    cp ../assets/meow.wav ../assets/meow2.wav ../assets/hiss.wav ../assets/crunch.wav ../assets/BGM.ogg assets/sounds/
}

# Gradle builds the game library with cargo-ndk (android/app/build.gradle.kts), packages it
# with mobile/assets and the Java glue, and signs the APK with the SDK's debug key.
build_apk() {
    stage_assets
    local gradle_args=(assembleDebug)
    if [ -n "${ABIS:-}" ]; then
        gradle_args+=("-PrustAbis=$ABIS")
    fi
    (cd android && ./gradlew "${gradle_args[@]}")
    echo "Built mobile/$APK ($(du -h "$APK" | cut -f1))"
}

case "${1:-web}" in
    assets)
        stage_assets ;;
    desktop)
        stage_assets
        cargo run --release ;;
    web)
        stage_assets
        cargo build --release --target wasm32-unknown-unknown
        rm -rf dist
        mkdir -p dist
        cp web/index.html web/mq_js_bundle.js dist/
        cp target/wasm32-unknown-unknown/release/kitty.wasm dist/
        cp -r assets/* dist/
        echo "Built dist/ ($(du -sh dist | cut -f1)); serve with: python3 -m http.server -d dist 8000" ;;
    android)
        build_apk ;;
    install)
        build_apk
        adb="$ANDROID_HOME/platform-tools/adb"
        "$adb" install -r "$APK"
        "$adb" shell am start -n "$APP_ID/.MainActivity" ;;
    *)
        echo "usage: $0 [desktop|web|android|install|assets]" >&2
        exit 1 ;;
esac

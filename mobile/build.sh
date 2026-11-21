#!/usr/bin/env bash
# Build the macroquad spike.
#   ./build.sh desktop   run it in a window
#   ./build.sh web       build dist/ (serve it with: python3 -m http.server -d dist 8000)
#   ./build.sh android   build an APK (needs the Android SDK/NDK; see README.md)
#   ./build.sh assets    just stage the sprites from ../assets
set -euo pipefail
cd "$(dirname "$0")"

stage_assets() {
    rm -rf assets
    mkdir -p assets/kitty
    cp ../assets/kitty/*.png ../assets/kitty/anchors.json assets/kitty/
    cp ../assets/Background_lvl1.png assets/background.png
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
        cp target/wasm32-unknown-unknown/release/kitty_mobile.wasm dist/
        cp -r assets/* dist/
        echo "Built dist/ ($(du -sh dist | cut -f1)); serve with: python3 -m http.server -d dist 8000" ;;
    android)
        stage_assets
        export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
        export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/25.2.9519653}"
        ./setup_android.sh
        PATH="$PWD/target/android-shims:$PATH" cargo quad-apk build --release
        echo "Built $(ls target/android-artifacts/release/apk/*.apk)" ;;
    *)
        echo "usage: $0 [desktop|web|android|assets]" >&2
        exit 1 ;;
esac

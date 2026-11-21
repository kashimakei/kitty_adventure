#!/usr/bin/env bash
# One-time shims so cargo-quad-apk 0.1.4 (written for NDK r22 and Java 8) works with NDK r25,
# the oldest NDK current Rust supports for Android, and a current JDK. Safe to re-run.
#
#  - It calls per-target `<triple>-ld`, `-ar` and `-readelf`; NDK r23+ only ships the LLVM tools.
#  - It links against libgcc; NDK r23+ replaced libgcc with libunwind, so a stub libgcc.a points
#    the linker at libunwind instead.
#  - It runs `java -verbose` to find Java 8's rt.jar for javac's boot classpath; Java 9+ has no
#    rt.jar. For Android the right boot classpath is the SDK's android.jar anyway, so build.sh
#    puts target/android-shims first on PATH, where a `java` wrapper reports android.jar.
set -euo pipefail
cd "$(dirname "$0")"

ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/25.2.9519653}"
ANDROID_JAR="$ANDROID_HOME/platforms/android-30/android.jar"
TOOLCHAIN="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64"
[ -d "$TOOLCHAIN" ] || { echo "NDK not found at $NDK_HOME" >&2; exit 1; }
[ -f "$ANDROID_JAR" ] || { echo "android.jar not found at $ANDROID_JAR" >&2; exit 1; }

for pair in aarch64-linux-android:aarch64 arm-linux-androideabi:arm; do
    triple="${pair%%:*}"
    arch="${pair##*:}"
    ln -sf ld.lld "$TOOLCHAIN/bin/$triple-ld"
    ln -sf llvm-ar "$TOOLCHAIN/bin/$triple-ar"
    ln -sf llvm-readelf "$TOOLCHAIN/bin/$triple-readelf"

    gcc_dir="$TOOLCHAIN/lib/gcc/$triple/4.9.x"
    mkdir -p "$gcc_dir"
    echo "INPUT(-lunwind)" > "$gcc_dir/libgcc.a"
    unwind="$(find "$TOOLCHAIN/lib64/clang" -path "*/lib/linux/$arch/libunwind.a" | head -1)"
    [ -n "$unwind" ] || { echo "libunwind.a for $arch not found" >&2; exit 1; }
    ln -sf "$unwind" "$gcc_dir/libunwind.a"
done

shims="$PWD/target/android-shims"
mkdir -p "$shims"
ln -sf "$ANDROID_JAR" "$shims/rt.jar"
real_java="$(command -v java)"
cat > "$shims/java" <<EOF
#!/bin/sh
# cargo-quad-apk only runs \`java -verbose\` to find rt.jar; answer with the SDK's android.jar.
if [ "\$1" = "-verbose" ]; then
    echo "[Opened $shims/rt.jar]"
    exit 0
fi
exec "$real_java" "\$@"
EOF
chmod +x "$shims/java"
echo "Android toolchain shims ready"

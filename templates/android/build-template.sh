#!/usr/bin/env bash
set -euo pipefail

# Build Android runtime template APKs (one per architecture).
#
# This script handles everything: env detection, Rust cross-compilation,
# native library bundling, Gradle build.
#
# Prerequisites (install once):
#   1. Android Studio (includes SDK, NDK, Java)
#   2. cargo-ndk:  cargo install cargo-ndk
#   3. Rust Android targets:
#        rustup target add aarch64-linux-android --toolchain nightly
#        rustup target add x86_64-linux-android --toolchain nightly
#
# Usage:
#   ./templates/android/build-template.sh              # Android ARM64 (Vulkan)
#   ./templates/android/build-template.sh --x86_64     # Android x86_64 (Vulkan)
#   ./templates/android/build-template.sh --firetv     # Fire TV ARM64 (Vulkan)
#   ./templates/android/build-template.sh --all        # Build all templates
#   ./templates/android/build-template.sh --firetv --x86_64   # Multiple targets

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
ANDROID_DIR="$SCRIPT_DIR"
ANDROID_CRATE="$PROJECT_ROOT/crates/platform/renzora_android"
JNILIBS_DIR="$ANDROID_DIR/app/src/main/jniLibs"

BUILD_ARM64=false
BUILD_X86_64=false
BUILD_FIRETV=false

for arg in "$@"; do
    case "$arg" in
        --arm64)    BUILD_ARM64=true ;;
        --x86_64)   BUILD_X86_64=true ;;
        --firetv)   BUILD_FIRETV=true ;;
        --all)      BUILD_ARM64=true; BUILD_X86_64=true; BUILD_FIRETV=true ;;
        *)          echo "Unknown option: $arg"; exit 1 ;;
    esac
done

# Default: build Android ARM64 if nothing specified
if [ "$BUILD_ARM64" = false ] && [ "$BUILD_X86_64" = false ] && [ "$BUILD_FIRETV" = false ]; then
    BUILD_ARM64=true
fi

# --- Auto-detect environment ---

# Java
if [ -z "${JAVA_HOME:-}" ]; then
    if [ -d "$PROGRAMFILES/Android/Android Studio/jbr" ]; then
        export JAVA_HOME="$PROGRAMFILES/Android/Android Studio/jbr"
    elif [ -d "/Applications/Android Studio.app/Contents/jbr/Contents/Home" ]; then
        export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
    else
        echo "Error: JAVA_HOME not set and Android Studio JBR not found."
        echo "  Set JAVA_HOME or install Android Studio."
        exit 1
    fi
fi
echo "JAVA_HOME: $JAVA_HOME"

# Android SDK
if [ -z "${ANDROID_HOME:-}" ]; then
    if [ -d "$LOCALAPPDATA/Android/Sdk" ]; then
        export ANDROID_HOME="$LOCALAPPDATA/Android/Sdk"
    elif [ -d "$HOME/Library/Android/sdk" ]; then
        export ANDROID_HOME="$HOME/Library/Android/sdk"
    elif [ -d "$HOME/Android/Sdk" ]; then
        export ANDROID_HOME="$HOME/Android/Sdk"
    else
        echo "Error: ANDROID_HOME not set and Android SDK not found."
        exit 1
    fi
fi
echo "ANDROID_HOME: $ANDROID_HOME"

# Android NDK
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    NDK_DIR="$ANDROID_HOME/ndk"
    if [ -d "$NDK_DIR" ]; then
        ANDROID_NDK_HOME="$NDK_DIR/$(ls "$NDK_DIR" | sort -V | tail -1)"
        export ANDROID_NDK_HOME
    else
        echo "Error: No NDK found in $NDK_DIR. Install via Android Studio SDK Manager."
        exit 1
    fi
fi
echo "ANDROID_NDK_HOME: $ANDROID_NDK_HOME"

# Ensure local.properties exists for Gradle
if [ ! -f "$ANDROID_DIR/local.properties" ]; then
    echo "sdk.dir=$(echo "$ANDROID_HOME" | sed 's|/|\\\\|g')" > "$ANDROID_DIR/local.properties"
fi

# Check cargo-ndk
if ! command -v cargo-ndk &>/dev/null; then
    echo "Error: cargo-ndk not found. Install with: cargo install cargo-ndk"
    exit 1
fi

# NDK sysroot for libc++_shared.so
NDK_SYSROOT="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt"
HOST_DIR="$(ls "$NDK_SYSROOT" | head -1)"
NDK_LIBS="$NDK_SYSROOT/$HOST_DIR/sysroot/usr/lib"

# Gradle command
cd "$ANDROID_DIR"
if [ -f "./gradlew.bat" ] && [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
    GRADLE_CMD="./gradlew.bat"
else
    GRADLE_CMD="./gradlew"
fi

# Gradle task is set per build_arch call via FLAVOR parameter

# Templates directory
if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
    TEMPLATES_DIR="$APPDATA/renzora/templates"
else
    TEMPLATES_DIR="$HOME/.config/renzora/templates"
fi
mkdir -p "$TEMPLATES_DIR"

OUTPUT_DIR="$PROJECT_ROOT/target/templates"
mkdir -p "$OUTPUT_DIR"

# --- Helper: build one architecture ---

build_arch() {
    local RUST_TARGET="$1"
    local ABI="$2"
    local TEMPLATE_NAME="$3"
    local FLAVOR="$4"
    local MIN_PLATFORM="${5:-30}"

    local FLAVOR_CAP="$(echo "$FLAVOR" | sed 's/./\U&/')"
    local TASK="assemble${FLAVOR_CAP}Release"
    local APK_PATH="$ANDROID_DIR/app/build/outputs/apk/$FLAVOR/release/app-${FLAVOR}-release-unsigned.apk"

    echo ""
    echo "=== Building $TEMPLATE_NAME ==="
    echo "    Arch: $ABI | Flavor: $FLAVOR | API: $MIN_PLATFORM"
    echo ""

    # Build native library
    echo "--- Building native library: $ABI ---"
    cd "$ANDROID_CRATE"
    cargo ndk --target "$RUST_TARGET" --platform "$MIN_PLATFORM" build --release

    # Clean jniLibs and copy only this arch
    rm -rf "$JNILIBS_DIR"
    mkdir -p "$JNILIBS_DIR/$ABI"
    cp "$ANDROID_CRATE/target/$RUST_TARGET/release/libmain.so" "$JNILIBS_DIR/$ABI/libmain.so"
    cp "$NDK_LIBS/$RUST_TARGET/libc++_shared.so" "$JNILIBS_DIR/$ABI/"
    echo "  -> $ABI: libmain.so + libc++_shared.so"

    # Build APK
    echo ""
    echo "--- Building APK: $TASK ---"
    cd "$ANDROID_DIR"
    "$GRADLE_CMD" ":app:$TASK"

    if [ ! -f "$APK_PATH" ]; then
        echo "Error: APK not found at $APK_PATH"
        exit 1
    fi

    cp "$APK_PATH" "$TEMPLATES_DIR/$TEMPLATE_NAME"
    cp "$APK_PATH" "$OUTPUT_DIR/$TEMPLATE_NAME"
    echo ""
    echo "  Template: $TEMPLATES_DIR/$TEMPLATE_NAME"
}

# --- Build selected targets ---
#                    rust-target              abi          template-name                         flavor     api

[ "$BUILD_ARM64" = true ] && \
    build_arch "aarch64-linux-android"  "arm64-v8a"  "renzora-runtime-android-arm64.apk"   "standard"  30

[ "$BUILD_X86_64" = true ] && \
    build_arch "x86_64-linux-android"   "x86_64"     "renzora-runtime-android-x86_64.apk"  "standard"  30

[ "$BUILD_FIRETV" = true ] && \
    build_arch "aarch64-linux-android"  "arm64-v8a"  "renzora-runtime-firetv-arm64.apk"    "firetv"    30

# --- Clean up ---

rm -rf "$JNILIBS_DIR"

echo ""
echo "=== Done! ==="
echo ""
echo "Export from the editor to build a signed APK ready to install."
echo ""

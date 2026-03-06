#!/usr/bin/env bash
set -euo pipefail

# Build the Android runtime template APK.
#
# This script handles everything: env detection, Rust cross-compilation,
# native library bundling, Gradle build, and optional signing.
#
# Prerequisites (install once):
#   1. Android Studio (includes SDK, NDK, Java)
#   2. cargo-ndk:  cargo install cargo-ndk
#   3. Rust Android targets:
#        rustup target add aarch64-linux-android --toolchain nightly
#        rustup target add x86_64-linux-android --toolchain nightly
#
# Usage:
#   ./scripts/build-android-template.sh              # Build arm64 template
#   ./scripts/build-android-template.sh --x86_64     # Also build x86_64 (for emulator)
#   ./scripts/build-android-template.sh --sign       # Sign the APK after build
#   ./scripts/build-android-template.sh --install    # Sign + install to connected device
#   ./scripts/build-android-template.sh --firetv     # Build Fire TV variant instead

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ANDROID_DIR="$PROJECT_ROOT/android"
ANDROID_CRATE="$PROJECT_ROOT/crates/platform/renzora_android"
JNILIBS_DIR="$ANDROID_DIR/app/src/main/jniLibs"

BUILD_X86_64=false
SIGN_APK=false
INSTALL_APK=false
FLAVOR="standard"

for arg in "$@"; do
    case "$arg" in
        --x86_64)   BUILD_X86_64=true ;;
        --sign)     SIGN_APK=true ;;
        --install)  SIGN_APK=true; INSTALL_APK=true ;;
        --firetv)   FLAVOR="firetv" ;;
        *)          echo "Unknown option: $arg"; exit 1 ;;
    esac
done

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
        # Pick the latest installed NDK version
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

# --- Build native libraries ---

echo ""
echo "=== Building Android runtime template ($FLAVOR) ==="
echo ""

# ARM64 (always)
echo "--- Building native library: aarch64 (ARM64) ---"
cd "$ANDROID_CRATE"
cargo ndk --target aarch64-linux-android --platform 30 build --release

mkdir -p "$JNILIBS_DIR/arm64-v8a"
cp "$ANDROID_CRATE/target/aarch64-linux-android/release/libmain.so" "$JNILIBS_DIR/arm64-v8a/libmain.so"

# Copy libc++_shared.so from NDK
NDK_SYSROOT="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt"
# Find the host platform dir (windows-x86_64, linux-x86_64, darwin-x86_64)
HOST_DIR="$(ls "$NDK_SYSROOT" | head -1)"
NDK_LIBS="$NDK_SYSROOT/$HOST_DIR/sysroot/usr/lib"

cp "$NDK_LIBS/aarch64-linux-android/libc++_shared.so" "$JNILIBS_DIR/arm64-v8a/"
echo "  -> arm64-v8a: libmain.so + libc++_shared.so"

# x86_64 (optional, for emulator testing)
if [ "$BUILD_X86_64" = true ]; then
    echo "--- Building native library: x86_64 (emulator) ---"
    cargo ndk --target x86_64-linux-android --platform 30 build --release

    mkdir -p "$JNILIBS_DIR/x86_64"
    cp "$ANDROID_CRATE/target/x86_64-linux-android/release/libmain.so" "$JNILIBS_DIR/x86_64/libmain.so"
    cp "$NDK_LIBS/x86_64-linux-android/libc++_shared.so" "$JNILIBS_DIR/x86_64/"
    echo "  -> x86_64: libmain.so + libc++_shared.so"
fi

# --- Build APK with Gradle ---

FLAVOR_CAP="$(echo "$FLAVOR" | sed 's/./\U&/')"
TASK="assemble${FLAVOR_CAP}Release"

echo ""
echo "--- Building APK: $TASK ---"
cd "$ANDROID_DIR"

if [ -f "./gradlew.bat" ] && [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
    GRADLE_CMD="./gradlew.bat"
else
    GRADLE_CMD="./gradlew"
fi

"$GRADLE_CMD" ":app:$TASK"

# --- Copy template ---

APK_PATH="$ANDROID_DIR/app/build/outputs/apk/$FLAVOR/release/app-${FLAVOR}-release-unsigned.apk"

if [ ! -f "$APK_PATH" ]; then
    echo "Error: APK not found at $APK_PATH"
    exit 1
fi

# Copy to templates dir
if [ "$FLAVOR" = "firetv" ]; then
    TEMPLATE_NAME="renzora-runtime-firetv-arm64.apk"
else
    TEMPLATE_NAME="renzora-runtime-android-arm64.apk"
fi

# Install to user templates directory
if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
    TEMPLATES_DIR="$APPDATA/renzora/templates"
else
    TEMPLATES_DIR="$HOME/.config/renzora/templates"
fi
mkdir -p "$TEMPLATES_DIR"
cp "$APK_PATH" "$TEMPLATES_DIR/$TEMPLATE_NAME"

# Also keep a copy in build/
OUTPUT_DIR="$PROJECT_ROOT/build/templates"
mkdir -p "$OUTPUT_DIR"
cp "$APK_PATH" "$OUTPUT_DIR/$TEMPLATE_NAME"

echo ""
echo "  Template: $TEMPLATES_DIR/$TEMPLATE_NAME"
echo "  Copy:     $OUTPUT_DIR/$TEMPLATE_NAME"

# --- Sign (optional) ---

if [ "$SIGN_APK" = true ]; then
    echo ""
    echo "--- Signing APK ---"

    # Find apksigner
    APKSIGNER=""
    for bt_dir in "$ANDROID_HOME"/build-tools/*/; do
        if [ -f "${bt_dir}apksigner.bat" ]; then
            APKSIGNER="${bt_dir}apksigner.bat"
        elif [ -f "${bt_dir}apksigner" ]; then
            APKSIGNER="${bt_dir}apksigner"
        fi
    done

    if [ -z "$APKSIGNER" ]; then
        echo "Error: apksigner not found in $ANDROID_HOME/build-tools/"
        exit 1
    fi

    KEYSTORE="$HOME/.android/debug.keystore"
    if [ ! -f "$KEYSTORE" ]; then
        echo "  Creating debug keystore..."
        "$JAVA_HOME/bin/keytool" -genkeypair -v \
            -keystore "$KEYSTORE" \
            -alias androiddebugkey \
            -keyalg RSA -keysize 2048 -validity 10000 \
            -storepass android -keypass android \
            -dname "CN=Debug,O=Android,C=US"
    fi

    "$APKSIGNER" sign \
        --ks "$KEYSTORE" \
        --ks-pass pass:android \
        --key-pass pass:android \
        "$TEMPLATES_DIR/$TEMPLATE_NAME"

    echo "  Signed: $TEMPLATES_DIR/$TEMPLATE_NAME"
fi

# --- Install to device (optional) ---

if [ "$INSTALL_APK" = true ]; then
    echo ""
    echo "--- Installing on device ---"
    ADB="$ANDROID_HOME/platform-tools/adb"
    if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
        ADB="${ADB}.exe"
    fi
    "$ADB" install -r "$TEMPLATES_DIR/$TEMPLATE_NAME"
fi

echo ""
echo "=== Done! ==="
echo ""
echo "The template APK is UNSIGNED (so the editor can inject game assets)."
echo "After exporting from the editor, sign the final APK with:"
echo ""
echo "  ./scripts/sign-apk.sh path/to/exported.apk"
echo ""

#!/usr/bin/env bash
set -euo pipefail

# Sign an APK with the debug keystore.
#
# Usage:
#   ./scripts/sign-apk.sh path/to/game.apk
#   ./scripts/sign-apk.sh path/to/game.apk --install   # Sign + install to device

if [ $# -lt 1 ]; then
    echo "Usage: $0 <apk-path> [--install]"
    exit 1
fi

APK_PATH="$1"
INSTALL=false
[ "${2:-}" = "--install" ] && INSTALL=true

if [ ! -f "$APK_PATH" ]; then
    echo "Error: APK not found: $APK_PATH"
    exit 1
fi

# Auto-detect Java
if [ -z "${JAVA_HOME:-}" ]; then
    if [ -d "$PROGRAMFILES/Android/Android Studio/jbr" ]; then
        export JAVA_HOME="$PROGRAMFILES/Android/Android Studio/jbr"
    elif [ -d "/Applications/Android Studio.app/Contents/jbr/Contents/Home" ]; then
        export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
    fi
fi

# Auto-detect Android SDK
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

# Ensure debug keystore exists
KEYSTORE="$HOME/.android/debug.keystore"
if [ ! -f "$KEYSTORE" ]; then
    echo "Creating debug keystore..."
    mkdir -p "$HOME/.android"
    "$JAVA_HOME/bin/keytool" -genkeypair -v \
        -keystore "$KEYSTORE" \
        -alias androiddebugkey \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass android -keypass android \
        -dname "CN=Debug,O=Android,C=US"
fi

echo "Signing: $APK_PATH"
"$APKSIGNER" sign \
    --ks "$KEYSTORE" \
    --ks-pass pass:android \
    --key-pass pass:android \
    "$APK_PATH"
echo "Signed!"

if [ "$INSTALL" = true ]; then
    ADB="$ANDROID_HOME/platform-tools/adb"
    [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]] && ADB="${ADB}.exe"
    echo "Installing..."
    "$ADB" install -r "$APK_PATH"
    echo "Done!"
fi

#!/usr/bin/env bash
set -euo pipefail

# Build iOS / tvOS runtime template (.app bundle as .zip).
#
# This script cross-compiles the Rust static library, then builds
# the Xcode project to produce a .app bundle.
#
# Prerequisites (macOS only):
#   1. Xcode with iOS/tvOS SDK (xcode-select --install)
#   2. Rust targets:
#        rustup target add aarch64-apple-ios --toolchain nightly
#        rustup target add aarch64-apple-ios-sim --toolchain nightly
#        rustup target add aarch64-apple-tvos --toolchain nightly
#
# Usage:
#   ./templates/ios/build-template.sh                    # iOS Device (ARM64)
#   ./templates/ios/build-template.sh --simulator        # iOS Simulator (ARM64)
#   ./templates/ios/build-template.sh --tvos             # Apple TV (ARM64)
#   ./templates/ios/build-template.sh --tvos-simulator   # Apple TV Simulator (ARM64)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
IOS_DIR="$SCRIPT_DIR"
IOS_CRATE="$PROJECT_ROOT/crates/platform/renzora_ios"
LIBS_DIR="$IOS_DIR/libs"

MODE="ios"

for arg in "$@"; do
    case "$arg" in
        --simulator)       MODE="ios-sim" ;;
        --tvos)            MODE="tvos" ;;
        --tvos-simulator)  MODE="tvos-sim" ;;
        *)                 echo "Unknown option: $arg"; exit 1 ;;
    esac
done

case "$MODE" in
    ios)
        RUST_TARGET="aarch64-apple-ios"
        TEMPLATE_NAME="renzora-runtime-ios-arm64"
        SDK="iphoneos"
        INFOPLIST="RenzoraRuntime/Info.plist"
        DEPLOYMENT_TARGET="IPHONEOS_DEPLOYMENT_TARGET=16.0"
        echo "=== Building iOS Device (ARM64) ==="
        ;;
    ios-sim)
        RUST_TARGET="aarch64-apple-ios-sim"
        TEMPLATE_NAME="renzora-runtime-ios-arm64-sim"
        SDK="iphonesimulator"
        INFOPLIST="RenzoraRuntime/Info.plist"
        DEPLOYMENT_TARGET="IPHONEOS_DEPLOYMENT_TARGET=16.0"
        echo "=== Building iOS Simulator (ARM64) ==="
        ;;
    tvos)
        RUST_TARGET="aarch64-apple-tvos"
        TEMPLATE_NAME="renzora-runtime-tvos-arm64"
        SDK="appletvos"
        INFOPLIST="RenzoraRuntime/tvos/Info.plist"
        DEPLOYMENT_TARGET="TVOS_DEPLOYMENT_TARGET=16.0"
        echo "=== Building Apple TV (ARM64) ==="
        ;;
    tvos-sim)
        RUST_TARGET="aarch64-apple-tvos-sim"
        TEMPLATE_NAME="renzora-runtime-tvos-arm64-sim"
        SDK="appletvsimulator"
        INFOPLIST="RenzoraRuntime/tvos/Info.plist"
        DEPLOYMENT_TARGET="TVOS_DEPLOYMENT_TARGET=16.0"
        echo "=== Building Apple TV Simulator (ARM64) ==="
        ;;
esac

# --- Verify environment ---

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "Error: iOS/tvOS builds require macOS with Xcode."
    exit 1
fi

if ! command -v xcrun &>/dev/null; then
    echo "Error: Xcode command-line tools not found."
    echo "  Install with: xcode-select --install"
    exit 1
fi

if ! rustup target list --installed | grep -q "$RUST_TARGET"; then
    echo "Error: Rust target $RUST_TARGET not installed."
    echo "  Install with: rustup target add $RUST_TARGET"
    exit 1
fi

# Templates directory
if [[ "$(uname -s)" == "Darwin" ]]; then
    TEMPLATES_DIR="$HOME/Library/Application Support/renzora/templates"
else
    TEMPLATES_DIR="$HOME/.config/renzora/templates"
fi
mkdir -p "$TEMPLATES_DIR"

OUTPUT_DIR="$PROJECT_ROOT/target/templates"
mkdir -p "$OUTPUT_DIR"

# --- Build Rust static library ---

echo ""
echo "--- Building static library: $RUST_TARGET ---"
cd "$IOS_CRATE"
cargo build --target "$RUST_TARGET" --release

# Copy static library to ios/libs/ for Xcode
mkdir -p "$LIBS_DIR"
cp "$IOS_CRATE/target/$RUST_TARGET/release/librenzora_ios.a" "$LIBS_DIR/librenzora_ios.a"
echo "  -> librenzora_ios.a"

# --- Build with xcodebuild ---

echo ""
echo "--- Building Xcode project ($SDK) ---"
cd "$IOS_DIR"

xcodebuild \
    -project RenzoraRuntime.xcodeproj \
    -target RenzoraRuntime \
    -configuration Release \
    -sdk "$SDK" \
    -arch arm64 \
    LIBRARY_SEARCH_PATHS="$LIBS_DIR" \
    INFOPLIST_FILE="$INFOPLIST" \
    "$DEPLOYMENT_TARGET" \
    SDKROOT="$SDK" \
    CODE_SIGN_IDENTITY="-" \
    CODE_SIGNING_ALLOWED=NO \
    BUILD_DIR="$IOS_DIR/build" \
    clean build

APP_PATH="$IOS_DIR/build/Release-${SDK}/RenzoraRuntime.app"
if [ ! -d "$APP_PATH" ]; then
    echo "Error: .app bundle not found at $APP_PATH"
    exit 1
fi

# --- Package as zip template ---

echo ""
echo "--- Packaging template ---"
cd "$IOS_DIR/build/Release-${SDK}"
zip -r "$TEMPLATES_DIR/${TEMPLATE_NAME}.zip" RenzoraRuntime.app
cp "$TEMPLATES_DIR/${TEMPLATE_NAME}.zip" "$OUTPUT_DIR/${TEMPLATE_NAME}.zip"

# --- Clean up ---

rm -rf "$LIBS_DIR"
rm -rf "$IOS_DIR/build"

echo ""
echo "=== Done! ==="
echo ""
echo "Template: $TEMPLATES_DIR/${TEMPLATE_NAME}.zip"
echo ""
echo "Export from the editor to inject game assets and sign for distribution."
echo ""

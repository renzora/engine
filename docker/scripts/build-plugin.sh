#!/usr/bin/env bash
# =============================================================================
# Plugin Build Script — used by the marketplace build server
# =============================================================================
#
# Takes a plugin source directory, adds it to the workspace, builds for all
# desktop targets, and outputs signed DLLs.
#
# Usage:
#   ./scripts/build-plugin.sh /path/to/plugin/source /path/to/output
#
# Environment:
#   SIGNING_KEY_PATH — path to ed25519 private key (optional, skips signing if unset)

set -euo pipefail

PLUGIN_SOURCE="${1:?Usage: build-plugin.sh <plugin-source-dir> <output-dir>}"
OUTPUT_DIR="${2:?Usage: build-plugin.sh <plugin-source-dir> <output-dir>}"

# Validate plugin source
if [ ! -f "$PLUGIN_SOURCE/Cargo.toml" ]; then
    echo "ERROR: No Cargo.toml found in $PLUGIN_SOURCE"
    exit 1
fi

# Extract plugin name from Cargo.toml
PLUGIN_NAME=$(grep '^name' "$PLUGIN_SOURCE/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Building plugin: $PLUGIN_NAME"

# Copy plugin source into workspace
PLUGIN_DIR="/app/plugins_build/$PLUGIN_NAME"
rm -rf "$PLUGIN_DIR"
mkdir -p "$PLUGIN_DIR"
cp -r "$PLUGIN_SOURCE"/* "$PLUGIN_DIR/"

# Add to workspace members (temporary)
# Back up original Cargo.toml
cp /app/Cargo.toml /app/Cargo.toml.bak

# Add the plugin to workspace members
sed -i "s|^members = \[|members = [\"plugins_build/$PLUGIN_NAME\", |" /app/Cargo.toml

echo "=== Building for Linux x86_64 ==="
cargo build --release -p "$PLUGIN_NAME" --target x86_64-unknown-linux-gnu 2>&1 || true

echo "=== Building for Windows x86_64 ==="
cargo build --release -p "$PLUGIN_NAME" --target x86_64-pc-windows-gnu 2>&1 || true

echo "=== Building for macOS x86_64 ==="
cargo build --release -p "$PLUGIN_NAME" --target x86_64-apple-darwin 2>&1 || true

echo "=== Building for macOS ARM64 ==="
cargo build --release -p "$PLUGIN_NAME" --target aarch64-apple-darwin 2>&1 || true

# Collect outputs
mkdir -p "$OUTPUT_DIR"

# Linux
LINUX_SO=$(find /app/target/x86_64-unknown-linux-gnu/release -name "lib${PLUGIN_NAME}.so" -o -name "${PLUGIN_NAME}.so" 2>/dev/null | head -1)
[ -n "$LINUX_SO" ] && cp "$LINUX_SO" "$OUTPUT_DIR/${PLUGIN_NAME}_linux_x64.so" && echo "  -> ${PLUGIN_NAME}_linux_x64.so"

# Windows
WIN_DLL=$(find /app/target/x86_64-pc-windows-gnu/release -name "${PLUGIN_NAME}.dll" 2>/dev/null | head -1)
[ -n "$WIN_DLL" ] && cp "$WIN_DLL" "$OUTPUT_DIR/${PLUGIN_NAME}_windows_x64.dll" && echo "  -> ${PLUGIN_NAME}_windows_x64.dll"

# macOS x64
MAC_X64=$(find /app/target/x86_64-apple-darwin/release -name "lib${PLUGIN_NAME}.dylib" 2>/dev/null | head -1)
[ -n "$MAC_X64" ] && cp "$MAC_X64" "$OUTPUT_DIR/${PLUGIN_NAME}_macos_x64.dylib" && echo "  -> ${PLUGIN_NAME}_macos_x64.dylib"

# macOS ARM
MAC_ARM=$(find /app/target/aarch64-apple-darwin/release -name "lib${PLUGIN_NAME}.dylib" 2>/dev/null | head -1)
[ -n "$MAC_ARM" ] && cp "$MAC_ARM" "$OUTPUT_DIR/${PLUGIN_NAME}_macos_arm64.dylib" && echo "  -> ${PLUGIN_NAME}_macos_arm64.dylib"

# TODO: Sign outputs with ed25519 key if SIGNING_KEY_PATH is set

# Restore original Cargo.toml
mv /app/Cargo.toml.bak /app/Cargo.toml

# Clean up plugin source from workspace
rm -rf "$PLUGIN_DIR"

echo "=== Build complete ==="
ls -la "$OUTPUT_DIR/"

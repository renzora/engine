#!/usr/bin/env bash
# =============================================================================
# Build all engine targets — used in the engine-builder Docker container
# =============================================================================
#
# Usage: ./scripts/build-all.sh /path/to/output

set -euo pipefail

OUTPUT_DIR="${1:?Usage: build-all.sh <output-dir>}"
mkdir -p "$OUTPUT_DIR"/{editor,runtime,server,templates}

echo "=== Building Linux Editor ==="
cargo build --release --bin renzora --no-default-features --features editor
cp target/release/renzora "$OUTPUT_DIR/editor/renzora-linux-x64"

echo "=== Building Linux Runtime (export template) ==="
cargo build --release --bin renzora-runtime --no-default-features
cp target/release/renzora-runtime "$OUTPUT_DIR/templates/renzora-runtime-linux-x64"

echo "=== Building Linux Server ==="
cargo build --release --bin renzora-server --no-default-features --features server
cp target/release/renzora-server "$OUTPUT_DIR/server/renzora-server-linux-x64"

echo "=== Building Windows Editor (cross-compile) ==="
cargo build --release --bin renzora --no-default-features --features editor --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/renzora.exe "$OUTPUT_DIR/editor/renzora-windows-x64.exe"

echo "=== Building Windows Runtime (export template) ==="
cargo build --release --bin renzora-runtime --no-default-features --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/renzora-runtime.exe "$OUTPUT_DIR/templates/renzora-runtime-windows-x64.exe"

echo "=== Building WASM Runtime (export template) ==="
cargo build --profile dist --bin renzora-runtime --no-default-features --features wasm --target wasm32-unknown-unknown
WASM_FILE=$(find target/wasm32-unknown-unknown/dist -name "renzora-runtime.wasm" | head -1)
if [ -n "$WASM_FILE" ]; then
    wasm-bindgen --out-dir "$OUTPUT_DIR/templates/wasm" --target web "$WASM_FILE"
    # Optimize with wasm-opt if available
    if command -v wasm-opt &>/dev/null; then
        wasm-opt -Oz "$OUTPUT_DIR/templates/wasm/renzora-runtime_bg.wasm" \
            -o "$OUTPUT_DIR/templates/wasm/renzora-runtime_bg.wasm"
    fi
fi

echo "=== Building Android ARM Runtime (export template) ==="
cargo build --release --bin renzora-runtime --no-default-features --target aarch64-linux-android 2>&1 || echo "WARN: Android ARM build failed (may need NDK adjustments)"

echo "=== Building Android x86 Runtime (export template) ==="
cargo build --release --bin renzora-runtime --no-default-features --target x86_64-linux-android 2>&1 || echo "WARN: Android x86 build failed (may need NDK adjustments)"

# macOS builds (only if osxcross is available)
if command -v x86_64-apple-darwin24-clang &>/dev/null; then
    echo "=== Building macOS x64 Editor ==="
    cargo build --release --bin renzora --no-default-features --features editor --target x86_64-apple-darwin
    cp target/x86_64-apple-darwin/release/renzora "$OUTPUT_DIR/editor/renzora-macos-x64"

    echo "=== Building macOS ARM Editor ==="
    cargo build --release --bin renzora --no-default-features --features editor --target aarch64-apple-darwin
    cp target/aarch64-apple-darwin/release/renzora "$OUTPUT_DIR/editor/renzora-macos-arm64"

    echo "=== Building macOS x64 Runtime (export template) ==="
    cargo build --release --bin renzora-runtime --no-default-features --target x86_64-apple-darwin
    cp target/x86_64-apple-darwin/release/renzora-runtime "$OUTPUT_DIR/templates/renzora-runtime-macos-x64"

    echo "=== Building macOS ARM Runtime (export template) ==="
    cargo build --release --bin renzora-runtime --no-default-features --target aarch64-apple-darwin
    cp target/aarch64-apple-darwin/release/renzora-runtime "$OUTPUT_DIR/templates/renzora-runtime-macos-arm64"
fi

echo "=== Build complete ==="
find "$OUTPUT_DIR" -type f | sort

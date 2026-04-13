#!/usr/bin/env bash
# Build the marketplace preview WASM module.
#
# Usage:
#   ./build-preview.sh                    # Build to ./dist/
#   ./build-preview.sh /path/to/output    # Build to custom output dir
#
# Prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="${1:-$SCRIPT_DIR/dist}"

echo "[preview] Building renzora_preview for wasm32-unknown-unknown..."
cd "$SCRIPT_DIR/renzora_preview"

cargo build --release --target wasm32-unknown-unknown

echo "[preview] Running wasm-bindgen..."
mkdir -p "$OUTPUT_DIR"
wasm-bindgen \
    --out-dir "$OUTPUT_DIR" \
    --target web \
    --no-typescript \
    "$SCRIPT_DIR/renzora_preview/target/wasm32-unknown-unknown/release/renzora_preview.wasm"

# Optimize with wasm-opt if available
if command -v wasm-opt &>/dev/null; then
    echo "[preview] Optimizing with wasm-opt..."
    wasm-opt -Oz -o "$OUTPUT_DIR/renzora_preview_bg.wasm" "$OUTPUT_DIR/renzora_preview_bg.wasm"
fi

echo "[preview] Done! Output in $OUTPUT_DIR"
ls -lh "$OUTPUT_DIR"

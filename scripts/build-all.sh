#!/usr/bin/env bash
# =============================================================================
# Build all engine targets — used in the engine-builder Docker container
# =============================================================================
#
# Usage: ./scripts/build-all.sh /path/to/output
#
# Desktop targets (Linux, Windows, macOS) are built with unified features
# (editor,runtime,server) so all binaries share the same bevy_dylib hash
# for plugin compatibility. WASM and Android are standalone.

set -euo pipefail

# Source cross-compiler env vars (CC/CXX/AR for osxcross + Android NDK)
if [ -f /etc/osxcross-env.sh ]; then
    source /etc/osxcross-env.sh
fi

OUTPUT_DIR="${1:?Usage: build-all.sh <output-dir>}"

UNIFIED_FEATURES="editor,runtime,server"

# ── Helper: copy shared libraries for a platform ────────────────────────────
# Usage: copy_shared_libs <target-dir> <output-dir> <lib-ext> <dylib-prefix>
copy_shared_libs() {
    local TARGET_DIR="$1"
    local OUT="$2"
    local EXT="$3"

    mkdir -p "$OUT/plugins"

    # bevy_dylib
    for f in "$TARGET_DIR"/deps/libbevy_dylib-*."$EXT" "$TARGET_DIR"/deps/bevy_dylib-*."$EXT"; do
        [ -f "$f" ] && cp "$f" "$OUT/"
    done

    # Core SDK libs (renzora, renzora_runtime)
    for name in renzora renzora_runtime renzora_macros; do
        for f in "$TARGET_DIR/lib${name}.${EXT}" "$TARGET_DIR/${name}.${EXT}"; do
            [ -f "$f" ] && cp "$f" "$OUT/"
        done
    done

    # Editor core plugins (non-standalone, ship in root)
    for name in renzora_camera renzora_console renzora_gizmo renzora_grid \
                renzora_keybindings renzora_scene renzora_shape_library renzora_viewport; do
        for f in "$TARGET_DIR/lib${name}.${EXT}" "$TARGET_DIR/${name}.${EXT}"; do
            [ -f "$f" ] && cp "$f" "$OUT/"
        done
    done

    # Everything else with the right extension goes to plugins/
    for f in "$TARGET_DIR"/*."$EXT"; do
        [ -f "$f" ] || continue
        local base=$(basename "$f")
        # Skip if already copied to root
        [[ "$base" == *bevy_dylib* ]] && continue
        [[ "$base" == *libstd-* ]] && continue
        # Check if it's already in root
        [ -f "$OUT/$base" ] && continue
        cp "$f" "$OUT/plugins/"
    done
}

# ── Linux (all three binaries in one invocation) ──────────────────────────

echo "=== Building Linux binaries ==="
mkdir -p "$OUTPUT_DIR/linux-x64"
cargo build --profile dist --workspace --bin renzora --bin renzora-runtime --bin renzora-server --no-default-features --features "$UNIFIED_FEATURES"
cp target/dist/renzora "$OUTPUT_DIR/linux-x64/"
cp target/dist/renzora-runtime "$OUTPUT_DIR/linux-x64/"
cp target/dist/renzora-server "$OUTPUT_DIR/linux-x64/"
copy_shared_libs "target/dist" "$OUTPUT_DIR/linux-x64" "so"

# Copy Rust std
SYSROOT=$(rustc --print sysroot)
for f in "$SYSROOT"/lib/rustlib/x86_64-unknown-linux-gnu/lib/libstd-*.so; do
    [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/linux-x64/"
done

# ── Windows (cross-compile) ───────────────────────────────────────────────

echo "=== Building Windows binaries ==="
mkdir -p "$OUTPUT_DIR/windows-x64"
cargo build --profile dist --workspace --bin renzora --bin renzora-runtime --no-default-features --features "$UNIFIED_FEATURES" --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/dist/renzora.exe "$OUTPUT_DIR/windows-x64/"
cp target/x86_64-pc-windows-gnu/dist/renzora-runtime.exe "$OUTPUT_DIR/windows-x64/"
copy_shared_libs "target/x86_64-pc-windows-gnu/dist" "$OUTPUT_DIR/windows-x64" "dll"

# MinGW runtime DLLs + Rust std for Windows
SYSROOT=$(rustc --print sysroot)
cp /usr/lib/gcc/x86_64-w64-mingw32/12-posix/libgcc_s_seh-1.dll "$OUTPUT_DIR/windows-x64/" 2>/dev/null || true
cp /usr/lib/gcc/x86_64-w64-mingw32/12-posix/libstdc++-6.dll "$OUTPUT_DIR/windows-x64/" 2>/dev/null || true
cp /usr/x86_64-w64-mingw32/lib/libwinpthread-1.dll "$OUTPUT_DIR/windows-x64/" 2>/dev/null || true
for f in "$SYSROOT"/lib/rustlib/x86_64-pc-windows-gnu/lib/std-*.dll; do
    [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/windows-x64/"
done

# ── WASM (standalone, no plugin hash needed) ──────────────────────────────

echo "=== Building WASM Runtime (export template) ==="
mkdir -p "$OUTPUT_DIR/wasm"
cargo build --profile dist --bin renzora-runtime --no-default-features --features wasm --target wasm32-unknown-unknown
WASM_FILE=$(find target/wasm32-unknown-unknown/dist -name "renzora-runtime.wasm" | head -1)
if [ -n "$WASM_FILE" ]; then
    wasm-bindgen --out-dir "$OUTPUT_DIR/wasm" --target web "$WASM_FILE"
    if command -v wasm-opt &>/dev/null; then
        wasm-opt -Oz "$OUTPUT_DIR/wasm/renzora-runtime_bg.wasm" \
            -o "$OUTPUT_DIR/wasm/renzora-runtime_bg.wasm"
    fi
fi

# ── Android (standalone, uses renzora-android package) ────────────────────

echo "=== Building Android ARM Runtime (export template) ==="
cargo build --profile dist -p renzora-android --target aarch64-linux-android 2>&1 || echo "WARN: Android ARM build failed"
if [ -f target/aarch64-linux-android/dist/libmain.so ]; then
    mkdir -p "$OUTPUT_DIR/android-arm64"
    cp target/aarch64-linux-android/dist/libmain.so "$OUTPUT_DIR/android-arm64/"
fi

echo "=== Building Android x86 Runtime (export template) ==="
cargo build --profile dist -p renzora-android --target x86_64-linux-android 2>&1 || echo "WARN: Android x86 build failed"
if [ -f target/x86_64-linux-android/dist/libmain.so ]; then
    mkdir -p "$OUTPUT_DIR/android-x86"
    cp target/x86_64-linux-android/dist/libmain.so "$OUTPUT_DIR/android-x86/"
fi

# ── macOS (cross-compile via osxcross, unified features) ──────────────────

# Detect osxcross clang by looking for any darwin clang on PATH
OSXCROSS_CLANG=$(command -v x86_64-apple-darwin23-clang 2>/dev/null || command -v x86_64-apple-darwin24-clang 2>/dev/null || true)

if [ -n "$OSXCROSS_CLANG" ]; then
    echo "=== Building macOS x64 binaries ==="
    mkdir -p "$OUTPUT_DIR/macos-x64"
    cargo build --profile dist --workspace --bin renzora --bin renzora-runtime --no-default-features --features "$UNIFIED_FEATURES" --target x86_64-apple-darwin
    cp target/x86_64-apple-darwin/dist/renzora "$OUTPUT_DIR/macos-x64/"
    cp target/x86_64-apple-darwin/dist/renzora-runtime "$OUTPUT_DIR/macos-x64/"
    copy_shared_libs "target/x86_64-apple-darwin/dist" "$OUTPUT_DIR/macos-x64" "dylib"

    # Rust std for macOS x64
    SYSROOT=$(rustc --print sysroot)
    for f in "$SYSROOT"/lib/rustlib/x86_64-apple-darwin/lib/libstd-*.dylib; do
        [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/macos-x64/"
    done

    echo "=== Building macOS ARM binaries ==="
    mkdir -p "$OUTPUT_DIR/macos-arm64"
    cargo build --profile dist --workspace --bin renzora --bin renzora-runtime --no-default-features --features "$UNIFIED_FEATURES" --target aarch64-apple-darwin
    cp target/aarch64-apple-darwin/dist/renzora "$OUTPUT_DIR/macos-arm64/"
    cp target/aarch64-apple-darwin/dist/renzora-runtime "$OUTPUT_DIR/macos-arm64/"
    copy_shared_libs "target/aarch64-apple-darwin/dist" "$OUTPUT_DIR/macos-arm64" "dylib"

    # Rust std for macOS ARM
    SYSROOT=$(rustc --print sysroot)
    for f in "$SYSROOT"/lib/rustlib/aarch64-apple-darwin/lib/libstd-*.dylib; do
        [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/macos-arm64/"
    done
else
    echo "WARN: osxcross not found, skipping macOS builds"
fi

# ── iOS (standalone, uses renzora-ios package) ────────────────────────────

if [ -n "$OSXCROSS_CLANG" ]; then
    echo "=== Building iOS ARM Runtime (export template) ==="
    cargo build --profile dist -p renzora-ios --target aarch64-apple-ios 2>&1 || echo "WARN: iOS build failed"
    if [ -f target/aarch64-apple-ios/dist/librenzora_ios.a ]; then
        mkdir -p "$OUTPUT_DIR/ios-arm64"
        cp target/aarch64-apple-ios/dist/librenzora_ios.a "$OUTPUT_DIR/ios-arm64/"
    fi
fi

echo "=== Build complete ==="
find "$OUTPUT_DIR" -type f | sort

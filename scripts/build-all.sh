#!/usr/bin/env bash
# =============================================================================
# Build all engine targets — used in the engine-builder Docker container
# =============================================================================
#
# Usage: ./scripts/build-all.sh /path/to/output
#
# Each target (editor, runtime, server) is built in isolation with its own
# feature flag and target directory. No feature unification, no hash mixing.

set -euo pipefail

# Source cross-compiler env vars (CC/CXX/AR for osxcross + Android NDK)
if [ -f /etc/osxcross-env.sh ]; then
    source /etc/osxcross-env.sh
fi

OUTPUT_DIR="${1:?Usage: build-all.sh <output-dir>}"
mkdir -p "$OUTPUT_DIR"

# Editor plugins to exclude from runtime/server builds
EDITOR_EXCLUDES=$(grep -h '^name' crates/editor/*/Cargo.toml 2>/dev/null | tr -d '\r' | sed 's/name = "\(.*\)"/--exclude \1/')

# ── Helper: copy shared libraries for a platform ────────────────────────────
# Usage: copy_shared_libs <target-dir> <output-dir> <lib-ext>
copy_shared_libs() {
    local SRC="$1"
    local OUT="$2"
    local EXT="$3"

    mkdir -p "$OUT/plugins"

    # bevy_dylib (newest only)
    local BEVY_DLL=$(ls -t "$SRC"/deps/libbevy_dylib-*."$EXT" "$SRC"/deps/bevy_dylib-*."$EXT" 2>/dev/null | head -1 || true)
    [ -n "$BEVY_DLL" ] && cp "$BEVY_DLL" "$OUT/"

    # SDK
    for f in "$SRC/librenzora.$EXT" "$SRC/renzora.$EXT"; do
        [ -f "$f" ] && cp "$f" "$OUT/"
    done

    # Plugins — everything else
    for f in "$SRC"/*."$EXT"; do
        [ -f "$f" ] || continue
        local base=$(basename "$f")
        [[ "$base" == *bevy_dylib* ]] && continue
        [[ "$base" == *libstd-* ]] && continue
        [[ "$base" == *renzora_macros* ]] && continue
        [[ "$base" == librenzora."$EXT" ]] && continue
        [[ "$base" == renzora."$EXT" ]] && continue
        cp "$f" "$OUT/plugins/"
    done
}

# ── Build a desktop target ───────────────────────────────────────────────────
# Usage: build_desktop <feature> <rust-target|native> <platform-name> <ext>
build_desktop() {
    local FEATURE="$1"
    local RUST_TARGET="$2"
    local PLATFORM="$3"
    local EXT="$4"

    local TARGET_DIR_FLAG="--target-dir target/$FEATURE"
    local TARGET_FLAG=""
    local SRC="target/$FEATURE/dist"

    if [ "$RUST_TARGET" != "native" ]; then
        TARGET_FLAG="--target $RUST_TARGET"
        SRC="target/$FEATURE/$RUST_TARGET/dist"
    fi

    local EXCLUDES=""
    if [ "$FEATURE" != "editor" ]; then
        EXCLUDES="$EDITOR_EXCLUDES"
    fi

    echo "=== Building $PLATFORM ($FEATURE) ==="
    cargo build --profile dist --workspace --no-default-features --features "$FEATURE" $TARGET_DIR_FLAG $TARGET_FLAG $EXCLUDES

    local OUT="$OUTPUT_DIR/$PLATFORM/$FEATURE"
    mkdir -p "$OUT"

    # Binary (rename based on feature)
    case "$FEATURE" in
        editor)  DEST="renzora" ;;
        runtime) DEST="renzora-runtime" ;;
        server)  DEST="renzora-server" ;;
    esac

    if [ "$EXT" = "dll" ]; then
        [ -f "$SRC/renzora.exe" ] && cp "$SRC/renzora.exe" "$OUT/$DEST.exe"
    else
        [ -f "$SRC/renzora" ] && cp "$SRC/renzora" "$OUT/$DEST"
        chmod +x "$OUT/$DEST" 2>/dev/null || true
    fi

    copy_shared_libs "$SRC" "$OUT" "$EXT"
}

# ── Linux ────────────────────────────────────────────────────────────────────

for feature in editor runtime server; do
    build_desktop "$feature" "native" "linux-x64" "so"

    # Rust std
    SYSROOT=$(rustc --print sysroot)
    for f in "$SYSROOT"/lib/rustlib/x86_64-unknown-linux-gnu/lib/libstd-*.so; do
        [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/linux-x64/$feature/"
    done
done

# Wrap the editor output into an AppDir + AppImage
EDITOR_DIR="$OUTPUT_DIR/linux-x64/editor"
if [ -f "$EDITOR_DIR/renzora" ]; then
    APPDIR="$EDITOR_DIR/Renzora Engine.AppDir"
    rm -rf "$APPDIR"
    mkdir -p "$APPDIR/plugins"
    # Move all artifacts into the AppDir
    mv "$EDITOR_DIR/renzora" "$APPDIR/renzora"
    for f in "$EDITOR_DIR"/*.so; do [ -f "$f" ] && mv "$f" "$APPDIR/"; done
    if [ -d "$EDITOR_DIR/plugins" ]; then
        for f in "$EDITOR_DIR/plugins"/*.so; do [ -f "$f" ] && mv "$f" "$APPDIR/plugins/"; done
        rmdir "$EDITOR_DIR/plugins" 2>/dev/null || true
    fi

    cat > "$APPDIR/AppRun" << 'APPRUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export LD_LIBRARY_PATH="$HERE:$HERE/plugins:${LD_LIBRARY_PATH:-}"
exec "$HERE/renzora" "$@"
APPRUN
    chmod +x "$APPDIR/AppRun"

    cat > "$APPDIR/renzora-engine.desktop" << 'DESKTOP'
[Desktop Entry]
Type=Application
Name=Renzora Engine
Exec=renzora
Icon=renzora-engine
Categories=Development;Graphics;
Terminal=false
DESKTOP

    if [ -f icon.png ]; then
        cp icon.png "$APPDIR/renzora-engine.png"
        cp icon.png "$APPDIR/.DirIcon"
    fi

    if command -v appimagetool >/dev/null 2>&1; then
        ARCH=x86_64 appimagetool "$APPDIR" "$EDITOR_DIR/Renzora Engine-x86_64.AppImage" \
            && echo "Built $EDITOR_DIR/Renzora Engine-x86_64.AppImage" \
            || echo "WARN: appimagetool failed"
    else
        echo "WARN: appimagetool not found; AppDir left at $APPDIR"
    fi
fi

# ── Windows (cross-compile) ──────────────────────────────────────────────────

for feature in editor runtime server; do
    build_desktop "$feature" "x86_64-pc-windows-gnu" "windows-x64" "dll"

    OUT="$OUTPUT_DIR/windows-x64/$feature"

    # MinGW runtime DLLs
    cp /usr/lib/gcc/x86_64-w64-mingw32/12-posix/libgcc_s_seh-1.dll "$OUT/" 2>/dev/null || true
    cp /usr/lib/gcc/x86_64-w64-mingw32/12-posix/libstdc++-6.dll "$OUT/" 2>/dev/null || true
    cp /usr/x86_64-w64-mingw32/lib/libwinpthread-1.dll "$OUT/" 2>/dev/null || true

    # Rust std for Windows
    SYSROOT=$(rustc --print sysroot)
    for f in "$SYSROOT"/lib/rustlib/x86_64-pc-windows-gnu/lib/std-*.dll; do
        [ -f "$f" ] && cp "$f" "$OUT/"
    done
done

# ── macOS (cross-compile via osxcross) ───────────────────────────────────────

OSXCROSS_CLANG=$(command -v x86_64-apple-darwin23-clang 2>/dev/null || command -v x86_64-apple-darwin24-clang 2>/dev/null || true)

if [ -n "$OSXCROSS_CLANG" ]; then
    for feature in editor runtime server; do
        build_desktop "$feature" "x86_64-apple-darwin" "macos-x64" "dylib"

        SYSROOT=$(rustc --print sysroot)
        for f in "$SYSROOT"/lib/rustlib/x86_64-apple-darwin/lib/libstd-*.dylib; do
            [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/macos-x64/$feature/"
        done
    done

    for feature in editor runtime server; do
        build_desktop "$feature" "aarch64-apple-darwin" "macos-arm64" "dylib"

        SYSROOT=$(rustc --print sysroot)
        for f in "$SYSROOT"/lib/rustlib/aarch64-apple-darwin/lib/libstd-*.dylib; do
            [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/macos-arm64/$feature/"
        done
    done
else
    echo "WARN: osxcross not found, skipping macOS builds"
fi

# ── WASM (runtime only, no plugins) ─────────────────────────────────────────

echo "=== Building WASM Runtime ==="
cargo build --profile dist -p renzora_app --no-default-features --features wasm --target wasm32-unknown-unknown --target-dir target/wasm
WASM_FILE=$(find target/wasm/wasm32-unknown-unknown/dist -name "renzora.wasm" 2>/dev/null | head -1)
if [ -n "$WASM_FILE" ]; then
    mkdir -p "$OUTPUT_DIR/web-wasm32/runtime"
    wasm-bindgen --out-dir "$OUTPUT_DIR/web-wasm32/runtime" --out-name renzora-runtime --target web "$WASM_FILE"
    if command -v wasm-opt &>/dev/null; then
        wasm-opt -Oz "$OUTPUT_DIR/web-wasm32/runtime/renzora-runtime_bg.wasm" \
            -o "$OUTPUT_DIR/web-wasm32/runtime/renzora-runtime_bg.wasm"
    fi
fi

# ── Android (runtime only) ──────────────────────────────────────────────────

echo "=== Building Android ARM64 Runtime ==="
cargo build --profile dist -p renzora-android --target aarch64-linux-android --target-dir target/android 2>&1 || echo "WARN: Android ARM build failed"
if [ -f target/android/aarch64-linux-android/dist/libmain.so ]; then
    mkdir -p "$OUTPUT_DIR/android-arm64/runtime"
    cp target/android/aarch64-linux-android/dist/libmain.so "$OUTPUT_DIR/android-arm64/runtime/"
fi

echo "=== Building Android x86_64 Runtime ==="
cargo build --profile dist -p renzora-android --target x86_64-linux-android --target-dir target/android 2>&1 || echo "WARN: Android x86 build failed"
if [ -f target/android/x86_64-linux-android/dist/libmain.so ]; then
    mkdir -p "$OUTPUT_DIR/android-x86/runtime"
    cp target/android/x86_64-linux-android/dist/libmain.so "$OUTPUT_DIR/android-x86/runtime/"
fi

# ── iOS (runtime only) ──────────────────────────────────────────────────────

if [ -n "${OSXCROSS_CLANG:-}" ]; then
    echo "=== Building iOS ARM64 Runtime ==="
    cargo build --profile dist -p renzora-ios --target aarch64-apple-ios --target-dir target/ios 2>&1 || echo "WARN: iOS build failed"
    if [ -f target/ios/aarch64-apple-ios/dist/librenzora_ios.a ]; then
        mkdir -p "$OUTPUT_DIR/ios-arm64/runtime"
        cp target/ios/aarch64-apple-ios/dist/librenzora_ios.a "$OUTPUT_DIR/ios-arm64/runtime/"
    fi
fi

echo "=== Build complete ==="
find "$OUTPUT_DIR" -type f | sort

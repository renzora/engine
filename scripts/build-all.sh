#!/usr/bin/env bash
# =============================================================================
# Build engine targets — used in the engine-builder Docker container
# =============================================================================
#
# Usage: ./scripts/build-all.sh <output-dir> [platform ...]
#
# Each target (editor, runtime, server) is built in isolation with its own
# feature flag and target directory. No feature unification, no hash mixing.
#
# Platforms (positional args after <output-dir>; pass none to build all):
#   linux        Linux x86_64 (native in container)
#   windows      Windows x86_64 MSVC (xwin)
#   macos        macOS x86_64 + arm64 (osxcross)
#   macos-x64    macOS x86_64 only
#   macos-arm64  macOS arm64 only
#   wasm         WASM runtime + best-effort editor
#   android      Android arm64 + x86_64
#   android-arm64
#   android-x86
#   ios          iOS arm64 staticlib

set -euo pipefail

# Source cross-compiler env vars (CC/CXX/AR for osxcross + Android NDK)
if [ -f /etc/osxcross-env.sh ]; then
    source /etc/osxcross-env.sh
fi

OUTPUT_DIR="${1:?Usage: build-all.sh <output-dir> [platform ...]}"
shift
mkdir -p "$OUTPUT_DIR"

# Platform filter: empty array = build everything; non-empty = filter set.
# `macos` expands to macos-x64+macos-arm64; `android` expands to both Android
# architectures. Anything unrecognised is left in the array (will simply not
# match any guard, so it's effectively a no-op — typo-safe by construction).
PLATFORMS=()
for arg in "$@"; do
    case "$arg" in
        macos)   PLATFORMS+=("macos-x64" "macos-arm64") ;;
        android) PLATFORMS+=("android-arm64" "android-x86") ;;
        *)       PLATFORMS+=("$arg") ;;
    esac
done

should_build() {
    # No filter → build everything.
    [ ${#PLATFORMS[@]} -eq 0 ] && return 0
    local target="$1"
    for p in "${PLATFORMS[@]}"; do
        [ "$p" = "$target" ] && return 0
    done
    return 1
}

# Editor crates aren't workspace members anymore — they're transitive
# path-deps of the binary, gated behind the `editor` feature. Runtime and
# server builds drop `--workspace` (build the binary's dep tree only) so
# editor crates never enter the build graph.

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

    # SDK — shared dylibs that the host binary AND every distribution
    # plugin link against. Each ships once next to the host, not in
    # plugins/. Adding a new SDK dylib (e.g. another contract crate
    # promoted to dual-mode dylib) means listing it here.
    for f in \
        "$SRC/librenzora.$EXT"           "$SRC/renzora.$EXT" \
        "$SRC/librenzora_editor.$EXT"    "$SRC/renzora_editor.$EXT" \
        "$SRC/librenzora_postprocess.$EXT" "$SRC/renzora_postprocess.$EXT"; do
        [ -f "$f" ] && cp "$f" "$OUT/"
    done

    # Plugins — every cdylib distribution plugin output. Excludes the
    # SDK dylibs above, the wasm-only `renzora_preview` (it produces a
    # cdylib for desktop too but isn't an engine plugin — no `add!`),
    # and rust-internal artifacts (libstd, renzora_macros).
    for f in "$SRC"/*."$EXT"; do
        [ -f "$f" ] || continue
        local base=$(basename "$f")
        [[ "$base" == *bevy_dylib* ]] && continue
        [[ "$base" == *libstd-* ]] && continue
        [[ "$base" == *renzora_macros* ]] && continue
        [[ "$base" == librenzora."$EXT" ]] && continue
        [[ "$base" == renzora."$EXT" ]] && continue
        [[ "$base" == librenzora_editor."$EXT" ]] && continue
        [[ "$base" == renzora_editor."$EXT" ]] && continue
        [[ "$base" == librenzora_postprocess."$EXT" ]] && continue
        [[ "$base" == renzora_postprocess."$EXT" ]] && continue
        [[ "$base" == librenzora_preview."$EXT" ]] && continue
        [[ "$base" == renzora_preview."$EXT" ]] && continue
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

    # Editor: build the whole workspace WITHOUT `--no-default-features`.
    # `--no-default-features` propagates to every workspace member, which
    # would suppress the `default = ["dlopen"]` on cdylib distribution
    # plugins → no FFI exports → host rejects them as ABI-incompatible.
    # `renzora_app`'s own default IS `editor`, so dropping the flag still
    # builds the right host configuration.
    #
    # Runtime/server: only the host binary, with controlled features. No
    # editor-only crates and no distribution plugins enter the build.
    #
    # `renzora-android` (cdylib) and `renzora-ios` (staticlib) are
    # workspace members but mobile-only; exclude them from desktop.
    echo "=== Building $PLATFORM ($FEATURE) ==="
    if [ "$FEATURE" = "editor" ]; then
        cargo build --profile dist --workspace \
            --exclude renzora-android --exclude renzora-ios \
            $TARGET_DIR_FLAG $TARGET_FLAG
    else
        cargo build --profile dist --bin renzora --no-default-features \
            --features "$FEATURE" $TARGET_DIR_FLAG $TARGET_FLAG
    fi

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

if should_build linux; then
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
fi  # should_build linux

# ── Windows (cross-compile via xwin + clang-cl + lld-link) ───────────────────
# MSVC ABI build — links to vcruntime140.dll / msvcp140.dll which Win10/11
# ship with by default (or via the VC++ Redistributable). No mingw runtime
# DLLs to bundle.

if should_build windows; then
for feature in editor runtime server; do
    build_desktop "$feature" "x86_64-pc-windows-msvc" "windows-x64" "dll"

    OUT="$OUTPUT_DIR/windows-x64/$feature"

    # Rust std for Windows
    SYSROOT=$(rustc --print sysroot)
    for f in "$SYSROOT"/lib/rustlib/x86_64-pc-windows-msvc/lib/std-*.dll; do
        [ -f "$f" ] && cp "$f" "$OUT/"
    done
done
fi  # should_build windows

# ── macOS (cross-compile via osxcross) ───────────────────────────────────────

OSXCROSS_CLANG=$(ls /opt/osxcross/target/bin/x86_64-apple-darwin*-clang 2>/dev/null | head -1 || true)

if [ -n "$OSXCROSS_CLANG" ]; then
    if should_build macos-x64; then
    for feature in editor runtime server; do
        build_desktop "$feature" "x86_64-apple-darwin" "macos-x64" "dylib"

        SYSROOT=$(rustc --print sysroot)
        for f in "$SYSROOT"/lib/rustlib/x86_64-apple-darwin/lib/libstd-*.dylib; do
            [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/macos-x64/$feature/"
        done
    done
    fi  # should_build macos-x64

    if should_build macos-arm64; then
    for feature in editor runtime server; do
        build_desktop "$feature" "aarch64-apple-darwin" "macos-arm64" "dylib"

        SYSROOT=$(rustc --print sysroot)
        for f in "$SYSROOT"/lib/rustlib/aarch64-apple-darwin/lib/libstd-*.dylib; do
            [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/macos-arm64/$feature/"
        done
    done
    fi  # should_build macos-arm64
elif should_build macos-x64 || should_build macos-arm64; then
    echo "WARN: osxcross not found, skipping macOS builds"
fi

# ── WASM ─────────────────────────────────────────────────────────────────────
# WASM bundles plugins statically (no dlopen). Native plugin crates compile
# as rlib for wasm — Cargo silently skips their dylib output for this target.

if should_build wasm; then
echo "=== Building WASM Runtime ==="
cargo build --profile dist -p renzora_app --no-default-features --features wasm --target wasm32-unknown-unknown --target-dir target/wasm
WASM_FILE=$(find target/wasm/wasm32-unknown-unknown/dist -name "renzora.wasm" 2>/dev/null | head -1)
if [ -n "$WASM_FILE" ]; then
    mkdir -p "$OUTPUT_DIR/web-wasm32/runtime"
    wasm-bindgen --out-dir "$OUTPUT_DIR/web-wasm32/runtime" --out-name renzora-runtime --target web "$WASM_FILE"
    if command -v wasm-opt &>/dev/null; then
        wasm-opt -Oz \
            --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int \
            --enable-mutable-globals --enable-reference-types --enable-multivalue \
            "$OUTPUT_DIR/web-wasm32/runtime/renzora-runtime_bg.wasm" \
            -o "$OUTPUT_DIR/web-wasm32/runtime/renzora-runtime_bg.wasm"
    fi
fi

# NOTE: Editor-on-web is incomplete — many editor deps are native-only sys
# crates (lzma-sys, coreaudio-sys, cpal, arboard, rfd, ...) that can't
# cross-compile to wasm32-unknown-unknown. Each needs a pure-rust alternative
# or a `cfg(not(target_arch = "wasm32"))` gate before the editor wasm bundle
# can succeed. Left as non-fatal so the rest of the build completes.
echo "=== Building WASM Editor (best-effort) ==="
WASM_EDITOR_LOG="$OUTPUT_DIR/wasm-editor-build.log"
if cargo build --profile dist -p renzora_app --no-default-features --features editor,wasm --target wasm32-unknown-unknown --target-dir target/wasm-editor >"$WASM_EDITOR_LOG" 2>&1; then
    WASM_EDITOR_FILE=$(find target/wasm-editor/wasm32-unknown-unknown/dist -name "renzora.wasm" 2>/dev/null | head -1)
    if [ -n "$WASM_EDITOR_FILE" ]; then
        mkdir -p "$OUTPUT_DIR/web-wasm32/editor"
        wasm-bindgen --out-dir "$OUTPUT_DIR/web-wasm32/editor" --out-name renzora-editor --target web "$WASM_EDITOR_FILE"
        if command -v wasm-opt &>/dev/null; then
            wasm-opt -Oz \
                --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int \
                --enable-mutable-globals --enable-reference-types --enable-multivalue \
                "$OUTPUT_DIR/web-wasm32/editor/renzora-editor_bg.wasm" \
                -o "$OUTPUT_DIR/web-wasm32/editor/renzora-editor_bg.wasm"
        fi
        rm -f "$WASM_EDITOR_LOG"
    fi
else
    echo "WARN: WASM editor build failed — see $WASM_EDITOR_LOG for details"
    echo "      (most likely: native-only sys crates like lzma-sys/bzip2-sys/ufbx"
    echo "       can't cross-compile to wasm32-unknown-unknown without a sysroot)"
fi
fi  # should_build wasm

# ── Android (runtime only) ──────────────────────────────────────────────────

if should_build android-arm64; then
echo "=== Building Android ARM64 Runtime ==="
cargo build --profile dist -p renzora-android --target aarch64-linux-android --target-dir target/android 2>&1 || echo "WARN: Android ARM build failed"
if [ -f target/android/aarch64-linux-android/dist/libmain.so ]; then
    mkdir -p "$OUTPUT_DIR/android-arm64/runtime"
    cp target/android/aarch64-linux-android/dist/libmain.so "$OUTPUT_DIR/android-arm64/runtime/"
fi
fi  # should_build android-arm64

if should_build android-x86; then
echo "=== Building Android x86_64 Runtime ==="
cargo build --profile dist -p renzora-android --target x86_64-linux-android --target-dir target/android 2>&1 || echo "WARN: Android x86 build failed"
if [ -f target/android/x86_64-linux-android/dist/libmain.so ]; then
    mkdir -p "$OUTPUT_DIR/android-x86/runtime"
    cp target/android/x86_64-linux-android/dist/libmain.so "$OUTPUT_DIR/android-x86/runtime/"
fi
fi  # should_build android-x86

# ── iOS (runtime only) ──────────────────────────────────────────────────────

if should_build ios; then
echo "=== Building iOS ARM64 Runtime ==="
# SDKROOT bypasses cc-rs's call to `xcrun --show-sdk-path --sdk iphoneos`,
# which fails because osxcross's xcrun only knows the macOS SDK.
# BINDGEN_EXTRA_CLANG_ARGS gives bindgen's libclang the iOS target + sysroot
# so it can find framework headers like <AudioUnit/AudioUnit.h>.
SDKROOT=/opt/iphoneos.sdk \
BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_ios="--target=arm64-apple-ios14.0 -isysroot /opt/iphoneos.sdk" \
cargo build --profile dist -p renzora-ios --target aarch64-apple-ios --target-dir target/ios 2>&1 || echo "WARN: iOS build failed"
if [ -f target/ios/aarch64-apple-ios/dist/librenzora_ios.a ]; then
    mkdir -p "$OUTPUT_DIR/ios-arm64/runtime"
    cp target/ios/aarch64-apple-ios/dist/librenzora_ios.a "$OUTPUT_DIR/ios-arm64/runtime/"
fi
fi  # should_build ios

echo "=== Build complete ==="
find "$OUTPUT_DIR" -type f | sort

#!/usr/bin/env bash
# =============================================================================
# Build engine targets — used in the engine-builder Docker container
# =============================================================================
#
# Usage: ./scripts/build-all.sh <output-dir> [platform ...]
#
# Each target (editor, runtime) is built in isolation with its own
# feature flag and target directory. No feature unification, no hash mixing.
# The dedicated server is not a separate target — it's the runtime launched
# with `--server`.
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
#
# ── Parallelism ──────────────────────────────────────────────────────────────
# Builds run as concurrent "lanes". The contention-free unit is the FEATURE,
# not the platform: editor/runtime each use their own `--target-dir`
# (target/editor, target/runtime), while every desktop platform
# for one feature shares that dir (different triple subdirs inside it). So we
# run one lane per feature, plus one each for wasm / android / ios. Lanes never
# share a target-dir, so cargo's per-target-dir build lock never serialises
# them — and the on-disk cache layout is identical to a sequential build.
#
# Within a feature lane, desktop platforms still build sequentially (they share
# that feature's target-dir and reuse its host-side proc-macro/build-script
# artifacts), exactly as before — only the lanes themselves overlap.
#
# Concurrency is capped by BUILD_JOBS (env). Default is derived from container
# memory (~4 GB per concurrent lane) and clamped to the CPU count, because the
# real ceiling on parallel bevy builds is RAM during codegen/link, not cores.
# On a memory-tight machine, set BUILD_JOBS=1 or 2. On a big build server, set
# it as high as the lane count (6) to overlap everything.

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

array_contains() {
    local needle="$1"; shift
    local x
    for x in "$@"; do [ "$x" = "$needle" ] && return 0; done
    return 1
}

# Editor crates aren't workspace members anymore — they're transitive
# path-deps of the binary, gated behind the `editor` feature. The runtime
# build drops `--workspace` (build the binary's dep tree only) so
# editor crates never enter the build graph.

# ── Helper: copy shared libraries for a platform ────────────────────────────
# Usage: copy_shared_libs <target-dir> <output-dir> <lib-ext>
copy_shared_libs() {
    local SRC="$1"
    local OUT="$2"
    local EXT="$3"
    local HOST_BIN="$4"

    mkdir -p "$OUT/plugins"

    # bevy_dylib — copy the EXACT one the host binary imports, NOT just the
    # newest by mtime. deps/ accumulates one bevy_dylib-<hash> per feature
    # config across builds; picking by mtime can copy a hash the binary does
    # not link, giving "bevy_dylib-<hash>.dll not found" at runtime.
    local WANT=""
    [ -n "$HOST_BIN" ] && [ -f "$HOST_BIN" ] && \
        WANT=$(grep -aoE "(lib)?bevy_dylib-[0-9a-f]+\.$EXT" "$HOST_BIN" 2>/dev/null | head -1)
    local BEVY_DLL=""
    [ -n "$WANT" ] && BEVY_DLL=$(ls "$SRC"/deps/"$WANT" 2>/dev/null | head -1)
    # Fallback to newest-by-mtime only if the import name couldn't be read.
    [ -z "$BEVY_DLL" ] && BEVY_DLL=$(ls -t "$SRC"/deps/libbevy_dylib-*."$EXT" "$SRC"/deps/bevy_dylib-*."$EXT" 2>/dev/null | head -1 || true)
    [ -n "$BEVY_DLL" ] && cp "$BEVY_DLL" "$OUT/"

    # SDK — shared dylibs that the host binary AND every distribution
    # plugin link against. Each ships once next to the host, not in
    # plugins/. Adding a new SDK dylib (e.g. another contract crate
    # promoted to dual-mode dylib) means listing it here.
    # NOTE: `renzora_postprocess` is no longer here — its framework folded
    # into `renzora` (module `renzora::postprocess`), so it ships inside
    # renzora.{dll,so,dylib} and emits no dylib of its own.
    # `renzora_editor.$EXT` is the editor BUNDLE cdylib (the removable editor):
    # present beside the exe → the binary is the editor; delete it → the same
    # binary is the exported game. The editor *framework* is now an rlib (folded
    # contract lives in renzora.dll), so it emits no dylib of its own.
    for f in \
        "$SRC/librenzora.$EXT"         "$SRC/renzora.$EXT" \
        "$SRC/librenzora_editor.$EXT"  "$SRC/renzora_editor.$EXT"; do
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
        # Editor bundle (renzora_editor.*) ships beside the exe (copied above),
        # never in plugins/. Also defensively skip the pre-rename name in case a
        # stale renzora_editor_bundle.* lingers in the cargo cache (cargo doesn't
        # delete a renamed crate's old dylib) — otherwise it'd be shipped as 100+
        # MB of dead weight and (now) skipped by the loader as a misplaced bundle.
        [[ "$base" == librenzora_editor."$EXT" ]] && continue
        [[ "$base" == renzora_editor."$EXT" ]] && continue
        [[ "$base" == librenzora_editor_bundle."$EXT" ]] && continue
        [[ "$base" == renzora_editor_bundle."$EXT" ]] && continue
        # `renzora_postprocess` is now an rlib shim and emits no dylib, but
        # keep this guard so a stale dylib left in the cargo cache (from
        # before the crate-type change) is never swept into plugins/ — it
        # has no `add!`/`plugin_bevy_hash`, so the loader would reject it.
        [[ "$base" == librenzora_postprocess."$EXT" ]] && continue
        [[ "$base" == renzora_postprocess."$EXT" ]] && continue
        [[ "$base" == librenzora_preview."$EXT" ]] && continue
        [[ "$base" == renzora_preview."$EXT" ]] && continue
        cp "$f" "$OUT/plugins/"
    done
    return 0
}

# ── Helper: copy the matching Rust std shared lib for a platform ─────────────
# Usage: copy_std <output-platform> <feature> <rust-triple> <glob>
copy_std() {
    local PLATFORM="$1" FEATURE="$2" TRIPLE="$3" GLOB="$4"
    local SYSROOT; SYSROOT=$(rustc --print sysroot)
    local f
    for f in "$SYSROOT"/lib/rustlib/"$TRIPLE"/lib/$GLOB; do
        [ -f "$f" ] && cp "$f" "$OUTPUT_DIR/$PLATFORM/"
    done
    return 0
}

# ── Build a desktop target ───────────────────────────────────────────────────
# Usage: build_desktop <feature> <rust-target|native> <platform-name> <ext>
# Returns non-zero if the cargo compile fails.
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
    # Runtime: only the host binary, with controlled features. No
    # editor-only crates and no distribution plugins enter the build.
    #
    # `renzora-android` (cdylib) and `renzora-ios` (staticlib) are
    # workspace members but mobile-only; exclude them from desktop.
    echo "=== Building $PLATFORM ($FEATURE) ==="
    if [ "$FEATURE" = "editor" ]; then
        cargo build --profile dist --workspace \
            --exclude renzora-android --exclude renzora-ios \
            $TARGET_DIR_FLAG $TARGET_FLAG || return 1
    else
        cargo build --profile dist --bin renzora --no-default-features \
            --features "$FEATURE" $TARGET_DIR_FLAG $TARGET_FLAG || return 1
    fi

    local OUT="$OUTPUT_DIR/$PLATFORM"
    mkdir -p "$OUT"

    # Binary (rename based on feature)
    case "$FEATURE" in
        editor)  DEST="renzora" ;;
        runtime) DEST="renzora-runtime" ;;
    esac

    if [ "$EXT" = "dll" ]; then
        [ -f "$SRC/renzora.exe" ] && cp "$SRC/renzora.exe" "$OUT/$DEST.exe"
    else
        [ -f "$SRC/renzora" ] && cp "$SRC/renzora" "$OUT/$DEST"
        chmod +x "$OUT/$DEST" 2>/dev/null || true
    fi

    local HOST_BIN
    if [ "$EXT" = "dll" ]; then HOST_BIN="$OUT/$DEST.exe"; else HOST_BIN="$OUT/$DEST"; fi
    copy_shared_libs "$SRC" "$OUT" "$EXT" "$HOST_BIN"
    return 0
}

# ── Build one (platform, feature) pair, incl. its Rust std ───────────────────
build_one() {
    local PLATFORM="$1" FEATURE="$2"
    case "$PLATFORM" in
        linux-x64)
            build_desktop "$FEATURE" native               "linux-x64"   "so"    || return 1
            copy_std "linux-x64"   "$FEATURE" "x86_64-unknown-linux-gnu" "libstd-*.so" ;;
        windows-x64)
            build_desktop "$FEATURE" x86_64-pc-windows-msvc "windows-x64" "dll"   || return 1
            # MSVC ABI build — links to vcruntime140.dll / msvcp140.dll which
            # Win10/11 ship by default (or via the VC++ Redistributable).
            copy_std "windows-x64" "$FEATURE" "x86_64-pc-windows-msvc"    "std-*.dll" ;;
        macos-x64)
            build_desktop "$FEATURE" x86_64-apple-darwin    "macos-x64"   "dylib" || return 1
            copy_std "macos-x64"   "$FEATURE" "x86_64-apple-darwin"       "libstd-*.dylib" ;;
        macos-arm64)
            build_desktop "$FEATURE" aarch64-apple-darwin   "macos-arm64" "dylib" || return 1
            copy_std "macos-arm64" "$FEATURE" "aarch64-apple-darwin"      "libstd-*.dylib" ;;
        *)
            echo "WARN: unknown desktop platform '$PLATFORM'"; return 1 ;;
    esac
    return 0
}

# ── Wrap the Linux editor output into an AppDir + AppImage ────────────────────
wrap_linux_appimage() {
    local EDITOR_DIR="$OUTPUT_DIR/linux-x64"
    [ -f "$EDITOR_DIR/renzora" ] || return 0

    local APPDIR="$EDITOR_DIR/Renzora Engine.AppDir"
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
    return 0
}

# ── Lane: build one feature across every requested desktop platform ──────────
lane_desktop_feature() {
    local FEATURE="$1" p
    for p in "${DESKTOP_PLATFORMS[@]}"; do
        build_one "$p" "$FEATURE" || return 1
    done
    # AppImage wrapping only applies to the editor on Linux.
    if [ "$FEATURE" = "editor" ] && array_contains "linux-x64" "${DESKTOP_PLATFORMS[@]}"; then
        wrap_linux_appimage || return 1
    fi
    return 0
}

# ── Lane: WASM ───────────────────────────────────────────────────────────────
# WASM bundles plugins statically (no dlopen). Native plugin crates compile
# as rlib for wasm — Cargo silently skips their dylib output for this target.
build_wasm() {
    echo "=== Building WASM Runtime ==="
    cargo build --profile dist -p renzora_app --no-default-features --features wasm \
        --target wasm32-unknown-unknown --target-dir target/wasm || return 1
    local WASM_FILE
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

    # Editor-on-web was removed with Operation Merge: there is no compile-time
    # `editor` feature anymore, and the editor now ships as a desktop dlopen
    # bundle (`renzora_editor`) which has no wasm equivalent. The web
    # lane builds the game runtime only (above).
    return 0
}

# ── Lane: Android (runtime only) ─────────────────────────────────────────────
# Both archs share target/android (sequential within this lane); best-effort.
build_android() {
    if should_build android-arm64; then
        echo "=== Building Android ARM64 Runtime ==="
        cargo build --profile dist -p renzora-android --target aarch64-linux-android --target-dir target/android 2>&1 || echo "WARN: Android ARM build failed"
        if [ -f target/android/aarch64-linux-android/dist/libmain.so ]; then
            mkdir -p "$OUTPUT_DIR/android-arm64/runtime"
            cp target/android/aarch64-linux-android/dist/libmain.so "$OUTPUT_DIR/android-arm64/runtime/"
        fi
    fi
    if should_build android-x86; then
        echo "=== Building Android x86_64 Runtime ==="
        cargo build --profile dist -p renzora-android --target x86_64-linux-android --target-dir target/android 2>&1 || echo "WARN: Android x86 build failed"
        if [ -f target/android/x86_64-linux-android/dist/libmain.so ]; then
            mkdir -p "$OUTPUT_DIR/android-x86/runtime"
            cp target/android/x86_64-linux-android/dist/libmain.so "$OUTPUT_DIR/android-x86/runtime/"
        fi
    fi
    return 0
}

# ── Lane: iOS (runtime only) ─────────────────────────────────────────────────
build_ios() {
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
    return 0
}

# =============================================================================
# Parallel lane orchestration
# =============================================================================

# Which desktop platforms are in scope (filter + osxcross availability).
OSXCROSS_CLANG=$(ls /opt/osxcross/target/bin/x86_64-apple-darwin*-clang 2>/dev/null | head -1 || true)
DESKTOP_PLATFORMS=()
if should_build linux;   then DESKTOP_PLATFORMS+=("linux-x64"); fi
if should_build windows; then DESKTOP_PLATFORMS+=("windows-x64"); fi
if [ -n "$OSXCROSS_CLANG" ]; then
    if should_build macos-x64;   then DESKTOP_PLATFORMS+=("macos-x64"); fi
    if should_build macos-arm64; then DESKTOP_PLATFORMS+=("macos-arm64"); fi
elif should_build macos-x64 || should_build macos-arm64; then
    echo "WARN: osxcross not found, skipping macOS builds"
fi

# Concurrency cap. Each parallel bevy lane peaks at a few GB during codegen and
# link, so memory — not cores — is the real ceiling. Derive a default from
# container RAM (~4 GB/lane), clamp to nproc, and let BUILD_JOBS override.
NPROC=$(nproc 2>/dev/null || echo 4)
MEM_GB=$(awk '/MemTotal/{printf "%d", $2/1024/1024}' /proc/meminfo 2>/dev/null || echo 8)
DEFAULT_JOBS=$(( MEM_GB / 4 ))
[ "$DEFAULT_JOBS" -lt 1 ] && DEFAULT_JOBS=1
[ "$DEFAULT_JOBS" -gt "$NPROC" ] && DEFAULT_JOBS="$NPROC"
JOBS="${BUILD_JOBS:-$DEFAULT_JOBS}"
echo "=== Parallel build: up to $JOBS concurrent lane(s) (cores=$NPROC, mem=${MEM_GB}GB; override with BUILD_JOBS) ==="

STATUS_DIR=$(mktemp -d)
trap 'rm -rf "$STATUS_DIR"' EXIT

# Launch a lane in the background: prefix its output with the lane name, and
# record its exit status to a file (pre-seeded with 255 so a lane that gets
# killed — e.g. OOM — before completing is counted as a failure, not a pass).
run_lane() {
    local name="$1" required="$2"; shift 2
    echo "$required" > "$STATUS_DIR/$name.required"
    echo "255" > "$STATUS_DIR/$name.status"
    ( set +e; "$@"; echo $? > "$STATUS_DIR/$name.status" ) 2>&1 | sed -u "s/^/[$name] /" &
}

# Block until fewer than $JOBS lanes are running.
throttle() {
    while [ "$(jobs -rp | wc -l)" -ge "$JOBS" ]; do
        wait -n 2>/dev/null || true
    done
}

# Desktop feature lanes — each owns its own target-dir, so they never contend.
if [ ${#DESKTOP_PLATFORMS[@]} -gt 0 ]; then
    throttle; run_lane "editor"  required lane_desktop_feature editor
fi
if should_build wasm; then
    throttle; run_lane "wasm" required build_wasm
fi
if should_build android-arm64 || should_build android-x86; then
    throttle; run_lane "android" optional build_android
fi
if should_build ios; then
    throttle; run_lane "ios" optional build_ios
fi

# Wait for every lane to finish.
wait || true

# ── Summary + overall exit code ──────────────────────────────────────────────
echo ""
echo "=== Lane summary ==="
overall=0
shopt -s nullglob
for s in "$STATUS_DIR"/*.status; do
    name=$(basename "$s" .status)
    rc=$(cat "$s" 2>/dev/null || echo 1)
    req=$(cat "$STATUS_DIR/$name.required" 2>/dev/null || echo optional)
    if [ "$rc" = "0" ]; then
        printf "  PASS  %-8s (%s)\n" "$name" "$req"
    else
        printf "  FAIL  %-8s (%s, exit %s)\n" "$name" "$req" "$rc"
        [ "$req" = "required" ] && overall=1
    fi
done
shopt -u nullglob

echo ""
echo "=== Build complete ==="
find "$OUTPUT_DIR" -type f | sort

exit $overall

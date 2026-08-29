#!/usr/bin/env bash
# =============================================================================
# Build engine targets — run inside the renzora per-platform toolchain containers
# =============================================================================
#
# Usage: ./scripts/build-all.sh <output-dir> [platform ...]
#
# Each target (editor, runtime) is built in isolation with its own
# feature flag and target directory. No feature unification, no hash mixing.
# The dedicated server is not a separate target — it's the runtime launched
# with `--server`.
#
# ── What this script produces: runtimes, not desktop editors ─────────────────
# The output of a desktop lane is a RUNTIME — the game binary that
# `renzora_export` uses as that platform's export template. The editor binary is
# compiled and then not staged, because an editor carries a plugin SDK and an
# SDK cannot be cross-built: its proc-macro dylibs are artifacts of the machine
# running the compiler, so a Linux container can only ever produce Linux ones.
# See the long note in `build_desktop`.
#
# So the three ways to build divide cleanly:
#
#   cargo renzora     your own platform, complete, editor and SDK included
#   this script       runtimes / export templates, every platform, no editor
#   CI native lanes   the published editors, one runner per platform
#
# The wasm lane is the exception that proves it: a wasm editor has no SDK and
# compiles no Rust at runtime, so it is still built here.
#
# ── Per-platform toolchain images ────────────────────────────────────────────
# The toolchain is split into one image per platform (base + linux
# / windows / macos / ios / android / wasm).
# The `renzora` CLI runs THIS script once inside each requested platform's
# container with that platform's arg, so only that platform's toolchain is
# present. This script needs no per-image awareness: it already filters by the
# platform arg and degrades gracefully when a toolchain is absent (osxcross /
# NDK / the linux cross marker are simply not there in the wrong container, so
# those guards no-op). Passing several platform args still works within a single
# container that happens to carry several toolchains (e.g. a local full image).
#
# Platforms (positional args after <output-dir>; pass none to build all):
#   linux        Linux, native container arch (x86_64 or arm64)
#   windows      Windows x86_64 MSVC (xwin)
#   macos        macOS x86_64 + arm64 (osxcross)
#   macos-x64    macOS x86_64 only
#   macos-arm64  macOS arm64 only
#   wasm         WASM game runtime + editor (two bundles under web-wasm32/)
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

# ── Which cargo profile the ENGINE is built with ─────────────────────────
# `release` (size-optimised: opt-level "s" + thin LTO), not `dist`, which is the
# fast-link profile a contributor iterates with. The two exist precisely so that
# shipping can be slow and small while local builds stay fast — see the profile
# comments in the root Cargo.toml.
#
# The cargo profile name doubles as the target-dir subdirectory, so this one
# variable moves both. Overridable (RENZORA_PROFILE=dist) to reproduce a lane
# quickly without waiting for LTO.
#
# NOTE: `plugins/*` and `tools/updater` are deliberately NOT built with it. Each
# is its own cargo workspace with its own tuned `[profile.dist]`, and their
# outputs are kilobytes — nothing to gain, a target-dir rename to lose.
PROFILE="${RENZORA_PROFILE:-release}"
# Exported so anything this script shells out to selects the same profile by
# name instead of falling back to its own default. That mattered acutely while
# the SDK was staged here — an unexported value had a `release` lane invoke an
# xtask that rebuilt the entire workspace under `dist` and then described a
# compilation no shipped binary came from — and it is kept because agreeing on
# one profile is cheap and disagreeing is expensive.
export RENZORA_PROFILE="$PROFILE"
echo "=== engine profile: $PROFILE ==="

# The native linux lane builds the container's own arch (full toolchain, mold,
# fastest path), so its platform name / Rust triple / AppImage arch follow uname.
if [ "$(uname -m)" = "aarch64" ]; then
    LINUX_PLATFORM="linux-arm64"
    LINUX_TRIPLE="aarch64-unknown-linux-gnu"
    LINUX_APPIMAGE_ARCH="aarch64"
else
    LINUX_PLATFORM="linux-x64"
    LINUX_TRIPLE="x86_64-unknown-linux-gnu"
    LINUX_APPIMAGE_ARCH="x86_64"
fi

# Cross linux lane — the OTHER desktop Linux arch. The Dockerfile drops a marker
# (/etc/renzora-linux-cross-triple) iff it installed the cross gcc + foreign
# libasound2/libudev dev packages for that arch. When present we emit BOTH
# arches from one container via true cross-compilation (rustc runs at native
# speed, emits foreign code, links with the cross gcc — no emulation). When
# absent (older image built before this support) we silently stay native-only,
# so an un-rebuilt image keeps working exactly as before.
LINUX_CROSS_TRIPLE=""
LINUX_CROSS_PLATFORM=""
LINUX_CROSS_APPIMAGE_ARCH=""
if [ -f /etc/renzora-linux-cross-triple ]; then
    LINUX_CROSS_TRIPLE=$(cat /etc/renzora-linux-cross-triple)
    case "$LINUX_CROSS_TRIPLE" in
        x86_64-*)  LINUX_CROSS_PLATFORM="linux-x64";   LINUX_CROSS_APPIMAGE_ARCH="x86_64"  ;;
        aarch64-*) LINUX_CROSS_PLATFORM="linux-arm64"; LINUX_CROSS_APPIMAGE_ARCH="aarch64" ;;
    esac
fi

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

# ── Helper: copy one executable into the staged tree ────────────────────────
# Usage: stage_bin <src> <dest>. Returns non-zero (without copying) if <src>
# isn't there, so callers can decide whether a missing binary is fatal.
stage_bin() {
    [ -f "$1" ] || return 1
    cp "$1" "$2"
    chmod +x "$2" 2>/dev/null || true
    return 0
}

# ── Helper: copy shared libraries for a platform ────────────────────────────
# Usage: copy_shared_libs <target-dir> <output-dir> <lib-ext>
copy_shared_libs() {
    local SRC="$1"
    local OUT="$2"
    local EXT="$3"
    local HOST_BIN="$4"
    local RUST_TARGET="${5:-native}"

    mkdir -p "$OUT/plugins"

    # ── std, the toolchain's own shared runtime ──────────────────────────────
    #
    # A `copy_std` helper used to do this and was deleted when Bevy went static,
    # on the reasoning that with nothing sharing a Bevy there was nothing left to
    # share a std with either. That reasoning expired: `dynamic_linking` is back
    # in the default features, so `-C prefer-dynamic` applies again and BOTH
    # executables import `std-<hash>.$EXT`.
    #
    # Missing it does not degrade gracefully. The OS loader refuses the binary
    # before `main` with a dialog naming the file and nothing else:
    #
    #     The code execution cannot proceed because std-<hash>.dll was not found
    #
    # Read out of the binary's own import strings, for the same reason
    # `bevy_dylib` is below: the hash is derived from the toolchain, so globbing
    # the sysroot or hardcoding a name ships the wrong one the moment the pin in
    # `rust-toolchain.toml` moves. `--print target-libdir` is asked for the
    # TARGET's sysroot, which for a cross build is not the host's.
    local STD_WANT=""
    [ -n "$HOST_BIN" ] && [ -f "$HOST_BIN" ] && \
        STD_WANT=$(grep -aoE "(lib)?std-[0-9a-f]+\.$EXT" "$HOST_BIN" 2>/dev/null | head -1)
    if [ -n "$STD_WANT" ]; then
        local LIBDIR="" STD_SRC="" cand
        if [ "$RUST_TARGET" = "native" ]; then
            LIBDIR=$(rustc --print target-libdir 2>/dev/null || true)
        else
            LIBDIR=$(rustc --print target-libdir --target "$RUST_TARGET" 2>/dev/null || true)
        fi
        for cand in "$LIBDIR/$STD_WANT" "$SRC/deps/$STD_WANT" "$SRC/$STD_WANT"; do
            [ -n "$cand" ] && [ -f "$cand" ] && { STD_SRC="$cand"; break; }
        done
        if [ -n "$STD_SRC" ]; then
            cp "$STD_SRC" "$OUT/"
            echo "    staged $STD_WANT"
        else
            # Loud, because the alternative is an artifact that looks complete and
            # cannot start on any machine.
            echo "WARN: $HOST_BIN imports $STD_WANT but it was not found (looked in ${LIBDIR:-<no libdir>})"
            echo "      the shipped binary will refuse to launch"
        fi
    fi

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
    # NOTE: the "static-Bevy split removed all of these" note that used to sit
    # here has been out of date since native plugins landed. `dynamic_linking` is
    # back in `renzora_app`'s default features, so a desktop build produces
    # `bevy_dylib` (handled above), `renzora_dylib` and `renzora_ember_dylib`
    # again — and the executables IMPORT them. Missing one does not degrade
    # gracefully: the OS loader refuses the binary before `main`, with a dialog
    # naming a filename and nothing else.
    #
    # `librenzora.$EXT` / `librenzora_editor.$EXT` stay in the list only so a
    # stale dylib left in a warm cargo cache from before that change lands beside
    # the exe rather than being swept into plugins/ below.
    for f in \
        "$SRC/librenzora_dylib.$EXT"        "$SRC/renzora_dylib.$EXT" \
        "$SRC/librenzora_ember_dylib.$EXT"  "$SRC/renzora_ember_dylib.$EXT" \
        "$SRC/librenzora.$EXT"              "$SRC/renzora.$EXT" \
        "$SRC/librenzora_editor.$EXT"       "$SRC/renzora_editor.$EXT"; do
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
        # The two shared engine images, staged beside the exe just above. Swept
        # into plugins/ they would be ~37 MB of duplicate dead weight AND get
        # `dlopen`'d by the C-ABI loader looking for an entry point they do not
        # export.
        [[ "$base" == *renzora_dylib* ]] && continue
        [[ "$base" == *renzora_ember_dylib* ]] && continue
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

# ── Build the standalone C-ABI plugins for one platform ─────────────────────
# Usage: build_plugins <rust-target|native> <platform-name> <ext>
#
# `plugins/*` are separate cargo projects, NOT workspace members — deliberately,
# because as members they would inherit the engine's feature unification and link
# Bevy, destroying the zero-dependency property that lets a plugin built by any
# rustc load into any engine. The cost is that `cargo build --workspace` never
# sees them, and for a long time nothing else in CI did either: every published
# artifact shipped an EMPTY `plugins/` directory, which is not a subtle
# degradation — it is no Lua scripting, no HTTP, and none of the ~50 post-process
# effects, with no error message, because the host simply finds nothing to load.
#
# `cargo renzora` (xtask) has always built them for the host; this is the
# cross-compiling equivalent, and the skip list below mirrors `is_not_a_plugin`
# there.
#
# **Best-effort per plugin, on purpose.** These have real third-party
# dependencies — mlua compiles C, the HTTP plugin pulls the rustls/ring stack —
# and any of them can fail to cross-compile for a given target without that being
# a reason to sink the whole engine build. A plugin that fails is named in the
# summary and simply absent from the artifact. Silence would be the bug: an empty
# `plugins/` looked exactly like a successful build for months.
build_plugins() {
    local RUST_TARGET="$1" PLATFORM="$2" EXT="$3"
    [ -d plugins ] || return 0

    local TARGET_FLAG=() SRC="plugins/target/dist"
    if [ "$RUST_TARGET" != "native" ]; then
        TARGET_FLAG=(--target "$RUST_TARGET")
        SRC="plugins/target/$RUST_TARGET/dist"
    fi

    echo "=== Building C-ABI plugins for $PLATFORM ==="
    local dir name log built=0 failed=()
    for dir in plugins/*/; do
        [ -f "$dir/Cargo.toml" ] || continue
        name=$(basename "$dir")
        # NATIVE plugins are skipped here, and running cargo on one would be
        # actively harmful rather than merely wrong. `crate-type = ["dylib"]`
        # means it links the real Bevy — and `plugins/` is outside the engine
        # workspace, so cargo would resolve it a FRESH Bevy from crates.io. That
        # is a ten-minute build per plugin whose output has different `TypeId`s
        # from the engine: it loads, runs, and corrupts the World.
        #
        # The only sound way to build one is against the artifacts the engine was
        # actually built from, which is what the staged SDK holds and what
        # `xtask/src/native_plugin.rs` does. The quoted `"dylib"` is matched
        # rather than the bare word because `"cdylib"` also ends in `dylib`.
        if grep -q 'crate-type.*"dylib"' "$dir/Cargo.toml"; then
            echo "    skipping '$name' (native plugin — built against the SDK, not by cargo)"
            continue
        fi
        log=$(mktemp)
        if ( cd "$dir" && cargo build --profile dist "${TARGET_FLAG[@]}" ) > "$log" 2>&1; then
            built=$((built + 1))
        else
            failed+=("$name")
            # The tail is what says *why*; a bare "failed" would send the next
            # person to reproduce a 67-plugin build by hand.
            echo "WARN: plugin '$name' failed to build for $PLATFORM:"
            tail -15 "$log" | sed 's/^/    /'
        fi
        rm -f "$log"
    done

    # Sweep the built cdylibs into the staged tree. Only the profile dir's ROOT
    # is read: dependency artifacts (including proc-macro dylibs, which crash the
    # loader if `dlopen`'d) live in `deps/`, and the guards below are belt and
    # braces for anything a warm cache leaves behind.
    local OUT="$OUTPUT_DIR/$PLATFORM/plugins"
    mkdir -p "$OUT"
    local staged=0 f base
    for f in "$SRC"/*."$EXT"; do
        [ -f "$f" ] || continue
        base=$(basename "$f")
        [[ "$base" == *renzora_macros* ]] && continue
        [[ "$base" == *renzora_plugin_derive* ]] && continue
        [[ "$base" == *avian_derive* ]] && continue
        [[ "$base" == libstd-* || "$base" == std-* ]] && continue
        cp "$f" "$OUT/"
        staged=$((staged + 1))
    done

    echo "=== $PLATFORM plugins: $built built, ${#failed[@]} failed, $staged staged ==="
    if [ ${#failed[@]} -gt 0 ]; then
        echo "    not shipped: ${failed[*]}"
    fi
    return 0
}

# ── Build the update sidecar for one platform ───────────────────────────────
# Usage: build_updater <rust-target|native> <platform-name> <exe-suffix>
#
# `tools/updater` is its own workspace (like `plugins/*`), so `--workspace` never
# sees it. It has to ship beside the editor: without it, Help ▸ Check for Updates
# can find and download an update and then has nothing to install it with.
#
# Best-effort. A missing sidecar costs the in-place update and nothing else — the
# editor detects its absence and says to download the new version by hand — so it
# is not worth failing an engine build over.
build_updater() {
    local RUST_TARGET="$1" PLATFORM="$2" SUF="$3"
    [ -f tools/updater/Cargo.toml ] || return 0

    local TARGET_FLAG=() SRC="tools/updater/target/dist"
    if [ "$RUST_TARGET" != "native" ]; then
        TARGET_FLAG=(--target "$RUST_TARGET")
        SRC="tools/updater/target/$RUST_TARGET/dist"
    fi

    echo "=== Building update sidecar for $PLATFORM ==="
    if ! ( cd tools/updater && cargo build --profile dist "${TARGET_FLAG[@]}" ); then
        echo "WARN: update sidecar failed to build for $PLATFORM — in-place updates disabled in this build"
        return 0
    fi
    if [ -f "$SRC/renzora-update$SUF" ]; then
        cp "$SRC/renzora-update$SUF" "$OUTPUT_DIR/$PLATFORM/"
        chmod +x "$OUTPUT_DIR/$PLATFORM/renzora-update$SUF" 2>/dev/null || true
    else
        echo "WARN: renzora-update$SUF not produced for $PLATFORM"
    fi
    return 0
}

# NOTE: staging `std-<hash>.{dll,so,dylib}` lives in `copy_shared_libs` above.
# It was deleted from here when Bevy went static, on the reasoning that nothing
# was left to share a std with. `dynamic_linking` is back in the default features
# and brings `-C prefer-dynamic` with it, so the executables import a hashed std
# again and it has to ship. The cost that argument named — a toolchain-versioned
# import under a hashed filename — is real and is simply the price of the shared
# images.

# ── Compress the staged executables with UPX ─────────────────────────────────
# Usage: compress_binaries <platform-name> <exe-suffix>
#
# UPX packs an executable and prepends a decompressor stub, so the shipped file
# unpacks itself into memory at launch. Measured on the `dist` runtime:
# **187.3 MB -> 31.7 MB, an 83% saving**, and the packed binary boots through the
# full plugin and scripting startup — nothing about Bevy's startup or the
# dlopen'd plugins minds a decompressor stub.
#
# `--best --lzma`, not `--brute`: MEASURED on the 187 MB runtime, the two produce
# a **byte-for-byte identical** file (33,363,456 bytes) — `--brute` took 1529 s,
# `--best --lzma` took ~100 s. `--lzma` already pins UPX's strongest compressor,
# and for an amd64 PE the filter space `--brute` additionally explores has
# nothing better to find. (Measured on PE only; ELF/Mach-O were not compared.)
#
# ── What is deliberately NOT packed ──────────────────────────────────────────
# * `renzora-update` — the update sidecar. It is the thing that repairs a broken
#   install; making it the one binary with an extra layer of machinery between
#   the OS loader and `main` is precisely the wrong trade. It is 320 KB anyway.
# * `plugins/*` — 68 libraries totalling ~15 MB against 450 MB of executables, so
#   the win is noise, and packing a `dlopen`ed library is the least-tested UPX
#   path of the three.
#
# ── Ordering ─────────────────────────────────────────────────────────────────
# This MUST run before `fixup_macos`. Packing rewrites the file, which
# invalidates any code signature it already carries, and arm64 macOS refuses to
# launch a binary with an invalid signature — so `rcodesign` has to sign the
# PACKED file, not the other way round.
#
# Best-effort per file: UPX refusing a particular binary must not sink an engine
# build that is otherwise complete. A skipped file just ships uncompressed.
compress_binaries() {
    local PLATFORM="$1" SUF="$2"
    local OUT="$OUTPUT_DIR/$PLATFORM"
    if ! command -v upx >/dev/null 2>&1; then
        echo "WARN: upx not found in this image — $PLATFORM ships uncompressed"
        return 0
    fi

    local f before after
    for f in "$OUT/renzora$SUF" "$OUT/renzora-editor$SUF" "$OUT/renzora-runtime$SUF"; do
        [ -f "$f" ] || continue
        before=$(stat -c %s "$f" 2>/dev/null || echo 0)
        if upx --best --lzma -q "$f" >/dev/null 2>&1; then
            after=$(stat -c %s "$f" 2>/dev/null || echo 0)
            if [ "$before" -gt 0 ] && [ "$after" -gt 0 ]; then
                awk -v b="$before" -v a="$after" -v n="$(basename "$f")" \
                    'BEGIN { printf "  packed %-22s %.1f MB -> %.1f MB (%.0f%% saved)\n", n, b/1048576, a/1048576, (1-a/b)*100 }'
            fi
        else
            echo "  WARN: upx declined $(basename "$f") — shipping it uncompressed"
        fi
    done
    return 0
}

# ── Helper: make a macOS dist folder relocatable ─────────────────────────────
# rustc records each dylib's absolute build path (/app/src/target/...) as its
# install name, so the exe and plugins would ask dyld for those container
# paths at runtime. Rewrite every id and build-path reference to
# @rpath/<name>: the exe already carries an @loader_path rpath (so deps
# resolve next to it), and plugins get @loader_path/.. so their deps resolve
# in the exe dir one level up. libstd already ships as @rpath (its id is
# rewritten to the same value).
#
# install_name_tool invalidates the linker's ad-hoc code signature, and arm64
# macOS kills binaries with invalid signatures — so every touched file is
# re-signed ad-hoc with rcodesign afterwards.
fixup_macos() {
    local OUT="$1"
    local INT OTOOL RCS
    INT=$(ls /opt/osxcross/target/bin/*-install_name_tool 2>/dev/null | head -1 || true)
    OTOOL=$(ls /opt/osxcross/target/bin/*-otool 2>/dev/null | head -1 || true)
    RCS=$(command -v rcodesign || true)
    if [ -z "$INT" ] || [ -z "$OTOOL" ]; then
        echo "WARN: cctools not found; macOS dist keeps build-path install names"
        return 0
    fi
    [ -z "$RCS" ] && echo "WARN: rcodesign not found; macOS arm64 binaries will have invalid signatures"

    local f dep
    for f in "$OUT/renzora" "$OUT/renzora-editor" "$OUT/renzora-runtime" "$OUT/renzora-update" "$OUT"/*.dylib "$OUT"/plugins/*.dylib; do
        [ -f "$f" ] || continue
        case "$f" in
            *.dylib) "$INT" -id "@rpath/$(basename "$f")" "$f" ;;
        esac
        for dep in $("$OTOOL" -L "$f" | awk 'NR>1 {print $1}' | grep -E '^/.*/target/' || true); do
            "$INT" -change "$dep" "@rpath/$(basename "$dep")" "$f"
        done
        case "$f" in
            "$OUT"/plugins/*.dylib) "$INT" -add_rpath "@loader_path/.." "$f" 2>/dev/null || true ;;
        esac
        if [ -n "$RCS" ]; then
            "$RCS" sign "$f" >/dev/null 2>&1 || echo "WARN: rcodesign failed on $f"
        fi
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
    local SRC="target/$FEATURE/$PROFILE"

    if [ "$RUST_TARGET" != "native" ]; then
        TARGET_FLAG="--target $RUST_TARGET"
        SRC="target/$FEATURE/$RUST_TARGET/$PROFILE"
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
        cargo build --profile "$PROFILE" --workspace \
            --exclude renzora-android --exclude renzora-ios \
            $TARGET_DIR_FLAG $TARGET_FLAG || return 1
    else
        cargo build --profile "$PROFILE" --bin renzora --no-default-features \
            --features "$FEATURE" $TARGET_DIR_FLAG $TARGET_FLAG || return 1
    fi

    local OUT="$OUTPUT_DIR/$PLATFORM"
    mkdir -p "$OUT"

    # ── The binaries ─────────────────────────────────────────────────────────
    # The workspace has two executables:
    #
    #   renzora         the runtime / shipped game (package `renzora_app`)
    #   renzora-editor  the editor              (package `renzora_editor_app`)
    #
    # This script stages the RUNTIME only. It never ships an editor.
    #
    # `renzora-editor` is compiled here (the lane builds `--workspace`) and then
    # deliberately left behind, because a container cannot produce a *usable*
    # editor for anything but its own Linux architecture, and it should not
    # produce a half-usable one for everything else.
    #
    # The reason is the plugin SDK. An editor compiles native plugins and Rust
    # scripts on the machine it runs on, so it ships the metadata and the
    # proc-macro dylibs `rustc` needs. Proc macros run *inside* the compiler, so
    # they are built for the host — and cross-compiling here means that host is
    # Linux. Ship that to a Windows or macOS user and their `rustc` cannot load
    # half of it: `can't find crate for bevy_derive`, and with it every name
    # behind `bevy::prelude`. Nor can the mismatch be patched afterwards; each
    # `.rmeta` records the hash of what it was compiled against, so the metadata
    # and the proc macros have to come out of one build on one machine whose own
    # platform is the platform being built for.
    #
    # That is not fixable in a cross-compiler, so the job moved instead:
    #
    #   cargo renzora     your own platform, complete, with a working SDK
    #   this script       runtime templates, any platform, no editor
    #   CI native lanes   editors, one runner per platform
    #
    # Nothing is lost. A game needs no SDK — it ships plugins already compiled —
    # so cross-built runtimes are correct, which is exactly what the export
    # templates in `renzora_export` are. And the editor for the machine you are
    # sitting at never wanted a container in the first place.
    #
    # The bundle wrappers below already expect this: `AppRun` and the `.app`'s
    # CFBundleExecutable both fall back to `renzora` when no editor binary is
    # present, so an AppImage/.app still forms and `TemplateManager::scan` still
    # finds the runtime inside it.
    local SUF=""
    [ "$EXT" = "dll" ] && SUF=".exe"

    local HOST_BIN
    case "$FEATURE" in
        editor)
            stage_bin "$SRC/renzora$SUF" "$OUT/renzora$SUF" || true
            HOST_BIN="$OUT/renzora$SUF"
            ;;
        runtime)
            # Runtime-only lane: rename so the artefact is self-describing.
            stage_bin "$SRC/renzora$SUF" "$OUT/renzora-runtime$SUF" || true
            HOST_BIN="$OUT/renzora-runtime$SUF"
            ;;
    esac

    # The triple matters: `std-<hash>` must come from the TARGET's sysroot, which
    # for a cross build is not the host's.
    copy_shared_libs "$SRC" "$OUT" "$EXT" "$HOST_BIN" "$RUST_TARGET"
    return 0
}

# The plugin SDK is deliberately absent from this script. It used to be staged
# and packed here, which is how cross-built editors came to ship an SDK whose
# proc macros were for the wrong operating system. Staging it belongs where it
# can be correct: `cargo renzora` for the machine you are on, and the native
# per-platform CI lanes for everything published. See `build_desktop`.

# ── Build one (platform, feature) pair, incl. its Rust std ───────────────────
# The C-ABI plugins are built here rather than in a lane of their own because
# they must land in `$OUTPUT_DIR/<platform>/plugins/` BEFORE the AppImage/.app
# wrap moves that directory inside the bundle. `fixup_macos` likewise has to run
# after them, so a plugin dylib gets its install name rewritten to @rpath along
# with everything else.
build_one() {
    local PLATFORM="$1" FEATURE="$2"

    # ── Windows cannot ship `release`, whoever asked for it ──────────────────
    #
    # A PE export ordinal is 16 bits, so a DLL exports at most 65,535 symbols —
    # a property of the file format that no linker flag gets past. `bevy_dylib`
    # lands either side of it on optimisation level alone:
    #
    #     opt-level = 2      41,958   links
    #     opt-level = "s"   269,482   rust-lld: too many exported symbols
    #
    # `"s"` inlines far less, so generic instantiations that `2` folds into
    # their callers survive as separate functions, and a Rust `dylib` exports
    # every one. `[profile.release]` is the size-optimised one, so a Windows
    # lane built with it does not link at all.
    #
    # This is decided HERE, next to the platform, rather than by each caller
    # passing `RENZORA_PROFILE=dist`. The CI workflow did pass it and was fine;
    # `renzora build windows` does not, fell through to the `release` default,
    # and hit the cap — a constraint of the target reached the script only by
    # every entry point happening to remember it. `local -x` scopes the
    # override to this call, so a multi-platform run still builds Linux and
    # macOS at `release`, which have no such ceiling.
    case "$PLATFORM" in
        windows-*)
            if [ "$PROFILE" != "dist" ]; then
                echo "    windows: profile $PROFILE -> dist (PE exports are 16-bit; bevy_dylib needs it)"
            fi
            local PROFILE="dist"
            local -x RENZORA_PROFILE="dist"
            ;;
    esac

    case "$PLATFORM" in
        "$LINUX_PLATFORM")
            build_desktop "$FEATURE" native           "$LINUX_PLATFORM" "so"    || return 1
            build_plugins native "$LINUX_PLATFORM" "so"
            build_updater native "$LINUX_PLATFORM" ""
            compress_binaries "$LINUX_PLATFORM" "" ;;
        "$LINUX_CROSS_PLATFORM")
            # Cross arch — explicit --target triple (like macOS/Windows), not
            # `native`. The .cargo/config.toml entry for this triple points the
            # linker at the GNU cross-gcc.
            build_desktop "$FEATURE" "$LINUX_CROSS_TRIPLE" "$LINUX_CROSS_PLATFORM" "so" || return 1
            build_plugins "$LINUX_CROSS_TRIPLE" "$LINUX_CROSS_PLATFORM" "so"
            build_updater "$LINUX_CROSS_TRIPLE" "$LINUX_CROSS_PLATFORM" ""
            compress_binaries "$LINUX_CROSS_PLATFORM" "" ;;
        windows-x64)
            build_desktop "$FEATURE" x86_64-pc-windows-msvc "windows-x64" "dll"   || return 1
            # MSVC ABI build — links to vcruntime140.dll / msvcp140.dll which
            # Win10/11 ship by default (or via the VC++ Redistributable).
            build_plugins x86_64-pc-windows-msvc "windows-x64" "dll"
            build_updater x86_64-pc-windows-msvc "windows-x64" ".exe"
            compress_binaries "windows-x64" ".exe" ;;
        macos-x64)
            build_desktop "$FEATURE" x86_64-apple-darwin    "macos-x64"   "dylib" || return 1
            build_plugins x86_64-apple-darwin "macos-x64" "dylib"
            build_updater x86_64-apple-darwin "macos-x64" ""
            # Pack BEFORE signing — packing invalidates a signature.
            compress_binaries "macos-x64" ""
            fixup_macos "$OUTPUT_DIR/macos-x64" ;;
        macos-arm64)
            build_desktop "$FEATURE" aarch64-apple-darwin   "macos-arm64" "dylib" || return 1
            build_plugins aarch64-apple-darwin "macos-arm64" "dylib"
            build_updater aarch64-apple-darwin "macos-arm64" ""
            # Pack BEFORE signing — packing invalidates a signature.
            compress_binaries "macos-arm64" ""
            fixup_macos "$OUTPUT_DIR/macos-arm64" ;;
        *)
            echo "WARN: unknown desktop platform '$PLATFORM'"; return 1 ;;
    esac
    return 0
}

# ── Wrap the Linux editor output into an AppDir + AppImage ────────────────────
wrap_linux_appimage() {
    local PLATFORM="$1" APPIMAGE_ARCH="$2"
    local EDITOR_DIR="$OUTPUT_DIR/$PLATFORM"
    [ -f "$EDITOR_DIR/renzora" ] || return 0

    local APPDIR="$EDITOR_DIR/Renzora Engine.AppDir"
    rm -rf "$APPDIR"
    mkdir -p "$APPDIR/plugins"
    # Move all artifacts into the AppDir. BOTH executables go in: the AppImage is
    # the editor, but the editor shells out to `renzora` next to itself for
    # external-runtime play mode, so shipping only one breaks Play.
    mv "$EDITOR_DIR/renzora" "$APPDIR/renzora"
    [ -f "$EDITOR_DIR/renzora-editor" ] && mv "$EDITOR_DIR/renzora-editor" "$APPDIR/renzora-editor"
    # The update sidecar rides along: the editor copies it out to a temp dir at
    # install time, which works fine from inside the AppImage's read-only mount.
    [ -f "$EDITOR_DIR/renzora-update" ] && mv "$EDITOR_DIR/renzora-update" "$APPDIR/renzora-update"
    for f in "$EDITOR_DIR"/*.so; do [ -f "$f" ] && mv "$f" "$APPDIR/"; done
    if [ -d "$EDITOR_DIR/plugins" ]; then
        for f in "$EDITOR_DIR/plugins"/*.so; do [ -f "$f" ] && mv "$f" "$APPDIR/plugins/"; done
        rmdir "$EDITOR_DIR/plugins" 2>/dev/null || true
    fi

    cat > "$APPDIR/AppRun" << 'APPRUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export LD_LIBRARY_PATH="$HERE:$HERE/plugins:${LD_LIBRARY_PATH:-}"
# This AppImage IS the editor, so launch the editor binary. `renzora` beside it
# is the runtime the editor spawns for Play — falling back to it keeps an
# editor-less (runtime-only) build launchable rather than silently dead.
if [ -x "$HERE/renzora-editor" ]; then
    exec "$HERE/renzora-editor" "$@"
fi
exec "$HERE/renzora" "$@"
APPRUN
    chmod +x "$APPDIR/AppRun"

    cat > "$APPDIR/renzora-engine.desktop" << 'DESKTOP'
[Desktop Entry]
Type=Application
Name=Renzora Engine
Exec=renzora-editor
Icon=renzora-engine
Categories=Development;Graphics;
Terminal=false
DESKTOP

    if [ -f icon.png ]; then
        cp icon.png "$APPDIR/renzora-engine.png"
        cp icon.png "$APPDIR/.DirIcon"
    fi

    # appimagetool embeds an arch-specific runtime into the .AppImage. When
    # cross-building (e.g. an x86_64 AppImage from an arm64 host) it can't reuse
    # its own host runtime, so prefer a pre-staged runtime for the TARGET arch
    # (the Dockerfile fetches both); fall back to appimagetool's own lookup.
    local RUNTIME_ARG=()
    [ -f "/opt/appimage-runtimes/runtime-$APPIMAGE_ARCH" ] \
        && RUNTIME_ARG=(--runtime-file "/opt/appimage-runtimes/runtime-$APPIMAGE_ARCH")

    if command -v appimagetool >/dev/null 2>&1; then
        ARCH="$APPIMAGE_ARCH" appimagetool "${RUNTIME_ARG[@]}" "$APPDIR" "$EDITOR_DIR/Renzora Engine-$APPIMAGE_ARCH.AppImage" \
            && echo "Built $EDITOR_DIR/Renzora Engine-$APPIMAGE_ARCH.AppImage" \
            || echo "WARN: appimagetool failed"
    else
        echo "WARN: appimagetool not found; AppDir left at $APPDIR"
    fi
    return 0
}

# ── Wrap a macOS editor output into a .app bundle ────────────────────────────
# Mirrors the Linux AppImage wrap: the flat dist files move INTO the bundle
# (exe + SDK dylibs + plugins/ under Contents/MacOS, so the @rpath /
# @loader_path layout from fixup_macos keeps working unchanged), plus an
# Info.plist and an icns built from icon.png. Runs after fixup_macos — the
# files are already signed, and moving them preserves signatures.
wrap_macos_app() {
    local OUT="$OUTPUT_DIR/$1"
    [ -f "$OUT/renzora" ] || return 0

    local APP="$OUT/Renzora Engine.app"
    local MACOS_DIR="$APP/Contents/MacOS"
    local RES_DIR="$APP/Contents/Resources"
    rm -rf "$APP"
    mkdir -p "$MACOS_DIR/plugins" "$RES_DIR"

    # Both executables move in — the .app is the editor, and the editor spawns
    # `renzora` from its own directory for external-runtime play mode.
    mv "$OUT/renzora" "$MACOS_DIR/renzora"
    [ -f "$OUT/renzora-editor" ] && mv "$OUT/renzora-editor" "$MACOS_DIR/renzora-editor"
    [ -f "$OUT/renzora-update" ] && mv "$OUT/renzora-update" "$MACOS_DIR/renzora-update"
    local f
    for f in "$OUT"/*.dylib; do [ -f "$f" ] && mv "$f" "$MACOS_DIR/"; done
    if [ -d "$OUT/plugins" ]; then
        for f in "$OUT/plugins"/*.dylib; do [ -f "$f" ] && mv "$f" "$MACOS_DIR/plugins/"; done
        rmdir "$OUT/plugins" 2>/dev/null || true
    fi

    # icns from icon.png: an icns is just a header plus typed PNG chunks, so a
    # single-size icon needs no Apple tooling. Chunk type is keyed off the
    # PNG's pixel width (256 -> ic08 today; the map covers other sizes).
    if [ -f icon.png ] && command -v python3 >/dev/null 2>&1; then
        python3 - icon.png "$RES_DIR/renzora.icns" <<'PY' || echo "WARN: icns generation failed"
import struct, sys
png = open(sys.argv[1], 'rb').read()
w = struct.unpack('>I', png[16:20])[0]
typ = {16:'icp4',32:'icp5',64:'icp6',128:'ic07',256:'ic08',512:'ic09',1024:'ic10'}.get(w)
if not typ:
    sys.exit(f'unsupported icon size {w}')
chunk = typ.encode() + struct.pack('>I', len(png) + 8) + png
open(sys.argv[2], 'wb').write(b'icns' + struct.pack('>I', len(chunk) + 8) + chunk)
PY
    fi

    # CFBundleExecutable names the EDITOR — double-clicking "Renzora Engine.app"
    # must open the editor, not the game runtime that ships alongside it for
    # Play. Falls back to `renzora` so a runtime-only tree still yields a
    # launchable bundle. (Unquoted heredoc so this interpolates; the plist body
    # contains no other `$` or backticks.)
    local MAIN_BIN="renzora"
    [ -f "$MACOS_DIR/renzora-editor" ] && MAIN_BIN="renzora-editor"
    cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>            <string>Renzora Engine</string>
    <key>CFBundleDisplayName</key>     <string>Renzora Engine</string>
    <key>CFBundleIdentifier</key>      <string>org.renzora.engine</string>
    <key>CFBundleExecutable</key>      <string>$MAIN_BIN</string>
    <key>CFBundleIconFile</key>        <string>renzora</string>
    <key>CFBundlePackageType</key>     <string>APPL</string>
    <key>CFBundleVersion</key>         <string>0.2.0</string>
    <key>CFBundleShortVersionString</key> <string>0.2.0</string>
    <key>LSMinimumSystemVersion</key>  <string>11.0</string>
    <key>NSHighResolutionCapable</key> <true/>
</dict>
</plist>
PLIST

    # Sign the assembled bundle: inside a .app, codesign verifies the main
    # executable under bundle rules (sealed _CodeResources), so the per-file
    # ad-hoc signatures from fixup_macos aren't sufficient for it.
    if command -v rcodesign >/dev/null 2>&1; then
        rcodesign sign "$APP" >/dev/null 2>&1 || echo "WARN: bundle signing failed for $APP"
    else
        echo "WARN: rcodesign not found; $APP is unsigned and arm64 macOS will refuse to launch it"
    fi

    echo "Built $APP"
    return 0
}

# ── Lane: build one feature across every requested desktop platform ──────────
lane_desktop_feature() {
    local FEATURE="$1" p
    for p in "${DESKTOP_PLATFORMS[@]}"; do
        # The cross Linux arch is best-effort: a cross-link failure must not
        # sink the native build that already succeeded. Every other platform
        # stays fatal. (The cross dir simply has no `renzora` on failure, so the
        # AppImage wrap below no-ops for it.)
        if [ -n "$LINUX_CROSS_PLATFORM" ] && [ "$p" = "$LINUX_CROSS_PLATFORM" ]; then
            build_one "$p" "$FEATURE" \
                || echo "WARN: cross Linux build ($p) failed — native arch still built"
        else
            build_one "$p" "$FEATURE" || return 1
        fi
    done
    # Bundle wrapping only applies to the editor (AppImage / .app).
    if [ "$FEATURE" = "editor" ]; then
        if array_contains "$LINUX_PLATFORM" "${DESKTOP_PLATFORMS[@]}"; then
            wrap_linux_appimage "$LINUX_PLATFORM" "$LINUX_APPIMAGE_ARCH" || return 1
        fi
        if [ -n "$LINUX_CROSS_PLATFORM" ] && array_contains "$LINUX_CROSS_PLATFORM" "${DESKTOP_PLATFORMS[@]}"; then
            wrap_linux_appimage "$LINUX_CROSS_PLATFORM" "$LINUX_CROSS_APPIMAGE_ARCH" || return 1
        fi
        if array_contains "macos-x64" "${DESKTOP_PLATFORMS[@]}"; then
            wrap_macos_app macos-x64 || return 1
        fi
        if array_contains "macos-arm64" "${DESKTOP_PLATFORMS[@]}"; then
            wrap_macos_app macos-arm64 || return 1
        fi
    fi
    return 0
}

# ── Lane: WASM ───────────────────────────────────────────────────────────────
# WASM bundles plugins statically (no dlopen). Native plugin crates compile
# as rlib for wasm — Cargo silently skips their dylib output for this target.
# wasm-bindgen the built module, then shrink it. Usage:
#   bindgen_wasm <built .wasm> <out-name>
#
# BOTH steps are checked. They used to be bare calls in a lane whose body runs
# under `set +e` (see `run_lane`), so a wasm-bindgen failure fell through to
# wasm-opt on a file that wasn't there and the lane still returned 0 — PASS, with
# a partial tree uploaded. `if-no-files-found: error` only catches a completely
# empty `dist/`, so a half-built web bundle would have shipped looking fine.
bindgen_wasm() {
    local WASM_FILE="$1" OUT_NAME="$2"
    local OUT="$OUTPUT_DIR/web-wasm32"
    mkdir -p "$OUT"

    wasm-bindgen --out-dir "$OUT" --out-name "$OUT_NAME" --target web "$WASM_FILE" || {
        echo "ERROR: wasm-bindgen failed for $OUT_NAME"
        return 1
    }

    # -Oz on a 150–200 MB module is measured in minutes and is the single
    # biggest cost in this lane — it is also what takes the bundle from hundreds
    # of MB to tens, so it stays.
    if command -v wasm-opt &>/dev/null; then
        wasm-opt -Oz \
            --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int \
            --enable-mutable-globals --enable-reference-types --enable-multivalue \
            "$OUT/${OUT_NAME}_bg.wasm" -o "$OUT/${OUT_NAME}_bg.wasm" || {
            echo "ERROR: wasm-opt failed for $OUT_NAME"
            return 1
        }
    else
        echo "WARN: wasm-opt not found — $OUT_NAME ships unoptimized (hundreds of MB)"
    fi
    return 0
}

build_wasm() {
    echo "=== Building WASM Runtime ==="
    cargo build --profile "$PROFILE" -p renzora_app --no-default-features --features wasm \
        --target wasm32-unknown-unknown --target-dir target/wasm || return 1
    local WASM_FILE
    WASM_FILE=$(find "target/wasm/wasm32-unknown-unknown/$PROFILE" -name "renzora.wasm" 2>/dev/null | head -1)
    if [ -z "$WASM_FILE" ]; then
        echo "ERROR: renzora.wasm not produced"
        return 1
    fi
    bindgen_wasm "$WASM_FILE" renzora-runtime || return 1

    # ── The editor ───────────────────────────────────────────────────────────
    # A SEPARATE target dir, deliberately. `renzora_app --features wasm` resolves
    # `renzora` WITHOUT its `editor` feature and `renzora_editor_app` resolves it
    # WITH — different feature sets, different `-C metadata`, so sharing one dir
    # would make each build evict ~86 shared packages the other just compiled.
    # Two dirs cost disk (the job's "Free disk space" step is why there is any);
    # one dir would cost a full extra recompile on every run.
    #
    # This is a COMPILE target, not yet a usable product: the web editor has no
    # way to open a project until the filesystem shim lands (a browser cannot
    # reach a local folder synchronously — `showDirectoryPicker` is async-only,
    # and `createSyncAccessHandle` is OPFS + Worker only). Play mode does work:
    # the in-viewport path needs no subprocess, and the Window/VR targets that
    # would need one are hidden on wasm.
    echo "=== Building WASM Editor ==="
    # `--features wasm` gates the binary target itself. On native the editor is a
    # loadable image beside one executable, so this package's `[[bin]]` would be
    # a redundant second exe — and an unlinkable one, since `renzora_editor` is
    # now both rlib and dylib and a binary linking the rlib gives rustc two
    # formats to choose between. wasm has no dynamic linking, so the editor there
    # stays a separate bundle and asks for the target explicitly.
    cargo build --profile "$PROFILE" -p renzora_editor_app --bin renzora-editor \
        --features wasm \
        --target wasm32-unknown-unknown --target-dir target/wasm-editor || return 1
    local EDITOR_WASM
    EDITOR_WASM=$(find "target/wasm-editor/wasm32-unknown-unknown/$PROFILE" -name "renzora-editor.wasm" 2>/dev/null | head -1)
    if [ -z "$EDITOR_WASM" ]; then
        echo "ERROR: renzora-editor.wasm not produced"
        return 1
    fi
    bindgen_wasm "$EDITOR_WASM" renzora-editor || return 1

    write_web_shell || return 1
    return 0
}

# Minimal host pages for the two web bundles. wasm-bindgen emits the JS glue and
# the module but no page to load them from, so without this the artifact is a
# pile of files with no entry point.
#
# The canvas id matches what bevy_winit looks for on wasm; without an explicit
# canvas Bevy creates its own and the CSS below never applies, which shows up as
# a viewport that ignores the window size.
write_web_shell() {
    local OUT="$OUTPUT_DIR/web-wasm32"
    [ -d "$OUT" ] || return 0
    local name title
    for name in renzora-runtime renzora-editor; do
        [ -f "$OUT/$name.js" ] || continue
        case "$name" in
            renzora-editor) title="Renzora Editor" ;;
            *)              title="Renzora" ;;
        esac
        cat > "$OUT/$name.html" <<HTML
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>$title</title>
<style>
  html, body { margin: 0; height: 100%; background: #14151a; overflow: hidden; }
  /* The canvas is sized by Bevy (`fit_canvas_to_parent`), from THIS element.
     Hence a fixed-size parent and no width/height on the canvas itself: sizing
     the canvas in CSS instead fights Bevy for control of the surface, and a
     parent whose size depends on its children feeds back into a canvas that
     grows on every resize. */
  #stage { position: fixed; inset: 0; }
  canvas { display: block; outline: none; }
  #boot { position: fixed; inset: 0; display: grid; place-items: center;
          font: 14px system-ui, sans-serif; color: #8a8f98; }
</style>
</head>
<body>
<div id="boot">Loading $title…</div>
<div id="stage"><canvas id="bevy"></canvas></div>
<script type="module">
  import init from './$name.js';
  // WebGPU only — the engine's wasm build enables bevy's \`webgpu\` feature, so
  // there is no WebGL fallback to degrade to. Say so plainly rather than
  // failing somewhere deep in adapter selection.
  if (!navigator.gpu) {
    document.getElementById('boot').textContent =
      'This build needs WebGPU. Try Chrome or Edge 113+.';
  } else {
    init()
      .then(() => document.getElementById('boot').remove())
      .catch(e => {
        document.getElementById('boot').textContent = 'Failed to start: ' + e;
        console.error(e);
      });
  }
</script>
</body>
</html>
HTML
    done
    return 0
}

# ── Lane: Android (runtime only) ─────────────────────────────────────────────
# Both archs share target/android (sequential within this lane); best-effort.
build_android() {
    if [ ! -d "${ANDROID_NDK_HOME:-/opt/android-ndk}" ]; then
        echo "WARN: Android NDK not present in this image — skipping Android builds"
        return 0
    fi
    if should_build android-arm64; then
        echo "=== Building Android ARM64 Runtime ==="
        cargo build --profile "$PROFILE" -p renzora-android --target aarch64-linux-android --target-dir target/android 2>&1 || echo "WARN: Android ARM build failed"
        if [ -f target/android/aarch64-linux-android/$PROFILE/libmain.so ]; then
            mkdir -p "$OUTPUT_DIR/android-arm64"
            cp target/android/aarch64-linux-android/$PROFILE/libmain.so "$OUTPUT_DIR/android-arm64/"
        fi
    fi
    if should_build android-x86; then
        echo "=== Building Android x86_64 Runtime ==="
        cargo build --profile "$PROFILE" -p renzora-android --target x86_64-linux-android --target-dir target/android 2>&1 || echo "WARN: Android x86 build failed"
        if [ -f target/android/x86_64-linux-android/$PROFILE/libmain.so ]; then
            mkdir -p "$OUTPUT_DIR/android-x86"
            cp target/android/x86_64-linux-android/$PROFILE/libmain.so "$OUTPUT_DIR/android-x86/"
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
    cargo build --profile "$PROFILE" -p renzora-ios --target aarch64-apple-ios --target-dir target/ios 2>&1 || echo "WARN: iOS build failed"
    if [ -f target/ios/aarch64-apple-ios/$PROFILE/librenzora_ios.a ]; then
        mkdir -p "$OUTPUT_DIR/ios-arm64"
        cp target/ios/aarch64-apple-ios/$PROFILE/librenzora_ios.a "$OUTPUT_DIR/ios-arm64/"
    fi
    return 0
}

# =============================================================================
# Parallel lane orchestration
# =============================================================================

# Which desktop platforms are in scope (filter + osxcross availability).
OSXCROSS_CLANG=$(ls /opt/osxcross/target/bin/x86_64-apple-darwin*-clang 2>/dev/null | head -1 || true)
DESKTOP_PLATFORMS=()
# `linux` builds BOTH desktop Linux arches when the image carries the cross
# toolchain (native + cross); the explicit per-arch names (`linux-x64` /
# `linux-arm64`) select just one, whichever the running container can produce.
if should_build linux || should_build "$LINUX_PLATFORM"; then DESKTOP_PLATFORMS+=("$LINUX_PLATFORM"); fi
if [ -n "$LINUX_CROSS_PLATFORM" ] && { should_build linux || should_build "$LINUX_CROSS_PLATFORM"; }; then
    DESKTOP_PLATFORMS+=("$LINUX_CROSS_PLATFORM")
fi
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

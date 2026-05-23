#!/usr/bin/env bash
# Compress Renzora binaries with UPX --brute.
#
# Usage:
#   ./scripts/upx-compress.sh                       # compress every platform under dist/
#   ./scripts/upx-compress.sh dist/windows-x64      # just one platform
#   ./scripts/upx-compress.sh dist/windows-x64 dist/linux-x64
#
# Targets per build (editor / runtime):
#   - host binary (renzora{,.exe} / renzora-runtime{,.exe})
#   - SDK dylibs (renzora, renzora_editor) on whatever extension the
#     platform produces (renzora_postprocess folded into renzora)
#   - bevy_dylib (any hashed name)
#   - everything in plugins/
#
# `--brute` is the slowest UPX setting (tries every algorithm + filter
# combo) but produces the smallest output. Expect minutes per file. The
# script processes files sequentially so progress is easy to follow.

set -uo pipefail

if ! command -v upx >/dev/null 2>&1; then
    echo "upx not found. Install one of:" >&2
    echo "  Windows: scoop install upx        (or grab a release from https://github.com/upx/upx)" >&2
    echo "  Linux:   sudo apt install upx-ucl" >&2
    echo "  macOS:   brew install upx" >&2
    exit 1
fi

# Default to every platform dir under dist/ if no args. Otherwise accept either
# short platform names (matching `build-all.sh` / `makers docker-build`) or
# explicit paths. `wasm`/`ios` have no UPX-compressible binaries (.wasm/.a), so
# they're intentionally not mapped — pass a path if you really want to try.
if [ $# -eq 0 ]; then
    PLATFORMS=(dist/*/)
else
    PLATFORMS=()
    for arg in "$@"; do
        case "$arg" in
            linux)         PLATFORMS+=("dist/linux-x64") ;;
            windows)       PLATFORMS+=("dist/windows-x64") ;;
            macos)         PLATFORMS+=("dist/macos-x64" "dist/macos-arm64") ;;
            macos-x64)     PLATFORMS+=("dist/macos-x64") ;;
            macos-arm64)   PLATFORMS+=("dist/macos-arm64") ;;
            android)       PLATFORMS+=("dist/android-arm64" "dist/android-x86") ;;
            android-arm64) PLATFORMS+=("dist/android-arm64") ;;
            android-x86)   PLATFORMS+=("dist/android-x86") ;;
            *)             PLATFORMS+=("$arg") ;;  # treat as an explicit path
        esac
    done
fi

# Specific binaries we care about, by basename. The `bevy_dylib*` and
# `plugins/*` cases are handled by glob below since their names vary.
SDK_NAMES=(
    "renzora" "renzora.exe"
    "renzora-runtime" "renzora-runtime.exe"
    "renzora.dll" "librenzora.so" "librenzora.dylib"
    "renzora_editor.dll" "librenzora_editor.so" "librenzora_editor.dylib"
)

human_size() {
    # Pretty-print bytes; works without `numfmt` (BSD systems).
    local b=$1
    if [ "$b" -ge 1048576 ]; then
        awk -v b="$b" 'BEGIN { printf "%.1f MB", b/1048576 }'
    elif [ "$b" -ge 1024 ]; then
        awk -v b="$b" 'BEGIN { printf "%.1f KB", b/1024 }'
    else
        echo "${b} B"
    fi
}

compress_one() {
    local f="$1"
    [ -f "$f" ] || return 0
    local before after pct
    before=$(wc -c <"$f")
    if [ "$before" -lt 1024 ]; then
        printf '  %-50s SKIP (too small)\n' "$(basename "$f")"
        return 0
    fi
    if upx --brute "$f" >/dev/null 2>&1; then
        after=$(wc -c <"$f")
        pct=$(( (before - after) * 100 / before ))
        printf '  %-50s %12s → %12s  (-%d%%)\n' \
            "$(basename "$f")" "$(human_size "$before")" "$(human_size "$after")" "$pct"
    else
        # UPX prints `AlreadyPackedException` for previously-compressed files
        # and rejects some Mach-O / unusual section layouts. Either way, leave
        # the file untouched and move on.
        printf '  %-50s SKIP (already packed / unsupported)\n' "$(basename "$f")"
    fi
}

# Collect the list of files to compress for one build target subdir
# (e.g. dist/windows-x64/editor/) into a global FILES array.
collect_files() {
    local out="$1"
    FILES=()

    local name
    for name in "${SDK_NAMES[@]}"; do
        [ -f "$out/$name" ] && FILES+=("$out/$name")
    done

    # bevy_dylib has a hash suffix (bevy_dylib-abc123.dll, libbevy_dylib-….so).
    local f
    shopt -s nullglob
    for f in "$out"/bevy_dylib*.dll "$out"/libbevy_dylib*.so "$out"/libbevy_dylib*.dylib; do
        [ -f "$f" ] && FILES+=("$f")
    done

    # Plugins.
    if [ -d "$out/plugins" ]; then
        for f in "$out/plugins"/*.dll "$out/plugins"/*.so "$out/plugins"/*.dylib; do
            [ -f "$f" ] && FILES+=("$f")
        done
    fi
    shopt -u nullglob
}

TOTAL_BEFORE=0
TOTAL_AFTER=0

for platform in "${PLATFORMS[@]}"; do
    [ -d "$platform" ] || { echo "skip: $platform (not a directory)"; continue; }
    platform="${platform%/}"

    for target in editor runtime server; do
        out="$platform/$target"
        [ -d "$out" ] || continue

        collect_files "$out"
        if [ ${#FILES[@]} -eq 0 ]; then
            continue
        fi

        echo "=== $out (${#FILES[@]} files) ==="
        for f in "${FILES[@]}"; do
            before=$(wc -c <"$f")
            TOTAL_BEFORE=$(( TOTAL_BEFORE + before ))
            compress_one "$f"
            after=$(wc -c <"$f")
            TOTAL_AFTER=$(( TOTAL_AFTER + after ))
        done
    done
done

if [ "$TOTAL_BEFORE" -gt 0 ]; then
    pct=$(( (TOTAL_BEFORE - TOTAL_AFTER) * 100 / TOTAL_BEFORE ))
    echo
    echo "Total: $(human_size "$TOTAL_BEFORE") → $(human_size "$TOTAL_AFTER")  (-${pct}%)"
fi

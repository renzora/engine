#!/usr/bin/env bash
# =============================================================================
# Turn the Build Engine workflow's per-platform artifacts into release assets
# =============================================================================
#
# Usage: scripts/package-release.sh <artifacts-dir> <out-dir> <tag> <commit>
#
# `<artifacts-dir>` is where `actions/download-artifact` dropped every build
# job's output, i.e. `<artifacts-dir>/<artifact-name>/<platform-dir>/…`. This
# script does not care what the artifacts are called — it walks two levels down
# and matches on the PLATFORM directory name, which is the one thing
# `docker/build-all.sh` and `cargo renzora dist` (xtask) agree on.
#
# Two assets come out of each desktop platform:
#
#   <platform>.zip                   the ENGINE — editor + runtime together.
#                                    Keeps the name the r1-alpha5/6 releases
#                                    already used (`windows-x64.zip`), so links
#                                    to it don't rot.
#   renzora-runtime-<platform>.zip   the EXPORT TEMPLATE — the game runtime and
#                                    its plugins, nothing else. This is what an
#                                    editor downloads to export for a platform it
#                                    isn't running on (`renzora_export::download`),
#                                    so the name here and
#                                    `Platform::release_asset_name()` in that
#                                    module must stay in lockstep.
#
# Plus `manifest.json` (what the editor reads to resolve + verify a template) and
# `SHA256SUMS` (for humans and `sha256sum -c`).
#
# ── The three tree layouts ───────────────────────────────────────────────────
# `build-all.sh` nests each platform's output differently, and the runtime
# extraction has to know all three. This mirrors `TemplateManager::scan()` in
# `crates/renzora_export/src/templates.rs` — if you add a layout, add it there
# too or a locally-built template stops being found.
#
#   windows-*   flat:      <dir>/renzora.exe
#   linux-*     AppDir:    <dir>/Renzora Engine.AppDir/renzora
#   macos-*     .app:      <dir>/Renzora Engine.app/Contents/MacOS/renzora
#
# ── Executable bits ──────────────────────────────────────────────────────────
# `actions/upload-artifact` does NOT preserve unix file modes, so every binary
# arrives here as 0644 and a Linux/macOS release built without the chmod pass
# below ships an engine that cannot be launched. `zip` stores whatever mode the
# file has at the moment it is zipped, so restoring the bits here is enough — but
# it has to happen BEFORE any zip call, which is why `restore_exec_bits` runs
# first in `package_desktop`.

set -euo pipefail

ARTIFACTS_DIR="${1:?Usage: package-release.sh <artifacts-dir> <out-dir> <tag> <commit>}"
OUT_DIR="${2:?missing <out-dir>}"
TAG="${3:?missing <tag>}"
COMMIT="${4:-}"

mkdir -p "$OUT_DIR"
OUT_DIR=$(cd "$OUT_DIR" && pwd)

# The version is the tag with any `-nightly-<date>` suffix removed, so a nightly
# and its eventual release both report `r1-alpha7`.
VERSION="${TAG%%-nightly-*}"
BUILT_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)

# Platform dirs we know how to package, in the order they appear in the
# manifest. Anything else found under <artifacts-dir> is reported and skipped
# rather than silently dropped.
KNOWN_PLATFORMS=(
    windows-x64 windows-arm64
    linux-x64 linux-arm64
    macos-x64 macos-arm64
    web-wasm32
)

MANIFEST_ROWS=()

# ── Helper: is $1 in the remaining args? ─────────────────────────────────────
contains() {
    local needle="$1"; shift
    local x
    for x in "$@"; do [ "$x" = "$needle" ] && return 0; done
    return 1
}

# ── Helper: record an asset in the manifest ──────────────────────────────────
# Usage: record <asset-file> <platform> <kind>
record() {
    local file="$1" platform="$2" kind="$3"
    local name size sha
    name=$(basename "$file")
    size=$(stat -c %s "$file")
    sha=$(sha256sum "$file" | cut -d' ' -f1)
    MANIFEST_ROWS+=("$(printf '{"name":"%s","platform":"%s","kind":"%s","size":%s,"sha256":"%s"}' \
        "$name" "$platform" "$kind" "$size" "$sha")")
    printf '  %-40s %10s bytes  %s\n' "$name" "$size" "${sha:0:12}"
}

# ── Helper: restore the executable bit upload-artifact dropped ───────────────
# Only files that are actually launched — the two engine binaries wherever they
# sit, the AppImage and its AppRun. Shared libraries under plugins/ are dlopen'd,
# not executed, so they stay 0644.
restore_exec_bits() {
    local dir="$1" f
    while IFS= read -r -d '' f; do
        chmod +x "$f"
    done < <(find "$dir" \
        \( -name 'renzora' -o -name 'renzora-editor' -o -name 'renzora-runtime' \
           -o -name 'renzora-update' \
           -o -name 'AppRun' -o -name '*.AppImage' \) -type f -print0)
}

# ── Locate the runtime pieces inside one platform tree ───────────────────────
# Echoes the directory that directly contains `renzora[.exe]` (and, beside it,
# `plugins/` plus any sibling shared libraries). Empty output = no runtime here.
runtime_root() {
    local dir="$1"
    if [ -f "$dir/renzora.exe" ] || [ -f "$dir/renzora" ]; then
        echo "$dir"; return 0
    fi
    local b
    for b in "$dir"/*.AppDir; do
        [ -f "$b/renzora" ] && { echo "$b"; return 0; }
    done
    for b in "$dir"/*.app; do
        [ -f "$b/Contents/MacOS/renzora" ] && { echo "$b/Contents/MacOS"; return 0; }
    done
    return 0
}

# ── Build the export template for one desktop platform ───────────────────────
# The template is the GAME, not the engine: `renzora[.exe]`, its plugins, the
# shared libraries beside it, and (Windows) the OpenXR loader a `--vr` game
# needs. `renzora-editor` is deliberately excluded — shipping it would double the
# download and hand every exported game an editor it will never load.
package_runtime_template() {
    local platform="$1" dir="$2"
    local src; src=$(runtime_root "$dir")
    if [ -z "$src" ]; then
        echo "WARN: no runtime binary found under $dir — no export template for $platform"
        return 0
    fi

    local stage; stage=$(mktemp -d)
    local f
    for f in "$src/renzora" "$src/renzora.exe" "$src/openxr_loader.dll"; do
        [ -f "$f" ] && cp -p "$f" "$stage/"
    done
    # Sibling shared libraries (libstd, and any dylib a warm cargo cache left
    # beside the exe). Skip the editor's own, which never ships with a game.
    for f in "$src"/*.so "$src"/*.dylib "$src"/*.dll; do
        [ -f "$f" ] || continue
        case "$(basename "$f")" in
            *renzora_editor*) continue ;;
            openxr_loader.dll) continue ;;  # already copied above
        esac
        cp -p "$f" "$stage/"
    done
    if [ -d "$src/plugins" ]; then
        mkdir -p "$stage/plugins"
        find "$src/plugins" -maxdepth 1 -type f -exec cp -p {} "$stage/plugins/" \;
    fi

    if [ ! -f "$stage/renzora" ] && [ ! -f "$stage/renzora.exe" ]; then
        rm -rf "$stage"
        echo "WARN: staged no runtime binary for $platform"
        return 0
    fi

    local asset="$OUT_DIR/renzora-runtime-$platform.zip"
    rm -f "$asset"
    ( cd "$stage" && zip -qry "$asset" . )
    rm -rf "$stage"
    record "$asset" "$platform" runtime
}

# ── Package one desktop platform ─────────────────────────────────────────────
package_desktop() {
    local platform="$1" dir="$2"
    echo "── $platform ($dir)"
    restore_exec_bits "$dir"

    package_runtime_template "$platform" "$dir"
    compress_sdk "$dir"

    # The engine zip. On Linux the AppImage IS the distribution — it already
    # contains everything in the AppDir — so shipping both would double the
    # asset for no gain. The AppDir stays on disk either way because the runtime
    # template is extracted from it.
    local asset="$OUT_DIR/$platform.zip"
    rm -f "$asset"
    local appimage=""
    for f in "$dir"/*.AppImage; do [ -f "$f" ] && appimage="$f"; done
    if [ -n "$appimage" ]; then
        # The SDK cannot go inside the AppImage — that is built upstream by
        # build-all.sh — so it rides beside it in the zip, which is also where
        # the editor looks for it.
        ( cd "$dir" && zip -qry "$asset" "$(basename "$appimage")" \
            $( [ -f "$dir/sdk.tar.zst" ] && echo "sdk.tar.zst" ) )
    else
        ( cd "$dir" && zip -qry "$asset" . )
    fi
    record "$asset" "$platform" engine
}

# ── Compress the plugin SDK in place ─────────────────────────────────────────
# `cargo renzora` and `build-all.sh` stage the SDK EXTRACTED, because in a dev
# tree it is hardlinked to `target/` and costs neither disk nor time. Shipping it
# that way would put ~1.9 GB of loose crate metadata into the engine zip.
#
# So it is compressed to a single `sdk.tar.zst` (~444 MB) and the extracted tree
# removed. The editor unpacks it on demand — Rust scripts and native plugins
# both need it, so that is part of setting the engine up rather than an optional
# extra.
#
# ── zstd, not xz ─────────────────────────────────────────────────────────────
# xz is smaller (341 MB), and while the SDK was plugin-only that was the right
# trade. It stopped being right once scripting needed it too: the unpack cost is
# now paid by every user, and the download's is paid once. Measured on the real
# tree, zstd -19 costs +103 MB and turns a 29.8 s unpack into ~2 s — and because
# its decoder streams, it also removes the ~1.9 GB temporary tarball the xz path
# had to write and read back. `crates/renzora_plugin_build/src/unpack.rs` records
# the full numbers, including why switching to C liblzma is NOT a speed-up.
#
# -19 rather than a lower level: measured 444 MB against 520 MB at -10, for 0.5 s
# more decode. --long=27 widens the match window past zstd's default 8 MB, which
# matters on a tree this repetitive.
#
# Bundling rather than downloading on demand is deliberate. It removes an entire
# subsystem — hosting, a URL, progress, resume, checksums, offline handling — and
# makes a version mismatch structurally impossible: the SDK in the folder is by
# construction the one that built the editor beside it.
#
# Runs AFTER `package_runtime_template`, which reads the same directory and must
# not see the tree disappear underneath it.
compress_sdk() {
    local dir="$1"
    [ -d "$dir/sdk" ] || return 0
    echo "   compressing sdk/ …"
    # -T0 uses every core. Compression is the slowest part of packaging, and it
    # only ever runs here — the decoder is single-threaded and does not care.
    ( cd "$dir" && tar -cf - sdk | zstd -19 --long=27 -T0 -q -o sdk.tar.zst -f ) || {
        echo "ERROR: failed to compress $dir/sdk" >&2
        return 1
    }
    rm -rf "$dir/sdk"
    echo "   sdk.tar.zst $(du -h "$dir/sdk.tar.zst" | cut -f1)"
}

# ── Package the web bundle ───────────────────────────────────────────────────
# Two bundles live side by side in `web-wasm32/` (`renzora-runtime.*` and
# `renzora-editor.*`). The engine asset is both; the export template is the
# runtime pair only, which is exactly what `renzora_export::overlay::export_web`
# opens — it reads `renzora-runtime.js` + the module out of this zip and adds the
# project's rpak.
package_web() {
    local dir="$1"
    echo "── web-wasm32 ($dir)"
    local asset="$OUT_DIR/web-wasm32.zip"
    rm -f "$asset"
    ( cd "$dir" && zip -qry "$asset" . )
    record "$asset" web-wasm32 engine

    local stage; stage=$(mktemp -d)
    local f found=0
    for f in "$dir"/renzora-runtime*; do
        [ -f "$f" ] && { cp -p "$f" "$stage/"; found=1; }
    done
    if [ "$found" = "1" ]; then
        local rasset="$OUT_DIR/renzora-runtime-web-wasm32.zip"
        rm -f "$rasset"
        ( cd "$stage" && zip -qry "$rasset" . )
        record "$rasset" web-wasm32 runtime
    else
        echo "WARN: no renzora-runtime.* in $dir — no web export template"
    fi
    rm -rf "$stage"
}

# =============================================================================
# Walk the artifacts
# =============================================================================

echo "=== Packaging $TAG (version $VERSION) ==="
echo "artifacts: $ARTIFACTS_DIR"
echo

FOUND=()
# Two levels: <artifacts-dir>/<artifact-name>/<platform-dir>. A build job that
# uploaded `dist/` gives exactly this shape.
for d in "$ARTIFACTS_DIR"/*/*/; do
    [ -d "$d" ] || continue
    platform=$(basename "$d")
    if ! contains "$platform" "${KNOWN_PLATFORMS[@]}"; then
        echo "SKIP: unrecognised platform dir '$platform' ($d)"
        continue
    fi
    if contains "$platform" "${FOUND[@]+"${FOUND[@]}"}"; then
        echo "SKIP: duplicate '$platform' ($d) — already packaged"
        continue
    fi
    FOUND+=("$platform")
    case "$platform" in
        web-wasm32) package_web "${d%/}" ;;
        *)          package_desktop "$platform" "${d%/}" ;;
    esac
done

if [ ${#FOUND[@]} -eq 0 ]; then
    echo "ERROR: no recognised platform directories under $ARTIFACTS_DIR" >&2
    exit 1
fi

# ── manifest.json ────────────────────────────────────────────────────────────
# The editor fetches this by its deterministic download URL, so it can resolve
# and checksum a template with ONE unauthenticated request — no GitHub API call,
# no 60-requests-per-hour rate limit to trip over on a shared network.
{
    printf '{\n'
    printf '  "tag": "%s",\n' "$TAG"
    printf '  "version": "%s",\n' "$VERSION"
    printf '  "commit": "%s",\n' "$COMMIT"
    printf '  "built_at": "%s",\n' "$BUILT_AT"
    printf '  "assets": [\n'
    for i in "${!MANIFEST_ROWS[@]}"; do
        printf '    %s' "${MANIFEST_ROWS[$i]}"
        [ "$i" -lt $(( ${#MANIFEST_ROWS[@]} - 1 )) ] && printf ','
        printf '\n'
    done
    printf '  ]\n'
    printf '}\n'
} > "$OUT_DIR/manifest.json"

( cd "$OUT_DIR" && sha256sum ./*.zip > SHA256SUMS )

echo
echo "=== Packaged ${#FOUND[@]} platform(s): ${FOUND[*]} ==="
ls -la "$OUT_DIR"

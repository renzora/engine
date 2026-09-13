#!/usr/bin/env bash
# =============================================================================
# Turn the Build Engine workflow's per-platform artifacts into release assets
# =============================================================================
#
# Usage: scripts/package-release.sh <artifacts-dir> <out-dir> <tag> <commit>
#        scripts/package-release.sh --template-only <platform> <dist-dir>
#
# The second form is for a BUILD lane, and it exists because two platforms stage
# their output twice. A `.dmg` *is* the `Renzora Engine.app` inside it, and an
# `.AppImage` *is* the `Renzora Engine.AppDir` it was squashed from, so a lane
# that uploaded its `dist/` whole shipped every macOS and Linux byte to the
# publish job twice: the macos-arm64 artifact was 1.16 GB against a 585 MB disk
# image. The bundle cannot simply be deleted before the upload, because this
# script reads the export template OUT of it, and neither a `.dmg` nor a
# squashfs can be opened on the Linux runner that publishes.
#
# So the lane cuts the template itself, with this, and then deletes the
# duplicate. Same function, same file: the alternative was a second copy of
# "what belongs in an export template" living in the workflow, drifting from
# this one, and going wrong in the only place nobody looks (a macOS game
# exported months later missing a dylib).
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

# `--template-only` runs on the BUILD lane's own runner, which is macOS or
# Linux rather than the Ubuntu box that publishes. Everything it reaches must
# therefore work on both, which rules out the GNU-only `stat -c` and
# `sha256sum` in `record` (macOS has `stat -f %z` and `shasum -a 256`). Rather
# than write both dialects for a number nothing reads in this mode, `record`
# just prints. The manifest is the publish job's job.
MODE=package
if [ "${1:-}" = "--template-only" ]; then
    MODE=template
    shift
fi

if [ "$MODE" = template ]; then
    PLATFORM="${1:?Usage: package-release.sh --template-only <platform> <dist-dir>}"
    DIST_DIR="${2:?missing <dist-dir>}"
    # The template lands in the platform directory itself, so it rides to the
    # publish job inside the artifact the lane already uploads.
    DIST_DIR=$(cd "$DIST_DIR" && pwd)
    OUT_DIR="$DIST_DIR"
    ARTIFACTS_DIR=""
    TAG=""
    COMMIT=""
else
    ARTIFACTS_DIR="${1:?Usage: package-release.sh <artifacts-dir> <out-dir> <tag> <commit>}"
    OUT_DIR="${2:?missing <out-dir>}"
    TAG="${3:?missing <tag>}"
    COMMIT="${4:-}"
    DIST_DIR=""
fi

mkdir -p "$OUT_DIR"
OUT_DIR=$(cd "$OUT_DIR" && pwd)
# Absolute for the same reason `OUT_DIR` is, and it was missing for long enough
# to ship a release without it. Several helpers below `cd` into a platform
# directory before using paths derived from this one, and the workflow passes it
# relative (`artifacts`), so a relative value silently resolves against the wrong
# base. That cost r1-alpha7's Linux editors their plugin SDK; see the note in
# `package_desktop`. Normalising here means no caller has to remember.
if [ -n "$ARTIFACTS_DIR" ]; then
    ARTIFACTS_DIR=$(cd "$ARTIFACTS_DIR" && pwd)
fi

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
    # A build lane has no manifest to add to, and `stat -c` / `sha256sum` are
    # GNU spellings a macOS runner does not have. Say what was cut and stop.
    if [ "$MODE" = template ]; then
        printf '  %-40s %s\n' "$name" "$(du -h "$file" | cut -f1)"
        return 0
    fi
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

# ── Helper: the single-file form of a platform's bundle, if there is one ─────
# `<dir>/<something>.dmg` on macOS, `<dir>/<something>.AppImage` on Linux. Empty
# output on Windows, which has no bundle at all. A `for` loop over the glob
# rather than `ls`: an unmatched glob is left literal, and `[ -f ]` is what
# rejects it, whereas `ls` would print an error and take `set -e` with it.
bundle_image() {
    local dir="$1" f
    for f in "$dir"/*.dmg "$dir"/*.AppImage; do
        [ -f "$f" ] && { echo "$f"; return 0; }
    done
    return 0
}

# ── Helper: the directory form of the same bundle ────────────────────────────
bundle_tree() {
    local dir="$1" d
    for d in "$dir"/*.app "$dir"/*.AppDir; do
        [ -d "$d" ] && { echo "$d"; return 0; }
    done
    return 0
}

# ── Helper: did the build lane delete the bundle after imaging it? ───────────
# True when there is a `.dmg`/`.AppImage` and no `.app`/`.AppDir` beside it. The
# image carries the same bytes, so keeping both doubled a ~585 MB artifact; the
# lane drops the directory form and this is how packaging knows the binaries it
# would otherwise read are no longer reachable.
stripped_bundle() {
    local dir="$1"
    [ -n "$(bundle_image "$dir")" ] && [ -z "$(bundle_tree "$dir")" ]
}

# ── Build the export template for one desktop platform ───────────────────────
# The template is the GAME, not the engine: `renzora[.exe]`, its plugins, the
# shared libraries beside it, and (Windows) the OpenXR loader a `--vr` game
# needs. `renzora-editor` is deliberately excluded — shipping it would double the
# download and hand every exported game an editor it will never load.
package_runtime_template() {
    local platform="$1" dir="$2"

    # Already cut by the build lane, with `--template-only`, just before it
    # deleted the bundle this would otherwise have been read out of. Moved
    # rather than copied: the engine zip below is the whole directory, and a
    # template zip left inside it would ship in both assets.
    local pre="$dir/renzora-runtime-$platform.zip"
    if [ "$MODE" != template ] && [ -f "$pre" ]; then
        local asset="$OUT_DIR/renzora-runtime-$platform.zip"
        rm -f "$asset"
        mv "$pre" "$asset"
        echo "   template cut by the build lane"
        record "$asset" "$platform" runtime
        return 0
    fi

    local src; src=$(runtime_root "$dir")
    if [ -z "$src" ]; then
        # A tree whose bundle was stripped MUST arrive with its template, and
        # the two cases are worth telling apart: an ordinary tree with no
        # runtime is odd but survivable, while a stripped one with no template
        # means the lane deleted the only copy of the binaries there was. That
        # is unrecoverable here and silently ships a platform with no export
        # template, which is exactly the class of quiet omission that cost
        # r1-alpha7's Linux editors their SDK.
        if stripped_bundle "$dir"; then
            echo "ERROR: $dir has a .dmg or .AppImage, no bundle beside it, and no" >&2
            echo "       renzora-runtime-$platform.zip. The build lane deleted the bundle" >&2
            echo "       without cutting the export template first (--template-only)." >&2
            return 1
        fi
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

    # The engine zip: the whole staged tree, on every platform.
    #
    # Linux is the only one where "the tree" and "the bundle" are two copies of
    # the same bytes. `xtask`'s `--bundle` moves the binaries and shared
    # libraries into `Renzora Engine.AppDir/`, then squashes that into an
    # `.AppImage` beside it, so shipping both would put ~128 MB in twice.
    #
    # The build lane now deletes the AppDir once it has cut the export template
    # out of it, so by the time this runs there is usually nothing to exclude.
    # The exclusion stays because it still has a case to cover: appimagetool is
    # optional (see `xtask/src/bundle.rs`), and a lane that could not build an
    # AppImage keeps its AppDir, which is then the only copy of the binaries
    # there is.
    #
    # Everything ELSE in the tree ships, `sdk.tar.zst` above all. The editor
    # unpacks that on first launch and cannot run a Rust script or a native
    # plugin without it, and it can only ride beside the bundle: an AppImage is
    # a read-only squashfs, and `renzora_native_build::install::root()` resolves
    # `$APPIMAGE` to the directory the user unzipped for exactly this reason.
    #
    # ── Why this is a whole-directory zip with an exclusion ──────────────────
    # It used to name the two files to include, and shipped r1-alpha7's Linux
    # editors with no SDK at all. The test was written `[ -f "$dir/sdk.tar.zst" ]`
    # INSIDE a subshell that had already `cd "$dir"`, and `$dir` is relative
    # (the workflow passes `artifacts`), so it resolved to `$dir/$dir/...`, found
    # nothing, and `echo`'d an empty string. `zip` was simply handed one fewer
    # argument: no error, no warning, a 127 MB asset where macOS shipped 573 MB.
    #
    # Listing what to include is what made that silent. A whole-directory zip
    # cannot omit a file nobody remembered to name, so anything added to the
    # staged tree later ships without editing this line.
    # ── macOS ships the disk image, and only the disk image ─────────────────
    # `.dmg` is not an installer, it is a container — the `.app` plus a symlink
    # to `/Applications`, so a person drags it where it belongs instead of
    # running the editor out of `~/Downloads`. It is also what the updater
    # consumes: `renzora_update` mounts it and copies the app out, exactly as it
    # used to unzip one.
    #
    # Built, signed, notarized and stapled on the macOS runner (see `Build the
    # macOS disk image` in `.github/workflows/build-engine.yml`) because none of
    # those four things can happen on the Linux runner this script runs on.
    # Here it is only carried across into the release assets.
    #
    # No `macos-*.zip` beside it. Shipping both would double a ~600 MB asset to
    # serve nobody: the only consumer of the zip was the updater, and it reads
    # the DMG now. r1-alpha7 shipped no macOS build at all and every nightly
    # since failed Gatekeeper, so there is no installed base still asking for
    # the old asset name — this is the one moment the swap is free.
    #
    # Keyed on the `.dmg`, not on the `.app`: the lane deletes the bundle once
    # the image carries it, so by the time this runs there is usually no `.app`
    # left to match on. A `.app` with no `.dmg` beside it is still an error, and
    # a louder one than it looks, since the image cannot be built or signed on
    # this runner.
    local dmg=""
    for f in "$dir"/*.dmg; do [ -f "$f" ] && dmg="$f"; done
    if [ -n "$dmg" ] || ls "$dir"/*.app >/dev/null 2>&1; then
        if [ -z "$dmg" ]; then
            echo "ERROR: $dir holds a .app but no .dmg." >&2
            echo "       The macOS lane must build one — it cannot be created or signed here." >&2
            return 1
        fi
        local asset="$OUT_DIR/$platform.dmg"
        rm -f "$asset"
        cp -p "$dmg" "$asset"
        echo "   $(basename "$asset") $(du -h "$asset" | cut -f1)"
        record "$asset" "$platform" engine
        return 0
    fi

    local asset="$OUT_DIR/$platform.zip"
    rm -f "$asset"
    local appimage=""
    for f in "$dir"/*.AppImage; do [ -f "$f" ] && appimage="$f"; done
    if [ -n "$appimage" ]; then
        ( cd "$dir" && zip -qry "$asset" . -x "*.AppDir/*" "*.AppDir/" )
    else
        # No AppImage: appimagetool is optional (see `xtask/src/bundle.rs`), and
        # without it the AppDir is the only copy of the binaries there is. Ship
        # the tree whole.
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

    # ── macOS: the archive belongs INSIDE the bundle, and only the mac runner
    # can put it there ───────────────────────────────────────────────────────
    # `renzora_native_build::install::root()` is `current_exe().parent()` on
    # macOS — `Renzora Engine.app/Contents/MacOS/` — with no `$APPIMAGE` escape
    # hatch to redirect it the way Linux has. An archive beside the `.app` is
    # somewhere the editor never looks, which is how every macOS release shipped
    # an SDK the editor reported as `Absent` and never unpacked.
    #
    # This job cannot fix that here: it runs on Linux, and writing into a signed
    # `.app` means re-sealing it, which needs `codesign`. So the mac lane packs
    # its own SDK before signing (see `Pack the plugin SDK` in
    # `.github/workflows/build-engine.yml`) and the only job left here is to
    # notice when that did not happen, loudly. A silent fallback is exactly what
    # produced the 127 MB Linux asset described above.
    # A macOS tree whose `.app` was deleted after the disk image was built. The
    # SDK is inside that image, and the lane checked it was there before
    # deleting anything, which is the better place for the check anyway since it
    # is also the place that packs it. All that is left here is to catch a stray
    # copy at the top of the tree, which on macOS is somewhere the editor never
    # looks.
    #
    # Linux is deliberately NOT part of this. Its SDK ships BESIDE the AppImage
    # and must: a squashfs is read-only, so `install::root()` follows `$APPIMAGE`
    # to the directory the user unzipped. A stripped Linux tree therefore falls
    # through to the ordinary top-level handling below, which is already right.
    local dmg; dmg=$(bundle_image "$dir")
    case "$dmg" in
        *.dmg)
            if [ -z "$(bundle_tree "$dir")" ]; then
                if [ -e "$dir/sdk" ] || [ -e "$dir/sdk.tar.zst" ]; then
                    echo "ERROR: $dir has an SDK beside the disk image." >&2
                    echo "       The editor only ever looks inside the .app, so this copy is" >&2
                    echo "       invisible to it; remove it in the build lane." >&2
                    return 1
                fi
                return 0
            fi
            ;;
    esac

    local app; app=$(find "$dir" -maxdepth 1 -name '*.app' -type d | head -1)
    if [ -n "$app" ]; then
        if [ -f "$app/Contents/MacOS/sdk.tar.zst" ]; then
            echo "   sdk.tar.zst $(du -h "$app/Contents/MacOS/sdk.tar.zst" | cut -f1) (inside the bundle, packed by the build lane)"
        elif [ -d "$app/Contents/MacOS/sdk" ]; then
            echo "   sdk/ $(du -sh "$app/Contents/MacOS/sdk" | cut -f1) (extracted, inside the bundle)"
        else
            echo "ERROR: $app carries no SDK." >&2
            echo "       The macOS build lane must write sdk.tar.zst into Contents/MacOS/ before signing;" >&2
            echo "       it cannot be added here without invalidating the bundle signature." >&2
            return 1
        fi
        # A leftover at the top of the platform directory is the old, broken
        # layout. Shipping both would double a 457 MB asset to hide a bug.
        if [ -e "$dir/sdk" ] || [ -e "$dir/sdk.tar.zst" ]; then
            echo "ERROR: $dir has an SDK beside the bundle as well as inside it." >&2
            echo "       The copy beside the .app is invisible to the editor; remove it in the build lane." >&2
            return 1
        fi
        return 0
    fi

    # Already packed by the build lane (`pack_sdk` in docker/build-all.sh), which
    # is where it should happen — the tree is ~1.9 GB and compressing it here
    # means every artifact was uploaded and downloaded extracted first. This stays
    # as the fallback for a lane that had no zstd, and for `windows-arm64`, which
    # builds through xtask outside any container.
    if [ -f "$dir/sdk.tar.zst" ]; then
        rm -rf "$dir/sdk"
        echo "   sdk.tar.zst $(du -h "$dir/sdk.tar.zst" | cut -f1) (packed by the build lane)"
        return 0
    fi
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
# `--template-only`: cut one platform's export template and stop
# =============================================================================
# The build lane's half of the job. Everything below this point belongs to the
# publish job, which is a different runner on a different operating system.
if [ "$MODE" = template ]; then
    echo "=== Cutting the $PLATFORM export template ==="
    # A no-op on a native runner, where the modes are whatever the compiler left
    # and nothing has passed through `upload-artifact` yet. Run anyway, because
    # `zip` stores the mode it finds and this is the last moment anything can
    # influence what the template carries: once it is a zip inside an artifact,
    # the pass at the top of `package_desktop` cannot reach into it.
    restore_exec_bits "$DIST_DIR"
    package_runtime_template "$PLATFORM" "$DIST_DIR"
    if [ ! -f "$DIST_DIR/renzora-runtime-$PLATFORM.zip" ]; then
        echo "ERROR: cut no export template for $PLATFORM from $DIST_DIR." >&2
        echo "       Nothing may delete the bundle until this succeeds: the image" >&2
        echo "       cannot be read on the runner that publishes." >&2
        exit 1
    fi
    exit 0
fi

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

# ── engine-source.zip ────────────────────────────────────────────────────────
# The engine source, published as one more release asset.
#
# A lean single-binary export RECOMPILES the engine, so it needs the source — and
# a canonical editor download ships binaries only. Without this, lean builds are
# a contributors-only feature and everyone else gets "run the editor from a
# source checkout", which is not something a game developer can act on. The
# editor fetches this into `~/.renzora/src/<version>/` exactly as it fetches a
# runtime template into `~/.renzora/templates/<version>/<platform>/`.
#
# `git archive` rather than a `zip` of the working tree: it takes what is
# COMMITTED at this tag, so a dirty tree on the packaging runner cannot leak
# local edits into a published archive, and `.gitignore`d output (`target/`,
# `dist/`, `node_modules/`) is excluded by construction rather than by a list
# that would drift.
#
# Skipped rather than fatal when this is not a git checkout — the platform
# assets are still valid, and lean builds simply keep needing a checkout.
# Resolved from this script's own location rather than the working directory,
# which the caller sets to wherever the artifacts are.
REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
if git -C "$REPO_ROOT" rev-parse --git-dir >/dev/null 2>&1; then
    echo "── engine source"
    src_asset="$OUT_DIR/engine-source.zip"
    # Trimmed to what a lean build actually compiles. Measured on r1-alpha7 the
    # full tree is 52 MB zipped and this is 22.5 MB, and the difference is all
    # things the compiler never reads:
    #
    #   templates/         20.4 MB  project scaffolding for `renzora new`
    #   assets/previews/    6.6 MB  asset-browser thumbnails, editor-only
    #   docs/               3.2 MB
    #   tools/              2.1 MB  the updater — its own workspace, not built here
    #   .github/            0.1 MB
    #
    # What deliberately STAYS, because cutting it would break the build or the
    # binary it produces:
    #   crates/ src/ Cargo.* build.rs rust-toolchain.toml .cargo/  the build itself
    #   docker/            the Dockerfiles a cross build hashes for its image tag
    #   plugins/           read by `stage_static_plugins` when linking plugins in
    #   assets/particles|images|materials|ui   `include_str!`d into the binary
    #   assets/shaders|fonts|themes            loaded by PATH at run time
    #   languages/                             loaded from disk at run time
    #
    # Verify with `unzip -l` after changing this list: a missing compile input
    # fails loudly, but a missing RUNTIME asset only shows up in the exported
    # game, long after anyone would connect it to this line.
    if git -C "$REPO_ROOT" archive --format=zip -o "$src_asset" HEAD -- . \
        ':(exclude)templates' \
        ':(exclude)docs' \
        ':(exclude)tools' \
        ':(exclude).github' \
        ':(exclude)assets/previews'; then
        record "$src_asset" all source
    else
        echo "WARN: git archive failed — publishing without the engine source"
        rm -f "$src_asset"
    fi
else
    echo "WARN: not a git checkout — publishing without the engine source"
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

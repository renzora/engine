#!/bin/bash
# Start the staged engine on the machine that built it, and fail the lane if it
# does not stay up. Run by every desktop lane of build-engine.yml right after
# `cargo renzora dist --bundle`.
#
# Every audit before this one reads the build: symbols, imports, signatures. This
# runs it, which is the only check that catches what a user actually hits first:
# the loader refusing the binary (a glibc or a DLL it cannot find), a shared
# image that will not link (`bevy_dylib`, `renzora_dylib`), a code signature
# macOS will not accept, or a panic while the engine assembles itself. Any of
# those used to reach a release and be found by whoever downloaded it.
#
# Two runs:
#
#   server  `renzora --server` on every platform. Headless: no window and no GPU,
#           which is what a CI runner has, yet it loads every library, builds the
#           full engine plugin set and runs the main loop.
#   editor  Linux only: the editor itself, started through the AppImage's
#           AppRun (so the Linux support check runs too), on a virtual X display
#           with Mesa's software Vulkan driver. Windows and macOS runners have no
#           GPU a renderer can use, so there the server run is the whole test.
#
# A run passes when the process is still alive after its time and has logged no
# panic. Exiting early, with any code, fails: neither mode ever exits by itself.
#
# Usage: smoke-test.sh <dist_dir>    (e.g. linux-x64, windows-arm64)
set -uo pipefail

dist_dir="$1"
out="dist/$dist_dir"
logs="${RUNNER_TEMP:-/tmp}/smoke"
mkdir -p "$logs"
failed=0

case "$dist_dir" in
    linux-*)   root="$out/Renzora Engine.AppDir"; bin="$root/renzora" ;;
    macos-*)   root="$out/Renzora Engine.app/Contents/MacOS"; bin="$root/renzora" ;;
    windows-*) root="$out"; bin="$root/renzora.exe" ;;
    *) echo "::error::smoke-test: unknown platform $dist_dir"; exit 1 ;;
esac
if [ ! -f "$bin" ]; then
    echo "::error::smoke-test: no engine binary at $bin"
    exit 1
fi

# Run "$@" for $1 seconds with output in $2. Returns 0 if it was still running at
# the end and logged no panic; otherwise prints the log and returns 1.
# A polling loop rather than `timeout`, which macOS does not ship.
run_for() {
    local seconds="$1" log="$2" label="$3"
    shift 3
    echo "::group::$label"
    "$@" > "$log" 2>&1 &
    local pid=$! waited=0 code=""
    while [ "$waited" -lt "$seconds" ]; do
        if ! kill -0 "$pid" 2>/dev/null; then
            wait "$pid"; code=$?
            break
        fi
        sleep 1
        waited=$((waited + 1))
    done
    if [ -z "$code" ]; then
        kill "$pid" 2>/dev/null
        sleep 2
        kill -9 "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
    fi
    tail -n 80 "$log"
    echo "::endgroup::"
    if [ -n "$code" ]; then
        echo "::error::$label exited after ${waited}s with code $code instead of running; its output is above"
        return 1
    fi
    if grep -q "panicked at" "$log"; then
        echo "::error::$label panicked; its output is above"
        return 1
    fi
    echo "$label ran for ${seconds}s"
    return 0
}

# ── Server: every platform ───────────────────────────────────────────────────
# An unusual port, so a runner image that happens to run something on the
# default one cannot fail the lane.
run_for 30 "$logs/server.log" "renzora --server ($dist_dir)" \
    "$bin" --server --port 47123 || failed=1

# ── Editor: Linux ────────────────────────────────────────────────────────────
if [[ "$dist_dir" == linux-* ]]; then
    icd=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | sed -n 1p)
    if ! command -v Xvfb >/dev/null 2>&1 || [ -z "$icd" ]; then
        echo "::warning::no Xvfb or Mesa software Vulkan (lavapipe) on this runner; the editor was not started, only the server"
    else
        Xvfb :99 -screen 0 1600x900x24 -nolisten tcp > "$logs/xvfb.log" 2>&1 &
        xvfb=$!
        sleep 2
        # Software Vulkan only, so a runner that ever gains a GPU driver does
        # not change what this measures. Wayland is unset so winit takes the X
        # display that exists. Through `env` rather than as a prefix on
        # `run_for`, because a prefix on a shell function is not reliably
        # passed on to what the function starts.
        run_for 60 "$logs/editor.log" "editor via AppRun ($dist_dir)" \
            env -u WAYLAND_DISPLAY DISPLAY=:99 VK_ICD_FILENAMES="$icd" \
            "$root/AppRun" || failed=1
        kill "$xvfb" 2>/dev/null
    fi
fi

if [ "$failed" -ne 0 ]; then
    echo "::error::the $dist_dir build does not start; see the groups above"
    exit 1
fi
echo "the $dist_dir build starts"

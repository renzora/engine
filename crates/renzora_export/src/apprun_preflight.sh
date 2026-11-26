# Linux support preflight, spliced into every AppImage's AppRun (the editor's in
# xtask/src/bundle.rs, an exported game's in renzora_export/src/bundle.rs).
#
# It runs BEFORE the dynamic loader opens the binary, which is the whole reason
# it is a shell script: a glibc that is too old, a musl distro, NixOS without
# nix-ld, or a missing linked library all make the loader refuse the executable
# before `main`, so nothing in Rust can ever report them. Launched from a file
# manager, the loader's own one-line error goes nowhere anyone reads.
#
# The checks a running process can do better (display server, Vulkan, the
# libraries winit dlopens) live in renzora_runtime/src/linux_support.rs.
#
# Expects APP_NAME, HERE and BIN to be set by the AppRun around it.
# RENZORA_SKIP_PREFLIGHT=1 turns it off.
#
# RENZORA_GLIBC_MIN is the floor the release is built for: the glibc of the
# AlmaLinux 8 container the Linux lanes build in (.github/scripts/el8-toolchain.sh).
# Change it only together with that container and the audit in build-engine.yml.
RENZORA_GLIBC_MIN="2.28"

renzora_unsupported() {
    title="$APP_NAME cannot start on this system"
    printf '%s\n\n%s\n' "$title" "$1" >&2
    if [ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]; then
        if command -v zenity >/dev/null 2>&1; then
            zenity --error --no-markup --title="$title" --text="$1" 2>/dev/null
        elif command -v kdialog >/dev/null 2>&1; then
            kdialog --title "$title" --error "$1" 2>/dev/null
        elif command -v notify-send >/dev/null 2>&1; then
            notify-send --urgency=critical "$title" "$1" 2>/dev/null
        elif command -v xmessage >/dev/null 2>&1; then
            xmessage -center "$title

$1" 2>/dev/null
        fi
    fi
    exit 1
}

renzora_is_nixos() {
    [ -e /etc/NIXOS ] || grep -qx 'ID=nixos' /etc/os-release 2>/dev/null
}

if [ "${RENZORA_SKIP_PREFLIGHT:-}" != "1" ]; then
    case "$(uname -m)" in
        x86_64)  RENZORA_LOADER=/lib64/ld-linux-x86-64.so.2 ;;
        aarch64) RENZORA_LOADER=/lib/ld-linux-aarch64.so.1 ;;
        *)       renzora_unsupported "This build is for x86_64 and aarch64 (ARM64) Linux. This machine is $(uname -m)." ;;
    esac

    # No glibc loader at the standard path: NixOS without nix-ld, or a musl
    # distro. Either way the kernel cannot start the binary at all.
    if [ ! -e "$RENZORA_LOADER" ]; then
        if renzora_is_nixos; then
            renzora_unsupported "NixOS does not run prebuilt Linux programs on its own.

Either enable nix-ld in configuration.nix:

    programs.nix-ld.enable = true;
    programs.nix-ld.libraries = with pkgs; [ vulkan-loader wayland libxkbcommon xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr alsa-lib udev fontconfig ];

then rebuild and log in again, or start this AppImage with appimage-run."
        fi
        renzora_unsupported "This system does not use the GNU C library (glibc), which this build needs: $RENZORA_LOADER does not exist. Distributions built on musl (Alpine, Void musl, Chimera) are not supported. Running it inside a glibc container with Distrobox works."
    fi

    # Too old a glibc: the loader would refuse with "version GLIBC_2.xx not found".
    renzora_glibc="$(getconf GNU_LIBC_VERSION 2>/dev/null | sed -n 's/^glibc //p')"
    if [ -n "$renzora_glibc" ]; then
        have_major="${renzora_glibc%%.*}"; have_minor="${renzora_glibc#*.}"; have_minor="${have_minor%%.*}"
        need_major="${RENZORA_GLIBC_MIN%%.*}"; need_minor="${RENZORA_GLIBC_MIN#*.}"
        if [ "$have_major" -lt "$need_major" ] 2>/dev/null \
           || { [ "$have_major" -eq "$need_major" ] && [ "$have_minor" -lt "$need_minor" ]; } 2>/dev/null; then
            # The distro list is only true at the release floor. A lean game
            # compiled on a newer machine gets its own floor written in above
            # (renzora_export's bundle step), and the plain sentence then.
            renzora_supported=""
            if [ "$RENZORA_GLIBC_MIN" = "2.28" ]; then
                renzora_supported="

Supported: Ubuntu 20.04 or newer, Linux Mint 20 or newer, Debian 10 or newer, Fedora 29 or newer, RHEL, Rocky Linux and AlmaLinux 8 or newer, Pop!_OS 20.04 or newer, and current Arch, openSUSE Tumbleweed and NixOS."
            fi
            renzora_unsupported "This build needs glibc $RENZORA_GLIBC_MIN or newer, and this system has glibc $renzora_glibc.$renzora_supported"
        fi
    fi

    # Libraries the binary links directly (ALSA, udev, fontconfig, ...). The
    # loader resolves them without running anything, so asking it is exact and
    # follows LD_LIBRARY_PATH the same way the real start will.
    if [ -x "$HERE/$BIN" ]; then
        renzora_missing="$("$RENZORA_LOADER" --list "$HERE/$BIN" 2>/dev/null | sed -n 's/^[[:space:]]*\([^ ]*\) => not found.*/\1/p' | sort -u | tr '\n' ' ')"
        if [ -n "$renzora_missing" ]; then
            if renzora_is_nixos; then
                renzora_unsupported "These libraries could not be found: $renzora_missing

Add the packages that provide them to programs.nix-ld.libraries, or start this AppImage with appimage-run."
            fi
            renzora_unsupported "These libraries could not be found: $renzora_missing

Install them with your package manager. On Debian, Ubuntu and Mint they come from libasound2, libudev1 and libfontconfig1; on Fedora from alsa-lib, systemd-libs and fontconfig; on Arch from alsa-lib, systemd-libs and fontconfig."
        fi
    fi
fi

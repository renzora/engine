#!/bin/bash
# The build toolchain for the Linux release lanes, inside an `almalinux:8`
# container. Shared by build-engine.yml (`desktop`) and third-party-plugins.yml
# (`sdk`, `build`), so a plugin is compiled with exactly what the engine was.
#
# Why AlmaLinux 8: a Linux binary needs at least the glibc it was linked
# against, so the build system's glibc IS the oldest distro the release runs on.
# EL8's is 2.28, the floor of Ubuntu 20.04, Debian 10, Mint 20 and RHEL 8, and
# unlike those Debian-family releases EL8 is still maintained (until 2029) and
# ships current compilers built for exactly this job: gcc-toolset links what a
# newer GCC needs from libstdc++ statically, through `libstdc++_nonshared.a`,
# so the output still runs against EL8's own GCC 8 runtime. 2.28 is also the
# lowest glibc GitHub's own Node runtime starts on, so no lower floor is
# reachable in a container job anyway.
#
# Changing the floor means changing this image, `GLIBC_MAX` / `GLIBCXX_MAX` in
# build-engine.yml's audit and `RENZORA_GLIBC_MIN` in
# crates/renzora_export/src/apprun_preflight.sh together.
#
# Usage: el8-toolchain.sh [--gh]   (--gh also installs the GitHub CLI)
set -euo pipefail

GCC_TOOLSET=13
MOLD_VERSION=2.34.1

dnf install -y dnf-plugins-core
# PowerTools carries several of the -devel packages below.
dnf config-manager --set-enabled powertools
dnf install -y --setopt=install_weak_deps=False \
    sudo git curl xz file binutils findutils which tar gzip jq zip unzip zstd python3 \
    make pkgconf-pkg-config libatomic \
    "gcc-toolset-${GCC_TOOLSET}-gcc" "gcc-toolset-${GCC_TOOLSET}-gcc-c++" "gcc-toolset-${GCC_TOOLSET}-binutils" \
    clang clang-devel \
    libX11-devel libXi-devel libXcursor-devel libXrandr-devel libXinerama-devel \
    wayland-devel libxkbcommon-devel libxkbcommon-x11-devel \
    alsa-lib-devel systemd-devel vulkan-loader-devel openssl-devel fontconfig-devel

if [ "${1:-}" = "--gh" ]; then
    dnf config-manager --add-repo https://cli.github.com/packages/rpm/gh-cli.repo
    dnf install -y gh
fi

# Every C and C++ compile and every link goes through the toolset's GCC. Not just
# CC/CXX: `rustc` links with `cc` unless told otherwise, and the plugin
# compiler (renzora_plugin_build) invokes rustc directly, so a `cc` on PATH that
# is still EL8's GCC 8 would link C++ objects from GCC 13 against the old
# libstdc++ and leave symbols that only fail when the library is loaded.
# /usr/local/bin comes before /usr/bin on PATH.
toolset="/opt/rh/gcc-toolset-${GCC_TOOLSET}/root/usr/bin"
for pair in cc:gcc c++:g++ gcc:gcc g++:g++; do
    ln -sf "$toolset/${pair#*:}" "/usr/local/bin/${pair%%:*}"
done

# EL8 does not package mold, so this is the upstream release, built to run on
# old glibc. Ships `ld.mold` for `-fuse-ld=mold`. Not fully static: the x86_64
# build loads `libatomic.so.1`, which is why that package is in the list above
# (without it every link failed with "cannot open shared object file").
curl -fsSL "https://github.com/rui314/mold/releases/download/v${MOLD_VERSION}/mold-${MOLD_VERSION}-$(uname -m)-linux.tar.gz" \
    | tar -xz -C /usr/local --strip-components=1

# Both homes named outright. A container job runs as root with HOME set to
# /github/home, and rustup notices the mismatch with root's own home (/root):
# "$HOME differs from euid-obtained home directory". Naming the directories
# leaves no question of which one it installs into, and they are the ones PATH
# points at below.
export RUSTUP_HOME="$HOME/.rustup" CARGO_HOME="$HOME/.cargo"
curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal

if [ -n "${GITHUB_ENV:-}" ]; then
    echo "$CARGO_HOME/bin" >> "$GITHUB_PATH"
    {
        echo "RUSTUP_HOME=$RUSTUP_HOME"
        echo "CARGO_HOME=$CARGO_HOME"
        echo "CC=gcc"
        echo "CXX=g++"
        # The repo's .cargo/config.toml links x86_64 with `clang`, which on EL8
        # would pick GCC 8's runtime for the link (see above). An env linker
        # outranks every config file; the config's `-fuse-ld=mold` still
        # applies and GCC 13 understands it.
        echo "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc"
        echo "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=gcc"
        # bindgen (DLSS) finds libclang here.
        echo "LIBCLANG_PATH=/usr/lib64"
    } >> "$GITHUB_ENV"
fi

# `sed -n 1p`, not `head -n1`: head exits after one line, the tool writing the
# rest gets SIGPIPE, and under `pipefail` that failed the whole step with exit
# code 141 after everything had installed.
gcc --version | sed -n 1p
mold --version
ldd --version | sed -n 1p

# =============================================================================
# Renzora Engine Builder — Shared BASE image
# =============================================================================
#
# The common foundation every platform toolchain image builds `FROM`. It carries
# only the stable, rarely-changing essentials — the Rust compiler, the native
# Linux build/link deps, and the LLVM-19 layer shared by the osxcross (macOS/iOS)
# and xwin (Windows) cross toolchains. Everything platform-specific (osxcross,
# xwin, the NDK, wasm tooling, appimagetool, UPX, the cross-arch Linux sysroot)
# lives in the per-platform images so a change to one platform never cascades a
# rebuild through the others.
#
# Why a shared base at all: platform images share this layer on pull (download
# once for N platforms, stored once on disk), and a platform-only change touches
# only that platform's image. The one event that *does* cascade to every
# platform is a change to THIS file — overwhelmingly a Rust version bump, which
# legitimately must rebuild every toolchain (it also shifts the bevy_dylib ABI
# hash). So keep this file minimal and stable; put volatile shared tools in the
# platform images, not here.
#
# This is the SINGLE source of truth for the engine's Rust version — there is no
# rust-toolchain.toml. The base tag = first 12 hex of this file's SHA-256, and
# each platform tag folds in this base tag, so editing this file re-rolls every
# image tag (CI rebuilds all; the CLI re-pulls).
#
# Multi-arch: builds natively as linux/amd64 and linux/arm64 (Apple Silicon).
# Arch-specific steps key off `uname -m` (x86_64 / aarch64).
FROM rust:1.95.0-bookworm

# ── System dependencies ──────────────────────────────────────────────────────
# Native Linux build/link deps (the workspace links alsa + udev at build; X11/
# Wayland/Vulkan/xkbcommon are dlopen'd at runtime but their dev headers are
# cheap and keep `cargo check` honest), the linkers, and the few tools the
# platform layers reuse to fetch SDKs. `wget`/`gnupg`/`lsb-release`/
# `software-properties-common` are required by the LLVM apt script below.
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libx11-dev \
    libxi-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxinerama-dev \
    libwayland-dev \
    libxkbcommon-dev \
    libasound2-dev \
    libudev-dev \
    libvulkan-dev \
    libssl-dev \
    mold \
    clang \
    lld \
    curl \
    unzip \
    file \
    wget \
    gnupg \
    lsb-release \
    software-properties-common \
    && rm -rf /var/lib/apt/lists/*

# ── Rust targets ─────────────────────────────────────────────────────────────
# Both Linux GNU triples so the native one is present on either arch (adding the
# host triple is a no-op) and the cross one is ready for the linux image to wire
# a cross-gcc linker to. Platform-specific targets (windows-msvc, wasm, android,
# darwin, ios) are added by their own images.
RUN rustup target add \
    x86_64-unknown-linux-gnu \
    aarch64-unknown-linux-gnu

# ── LLVM 19 (shared by osxcross + xwin) ──────────────────────────────────────
# macOS SDK 26.x's libc++ requires Clang 18+; bookworm ships Clang 14. Clang 19
# also gives us clang-cl, lld-link, and llvm-lib for the Windows MSVC
# cross-compile. This is the one heavyweight layer the cross toolchains share, so
# it lives in the base to avoid building it three times (windows/macos/ios). It
# changes only on an LLVM bump.
RUN wget -qO /tmp/llvm.sh https://apt.llvm.org/llvm.sh && \
    chmod +x /tmp/llvm.sh && \
    /tmp/llvm.sh 19 && \
    apt-get install -y --no-install-recommends llvm-19-dev && \
    update-alternatives --install /usr/bin/clang clang /usr/bin/clang-19 100 && \
    update-alternatives --install /usr/bin/clang++ clang++ /usr/bin/clang++-19 100 && \
    update-alternatives --install /usr/bin/clang-cl clang-cl /usr/bin/clang-cl-19 100 && \
    update-alternatives --install /usr/bin/lld-link lld-link /usr/bin/lld-link-19 100 && \
    update-alternatives --install /usr/bin/llvm-lib llvm-lib /usr/bin/llvm-lib-19 100 && \
    update-alternatives --install /usr/bin/llvm-rc llvm-rc /usr/bin/llvm-rc-19 100 && \
    ln -sf /usr/bin/ld64.lld-19 /usr/local/bin/ld64.lld && \
    ln -sf /usr/bin/ld.lld-19 /usr/local/bin/ld.lld && \
    rm /tmp/llvm.sh && \
    rm -rf /var/lib/apt/lists/*

# ── Shared build env (sourced by bash before every RUN, and by build-all.sh) ──
# Each platform image APPENDS its own CC/CXX/AR + linker-search exports to this
# file (darwin, msvc, android, linux-cross). The base seeds it empty so the
# BASH_ENV source never fails and so a platform image can append unconditionally.
RUN : > /etc/osxcross-env.sh
ENV BASH_ENV=/etc/osxcross-env.sh
SHELL ["/bin/bash", "-c"]

# ── Cargo linker config ──────────────────────────────────────────────────────
# Only the NATIVE Linux section lives here (clang + mold + prefer-dynamic) — it's
# all the base needs for `cargo check`/`cargo test` and the native Linux build.
# Each platform image APPENDS its own [target.*] sections (the linker names
# depend on tools installed in that image, e.g. osxcross's darwin<ver>-clang or
# the linux image's cross-gcc), so they can't be written here.
WORKDIR /app
RUN if [ "$(uname -m)" = "aarch64" ]; then HOSTRUST=aarch64-unknown-linux-gnu; else HOSTRUST=x86_64-unknown-linux-gnu; fi && \
    mkdir -p .cargo && cat > .cargo/config.toml <<TOML
# Native Linux arch — clang + mold (fast incremental link). The cross Linux arch
# section (cross-gcc linker) is appended by docker/linux/Dockerfile.
[target.${HOSTRUST}]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=/usr/bin/mold", "-C", "prefer-dynamic"]
TOML

# The persistent container mounts the repo at /app/src; the image just provides
# the toolchain (no source baked in).
CMD ["sleep", "infinity"]

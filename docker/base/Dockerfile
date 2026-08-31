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
# This is the source of truth for the CONTAINER's Rust version. A second copy
# lives in `/rust-toolchain.toml` at the repo root, which pins the same version
# for NATIVE (no-Docker) `cargo renzora` builds — keep the two in lockstep; a
# bump must edit both. The base tag = first 12 hex of this file's SHA-256, and
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
    # Compressing the staged plugin SDK into `sdk.tar.zst` inside the build lane,
    # so the ~1.9 GB tree never travels as loose files. Without it the lane
    # uploads an artifact several times larger and the packaging job compresses
    # the same bytes again later.
    zstd \
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
#
# ── Why the apt source is written out instead of running `llvm.sh` ───────────
# This used to `wget https://apt.llvm.org/llvm.sh` and run it. That script is
# fetched UNPINNED at build time, so the image depended on whatever upstream
# happened to be serving that day — and upstream dropped Debian 12:
#
#     [error] Distribution 'debian' in version '12 (bookworm)' is not
#             supported by this script.
#
# The image had not actually re-run that layer in a long time (every rebuild hit
# the registry buildx cache), so the breakage sat invisible until an unrelated
# edit above invalidated the cache and the layer executed for real.
#
# All the script does is add this apt source and install the packages, so doing
# it directly removes the dependency on a moving remote script without changing
# what lands in the image. The distro codename is now explicit, which is also
# what has to change on a base-image bump.
RUN wget -qO- https://apt.llvm.org/llvm-snapshot.gpg.key \
      | gpg --dearmor -o /usr/share/keyrings/llvm-archive-keyring.gpg && \
    echo "deb [signed-by=/usr/share/keyrings/llvm-archive-keyring.gpg] http://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-19 main" \
      > /etc/apt/sources.list.d/llvm.list && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
      clang-19 \
      lld-19 \
      llvm-19-dev && \
    # Everything below points into LLVM's own install prefix rather than at
    # `/usr/bin/<tool>-19`. Those versioned symlinks are created by the packaging
    # for SOME tools and not others, and which ones depends on the package set —
    # `clang-cl-19` in particular does not exist under `--no-install-recommends`,
    # which is what broke this layer the first time it ran for real:
    #
    #     update-alternatives: error: alternative path /usr/bin/clang-cl-19
    #                                 doesn't exist
    #
    # `/usr/lib/llvm-19/bin/` is where LLVM installs every tool unconditionally,
    # so it does not depend on which convenience symlinks a given release ships.
    #
    # The `ls` is deliberate and stays: this layer is only exercised on a cache
    # miss, so when it does break it tends to be a long time later and for a
    # reason nobody can guess. Printing the inventory means the log answers "what
    # is actually in there" without another round trip.
    ls -1 /usr/lib/llvm-19/bin/ && \
    # `clang-cl` is the one tool the packages do not lay down. Confirmed from the
    # listing above: the prefix has clang, clang++, clang-cpp, lld, lld-link,
    # llvm-lib, llvm-rc, ld.lld, ld64.lld — and no clang-cl.
    #
    # It is not a separate program. clang picks its driver from `argv[0]`, so
    # `clang-cl` IS clang under another name, which is how upstream ships it and
    # why making the link here is not a workaround. The MSVC cross-build needs it
    # by that name (see docker/windows/Dockerfile).
    ln -sf /usr/lib/llvm-19/bin/clang /usr/lib/llvm-19/bin/clang-cl && \
    update-alternatives --install /usr/bin/clang clang /usr/lib/llvm-19/bin/clang 100 && \
    update-alternatives --install /usr/bin/clang++ clang++ /usr/lib/llvm-19/bin/clang++ 100 && \
    update-alternatives --install /usr/bin/clang-cl clang-cl /usr/lib/llvm-19/bin/clang-cl 100 && \
    update-alternatives --install /usr/bin/lld-link lld-link /usr/lib/llvm-19/bin/lld-link 100 && \
    update-alternatives --install /usr/bin/llvm-lib llvm-lib /usr/lib/llvm-19/bin/llvm-lib 100 && \
    update-alternatives --install /usr/bin/llvm-rc llvm-rc /usr/lib/llvm-19/bin/llvm-rc 100 && \
    ln -sf /usr/lib/llvm-19/bin/ld64.lld /usr/local/bin/ld64.lld && \
    ln -sf /usr/lib/llvm-19/bin/ld.lld /usr/local/bin/ld.lld && \
    rm -rf /var/lib/apt/lists/*

# ── Shared build env (sourced by bash before every RUN, and by build-all.sh) ──
# Each platform image APPENDS its own CC/CXX/AR + linker-search exports to this
# file (darwin, msvc, android, linux-cross). The base seeds it empty so the
# BASH_ENV source never fails and so a platform image can append unconditionally.
RUN : > /etc/osxcross-env.sh
ENV BASH_ENV=/etc/osxcross-env.sh
SHELL ["/bin/bash", "-c"]

# ── Cargo linker config ──────────────────────────────────────────────────────
# Only the NATIVE Linux section lives here (clang + mold) — it's
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
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
TOML

# ── UPX ──────────────────────────────────────────────────────────────────────
# The official static binary (not the apt `upx-ucl`, absent from this image's
# sources). UPX packs PE/ELF/Mach-O regardless of host OS.
#
# It lives in the BASE, not in the linux image, because every platform lane now
# packs its own output (`compress_binaries` in docker/build-all.sh) — the
# Windows and macOS lanes run in their own containers and cannot reach a tool
# installed only in the Linux one. `renzora upx`, which the CLI still routes to
# the linux container to pack an already-built tree, keeps working by
# inheritance.
ENV UPX_VERSION=5.1.1
RUN UPX_ARCH=$([ "$(uname -m)" = "aarch64" ] && echo arm64_linux || echo amd64_linux) \
    && curl -sSL "https://github.com/upx/upx/releases/download/v${UPX_VERSION}/upx-${UPX_VERSION}-${UPX_ARCH}.tar.xz" \
    -o /tmp/upx.tar.xz \
    && tar -xJf /tmp/upx.tar.xz -C /tmp \
    && mv /tmp/upx-${UPX_VERSION}-${UPX_ARCH}/upx /usr/local/bin/upx \
    && chmod +x /usr/local/bin/upx \
    && rm -rf /tmp/upx.tar.xz /tmp/upx-${UPX_VERSION}-${UPX_ARCH}

# The persistent container mounts the repo at /app/src; the image just provides
# the toolchain (no source baked in).
CMD ["sleep", "infinity"]

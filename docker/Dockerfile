# =============================================================================
# Renzora Engine Builder — Cross-Platform Toolchain
# =============================================================================
#
# Provides the build toolchain for all supported platforms. No source code
# baked in — the repo is mounted at runtime via a persistent container.
#
# Set up (build image + create + start container; idempotent):
#   renzora init
#
# Build all platforms / a subset:
#   renzora build
#   renzora build windows linux

# This is the SINGLE source of truth for the engine's Rust version — there is no
# rust-toolchain.toml. Everything builds in this image, so the compiler is
# whatever this base provides. Bumping the version = editing this line (which
# also changes the image's content hash, so the CLI auto-pulls the new image).
#
# Multi-arch: builds natively as linux/amd64 and linux/arm64 (Apple Silicon).
# Arch-specific steps key off `uname -m` (x86_64 / aarch64). One arm64 gap:
# Google ships no arm64-Linux NDK, so Android cross-compile is amd64-only
# (the android lane in build-all.sh is best-effort and degrades gracefully).
FROM rust:1.93.0-bookworm

# ── System dependencies ──────────────────────────────────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
    # Linux build deps
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
    # Linkers
    mold \
    clang \
    lld \
    # WASM post-processing
    binaryen \
    # General
    curl \
    unzip \
    file \
    desktop-file-utils \
    && rm -rf /var/lib/apt/lists/*

# ── AppImage tooling ─────────────────────────────────────────────────────────
# appimagetool release assets are named by `uname -m` (x86_64 / aarch64).
RUN curl -sL "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$(uname -m).AppImage" \
    -o /usr/local/bin/appimagetool && \
    chmod +x /usr/local/bin/appimagetool && \
    cd /usr/local/bin && ./appimagetool --appimage-extract >/dev/null && \
    mv squashfs-root /opt/appimagetool && \
    ln -sf /opt/appimagetool/AppRun /usr/local/bin/appimagetool

# ── All Rust targets ─────────────────────────────────────────────────────────
# Both Linux triples are listed so the native one is present on either arch
# (adding the host triple is a no-op). The non-host Linux triple has no cross
# sysroot in this image — the linux lane builds native-only (see build-all.sh).
RUN rustup target add \
    x86_64-unknown-linux-gnu \
    aarch64-unknown-linux-gnu \
    x86_64-pc-windows-msvc \
    wasm32-unknown-unknown \
    aarch64-linux-android \
    x86_64-linux-android

RUN rustup target add x86_64-apple-darwin aarch64-apple-darwin

# iOS (share the macOS SDK via osxcross)
RUN rustup target add aarch64-apple-ios aarch64-apple-ios-sim

# ── WASM tooling ─────────────────────────────────────────────────────────────
RUN cargo install wasm-bindgen-cli --version 0.2.108 --locked

# ── Android NDK (amd64 only) ─────────────────────────────────────────────────
# Google ships no arm64-Linux NDK host binaries; on aarch64 we skip the install
# and build-all.sh skips the android lane when $ANDROID_NDK_HOME is absent.
ENV ANDROID_NDK_VERSION=r27c
ENV ANDROID_NDK_HOME=/opt/android-ndk

RUN if [ "$(uname -m)" = "x86_64" ]; then \
        curl -sL https://dl.google.com/android/repository/android-ndk-${ANDROID_NDK_VERSION}-linux.zip \
            -o /tmp/ndk.zip && \
        unzip -q /tmp/ndk.zip -d /opt && \
        mv /opt/android-ndk-${ANDROID_NDK_VERSION} ${ANDROID_NDK_HOME} && \
        rm /tmp/ndk.zip; \
    else \
        echo "Skipping Android NDK: no arm64-Linux host binaries published by Google"; \
    fi

# ── osxcross for macOS / iOS cross-compilation ──────────────────────────────
ENV OSXCROSS_SDK=MacOSX26.1.sdk
ENV OSXCROSS_IOS_SDK=iPhoneOS15.5.sdk
ENV MACOSX_DEPLOYMENT_TARGET=11.0
ENV IPHONEOS_DEPLOYMENT_TARGET=14.0

RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake \
    libxml2-dev \
    patch \
    python3 \
    xz-utils \
    zlib1g-dev \
    gnupg \
    wget \
    lsb-release \
    software-properties-common \
    && rm -rf /var/lib/apt/lists/*

# SDK 26.x's libc++ requires Clang 18+; bookworm ships Clang 14.
# Clang 19 also gives us clang-cl, lld-link, and llvm-lib for MSVC cross-compile.
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

# ── xwin: MSVC SDK + CRT for Windows MSVC cross-compile ─────────────────────
# xwin downloads the redistributable Visual Studio Build Tools from Microsoft
# at image-build time. Microsoft permits this redistribution for development
# under the VS Build Tools EULA. Combined with rustc's built-in MSVC support,
# clang-cl, and lld-link, this gives a fully open Linux→Windows-MSVC pipeline.
ENV XWIN_VERSION=0.6.5
ENV XWIN_PREFIX=/xwin

# xwin's release triple matches `uname -m` (x86_64 / aarch64). The splat
# `--arch x86_64` below is the *Windows target* arch, independent of host.
RUN curl -sL "https://github.com/Jake-Shadle/xwin/releases/download/${XWIN_VERSION}/xwin-${XWIN_VERSION}-$(uname -m)-unknown-linux-musl.tar.gz" \
    | tar xzf - -C /tmp \
    && mv /tmp/xwin-${XWIN_VERSION}-$(uname -m)-unknown-linux-musl/xwin /usr/local/bin/ \
    && rm -rf /tmp/xwin-* \
    && xwin --accept-license --arch x86_64 splat --output ${XWIN_PREFIX} \
    && rm -rf /root/.cache/xwin

RUN git clone --depth 1 https://github.com/tpoechtrager/osxcross /opt/osxcross

# Fetch the macOS + iOS SDKs at build time so the image is self-contained.
# These URLs point to community mirrors that redistribute Apple-licensed SDK
# content; use at your own discretion. For license-clean SDKs, generate your
# own from Xcode on a Mac via osxcross's tools/gen_sdk_package.sh (macOS) and
# tools/gen_sdk_package_p.sh iPhoneOS (iOS), host the tarballs, and point these
# URLs at them. To pin versions, update these URLs together with the
# OSXCROSS_SDK / OSXCROSS_IOS_SDK names above.
ENV OSXCROSS_SDK_URL=https://github.com/joseluisq/macosx-sdks/releases/download/26.1/MacOSX26.1.sdk.tar.xz
ENV OSXCROSS_IOS_SDK_URL=https://github.com/growtopiajaw/iPhoneOS-SDK/releases/download/v1.0/iPhoneOS15.5.sdk.tar.xz
RUN mkdir -p /opt/osxcross/tarballs && \
    curl -fSL "${OSXCROSS_SDK_URL}" -o "/opt/osxcross/tarballs/${OSXCROSS_SDK}.tar.xz" && \
    curl -fSL "${OSXCROSS_IOS_SDK_URL}" -o "/opt/osxcross/tarballs/${OSXCROSS_IOS_SDK}.tar.xz"
RUN cd /opt/osxcross && \
    LLVM_CONFIG=/usr/bin/llvm-config-19 \
    UNATTENDED=1 TARGET_DIR=/opt/osxcross/target ./build.sh

# Extract iPhoneOS SDK to /opt/iphoneos.sdk for direct use by clang via
# --target=arm64-apple-ios -isysroot. We don't run osxcross's build_ios.sh —
# wrapper scripts driving osxcross's macOS clang against the iOS sysroot
# give cleaner control over the deployment target and arch flags.
RUN mkdir -p /opt/iphoneos.sdk && \
    tar -xJf /opt/osxcross/tarballs/${OSXCROSS_IOS_SDK}.tar.xz -C /opt/iphoneos.sdk --strip-components=1 && \
    ls /opt/iphoneos.sdk/usr/include/stdlib.h >/dev/null && \
    echo "iPhoneOS SDK extracted to /opt/iphoneos.sdk"

ENV PATH="/opt/osxcross/target/bin:${PATH}"

# Detect Darwin version and write a shell snippet that sets CC/AR env vars.
# osxcross clang uses its binary name to detect the target triple, so we
# must use the real names (e.g., x86_64-apple-darwin23-clang), not symlinks.
# Docker ENV can't use shell expansion, so we write a script that cargo
# invocations source via BASH_ENV.
RUN DARWIN_VER=$(ls /opt/osxcross/target/bin/ | grep -oP 'darwin\K[0-9]+(\.[0-9]+)?' | head -1) && \
    echo "Detected osxcross Darwin version: ${DARWIN_VER}" && \
    echo "${DARWIN_VER}" > /tmp/darwin_ver && \
    cat > /usr/local/bin/aarch64-apple-ios-clang <<'IOSWRAP' && chmod +x /usr/local/bin/aarch64-apple-ios-clang
#!/bin/bash
# Use clang-19 directly, not osxcross's darwin clang. osxcross's wrapper
# hardcodes -mmacosx-version-min which conflicts with -miphoneos-version-min.
# clang-19 has full Apple target support; combined with the iPhoneOS sysroot
# and -fuse-ld=lld (which routes to ld64.lld for Mach-O), this is enough to
# compile and link iOS arm64 binaries.
exec clang-19 \
    --target=arm64-apple-ios${IPHONEOS_DEPLOYMENT_TARGET:-14.0} \
    -isysroot /opt/iphoneos.sdk \
    -fuse-ld=lld \
    "$@"
IOSWRAP
RUN cat > /usr/local/bin/aarch64-apple-ios-clang++ <<'IOSWRAP' && chmod +x /usr/local/bin/aarch64-apple-ios-clang++
#!/bin/bash
exec clang++-19 \
    --target=arm64-apple-ios${IPHONEOS_DEPLOYMENT_TARGET:-14.0} \
    -isysroot /opt/iphoneos.sdk \
    -fuse-ld=lld \
    "$@"
IOSWRAP
RUN DARWIN_VER=$(cat /tmp/darwin_ver) && \
    cat > /etc/osxcross-env.sh <<EOF
export CC_x86_64_apple_darwin=x86_64-apple-darwin${DARWIN_VER}-clang
export CXX_x86_64_apple_darwin=x86_64-apple-darwin${DARWIN_VER}-clang++
export AR_x86_64_apple_darwin=x86_64-apple-darwin${DARWIN_VER}-ar
export CC_aarch64_apple_darwin=aarch64-apple-darwin${DARWIN_VER}-clang
export CXX_aarch64_apple_darwin=aarch64-apple-darwin${DARWIN_VER}-clang++
export AR_aarch64_apple_darwin=aarch64-apple-darwin${DARWIN_VER}-ar
export CC_aarch64_apple_ios=aarch64-apple-ios-clang
export CXX_aarch64_apple_ios=aarch64-apple-ios-clang++
export AR_aarch64_apple_ios=aarch64-apple-darwin${DARWIN_VER}-ar
export CC_aarch64_apple_ios_sim=aarch64-apple-ios-clang
export CXX_aarch64_apple_ios_sim=aarch64-apple-ios-clang++
export AR_aarch64_apple_ios_sim=aarch64-apple-darwin${DARWIN_VER}-ar
# Windows MSVC cross-compile via clang-cl + xwin sysroot.
# /imsvc <path> tells clang-cl to treat the path as a system include
# (suppresses warnings from MSVC headers themselves).
export CC_x86_64_pc_windows_msvc=clang-cl
export CXX_x86_64_pc_windows_msvc=clang-cl
export AR_x86_64_pc_windows_msvc=llvm-lib
export CFLAGS_x86_64_pc_windows_msvc="/imsvc /xwin/crt/include /imsvc /xwin/sdk/include/ucrt /imsvc /xwin/sdk/include/um /imsvc /xwin/sdk/include/shared /imsvc /xwin/sdk/include/winrt"
export CXXFLAGS_x86_64_pc_windows_msvc="\$CFLAGS_x86_64_pc_windows_msvc"
EOF
# Android NDK env vars live in their own file, sourced only where the NDK
# exists (amd64 — see the NDK layer above), so the arm64 image never exports
# paths into a toolchain it doesn't have.
RUN cat > /etc/android-env.sh <<'EOF'
export CC_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang
export CXX_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang++
export AR_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
export CC_x86_64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android33-clang
export CXX_x86_64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android33-clang++
export AR_x86_64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
EOF
RUN echo 'if [ -d "$ANDROID_NDK_HOME" ]; then . /etc/android-env.sh; fi' >> /etc/osxcross-env.sh

# BASH_ENV is sourced by bash before every RUN command
ENV BASH_ENV=/etc/osxcross-env.sh
SHELL ["/bin/bash", "-c"]

# ── UPX (binary compression for `makers upx`) ────────────────────────────────
# Installed here as a late, cheap layer so adding it doesn't invalidate the
# expensive osxcross/xwin/NDK layers above. The official static binary (not the
# `upx-ucl` apt package, which isn't in this image's sources). UPX packs
# PE/ELF/Mach-O regardless of host OS, so the Linux container can compress
# Windows/macOS binaries too.
ENV UPX_VERSION=5.1.1
RUN UPX_ARCH=$([ "$(uname -m)" = "aarch64" ] && echo arm64_linux || echo amd64_linux) \
    && curl -sSL "https://github.com/upx/upx/releases/download/v${UPX_VERSION}/upx-${UPX_VERSION}-${UPX_ARCH}.tar.xz" \
    -o /tmp/upx.tar.xz \
    && tar -xJf /tmp/upx.tar.xz -C /tmp \
    && mv /tmp/upx-${UPX_VERSION}-${UPX_ARCH}/upx /usr/local/bin/upx \
    && chmod +x /usr/local/bin/upx \
    && rm -rf /tmp/upx.tar.xz /tmp/upx-${UPX_VERSION}-${UPX_ARCH}

# ── Cargo linker config ─────────────────────────────────────────────────────
WORKDIR /app
RUN DARWIN_VER=$(cat /tmp/darwin_ver) && \
    mkdir -p .cargo && cat > .cargo/config.toml <<TOML
# Both Linux sections exist on both arches; only the native one is ever built
# (the image carries no cross-libc sysroot for the other).
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=/usr/bin/mold", "-C", "prefer-dynamic"]

[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=/usr/bin/mold", "-C", "prefer-dynamic"]

[target.x86_64-pc-windows-msvc]
linker = "lld-link"
rustflags = [
    "-C", "prefer-dynamic",
    "-Lnative=/xwin/crt/lib/x86_64",
    "-Lnative=/xwin/sdk/lib/um/x86_64",
    "-Lnative=/xwin/sdk/lib/ucrt/x86_64",
]

[target.aarch64-linux-android]
linker = "/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang"

[target.x86_64-linux-android]
linker = "/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android33-clang"

[target.x86_64-apple-darwin]
linker = "x86_64-apple-darwin${DARWIN_VER}-clang"
rustflags = ["-C", "prefer-dynamic"]

[target.aarch64-apple-darwin]
linker = "aarch64-apple-darwin${DARWIN_VER}-clang"
rustflags = ["-C", "prefer-dynamic"]

[target.aarch64-apple-ios]
linker = "aarch64-apple-ios-clang"

[target.aarch64-apple-ios-sim]
linker = "aarch64-apple-darwin${DARWIN_VER}-clang"
TOML

# ── Working directory ───────────────────────────────────────────────────────
# The persistent container mounts the repo at /app/src.
# The image just provides the toolchain — no source code baked in.

CMD ["sleep", "infinity"]

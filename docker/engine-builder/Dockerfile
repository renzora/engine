# =============================================================================
# Renzora Engine Builder — Cross-Platform Toolchain
# =============================================================================
#
# Provides the build toolchain for all supported platforms. No source code
# baked in — the repo is mounted at runtime via a persistent container.
#
# Build image:
#   makers docker-build
#
# Create persistent container:
#   makers docker-create
#
# Build all platforms:
#   makers docker-run

FROM --platform=linux/amd64 rust:1.93.0-bookworm

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
    # Windows cross-compilation
    gcc-mingw-w64-x86-64 \
    g++-mingw-w64-x86-64 \
    # WASM post-processing
    binaryen \
    # General
    curl \
    unzip \
    file \
    desktop-file-utils \
    && rm -rf /var/lib/apt/lists/*

# ── AppImage tooling ─────────────────────────────────────────────────────────
RUN curl -sL https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage \
    -o /usr/local/bin/appimagetool && \
    chmod +x /usr/local/bin/appimagetool && \
    cd /usr/local/bin && ./appimagetool --appimage-extract >/dev/null && \
    mv squashfs-root /opt/appimagetool && \
    ln -sf /opt/appimagetool/AppRun /usr/local/bin/appimagetool

# ── All Rust targets ─────────────────────────────────────────────────────────
RUN rustup target add \
    x86_64-unknown-linux-gnu \
    x86_64-pc-windows-gnu \
    wasm32-unknown-unknown \
    aarch64-linux-android \
    x86_64-linux-android

RUN rustup target add x86_64-apple-darwin aarch64-apple-darwin

# iOS (share the macOS SDK via osxcross)
RUN rustup target add aarch64-apple-ios aarch64-apple-ios-sim

# ── WASM tooling ─────────────────────────────────────────────────────────────
RUN cargo install wasm-bindgen-cli --version 0.2.108 --locked

# ── Android NDK ──────────────────────────────────────────────────────────────
ENV ANDROID_NDK_VERSION=r27c
ENV ANDROID_NDK_HOME=/opt/android-ndk

RUN curl -sL https://dl.google.com/android/repository/android-ndk-${ANDROID_NDK_VERSION}-linux.zip \
    -o /tmp/ndk.zip && \
    unzip -q /tmp/ndk.zip -d /opt && \
    mv /opt/android-ndk-${ANDROID_NDK_VERSION} ${ANDROID_NDK_HOME} && \
    rm /tmp/ndk.zip

# ── osxcross for macOS / iOS cross-compilation ──────────────────────────────
ENV OSXCROSS_SDK=MacOSX26.2.sdk
ENV MACOSX_DEPLOYMENT_TARGET=14.0

RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake \
    libxml2-dev \
    llvm-dev \
    patch \
    python3 \
    xz-utils \
    zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

RUN git clone --depth 1 https://github.com/tpoechtrager/osxcross /opt/osxcross
COPY docker/sdk/${OSXCROSS_SDK}.tar.bz2 /opt/osxcross/tarballs/
RUN cd /opt/osxcross && \
    UNATTENDED=1 TARGET_DIR=/opt/osxcross/target ./build.sh

ENV PATH="/opt/osxcross/target/bin:${PATH}"

# Detect Darwin version and write a shell snippet that sets CC/AR env vars.
# osxcross clang uses its binary name to detect the target triple, so we
# must use the real names (e.g., x86_64-apple-darwin23-clang), not symlinks.
# Docker ENV can't use shell expansion, so we write a script that cargo
# invocations source via BASH_ENV.
RUN DARWIN_VER=$(ls /opt/osxcross/target/bin/ | grep -oP 'darwin\K[0-9]+' | head -1) && \
    echo "Detected osxcross Darwin version: ${DARWIN_VER}" && \
    echo "${DARWIN_VER}" > /tmp/darwin_ver && \
    cat > /etc/osxcross-env.sh <<EOF
export CC_x86_64_apple_darwin=x86_64-apple-darwin${DARWIN_VER}-clang
export CXX_x86_64_apple_darwin=x86_64-apple-darwin${DARWIN_VER}-clang++
export AR_x86_64_apple_darwin=x86_64-apple-darwin${DARWIN_VER}-ar
export CC_aarch64_apple_darwin=aarch64-apple-darwin${DARWIN_VER}-clang
export CXX_aarch64_apple_darwin=aarch64-apple-darwin${DARWIN_VER}-clang++
export AR_aarch64_apple_darwin=aarch64-apple-darwin${DARWIN_VER}-ar
export CC_aarch64_apple_ios=aarch64-apple-darwin${DARWIN_VER}-clang
export CXX_aarch64_apple_ios=aarch64-apple-darwin${DARWIN_VER}-clang++
export AR_aarch64_apple_ios=aarch64-apple-darwin${DARWIN_VER}-ar
export CC_aarch64_apple_ios_sim=aarch64-apple-darwin${DARWIN_VER}-clang
export CXX_aarch64_apple_ios_sim=aarch64-apple-darwin${DARWIN_VER}-clang++
export AR_aarch64_apple_ios_sim=aarch64-apple-darwin${DARWIN_VER}-ar
export CC_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang
export CXX_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang++
export AR_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
export CC_x86_64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android33-clang
export CXX_x86_64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android33-clang++
export AR_x86_64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
EOF

# BASH_ENV is sourced by bash before every RUN command
ENV BASH_ENV=/etc/osxcross-env.sh
SHELL ["/bin/bash", "-c"]

# ── Cargo linker config ─────────────────────────────────────────────────────
WORKDIR /app
RUN DARWIN_VER=$(cat /tmp/darwin_ver) && \
    mkdir -p .cargo && cat > .cargo/config.toml <<TOML
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=/usr/bin/mold"]

[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"

[target.aarch64-linux-android]
linker = "/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang"

[target.x86_64-linux-android]
linker = "/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android33-clang"

[target.x86_64-apple-darwin]
linker = "x86_64-apple-darwin${DARWIN_VER}-clang"

[target.aarch64-apple-darwin]
linker = "aarch64-apple-darwin${DARWIN_VER}-clang"

[target.aarch64-apple-ios]
linker = "aarch64-apple-darwin${DARWIN_VER}-clang"

[target.aarch64-apple-ios-sim]
linker = "aarch64-apple-darwin${DARWIN_VER}-clang"
TOML

# ── Working directory ───────────────────────────────────────────────────────
# The persistent container mounts the repo at /app/src.
# The image just provides the toolchain — no source code baked in.

CMD ["sleep", "infinity"]

# =============================================================================
# Renzora Engine Builder — CI/CD Release Builds
# =============================================================================
#
# Builds the full engine + export templates for all supported platforms:
#   - Editor: Windows, macOS, Linux
#   - Runtime/Export Templates: Windows, macOS, Linux, WASM, Android ARM, Android x86
#   - Server: Linux (headless)
#
# Build image:
#   docker build -f docker/engine-builder/Dockerfile -t renzora/engine-builder:0.2.0 .
#
# Build all targets:
#   docker run --rm -v /path/to/output:/output renzora/engine-builder:0.2.0 /app/scripts/build-all.sh /output
#
# Build specific target:
#   docker run --rm renzora/engine-builder:0.2.0 cargo build-editor
#   docker run --rm renzora/engine-builder:0.2.0 cargo build-web

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
    # Windows cross-compilation
    gcc-mingw-w64-x86-64 \
    g++-mingw-w64-x86-64 \
    # WASM post-processing
    binaryen \
    # General
    curl \
    unzip \
    && rm -rf /var/lib/apt/lists/*

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
ENV OSXCROSS_SDK=MacOSX14.0.sdk
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
COPY docker/sdk/${OSXCROSS_SDK}.tar.xz /opt/osxcross/tarballs/
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

# ── Copy engine source ──────────────────────────────────────────────────────
COPY Cargo.toml Cargo.lock rust-toolchain.toml build.rs ./
RUN rm rust-toolchain.toml
COPY crates/ crates/
COPY src/ src/
COPY assets/ assets/

COPY templates/ templates/
COPY scripts/ scripts/
RUN chmod +x scripts/*.sh

# ── Pre-build deps (cached layer) ───────────────────────────────────────────
# Build the engine libs for each target to cache all dependencies.
# Desktop targets use unified features (editor,runtime,server) so all
# binaries share the same bevy_dylib hash for plugin compatibility.
# --workspace ensures plugin crate hashes match the engine binary.
RUN cargo build --workspace --lib --profile dist --no-default-features --features editor,runtime,server
RUN cargo build --workspace --lib --profile dist --no-default-features --features editor,runtime,server --target x86_64-pc-windows-gnu
RUN cargo build --workspace --lib --profile dist --no-default-features --features editor,runtime,server --target x86_64-apple-darwin
RUN cargo build --workspace --lib --profile dist --no-default-features --features editor,runtime,server --target aarch64-apple-darwin
# WASM: build only the runtime binary (no dylib support)
RUN cargo build -p renzora_app --bin renzora-runtime --profile dist --no-default-features --features wasm --target wasm32-unknown-unknown
# Android and iOS are built via their template packages (templates/android/, templates/ios/)
# and are not pre-cached here — build-all.sh handles them at runtime.

CMD ["/bin/bash"]

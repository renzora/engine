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
#   docker run --rm renzora/engine-builder:0.2.0 cargo release-editor
#   docker run --rm renzora/engine-builder:0.2.0 cargo dist-web

FROM rust:1.86-bookworm

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

# iOS / tvOS / visionOS (share the macOS SDK via osxcross)
RUN rustup target add aarch64-apple-ios aarch64-apple-ios-sim
RUN rustup target add aarch64-apple-tvos aarch64-apple-tvos-sim

# ── WASM tooling ─────────────────────────────────────────────────────────────
RUN cargo install wasm-bindgen-cli --version 0.2.108

# ── Android NDK ──────────────────────────────────────────────────────────────
ENV ANDROID_NDK_VERSION=r27c
ENV ANDROID_NDK_HOME=/opt/android-ndk

RUN curl -sL https://dl.google.com/android/repository/android-ndk-${ANDROID_NDK_VERSION}-linux.zip \
    -o /tmp/ndk.zip && \
    unzip -q /tmp/ndk.zip -d /opt && \
    mv /opt/android-ndk-${ANDROID_NDK_VERSION} ${ANDROID_NDK_HOME} && \
    rm /tmp/ndk.zip

# ── osxcross for macOS / iOS / tvOS cross-compilation ────────────────────────
ENV OSXCROSS_SDK=MacOSX15.2.sdk

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

# ── Cargo linker config ─────────────────────────────────────────────────────
WORKDIR /app
RUN mkdir -p .cargo && cat > .cargo/config.toml <<'TOML'
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
linker = "x86_64-apple-darwin24-clang"

[target.aarch64-apple-darwin]
linker = "aarch64-apple-darwin24-clang"

[target.aarch64-apple-ios]
linker = "aarch64-apple-darwin24-clang"

[target.aarch64-apple-ios-sim]
linker = "aarch64-apple-darwin24-clang"

[target.aarch64-apple-tvos]
linker = "aarch64-apple-darwin24-clang"

[target.aarch64-apple-tvos-sim]
linker = "aarch64-apple-darwin24-clang"
TOML

# ── Copy engine source ──────────────────────────────────────────────────────
COPY Cargo.toml Cargo.lock rust-toolchain.toml build.rs ./
COPY crates/ crates/
COPY src/ src/
COPY assets/ assets/

COPY scripts/ scripts/
RUN chmod +x scripts/*.sh

# ── Pre-build deps (cached layer) ───────────────────────────────────────────
# Build the engine lib for each target to cache all dependencies.
# Subsequent builds only recompile changed engine code.
RUN cargo build --lib --release 2>&1 || true
RUN cargo build --lib --release --target x86_64-pc-windows-gnu 2>&1 || true
RUN cargo build --lib --release --target x86_64-apple-darwin --no-default-features 2>&1 || true
RUN cargo build --lib --release --target aarch64-apple-darwin --no-default-features 2>&1 || true
RUN cargo build --lib --release --target wasm32-unknown-unknown --no-default-features --features wasm 2>&1 || true
RUN cargo build --lib --release --target aarch64-linux-android --no-default-features 2>&1 || true
RUN cargo build --lib --release --target aarch64-apple-ios --no-default-features 2>&1 || true
RUN cargo build --lib --release --target aarch64-apple-tvos --no-default-features 2>&1 || true

CMD ["/bin/bash"]

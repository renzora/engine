# =============================================================================
# Renzora Plugin Builder — Marketplace Build Server
# =============================================================================
#
# Compiles community plugins against the engine's pre-built Bevy dylib.
# Targets: Windows (.dll), Linux (.so), macOS (.dylib)
#
# Build image (once per engine release, ~10-20 min):
#   docker build -f docker/plugin-builder/Dockerfile -t renzora/plugin-builder:0.2.0 .
#
# Build a plugin (~5-10 seconds):
#   docker run --rm \
#     -v /path/to/plugin/source:/plugin \
#     -v /path/to/output:/output \
#     renzora/plugin-builder:0.2.0 \
#     /app/scripts/build-plugin.sh /plugin /output

FROM rust:1.86-bookworm

# ── System dependencies ──────────────────────────────────────────────────────
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
    gcc-mingw-w64-x86-64 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# ── Desktop cross-compilation targets ────────────────────────────────────────
RUN rustup target add \
    x86_64-unknown-linux-gnu \
    x86_64-pc-windows-gnu

RUN rustup target add x86_64-apple-darwin aarch64-apple-darwin

# ── osxcross for macOS cross-compilation ─────────────────────────────────────
# Builds the macOS cross-compilation toolchain from Xcode Command Line Tools.
# The SDK tarball must be created on a Mac using osxcross's packaging script:
#   ./tools/gen_sdk_package_pbzx.sh /path/to/Xcode_16.xip
# Then place the resulting .tar.xz in docker/sdk/ before building this image.
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

# Copy the macOS SDK tarball (must exist at build time)
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

[target.x86_64-apple-darwin]
linker = "x86_64-apple-darwin24-clang"

[target.aarch64-apple-darwin]
linker = "aarch64-apple-darwin24-clang"
TOML

# ── Copy engine source ──────────────────────────────────────────────────────
COPY Cargo.toml Cargo.lock rust-toolchain.toml build.rs ./
COPY crates/ crates/
COPY src/ src/

RUN mkdir -p plugins_build

COPY scripts/ scripts/
RUN chmod +x scripts/*.sh

# ── Pre-build engine deps (cached — only plugin crate compiles at runtime) ──
RUN cargo build --lib --release 2>&1 || true
RUN cargo build --lib --release --target x86_64-pc-windows-gnu 2>&1 || true
RUN cargo build --lib --release --target x86_64-apple-darwin 2>&1 || true
RUN cargo build --lib --release --target aarch64-apple-darwin 2>&1 || true

CMD ["/bin/bash"]

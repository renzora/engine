# =============================================================================
# Renzora Engine Builder — macOS (x86_64 + arm64) toolchain image
# =============================================================================
#
# Linux → macOS cross-compile via osxcross (the macOS SDK + cctools driven by the
# base's Clang 19). Produces both desktop macOS slices; build-all.sh wraps the
# editor output into a signed .app. rcodesign re-signs arm64 binaries after their
# install-name fixup (arm64 macOS refuses binaries with an invalid signature).
#
# BASE_TAG is injected by CI / the renzora CLI (SHA-256 of docker/base/Dockerfile)
# so the FROM is pinned without being hand-edited.
ARG BASE_TAG=latest
FROM ghcr.io/renzora/base:${BASE_TAG}

ENV OSXCROSS_SDK=MacOSX26.1.sdk
ENV MACOSX_DEPLOYMENT_TARGET=11.0

RUN rustup target add x86_64-apple-darwin aarch64-apple-darwin

# osxcross build deps (the base already carries clang-19 + the apt prereqs for
# the LLVM script; these are the extra ones osxcross's build.sh needs). python3
# is also used by build-all.sh's wrap_macos_app to synthesize the .icns.
RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake \
    libxml2-dev \
    patch \
    python3 \
    xz-utils \
    zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

# ── osxcross ─────────────────────────────────────────────────────────────────
RUN git clone --depth 1 https://github.com/tpoechtrager/osxcross /opt/osxcross

# Fetch the macOS SDK at build time so the image is self-contained. This URL
# points to a community mirror that redistributes Apple-licensed SDK content;
# use at your own discretion. For a license-clean SDK, generate your own from
# Xcode on a Mac via osxcross's tools/gen_sdk_package.sh, host the tarball, and
# point this URL at it (update OSXCROSS_SDK above to match the version).
ENV OSXCROSS_SDK_URL=https://github.com/joseluisq/macosx-sdks/releases/download/26.1/MacOSX26.1.sdk.tar.xz
RUN mkdir -p /opt/osxcross/tarballs && \
    curl -fSL "${OSXCROSS_SDK_URL}" -o "/opt/osxcross/tarballs/${OSXCROSS_SDK}.tar.xz"
RUN cd /opt/osxcross && \
    LLVM_CONFIG=/usr/bin/llvm-config-19 \
    UNATTENDED=1 TARGET_DIR=/opt/osxcross/target ./build.sh

ENV PATH="/opt/osxcross/target/bin:${PATH}"

# ── Build env + cargo linker config ──────────────────────────────────────────
# osxcross clang detects its target triple from its binary NAME (e.g.
# x86_64-apple-darwin23-clang), so we must reference the real darwin<ver> names,
# not symlinks. The darwin version is discovered from the built osxcross bin dir.
RUN DARWIN_VER=$(ls /opt/osxcross/target/bin/ | grep -oP 'darwin\K[0-9]+(\.[0-9]+)?' | head -1) && \
    echo "Detected osxcross Darwin version: ${DARWIN_VER}" && \
    echo "${DARWIN_VER}" > /tmp/darwin_ver && \
    cat >> /etc/osxcross-env.sh <<EOF
export CC_x86_64_apple_darwin=x86_64-apple-darwin${DARWIN_VER}-clang
export CXX_x86_64_apple_darwin=x86_64-apple-darwin${DARWIN_VER}-clang++
export AR_x86_64_apple_darwin=x86_64-apple-darwin${DARWIN_VER}-ar
export CC_aarch64_apple_darwin=aarch64-apple-darwin${DARWIN_VER}-clang
export CXX_aarch64_apple_darwin=aarch64-apple-darwin${DARWIN_VER}-clang++
export AR_aarch64_apple_darwin=aarch64-apple-darwin${DARWIN_VER}-ar
EOF
RUN DARWIN_VER=$(cat /tmp/darwin_ver) && \
    cat >> /app/.cargo/config.toml <<TOML

[target.x86_64-apple-darwin]
linker = "x86_64-apple-darwin${DARWIN_VER}-clang"
rustflags = ["-C", "prefer-dynamic"]

[target.aarch64-apple-darwin]
linker = "aarch64-apple-darwin${DARWIN_VER}-clang"
rustflags = ["-C", "prefer-dynamic"]
TOML

# ── rcodesign: ad-hoc signing for macOS arm64 binaries ───────────────────────
# build-all.sh rewrites dylib install names after the build, which invalidates
# the linker's ad-hoc signature; arm64 macOS refuses to run such binaries, so the
# fixup re-signs with rcodesign. Pure Rust, runs on any host arch.
RUN cargo install apple-codesign --locked

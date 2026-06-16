# =============================================================================
# Renzora Engine Builder — iOS (arm64) toolchain image
# =============================================================================
#
# Linux → iOS cross-compile. The actual iOS compiles use Clang 19 directly
# against the iPhoneOS sysroot (the wrappers below), but the toolchain still
# leans on osxcross for the darwin `ar` and the iOS-simulator linker — so this
# image builds osxcross (which needs the macOS SDK) AND adds the iPhoneOS SDK.
# That osxcross build is duplicated with the macOS image by design (this image is
# FROM base for uniform tag derivation); CI's registry layer cache absorbs it.
#
# BASE_TAG is injected by CI / the renzora CLI (SHA-256 of docker/base/Dockerfile)
# so the FROM is pinned without being hand-edited.
ARG BASE_TAG=latest
FROM ghcr.io/renzora/base:${BASE_TAG}

ENV OSXCROSS_SDK=MacOSX26.1.sdk
ENV OSXCROSS_IOS_SDK=iPhoneOS15.5.sdk
ENV MACOSX_DEPLOYMENT_TARGET=11.0
ENV IPHONEOS_DEPLOYMENT_TARGET=14.0

RUN rustup target add aarch64-apple-ios aarch64-apple-ios-sim

RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake \
    libxml2-dev \
    patch \
    python3 \
    xz-utils \
    zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

# ── osxcross (for darwin ar + the iOS-simulator linker) ──────────────────────
RUN git clone --depth 1 https://github.com/tpoechtrager/osxcross /opt/osxcross

# Both SDKs come from community mirrors that redistribute Apple-licensed content;
# use at your own discretion (see docker/macos/Dockerfile for the license-clean
# path). The macOS SDK is needed to BUILD osxcross; the iPhoneOS SDK is the iOS
# compile sysroot.
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
# wrapper scripts driving clang against the iOS sysroot give cleaner control over
# the deployment target and arch flags.
RUN mkdir -p /opt/iphoneos.sdk && \
    tar -xJf /opt/osxcross/tarballs/${OSXCROSS_IOS_SDK}.tar.xz -C /opt/iphoneos.sdk --strip-components=1 && \
    ls /opt/iphoneos.sdk/usr/include/stdlib.h >/dev/null && \
    echo "iPhoneOS SDK extracted to /opt/iphoneos.sdk"

ENV PATH="/opt/osxcross/target/bin:${PATH}"

# ── iOS clang wrappers ───────────────────────────────────────────────────────
# Use clang-19 directly, not osxcross's darwin clang: osxcross's wrapper
# hardcodes -mmacosx-version-min which conflicts with -miphoneos-version-min.
# clang-19 has full Apple target support; combined with the iPhoneOS sysroot and
# -fuse-ld=lld (which routes to ld64.lld for Mach-O) this compiles+links iOS
# arm64 binaries.
RUN cat > /usr/local/bin/aarch64-apple-ios-clang <<'IOSWRAP' && chmod +x /usr/local/bin/aarch64-apple-ios-clang
#!/bin/bash
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

# ── Build env + cargo linker config ──────────────────────────────────────────
# AR + the simulator linker come from osxcross's darwin toolchain.
RUN DARWIN_VER=$(ls /opt/osxcross/target/bin/ | grep -oP 'darwin\K[0-9]+(\.[0-9]+)?' | head -1) && \
    echo "${DARWIN_VER}" > /tmp/darwin_ver && \
    cat >> /etc/osxcross-env.sh <<EOF
export CC_aarch64_apple_ios=aarch64-apple-ios-clang
export CXX_aarch64_apple_ios=aarch64-apple-ios-clang++
export AR_aarch64_apple_ios=aarch64-apple-darwin${DARWIN_VER}-ar
export CC_aarch64_apple_ios_sim=aarch64-apple-ios-clang
export CXX_aarch64_apple_ios_sim=aarch64-apple-ios-clang++
export AR_aarch64_apple_ios_sim=aarch64-apple-darwin${DARWIN_VER}-ar
EOF
RUN DARWIN_VER=$(cat /tmp/darwin_ver) && \
    cat >> /app/.cargo/config.toml <<TOML

[target.aarch64-apple-ios]
linker = "aarch64-apple-ios-clang"

[target.aarch64-apple-ios-sim]
linker = "aarch64-apple-darwin${DARWIN_VER}-clang"
TOML

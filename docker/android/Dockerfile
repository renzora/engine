# =============================================================================
# Renzora Engine Builder — Android (arm64 + x86_64) toolchain image
# =============================================================================
#
# Linux → Android cross-compile via the Android NDK. Runtime only (no editor).
# The NDK (x86_64 binaries) is installed on every host arch; on arm64 its clang
# runs under Docker's x86_64 emulation at build time — see the NDK layer below.
#
# BASE_TAG is injected by CI / the renzora CLI (SHA-256 of docker/base/Dockerfile)
# so the FROM is pinned without being hand-edited.
ARG BASE_TAG=latest
FROM ghcr.io/renzora/base:${BASE_TAG}

RUN rustup target add aarch64-linux-android x86_64-linux-android

# ── Android NDK ──────────────────────────────────────────────────────────────
# Google only ships the Linux NDK for x86_64 hosts, but installing it is just a
# download + unzip — no execution — so we install it on EVERY arch. Its clang
# binaries are x86_64; when `renzora build android` runs them on an arm64 host,
# Docker's x86_64 emulation (Docker Desktop's Rosetta/qemu) executes them
# transparently. Only the NDK link step is emulated; rustc stays native.
ENV ANDROID_NDK_VERSION=r27c
ENV ANDROID_NDK_HOME=/opt/android-ndk

RUN curl -sL https://dl.google.com/android/repository/android-ndk-${ANDROID_NDK_VERSION}-linux.zip \
        -o /tmp/ndk.zip && \
    unzip -q /tmp/ndk.zip -d /opt && \
    mv /opt/android-ndk-${ANDROID_NDK_VERSION} ${ANDROID_NDK_HOME} && \
    rm /tmp/ndk.zip

# On an arm64 host the NDK's x86_64 clang runs under Docker's Rosetta emulation,
# which translates the CPU instructions but still needs the x86_64 glibc runtime
# present on disk — without it Rosetta fails with "failed to open elf at
# /lib64/ld-linux-x86-64.so.2". The NDK clang binary links exactly libc6 (ld.so
# + pthread/rt/dl/m/c), zlib1g (libz), and libgcc-s1 (libgcc_s); libstdc++6 is
# added for the LLVM linker tools. Install them via amd64 multiarch — only
# meaningful on arm64 (on an amd64 host these are the native libs already).
RUN if [ "$(uname -m)" = "aarch64" ]; then \
        dpkg --add-architecture amd64 && \
        apt-get update && \
        apt-get install -y --no-install-recommends \
            libc6:amd64 zlib1g:amd64 libgcc-s1:amd64 libstdc++6:amd64 && \
        rm -rf /var/lib/apt/lists/*; \
    fi

# ── Build env (sourced only where the NDK actually exists) ───────────────────
RUN cat > /etc/android-env.sh <<'EOF'
export CC_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang
export CXX_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang++
export AR_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
export CC_x86_64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android33-clang
export CXX_x86_64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android33-clang++
export AR_x86_64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
EOF
RUN echo 'if [ -d "$ANDROID_NDK_HOME" ]; then . /etc/android-env.sh; fi' >> /etc/osxcross-env.sh

# ── Cargo linker config ──────────────────────────────────────────────────────
RUN cat >> /app/.cargo/config.toml <<'TOML'

[target.aarch64-linux-android]
linker = "/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang"

[target.x86_64-linux-android]
linker = "/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android33-clang"
TOML

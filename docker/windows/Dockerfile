# =============================================================================
# Renzora Engine Builder — WINDOWS (x86_64 MSVC) toolchain image
# =============================================================================
#
# Linux → Windows-MSVC cross-compile via xwin (the redistributable MSVC SDK +
# CRT) combined with rustc's MSVC support and the base's clang-cl / lld-link.
#
# x86_64 only. Native arm64 Windows is intentionally NOT built here: the only
# parts Microsoft lets you redistribute (so xwin can bake them into this public
# image) are the CRT + SDK, which means the C compiler is clang — and clang
# can't fully stand in for MSVC on ARM64 (e.g. it emits MSVC NEON intrinsics like
# `neon_*` as undefined externals that no redistributable lib provides). That's
# an unwinnable per-crate mole hunt. ARM64 Windows users run the x86_64 build via
# Windows 11's built-in x64 emulation; a genuinely native arm64 build would need
# the real MSVC toolchain (e.g. msvc-wine, or a native arm64-Windows CI runner),
# which is a separate, non-redistributable path.
#
# BASE_TAG is injected by CI / the renzora CLI (SHA-256 of docker/base/Dockerfile)
# so the FROM is pinned without being hand-edited.
ARG BASE_TAG=latest
FROM ghcr.io/renzora/base:${BASE_TAG}

RUN rustup target add x86_64-pc-windows-msvc

# ── xwin: MSVC SDK + CRT ─────────────────────────────────────────────────────
# xwin downloads the redistributable Visual Studio Build Tools from Microsoft at
# image-build time. Microsoft permits this redistribution for development under
# the VS Build Tools EULA. Combined with rustc's MSVC support, clang-cl, and
# lld-link (from the base), this gives a fully open Linux→Windows-MSVC pipeline.
ENV XWIN_VERSION=0.6.5
ENV XWIN_PREFIX=/xwin

# xwin's release triple matches `uname -m` (x86_64 / aarch64). The splat
# `--arch x86_64` is the *Windows target* arch, independent of host.
RUN curl -sL "https://github.com/Jake-Shadle/xwin/releases/download/${XWIN_VERSION}/xwin-${XWIN_VERSION}-$(uname -m)-unknown-linux-musl.tar.gz" \
    | tar xzf - -C /tmp \
    && mv /tmp/xwin-${XWIN_VERSION}-$(uname -m)-unknown-linux-musl/xwin /usr/local/bin/ \
    && rm -rf /tmp/xwin-* \
    && xwin --accept-license --arch x86_64 splat --output ${XWIN_PREFIX} \
    && rm -rf /root/.cache/xwin

# ── Build env: clang-cl + xwin sysroot for any C deps ────────────────────────
# /imsvc <path> tells clang-cl to treat the path as a system include (suppresses
# warnings from MSVC headers themselves).
#
# CC/CXX point at a WRAPPER that pins `--target=x86_64-pc-windows-msvc`, not bare
# clang-cl. clang-cl defaults to the HOST arch, and cc-rs does NOT inject a
# target for the MSVC ABI — so on an arm64 host the x86 hand-written assembly in
# blake3 / ring / aws-lc-sys gets assembled as aarch64 and fails ("unrecognized
# instruction mnemonic"). Forcing the target on every invocation makes them all
# emit x86-64 COFF. This only affects TARGET C/asm; host build scripts use the
# plain host CC, so it's safe.
RUN printf '#!/bin/sh\nexec clang-cl --target=x86_64-pc-windows-msvc "$@"\n' \
        > /usr/local/bin/clang-cl-win64 && \
    chmod +x /usr/local/bin/clang-cl-win64 && \
    cat >> /etc/osxcross-env.sh <<'EOF'
WIN_IMSVC="/imsvc /xwin/crt/include /imsvc /xwin/sdk/include/ucrt /imsvc /xwin/sdk/include/um /imsvc /xwin/sdk/include/shared /imsvc /xwin/sdk/include/winrt"
export CC_x86_64_pc_windows_msvc=clang-cl-win64
export CXX_x86_64_pc_windows_msvc=clang-cl-win64
export AR_x86_64_pc_windows_msvc=llvm-lib
export CFLAGS_x86_64_pc_windows_msvc="$WIN_IMSVC"
export CXXFLAGS_x86_64_pc_windows_msvc="$WIN_IMSVC"
EOF

# ── Cargo linker config ──────────────────────────────────────────────────────
# The `linker` here is overridden by the repo's /app/src/.cargo (which pins
# `rust-lld`); the `-Lnative` rustflags MERGE in (cargo concatenates rustflags
# arrays across config files), so the xwin lib paths still apply.
RUN cat >> /app/.cargo/config.toml <<'TOML'

[target.x86_64-pc-windows-msvc]
linker = "lld-link"
rustflags = [
    "-C", "prefer-dynamic",
    "-Lnative=/xwin/crt/lib/x86_64",
    "-Lnative=/xwin/sdk/lib/um/x86_64",
    "-Lnative=/xwin/sdk/lib/ucrt/x86_64",
]
TOML

# ── UPX ──────────────────────────────────────────────────────────────────────
# Built Windows binaries can be UPX-compressed by `renzora upx`, which the CLI
# routes to the linux container — so no UPX is needed here.

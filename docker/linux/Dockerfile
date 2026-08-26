# =============================================================================
# Renzora Engine Builder — LINUX toolchain image
# =============================================================================
#
# The host build environment for Linux users plus everything to emit BOTH
# desktop Linux arches (x86_64 + aarch64) from one container: appimagetool (with
# both-arch runtimes pre-staged), the GNU cross-gcc + foreign-arch link deps for
# the OTHER arch, and UPX. `cargo check`/`test` and native-Linux builds only
# need the base; this image adds the packaging + cross bits on top.
#
# BASE_TAG is injected by CI / the renzora CLI (both compute it as the SHA-256
# of docker/base/Dockerfile) so the FROM is pinned without being hand-edited.
ARG BASE_TAG=latest
FROM ghcr.io/renzora/base:${BASE_TAG}

# ── AppImage tooling ─────────────────────────────────────────────────────────
# desktop-file-utils validates the .desktop file appimagetool embeds.
RUN apt-get update && apt-get install -y --no-install-recommends \
    desktop-file-utils \
    && rm -rf /var/lib/apt/lists/*

# appimagetool release assets are named by `uname -m` (x86_64 / aarch64).
RUN curl -sL "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$(uname -m).AppImage" \
    -o /usr/local/bin/appimagetool && \
    chmod +x /usr/local/bin/appimagetool && \
    cd /usr/local/bin && ./appimagetool --appimage-extract >/dev/null && \
    mv squashfs-root /opt/appimagetool && \
    ln -sf /opt/appimagetool/AppRun /usr/local/bin/appimagetool

# Pre-stage the type2 runtimes for BOTH arches. appimagetool embeds an
# arch-specific runtime into each .AppImage; when cross-building (e.g. an x86_64
# AppImage produced from the arm64 image) it would otherwise fetch the foreign
# runtime at build time. Staging both lets build-all.sh pass --runtime-file for
# the target arch with no network dependency. Best-effort so a mirror hiccup
# never fails the image build.
RUN mkdir -p /opt/appimage-runtimes && \
    for a in x86_64 aarch64; do \
        curl -sfL "https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-$a" \
            -o "/opt/appimage-runtimes/runtime-$a" \
            && chmod +x "/opt/appimage-runtimes/runtime-$a" \
            || echo "WARN: could not pre-stage AppImage runtime-$a"; \
    done

# ── Cross-arch Linux toolchain (the OTHER desktop Linux arch) ─────────────────
# One `renzora build linux` emits both arches: the native arch builds via the
# base's clang+mold, and this layer adds true cross-compilation of the opposite
# arch (rustc runs at native speed, emits foreign code, links with the cross
# gcc — no emulation). The build-time link deps (from the `-l` flags in the
# native link line) are: alsa, udev, wayland-client, and lzma — X11/Vulkan/
# xkbcommon are dlopen'd at runtime (x11-dl/*-dl/ash in Cargo.lock) and the
# engine uses rustls not openssl, so the foreign sysroot is just the cross gcc
# plus those dev packages (xkbcommon kept as a cheap safety margin). All are
# Multi-Arch: same → co-installable.
RUN set -eux; \
    if [ "$(uname -m)" = "aarch64" ]; then \
        XARCH=amd64; XPKG=x86-64-linux-gnu;  XGNU=x86_64-linux-gnu;  XRUST=x86_64-unknown-linux-gnu; \
    else \
        XARCH=arm64; XPKG=aarch64-linux-gnu; XGNU=aarch64-linux-gnu; XRUST=aarch64-unknown-linux-gnu; \
    fi; \
    dpkg --add-architecture "$XARCH"; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        "gcc-${XPKG}" "g++-${XPKG}" \
        "libasound2-dev:${XARCH}" "libudev-dev:${XARCH}" \
        "libwayland-dev:${XARCH}" "libxkbcommon-dev:${XARCH}" \
        "liblzma-dev:${XARCH}"; \
    rm -rf /var/lib/apt/lists/*; \
    printf '%s\n' "$XRUST" > /etc/renzora-linux-cross-triple; \
    printf '%s\n' "$XGNU"  > /etc/renzora-linux-cross-gnu

# Cross env (cc-rs + pkg-config + the cargo linker). The cross-gcc drives its own
# ld and knows the foreign multiarch lib path (/usr/lib/<gnu>); pkg-config needs
# ALLOW_CROSS + a target-scoped path so the native build's pkg-config never picks
# up the foreign .pc files.
#
# The linker MUST be set via CARGO_TARGET_<triple>_LINKER, not just a cargo
# `[target.*]` section: the repo's own /app/src/.cargo/config.toml pins
# `[target.x86_64-unknown-linux-gnu] linker = "clang"`, and because it sits
# closer to the build cwd (/app/src) than this image's /app/.cargo/config.toml,
# it WINS — so an image `[target.*]` linker is silently shadowed for any triple
# the repo defines. An env var outranks every config file, so it's the only
# place the cross linker reliably takes effect (the repo keeps clang for the
# NATIVE arch, which is correct).
#
# The linker is a WRAPPER, not the cross-gcc directly, because the repo's Linux
# `target.rustflags` pass `-fuse-ld=mold`, which aims the NATIVE mold at a
# foreign target. We can't
# drop it via CARGO_TARGET_<triple>_RUSTFLAGS: unlike LINKER, that env does NOT
# override a config file's `target.rustflags` (a cargo precedence quirk — config
# wins), so the flag always reaches the linker. The wrapper strips exactly that
# one arg and forwards everything else (incl. `-Wl,-rpath,$ORIGIN`, which GCC
# accepts) to the real cross-gcc, so the repo's rpath settings survive.
RUN XGNU=$(cat /etc/renzora-linux-cross-gnu); XRUST=$(cat /etc/renzora-linux-cross-triple); \
    XRUST_US=$(echo "$XRUST" | tr - _); \
    XRUST_ENV=$(echo "$XRUST" | tr 'a-z-' 'A-Z_'); \
    printf '#!/bin/bash\n# Strip three args before handing them to the cross-gcc:\n#   -fuse-ld=mold             the native mold, aimed at a foreign target.\n#   -fuse-ld=lld, -B*/gcc-ld  rustc aims these at its own bundled rust-lld in the\n#     HOST sysroot (.../x86_64-unknown-linux-gnu/bin/gcc-ld). Following them links\n#     an aarch64 target with an x86_64-flavoured lld, which then rejects the\n#     aarch64-only flag the cross-gcc driver adds:\n#       rust-lld: error: --fix-cortex-a53-843419 is only supported on AArch64\n#     That is what cost every release its linux-arm64 slice. The lane is\n#     best-effort, so it was a WARN and the build still went green.\nargs=()\nfor a in "$@"; do\n  case "$a" in\n    -fuse-ld=mold|-fuse-ld=/usr/bin/mold|-fuse-ld=lld) continue ;;\n    -B*/gcc-ld) continue ;;\n  esac\n  args+=("$a")\ndone\nexec %s-gcc "${args[@]}"\n' "$XGNU" \
        > /usr/local/bin/renzora-cross-linker && \
    chmod +x /usr/local/bin/renzora-cross-linker && \
    { \
      echo "export CC_${XRUST_US}=${XGNU}-gcc"; \
      echo "export CXX_${XRUST_US}=${XGNU}-g++"; \
      echo "export AR_${XRUST_US}=${XGNU}-ar"; \
      echo "export PKG_CONFIG_ALLOW_CROSS=1"; \
      echo "export PKG_CONFIG_PATH_${XRUST_US}=/usr/lib/${XGNU}/pkgconfig"; \
      echo "export CARGO_TARGET_${XRUST_ENV}_LINKER=/usr/local/bin/renzora-cross-linker"; \
    } >> /etc/osxcross-env.sh && \
    cat >> /app/.cargo/config.toml <<TOML

# Cross Linux arch — the cross-gcc (via the strip wrapper) drives its own ld and
# finds the foreign multiarch libs in /usr/lib/${XGNU}; only alsa+udev link, both
# installed for this arch. The linker here is a fallback for hosts where the repo
# config doesn't define this triple; the CARGO_TARGET_*_LINKER env above is what
# actually wins. Lets one container emit both Linux arches.
[target.${XRUST}]
linker = "/usr/local/bin/renzora-cross-linker"
TOML

# (UPX moved to docker/base/Dockerfile — every platform lane packs its own
# binaries now, not just this one. See `compress_binaries` in build-all.sh.)

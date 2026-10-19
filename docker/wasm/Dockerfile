# =============================================================================
# Renzora Engine Builder — WASM (web) toolchain image
# =============================================================================
#
# Builds the WASM game runtime (no dlopen; plugins are bundled statically as
# rlibs). wasm-bindgen generates the JS glue; binaryen's wasm-opt shrinks the
# output. No editor on web — the editor ships as a desktop dlopen bundle.
#
# BASE_TAG is injected by CI / the renzora CLI (SHA-256 of docker/base/Dockerfile)
# so the FROM is pinned without being hand-edited.
ARG BASE_TAG=latest
FROM ghcr.io/renzora/base:${BASE_TAG}

RUN rustup target add wasm32-unknown-unknown

# binaryen provides wasm-opt (the -Oz pass in build-all.sh's wasm lane).
RUN apt-get update && apt-get install -y --no-install-recommends \
    binaryen \
    && rm -rf /var/lib/apt/lists/*

# Must be the exact version of the `wasm-bindgen` crate in Cargo.lock: the CLI
# refuses a module built against any other schema. Bevy 0.20 moved the lock to
# 0.2.129 while this stayed at 0.2.108, and every wasm build failed at the
# bindgen step. `bindgen_wasm` in build-all.sh installs the locked version if
# the two drift again, so a stale pin here costs minutes instead of a lane.
RUN cargo install wasm-bindgen-cli --version 0.2.129 --locked

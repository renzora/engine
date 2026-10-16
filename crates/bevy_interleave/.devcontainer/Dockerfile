FROM mcr.microsoft.com/devcontainers/rust:0-1

WORKDIR /workspace

RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y \
    build-essential \
    libpulse-dev \
    libdbus-1-dev \
    libudev-dev \
    libssl-dev \
    xorg \
    openbox \
    alsa-tools \
    librust-alsa-sys-dev \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target install wasm32-unknown-unknown

RUN cargo install flamegraph
RUN cargo install wasm-server-runner

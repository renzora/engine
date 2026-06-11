# Installation

Welcome! Let's get Renzora running on your computer. There are three ways to install it — pick whichever feels easiest, and you'll be in the editor in a few minutes.

Here's what you're installing — the Renzora editor, where you'll build your games:

![The Renzora editor with a 3D city scene open: a scene list on the left, the viewport with move/rotate gizmos in the middle, a component library and Transform panel on the right, and an asset browser along the bottom.](/assets/previews/interface.png)

The three ways to install:

- **Download a prebuilt build** — the easiest start. No tools to set up; just download, extract, and run.
- **Install the `renzora` CLI** — best if you're comfortable with a terminal and want to scaffold and build projects.
- **Build from source** — for contributors or anyone who wants a custom build.

## System requirements

| | Minimum |
|---|---|
| **Windows** | Windows 10+, 64-bit |
| **macOS** | macOS 12 Monterey or newer |
| **Linux** | Ubuntu 22.04+ / Fedora 38+ |
| **GPU** | A GPU with Vulkan, Metal, or DX12 (Renzora renders through `wgpu`) |
| **RAM** | 4 GB minimum, 8 GB recommended |

> Renzora is a Bevy 0.18 engine and uses WebGPU/`wgpu` for rendering. Very old GPUs without a Vulkan/Metal/DX12 backend are not supported.

## Download a prebuilt build (easiest)

Want the quickest start? Grab a prebuilt engine for your platform from the download page — no Cargo, no terminal required:

**[renzora.com/download](/download)**

Each platform ships as a `.zip` archive built from the [GitHub releases](https://github.com/renzora/engine/releases) — download it, extract, and run the engine directly. The editor opens automatically because the `renzora_editor` bundle ships beside the binary inside the archive.

### Windows

Download the Windows `.zip`, extract it anywhere, and double-click `renzora.exe`.

### macOS

Download the macOS `.zip` and extract it, then move the Renzora app to your Applications folder.

> On first launch macOS Gatekeeper may block an unsigned build. Right-click the app, choose **Open**, then confirm in the security dialog.

### Linux

Download the Linux `.zip` and extract it:

```bash
unzip renzora-linux-*.zip
cd renzora
./renzora
```

## Install the CLI (recommended for developers)

The `renzora` CLI scaffolds projects and runs every build inside the pinned `ghcr.io/renzora/engine` Docker toolchain, so your build environment matches everyone else's — and the plugin ABI. Install it with Cargo:

```bash
cargo install renzora
```

It needs [Docker](https://docs.docker.com/get-docker/) (the toolchain runs in a container) and `git` (for `renzora new`). Then scaffold and run a project:

```bash
renzora new my-game     # clone the engine into ./my-game
cd my-game
renzora init            # build/pull the toolchain image + container (first run is slow)
renzora run             # build the editor in the container and launch it
```

`renzora run` launches the editor (`renzora run runtime` runs the game shape). Other commands include `renzora build [platforms]`, `renzora test`, `renzora add <name>`, `renzora shell`, and `renzora destroy`. Only the build runs in the container — the GPU editor and game run natively.

> The CLI is the published [`renzora` crate](https://crates.io/crates/renzora). The crate *named* `renzora` inside the engine repo is a different thing (the SDK library), so `cargo install renzora` installs the CLI — not that library.

## Build from source

For contributors and anyone who wants a custom build. You only need a Rust toolchain and Git for a native build; cross-compiling to every platform is done in a container (see below).

### Prerequisites

- **Rust** — install via [rustup](https://rustup.rs). The project pins its toolchain in `docker/Dockerfile` (currently **Rust 1.93.0**); there is no `rust-toolchain.toml`, so match that version with `rustup` for a clean native build.
- **Git**
- **A C/C++ toolchain** — MSVC Build Tools on Windows, Xcode Command Line Tools on macOS, GCC/Clang on Linux.

**Linux (Ubuntu/Debian)** also needs the usual Bevy system libraries:

```bash
sudo apt install build-essential pkg-config libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev
```

### Clone and run

```bash
git clone https://github.com/renzora/engine.git
cd engine
cargo renzora        # build the workspace and run the editor
```

The repository ships [`.cargo/config.toml`](https://github.com/renzora/engine) aliases that are the real local interface:

| Alias | What it does |
|---|---|
| `cargo renzora` | Build the workspace and run the **editor** |
| `cargo runtime` | Run the **shipped-game** shape (the same binary with `--no-editor`) |
| `cargo server` | Run a headless **dedicated server** |
| `cargo build-editor` / `cargo build-all` | Build the binary + editor bundle + shared `bevy_dylib` |
| `cargo build-runtime` | Build the lean game binary only |

The first build takes several minutes; later builds are incremental.

> If you installed the [`renzora` CLI](#install-the-cli-recommended-for-developers), `renzora run` / `renzora build` run these same builds inside the Docker container. The cargo aliases here build **natively** on your host instead — faster to iterate, but you supply the toolchain.

### Good to know: one binary, editor as a removable bundle

There is exactly one workspace binary: `renzora` (`renzora.exe` on Windows). The editor is **not** a compile-time feature — it ships as a removable bundle (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`) placed **beside the exe**:

- Bundle present → the binary launches as the **editor**.
- Delete that one file (or pass `--no-editor`) → the same binary is the **shipped game**.

You don't need the deeper details to get started — the cross-compile toolchain and every launch flag are covered in the build reference below and in the Advanced docs.

### Cross-compiling with Docker

Cross-platform builds run inside the engine's Docker image, `ghcr.io/renzora/engine`. The image bundles every cross toolchain it needs, so the host only needs Docker:

```bash
docker/build-all.sh dist windows linux
```

`build-all.sh <output-dir> [platforms...]` accepts these platform tokens: `windows`, `linux`, `macos`, `wasm` (Web), `android`, and `ios`.

> The web build is **game-runtime only** — there is no WebAssembly editor. tvOS is **not** a supported target.

## What's next?

- [Core concepts](/docs/r1-alpha5/getting-started/concepts) — how scenes, entities, and scripts fit together
- [Your first project](/docs/r1-alpha5/getting-started/first-project) — build something in the editor

# Installation

Welcome! Let's get Renzora running on your computer. There are two ways to get it — pick whichever feels easiest, and you'll be in the editor in a few minutes.

Here's what you're installing — the Renzora editor, where you'll build your games:

![The Renzora editor with a 3D city scene open: a scene list on the left, the viewport with move/rotate gizmos in the middle, a component library and Transform panel on the right, and an asset browser along the bottom.](/assets/previews/interface.png)

The two ways to get Renzora:

- **Download a prebuilt build** — the easiest start. No tools to set up; just download, extract, and run.
- **Install the `renzora` CLI** — the way to scaffold and build your own projects. Every build runs inside the engine's Docker toolchain.

> Renzora's canonical build is its pinned Docker toolchain — the container guarantees your `bevy_dylib` and engine build hash match everyone else's, which keeps community plugins ABI-compatible across machines, and it's required for cross-platform builds. A native (no-Docker) build of your own platform is also supported via `cargo renzora`. The editor and game run natively on your GPU either way.

## System requirements

| | Minimum |
|---|---|
| **Windows** | Windows 10+, 64-bit |
| **macOS** | macOS 12 Monterey or newer |
| **Linux** | Ubuntu 22.04+ / Fedora 38+ |
| **GPU** | A GPU with Vulkan, Metal, or DX12 (Renzora renders through `wgpu`) |
| **RAM** | 4 GB minimum, 8 GB recommended |

> Renzora is a Bevy 0.19 engine and uses WebGPU/`wgpu` for rendering. Very old GPUs without a Vulkan/Metal/DX12 backend are not supported.

## Download a prebuilt build (easiest)

Want the quickest start? Grab a prebuilt engine for your platform from the download page — no Docker, no Cargo, no terminal required:

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

## Install the CLI (to build your own projects)

The `renzora` CLI scaffolds projects and runs **every** build inside the pinned `ghcr.io/renzora/*` Docker toolchain images (a shared `base` plus one per platform), so your build environment matches everyone else's — and the plugin ABI. Install it with Cargo:

```bash
cargo install renzora
```

It needs [Docker](https://docs.docker.com/get-docker/) (the toolchain runs in a container) and `git` (for `renzora new`). Rust/Cargo is needed only to install the CLI itself — not to build the engine. Then scaffold and run a project:

```bash
renzora new my-game     # clone the engine into ./my-game
cd my-game
renzora init            # build/pull the toolchain image + container (first run is slow)
renzora run             # build the editor in the container and launch it
```

`renzora run` launches the editor (`renzora run runtime` runs the game shape). Other commands include `renzora build [platforms]`, `renzora test`, `renzora add <name>`, `renzora shell`, and `renzora destroy`. Only the build runs in the container — the GPU editor and game run natively.

> The CLI is the published [`renzora` crate](https://crates.io/crates/renzora). The crate *named* `renzora` inside the engine repo is a different thing (the SDK library), so `cargo install renzora` installs the CLI — not that library.

## Working from a checkout

Contributors and anyone who wants to hack on the engine itself can clone the repo directly and drive it with the same CLI — the build still happens in the container, so the plugin ABI stays consistent:

```bash
git clone https://github.com/renzora/engine.git
cd engine
renzora init            # build/pull the toolchain image + container (first run is slow)
renzora run             # build the editor in the container and launch it
```

The first build takes several minutes (Bevy is large); subsequent builds are incremental. `renzora run runtime` runs the shipped-game shape, `renzora run` (no args) the editor, and `renzora test` / `renzora check` reproduce CI — all inside the container.

### Good to know: one binary, editor as a removable bundle

There is exactly one workspace binary: `renzora` (`renzora.exe` on Windows). The editor is **not** a compile-time feature — it ships as a removable bundle (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`) placed **beside the exe**:

- Bundle present → the binary launches as the **editor**.
- Delete that one file (or pass `--no-editor`) → the same binary is the **shipped game**.

You don't need the deeper details to get started — the cross-compile toolchain and every launch flag are covered in the build reference below and in the Advanced docs.

### Cross-compiling for other platforms

Cross-platform builds run inside the `ghcr.io/renzora/<platform>` toolchain images — the host only needs Docker, and the CLI pulls just the platform(s) you build. The CLI drives it:

```bash
renzora build windows linux
```

`renzora build [platforms...]` (no args = every platform the container can produce) accepts these platform tokens: `windows`, `linux`, `macos`, `wasm` (Web), `android`, and `ios`. Builds land in `dist/<platform>/`.

> The web build is **game-runtime only** — there is no WebAssembly editor. tvOS is **not** a supported target.

## What's next?

- [Core concepts](/docs/r1-alpha5/getting-started/concepts) — how scenes, entities, and scripts fit together
- [Your first project](/docs/r1-alpha5/getting-started/first-project) — build something in the editor

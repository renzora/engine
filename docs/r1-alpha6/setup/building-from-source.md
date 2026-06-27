# Building from a Checkout

Clone the Renzora engine workspace and build the editor, the game runtime, or every export target. There are two supported paths: the `renzora` CLI (Docker — the canonical path, and the only one for **cross-platform** builds), and a **native** `cargo renzora` for building on your own machine for your own platform.

> **Docker vs native.** Renzora's dynamic-plugin system needs the host binary, the editor bundle, and every plugin to share **one** compiled copy of Bevy and the `renzora` SDK. Docker guarantees that by building everyone in the pinned `ghcr.io/renzora/*` images, which is why it's canonical and required for cross-platform/release builds. But the same guarantee holds **natively** as long as you build the host and its plugins from source in one environment — which `cargo renzora` does — and `rust-toolchain.toml` pins the same rustc the images use. So a native build is supported for **your host platform**; it produces its own `bevy_dylib`, which is fine because the host and every plugin in that build share it. (Don't mix a Docker-built host with native-built plugins, or vice versa.) See [Building natively](#building-natively-without-docker) below.

## What you're building

Renzora is a single Bevy 0.19 Cargo workspace with exactly **one binary**: `renzora_app`, which produces `renzora` (`renzora.exe` on Windows) from `src/main.rs`. That one binary is the engine — editor, game runtime, and dedicated server in one. The editor is **not** a compile-time feature: it ships as a removable cdylib, `renzora_editor` (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`), that the binary dlopens from beside itself at startup.

- Bundle present beside the exe → the binary launches as the **editor**.
- Delete that one file (or pass `--no-editor`) → the same binary is the **shipped game**.

> Two things share the name `renzora`. The crate in *this* workspace (`crates/renzora`) is the SDK **library** (`crate-type = ["dylib", "rlib"]`) — installing it produces nothing runnable. The **`renzora` CLI** (`cargo install renzora`, a separate published crate) scaffolds projects and drives the Docker image below; most people start with it (`renzora new` → `renzora run`). This page covers building a checkout you already have with that same CLI.

## Prerequisites

- **Docker** — the toolchain runs in a container. Install [Docker](https://docs.docker.com/get-docker/) and make sure the daemon is running.
- **Git** — to clone the engine.
- **Rust** — only to install the CLI (`cargo install renzora`); it is **not** used to build the engine. Install via [rustup](https://rustup.rs) if you don't have it.

> For the **container** build you do **not** install a Rust toolchain, a C/C++ toolchain, or any Bevy system libraries — they're all baked into the Docker image. (A native `cargo renzora` build does need them on your machine; see [Building natively](#building-natively-without-docker).) The pinned Rust version lives in two lockstep files: `docker/base/Dockerfile` (`FROM rust:1.95.0-bookworm`, container) and `rust-toolchain.toml` (native).

## Clone and run

```bash
git clone https://github.com/renzora/engine.git
cd engine
renzora init         # build/pull the toolchain image + container (first run is slow)
renzora run          # build the workspace in the container and run the editor
```

The first build takes several minutes (Bevy is large); subsequent builds are incremental.

## Building natively (without Docker)

If you'd rather build and run on your own machine for your own platform — no Docker — use `cargo renzora`. It's the local mirror of what the container does:

```bash
git clone https://github.com/renzora/engine.git
cd engine
cargo renzora           # build the workspace, stage dist/, and launch the editor
```

That's the whole flow. You need only **Git** and **rustup** — no Docker, no `cargo install`. On first run, `rust-toolchain.toml` makes rustup auto-install and select the exact pinned Rust version (the same one the images use), so you compile with the same compiler everyone else does. You will need your platform's usual native build dependencies (a C/C++ toolchain; on Linux also the X11/Wayland/ALSA/udev dev headers — the list mirrors `docker/base/Dockerfile`).

| Command | What it does |
|---|---|
| `cargo renzora` | Build the workspace, stage `dist/<platform>/`, and **launch** the editor |
| `cargo renzora dist` | Same build + stage, but **don't** launch — just produce the folder |
| `cargo renzora -- --no-editor` | Build + stage + launch in shipped-game mode (args after the binary are forwarded) |

**Why not just `cargo run`?** A bare `cargo run` compiles everything but leaves the distribution plugin cdylibs (`renzora_lumen`, `renzora_cloth`, …) flat in `target/dist/`, while the dynamic loader looks for them in `<exe-dir>/plugins/`. So those plugins build but never load. `cargo renzora` adds the one missing step — staging the artifacts into the runnable `dist/` layout (`bevy_dylib`, `renzora`, and the editor bundle beside the exe; every other plugin cdylib in `plugins/`), exactly like the container's `build-all.sh`. The staging lives in the `xtask/` crate and runs on plain `cargo` (the `renzora` cargo alias points at it).

**What native does *not* do:** cross-compile. `cargo renzora` only ever produces artifacts for the machine it runs on. For Windows/macOS/Linux/wasm/mobile builds from one host, use Docker (`renzora build`, below).

### The `renzora` CLI — the build interface

The CLI is the canonical way to build and run a checkout. Each command runs `cargo` inside the container against the custom `dist` profile (`inherits = "release"`, `opt-level = 2`, `strip = "symbols"`) so that plugin ABI hashes stay consistent across every build and machine.

| Command | What it builds / runs |
|---|---|
| `renzora run` | Build the workspace and run the **editor** |
| `renzora run runtime` | Run the **shipped-game** shape (same binary, `--no-editor`) |
| `renzora run -- --server` | Run a headless **dedicated server** |
| `renzora build [platforms...]` | Cross-build the binary + editor bundle + shared `bevy_dylib` (no args = all platforms) |
| `renzora add <name> [--editor\|--dylib]` | Scaffold a plugin crate |
| `renzora remove <name>` | Delete a plugin crate and unregister it |
| `renzora test` / `renzora check` | Reproduce the CI test + clippy jobs |
| `renzora shell` | Open a shell inside the build container |

> Under the hood the CLI maps to `cargo` invocations on the `dist` profile inside the container — e.g. the editor is `run --profile dist --workspace --bin renzora`, the lean runtime is `build --profile dist --bin renzora` (deliberately **not** `--workspace`, so editor-only crates and distribution plugins never enter the build graph). You never run these natively; the CLI runs them in the image for you.

### Runtime modes

The same binary picks its mode at launch by flag — there are no separate "editor" / "server" builds:

| Flag | Mode |
|---|---|
| *(none)* | Editor if the bundle dll is present, otherwise the shipped game |
| `--no-editor` (or `RENZORA_NO_EDITOR`) | Force the shipped-game runtime |
| `--server` | Headless **dedicated server** (no window, no GPU) |
| `--host` | Windowed **listen server** (client + server in one process; **wins over `--server`**) |

A `--server`/`--host` launch is never an editor session even if the bundle dll is present. Server flags `--port`, `--addr`/`--address`, `--tick-rate`, and `--max-clients` overlay the project's `[network]` settings; `--project <path>` and `--rpak <path>` are also recognized. The dedicated server is the same `renzora` binary, not a separate executable.

## How the shared-library build works

Renzora's dynamic-plugin system requires that the host binary, the dlopened editor bundle, and any distribution plugins all share **one compiled copy** of Bevy and of the `renzora` SDK so their `TypeId`s match across the dlopen boundary. `.cargo/config.toml` arranges this with `-C prefer-dynamic` plus `bevy/dynamic_linking`:

- `bevy` ships as a single `bevy_dylib-<hash>` shared library.
- `renzora` ships as a single `renzora.dll` / `librenzora.so` / `librenzora.dylib` (it folds in the post-process framework and the editor contract).
- Workspace plugins are plain **rlibs** statically linked into the binary; **distribution plugins** are cdylibs loaded at runtime from `plugins/`.

Because the binary links these by name, the `.dll`/`.so`/`.dylib` files must travel **beside** the binary (Linux/macOS use an rpath of `$ORIGIN` / `@loader_path`; on Windows they sit in the same folder).

> Windows uses `rust-lld` as the linker (MSVC `link.exe` hits the 65535-object limit on `bevy_dylib`). `crt-static` is intentionally **disabled** because it changes crate disambiguators and would break `TypeId` equality across the dylib boundary. This is one more reason the build is container-only — the linker setup is fixed inside the image.

### build.rs

The root `build.rs` emits two environment values used by the dynamic-plugin ABI guard:

- `RENZORA_ENGINE_VERSION` — the package version.
- `RENZORA_BUILD_HASH` — an FNV-1a hash of `"<version>-<rustc version>-bevy0.19"`. The loader rejects any plugin whose hash differs, so a plugin built against a different compiler or engine version is refused rather than crashing. (Building everyone in the same image is what keeps this hash equal across machines.)

It also embeds the Windows icon/version resource (via `winres` on a Windows host, or a hand-written `.rc` + `llvm-rc` when cross-compiling Linux→Windows-MSVC) and re-emits the static `zstd` link directive.

## Cross-compiling for other platforms

Every cross-platform target builds inside the engine's Docker toolchain, split into a shared base plus one image per platform (all under **`ghcr.io/renzora/*`**). The base (`docker/base/Dockerfile`, `FROM rust:1.95.0-bookworm`) is the single source of truth for the Rust version and carries the linux-gnu targets + `mold`/`clang`/`lld` linkers + LLVM-19; each platform image builds `FROM` it and adds its cross toolchain — `windows` (xwin/MSVC), `macos` & `ios` (osxcross + SDKs), `android` (NDK r27c), `wasm` (`wasm-bindgen` + `binaryen`), `linux` (dual-arch cross-gcc + `appimagetool` + UPX). The host only needs Docker, and the CLI pulls just the images a command needs — the GPU editor/game still runs natively from `dist/`.

```bash
# Build specific platforms into ./dist
renzora build windows linux

# Build everything the container can produce
renzora build
```

`renzora build [platform ...]` (no args = all) accepts these platform tokens:

| Token | Output directory |
|---|---|
| `linux` | `dist/linux-x64/` |
| `windows` | `dist/windows-x64/` |
| `macos` (= `macos-x64` + `macos-arm64`) | `dist/macos-x64/`, `dist/macos-arm64/` |
| `macos-x64` / `macos-arm64` | the individual macOS dir |
| `wasm` | `dist/web-wasm32/` |
| `android` (= `android-arm64` + `android-x86`) | `dist/android-arm64/`, `dist/android-x86/` |
| `ios` | `dist/ios-arm64/` |

Desktop targets place the binary and its shared libraries directly in the platform dir; wasm and mobile targets nest their output under a `runtime/` subdirectory.

> Notes: macOS lanes build only when osxcross is present; the Android and iOS lanes are **best-effort** (a failure there does not fail the whole build). The web build is **game-runtime only** — there is no WebAssembly editor (the binary has no `editor` compile feature, and the editor bundle is a desktop dlopen target). On Linux the editor build is additionally wrapped into an AppDir and `.AppImage` when `appimagetool` is available.

### Plugin scaffolding

The CLI creates and removes plugin crates with the right `Cargo.toml` wiring:

```bash
renzora add cool_fx              # statically-linked engine plugin (Runtime scope)
renzora add cool_fx --editor     # editor-only plugin (Editor scope, optional dep)
renzora add cool_fx --dylib      # distribution plugin (standalone cdylib, dlopen)
renzora remove cool_fx           # delete the crate and unregister it
```

`--editor` and `--dylib` are mutually exclusive. A default (no-flag) plugin builds as an rlib baked into the host binary and self-registers via its inventory constructor; `--dylib` adds the `dlopen` feature so the plugin emits the FFI exports the dynamic loader needs.

### Compressing binaries with UPX

```bash
renzora upx                      # compress every platform under dist/
renzora upx dist/windows-x64     # just one platform
```

This runs `upx --brute` (slowest, smallest) over the host binary, the SDK dylibs (`renzora`, `renzora_editor`), `bevy_dylib`, and everything in `plugins/`. The `wasm` and `ios` outputs (`.wasm` / `.a`) are not UPX-compressible and are skipped.

## What is NOT in this repo

A few things that live outside this workspace, or that older docs got wrong:

| Often referenced | Reality |
|---|---|
| The `renzora` CLI source | The CLI (`cargo install renzora`) is real, but its **source is a separate published crate**, not this workspace. Its commands drive the `docker/` toolchain: `build`/`add`/`remove`/`upx` wrap the `docker/*.sh` scripts here; `new`/`init`/`run`/`test`/`check`/`shell`/`clean`/`destroy` are CLI-level (container lifecycle + cargo wrappers that run *inside* the image). |
| A native `cargo run` / `cargo build` build | A bare `cargo run` works but silently skips the dlopen distribution plugins (they're built but not staged into `plugins/`). For a complete native build use **`cargo renzora`** (see [Building natively](#building-natively-without-docker)); for cross-platform use `renzora build`. |
| `rust-toolchain.toml` | **Exists** — it pins the Rust version for native builds. The container's version lives in `docker/base/Dockerfile`; the two are kept in lockstep. |
| `Makefile.toml` / `cargo-make` (`makers ...`) | No `Makefile.toml` / `cargo-make` — the old `makers` staging was replaced by the `xtask` crate behind `cargo renzora` (no extra install). Cross-platform/release still go through the `renzora` CLI + `docker/` scripts. |
| A separate dedicated-server binary | Gone — the server is the same `renzora` binary launched with `--server`. |
| An `editor` compile-time feature / separate editor binary | Removed — the editor is the removable `renzora_editor` cdylib bundle. |
| tvOS / Apple TV target | Aspirational only — no tvOS toolchain in the image and no `build-all.sh` lane. |

## What's next?

- [Project Structure](/docs/r1-alpha5/setup/project-structure) — how the workspace and its ~187 crates are laid out
- [Architecture](/docs/r1-alpha5/setup/architecture) — the one-binary, editor-as-removable-cdylib model in depth
- [Building Plugins](/docs/r1-alpha5/extending/plugins) — extend the engine with `renzora::add!`
- [Building Export Templates](/docs/r1-alpha5/packaging/export-templates) — produce shippable game builds

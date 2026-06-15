# Building from a Checkout

Clone the Renzora engine workspace and build the editor, the game runtime, or every export target — all through the `renzora` CLI, which runs every build inside the pinned Docker toolchain.

> **Why the container, always.** Renzora's dynamic-plugin system needs the host binary, the editor bundle, and every plugin to share **one** compiled copy of Bevy and the `renzora` SDK. A native `cargo` build on your own machine produces a different `bevy_dylib` and engine build hash, so plugins built elsewhere would be refused. Building in the pinned `ghcr.io/renzora/engine` image guarantees everyone's hash matches — so there is **no supported native build**. The GPU editor and game still *run* natively from `dist/`; only the *build* happens in the container.

## What you're building

Renzora is a single Bevy 0.18 Cargo workspace with exactly **one binary**: `renzora_app`, which produces `renzora` (`renzora.exe` on Windows) from `src/main.rs`. That one binary is the engine — editor, game runtime, and dedicated server in one. The editor is **not** a compile-time feature: it ships as a removable cdylib, `renzora_editor` (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`), that the binary dlopens from beside itself at startup.

- Bundle present beside the exe → the binary launches as the **editor**.
- Delete that one file (or pass `--no-editor`) → the same binary is the **shipped game**.

> Two things share the name `renzora`. The crate in *this* workspace (`crates/renzora`) is the SDK **library** (`crate-type = ["dylib", "rlib"]`) — installing it produces nothing runnable. The **`renzora` CLI** (`cargo install renzora`, a separate published crate) scaffolds projects and drives the Docker image below; most people start with it (`renzora new` → `renzora run`). This page covers building a checkout you already have with that same CLI.

## Prerequisites

- **Docker** — the toolchain runs in a container. Install [Docker](https://docs.docker.com/get-docker/) and make sure the daemon is running.
- **Git** — to clone the engine.
- **Rust** — only to install the CLI (`cargo install renzora`); it is **not** used to build the engine. Install via [rustup](https://rustup.rs) if you don't have it.

> You do **not** install a Rust toolchain, a C/C++ toolchain, or any Bevy system libraries for the build — they're all baked into the Docker image. The pinned Rust version lives in **one place only**, `docker/Dockerfile` (`FROM rust:1.93.0-bookworm`); there is **no `rust-toolchain.toml`**.

## Clone and run

```bash
git clone https://github.com/renzora/engine.git
cd engine
renzora init         # build/pull the toolchain image + container (first run is slow)
renzora run          # build the workspace in the container and run the editor
```

The first build takes several minutes (Bevy is large); subsequent builds are incremental.

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
- `RENZORA_BUILD_HASH` — an FNV-1a hash of `"<version>-<rustc version>-bevy0.18"`. The loader rejects any plugin whose hash differs, so a plugin built against a different compiler or engine version is refused rather than crashing. (Building everyone in the same image is what keeps this hash equal across machines.)

It also embeds the Windows icon/version resource (via `winres` on a Windows host, or a hand-written `.rc` + `llvm-rc` when cross-compiling Linux→Windows-MSVC) and re-emits the static `zstd` link directive.

## Cross-compiling for other platforms

Every cross-platform target builds inside the engine's Docker image, **`ghcr.io/renzora/engine`** (`docker/Dockerfile`, `FROM rust:1.93.0-bookworm`). That image is the single source of truth for the Rust version and bundles every cross toolchain: rustup targets for linux-gnu, windows-msvc (via xwin), wasm32, Android arm64/x86_64, and Apple darwin/iOS; the `mold`/`clang`/`lld`/`lld-link` linkers; osxcross (macOS + iOS SDKs); the Android NDK r27c; `wasm-bindgen`; `binaryen` (`wasm-opt`); UPX; and `appimagetool`. The host only needs Docker — the GPU editor/game still runs natively from `dist/`.

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
| `wasm` | `dist/web-wasm32/runtime/` |
| `android` (= `android-arm64` + `android-x86`) | `dist/android-arm64/runtime/`, `dist/android-x86/runtime/` |
| `ios` | `dist/ios-arm64/runtime/` |

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
| A native `cargo run` / `cargo build` build | Not supported — every build runs in the container so the plugin ABI stays consistent. Use `renzora run` / `renzora build`. |
| `rust-toolchain.toml` | Does not exist — the Rust version lives only in `docker/Dockerfile`. |
| `Makefile.toml` / `cargo-make` (`makers ...`) | Referenced in some comments but no `Makefile.toml` exists; use the `renzora` CLI and `docker/` scripts. |
| A separate dedicated-server binary | Gone — the server is the same `renzora` binary launched with `--server`. |
| An `editor` compile-time feature / separate editor binary | Removed — the editor is the removable `renzora_editor` cdylib bundle. |
| tvOS / Apple TV target | Aspirational only — no tvOS toolchain in the image and no `build-all.sh` lane. |

## What's next?

- [Project Structure](/docs/r1-alpha5/setup/project-structure) — how the workspace and its ~187 crates are laid out
- [Architecture](/docs/r1-alpha5/setup/architecture) — the one-binary, editor-as-removable-cdylib model in depth
- [Building Plugins](/docs/r1-alpha5/extending/plugins) — extend the engine with `renzora::add!`
- [Building Export Templates](/docs/r1-alpha5/packaging/export-templates) — produce shippable game builds

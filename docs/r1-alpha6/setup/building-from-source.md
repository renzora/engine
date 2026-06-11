# Building from Source

Clone the Renzora engine workspace and build the editor, the game runtime, or every export target from one Cargo workspace.

## What you're building

Renzora is a single Bevy 0.18 Cargo workspace with exactly **one binary**: `renzora_app`, which produces `renzora` (`renzora.exe` on Windows) from `src/main.rs`. That one binary is the engine — editor, game runtime, and dedicated server in one. The editor is **not** a compile-time feature: it ships as a removable cdylib, `renzora_editor` (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`), that the binary dlopens from beside itself at startup.

- Bundle present beside the exe → the binary launches as the **editor**.
- Delete that one file (or pass `--no-editor`) → the same binary is the **shipped game**.

> Two things share the name `renzora`. The crate in *this* workspace (`crates/renzora`) is the SDK **library** (`crate-type = ["dylib", "rlib"]`) — installing it produces nothing runnable. The **`renzora` CLI** (`cargo install renzora`, a separate published crate) scaffolds projects and drives the Docker image below; most people start with it (`renzora new` → `renzora run`). This page covers building a checkout **directly** with the cargo aliases — which is exactly what the CLI runs under the hood.

## Prerequisites

- **Rust** — install via [rustup](https://rustup.rs). The toolchain version lives in **one place only**: `docker/Dockerfile` (`FROM rust:1.93.0-bookworm`). There is **no `rust-toolchain.toml`**, so for a native build match that version with `rustup default 1.93.0`.
- **Git**
- **A C/C++ toolchain** — MSVC Build Tools (with "Desktop development with C++") on Windows, Xcode Command Line Tools (`xcode-select --install`) on macOS, GCC/Clang on Linux.

**Linux (Ubuntu/Debian)** also needs the usual Bevy system libraries:

```bash
sudo apt install build-essential pkg-config libasound2-dev libudev-dev \
    libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libxcursor-dev \
    libxrandr-dev libxinerama-dev libvulkan-dev libssl-dev
```

> For cross-compiling to *other* platforms you do **not** need any of these toolchains locally — that all happens inside the Docker image (see [Cross-compiling with Docker](#cross-compiling-with-docker)). A native checkout only needs Rust + Git + your own platform's C/C++ toolchain.

## Clone and run

```bash
git clone https://github.com/renzora/engine.git
cd engine
cargo renzora        # build the workspace and run the editor
```

The first build takes several minutes (Bevy is large); subsequent builds are incremental.

### Cargo aliases — the real local interface

The repo ships `.cargo/config.toml` aliases that are the canonical way to build and run locally. Everything uses the custom `dist` profile (`inherits = "release"`, `opt-level = 2`, `strip = "symbols"`) so that plugin ABI hashes stay consistent across builds.

| Alias | Expands to | Result |
|---|---|---|
| `cargo renzora` | `run --profile dist --workspace --bin renzora` | Build the workspace and run the **editor** |
| `cargo runtime` | `run --profile dist --bin renzora -- --no-editor` | Run the **shipped-game** shape (same binary, no editor) |
| `cargo server` | `run --profile dist --bin renzora -- --server` | Run a headless **dedicated server** |
| `cargo build-editor` | `build --profile dist --workspace` | Build binary + editor bundle + shared `bevy_dylib` |
| `cargo build-all` | `build --profile dist --workspace` | Same as `build-editor` (binary + bundle + distribution plugins) |
| `cargo build-runtime` | `build --profile dist --bin renzora` | Build the **lean game binary only** — note: **not** `--workspace` |
| `cargo build-plugins` | `build --profile dist --workspace` | Build all workspace crates incl. distribution-plugin cdylibs |
| `cargo build-web` | `build --profile dist --bin renzora --no-default-features --features wasm --target wasm32-unknown-unknown` | WASM **game runtime** (no wasm editor) |
| `cargo build-android` | `build --profile dist --no-default-features --target aarch64-linux-android` | Android arm64 runtime |
| `cargo build-ios` / `build-ios-sim` | `build --profile dist --target aarch64-apple-ios[-sim] -p renzora-ios` | iOS staticlib |

> `cargo build-runtime` deliberately drops `--workspace`: it compiles only the binary's own dependency tree, so editor-only crates and distribution plugins never enter the build graph. Only `build-editor`/`build-all` use `--workspace` to also emit the editor bundle and the dlopen plugins. Local aliases all build into a single `target/dist/` directory.

> `cargo build-tvos` / `build-tvos-sim` aliases also exist (targeting `aarch64-apple-tvos`), but tvOS is **aspirational** — the Docker toolchain installs no tvOS target and `build-all.sh` has no tvOS lane, so these aliases cannot actually build today.

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

> Windows uses `rust-lld` as the linker (MSVC `link.exe` hits the 65535-object limit on `bevy_dylib`). `crt-static` is intentionally **disabled** because it changes crate disambiguators and would break `TypeId` equality across the dylib boundary.

### build.rs

The root `build.rs` emits two environment values used by the dynamic-plugin ABI guard:

- `RENZORA_ENGINE_VERSION` — the package version.
- `RENZORA_BUILD_HASH` — an FNV-1a hash of `"<version>-<rustc version>-bevy0.18"`. The loader rejects any plugin whose hash differs, so a plugin built against a different compiler or engine version is refused rather than crashing.

It also embeds the Windows icon/version resource (via `winres` on a Windows host, or a hand-written `.rc` + `llvm-rc` when cross-compiling Linux→Windows-MSVC) and re-emits the static `zstd` link directive.

## Cross-compiling with Docker

Every cross-platform target builds inside the engine's Docker image, **`ghcr.io/renzora/engine`** (`docker/Dockerfile`, `FROM rust:1.93.0-bookworm`). That image is the single source of truth for the Rust version and bundles every cross toolchain: rustup targets for linux-gnu, windows-msvc (via xwin), wasm32, Android arm64/x86_64, and Apple darwin/iOS; the `mold`/`clang`/`lld`/`lld-link` linkers; osxcross (macOS + iOS SDKs); the Android NDK r27c; `wasm-bindgen`; `binaryen` (`wasm-opt`); UPX; and `appimagetool`. The host only needs Docker — the GPU editor/game still runs natively from `dist/`.

```bash
# Build specific platforms into ./dist
docker/build-all.sh dist windows linux

# Build everything the container can produce
docker/build-all.sh dist
```

`docker/build-all.sh <output-dir> [platform ...]` accepts these platform tokens:

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

### Plugin scaffolding scripts

Two helper scripts under `docker/` create and remove plugin crates with the right `Cargo.toml` wiring:

```bash
docker/add-plugin.sh cool_fx            # statically-linked engine plugin (Runtime scope)
docker/add-plugin.sh cool_fx --editor   # editor-only plugin (Editor scope, optional dep)
docker/add-plugin.sh cool_fx --dylib    # distribution plugin (standalone cdylib, dlopen)
docker/remove-plugin.sh cool_fx         # delete the crate and unregister it
```

`--editor` and `--dylib` are mutually exclusive. A default (no-flag) plugin builds as an rlib baked into the host binary and self-registers via its inventory constructor; `--dylib` adds the `dlopen` feature so the plugin emits the FFI exports the dynamic loader needs.

### Compressing binaries with UPX

```bash
docker/upx-compress.sh                  # compress every platform under dist/
docker/upx-compress.sh dist/windows-x64 # just one platform
```

This runs `upx --brute` (slowest, smallest) over the host binary, the SDK dylibs (`renzora`, `renzora_editor`), `bevy_dylib`, and everything in `plugins/`. The `wasm` and `ios` outputs (`.wasm` / `.a`) are not UPX-compressible and are skipped.

## What is NOT in this repo

A few things that live outside this workspace, or that older docs got wrong:

| Often referenced | Reality |
|---|---|
| The `renzora` CLI source | The CLI (`cargo install renzora`) is real, but its **source is a separate published crate**, not this workspace. Its commands are genuine: `build`/`add`/`remove`/`upx` wrap the `docker/*.sh` scripts here; `new`/`init`/`run`/`test`/`check`/`shell`/`clean`/`destroy` are CLI-level (cargo wrappers + container lifecycle). |
| `rust-toolchain.toml` | Does not exist — the Rust version lives only in `docker/Dockerfile`. |
| `Makefile.toml` / `cargo-make` (`makers ...`) | Referenced in some comments but no `Makefile.toml` exists; use the `cargo` aliases and `docker/` scripts. |
| A separate dedicated-server binary | Gone — the server is the same `renzora` binary launched with `--server`. |
| An `editor` compile-time feature / separate editor binary | Removed — the editor is the removable `renzora_editor` cdylib bundle. |
| tvOS / Apple TV target | Aspirational only — no tvOS toolchain in the image and no `build-all.sh` lane. |

## What's next?

- [Project Structure](/docs/r1-alpha5/setup/project-structure) — how the workspace and its ~187 crates are laid out
- [Architecture](/docs/r1-alpha5/setup/architecture) — the one-binary, editor-as-removable-cdylib model in depth
- [Building Plugins](/docs/r1-alpha5/extending/plugins) — extend the engine with `renzora::add!`
- [Building Export Templates](/docs/r1-alpha5/packaging/export-templates) — produce shippable game builds

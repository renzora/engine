# Export: Windows

Cross-compile your game to a native Windows x64 (`x86_64-pc-windows-msvc`) build from any machine with Docker — no Windows or Visual Studio required.

## How Windows builds work

Renzora is one binary. The Windows target is the same `renzora` binary as every other platform, compiled for the MSVC ABI and emitted to `dist/windows-x64/`. There are two ways to produce it:

- **Cross-compile in the Docker image (recommended).** The engine's builder image carries a full Linux→Windows-MSVC toolchain (xwin + `lld-link` + `clang-cl`), so you can build a Windows `.exe` from Linux, macOS, or Windows.
- **Build natively on a Windows host** with the MSVC C++ Build Tools and a matching Rust toolchain.

> The editor is not a compile-time variant. The same `renzora.exe` is the editor when `renzora_editor.dll` sits beside it, and the shipped game when that one file is removed (or you pass `--no-editor`). "Exporting" a Windows game is really "build the binary, then drop the editor bundle." See [Building from Source](/docs/r1-alpha5/setup/building-from-source) for the full one-binary model.

## Building with Docker

Every cross-platform target builds inside `ghcr.io/renzora/engine` (`docker/Dockerfile`, `FROM rust:1.93.0-bookworm`). The image bundles xwin (which splats Microsoft's redistributable MSVC SDK + CRT at image-build time), `lld-link`, `clang-cl`, and the `x86_64-pc-windows-msvc` rustup target. The host only needs Docker:

```bash
docker/build-all.sh dist windows
```

`build-all.sh <output-dir> [platforms...]` runs inside the container and writes the Windows build to `dist/windows-x64/`. Pass several tokens to build more than one platform in one run:

```bash
docker/build-all.sh dist windows linux
```

> `renzora build windows` (the CLI) derives the image tag, pulls it, and runs this same `docker/build-all.sh` step in the container. From a checkout you can call `docker/build-all.sh` or the `cargo` aliases below directly instead.

## Output layout

A Windows build produces `dist/windows-x64/` containing the engine binary and the shared libraries it links at runtime:

```
dist/windows-x64/
├── renzora.exe              # the engine binary (editor + runtime + server)
├── renzora.dll              # shared SDK contract (one copy for exe + plugins)
├── renzora_editor.dll       # the editor bundle — DELETE this to ship the game
├── bevy_dylib-<hash>.dll    # shared Bevy (dynamic_linking)
├── std-<hash>.dll           # Rust standard library (prefer-dynamic)
└── plugins/
    └── *.dll                # distribution plugins (rendering, GI, etc.)
```

`build-all.sh` only emits executables and libraries — it does not pack your assets (see [Packaging assets](#packaging-assets)).

### Editor vs. shipped game

The directory above is the **editor**. To turn it into a distributable game, remove the editor bundle:

```bash
rm dist/windows-x64/renzora_editor.dll
```

The remaining `renzora.exe` now launches straight into your game. Everything else (`renzora.dll`, `bevy_dylib-*.dll`, `std-*.dll`, `plugins/`) must stay beside the exe.

> You can also keep the bundle and launch the same exe in game mode for testing with `renzora.exe --no-editor`. The dedicated server is this binary too: `renzora.exe --server` (headless) or `renzora.exe --host` (windowed listen server).

## Building natively on Windows

If you are on Windows and want a local build without Docker, install:

- **MSVC C++ Build Tools** (Visual Studio Build Tools with "Desktop development with C++"), which provides the linker and Windows SDK.
- **Rust** via [rustup](https://rustup.rs). The toolchain is pinned in `docker/Dockerfile` (currently **Rust 1.93.0**) — there is no `rust-toolchain.toml`, so match that version for a clean build.

Then use the workspace `cargo` aliases from `.cargo/config.toml`:

```bash
cargo build-all     # build --profile dist --workspace (binary + editor bundle + plugins)
cargo renzora       # build the workspace and launch the editor
cargo build-runtime # build --profile dist --bin renzora (lean game binary, no bundle)
```

The native build uses the same MSVC target settings the container does (see below), so the on-disk output matches `dist/windows-x64/`.

## The Windows toolchain

These details are fixed by the engine's build config; there is no per-export options dialog.

| Setting | Value | Where |
|---|---|---|
| Target triple | `x86_64-pc-windows-msvc` | `docker/build-all.sh`, Dockerfile rustup target |
| Linker | `rust-lld` (host) / `lld-link` (container) | `.cargo/config.toml`, Dockerfile |
| C compiler (for C deps) | `clang-cl` with xwin include paths | Dockerfile |
| MSVC SDK + CRT | downloaded by **xwin** at image build | Dockerfile |
| Build profile | `dist` (`inherits = "release"`, `opt-level = 2`, `strip = "symbols"`) | root `Cargo.toml` |

A few consequences worth knowing:

- **MSVC `link.exe` is not used.** `rust-lld` (rustc's bundled LLD) avoids LNK1189 — MSVC `link.exe` overflows its 65535-object limit on `bevy_dylib` built with `dynamic_linking`.
- **The CRT is linked dynamically.** `crt-static` is deliberately **off**, because static CRT linking changes crate disambiguators and breaks the `TypeId` matching the dynamic-plugin system relies on. The exe therefore depends on `vcruntime140.dll` / `msvcp140.dll`, which Windows 10/11 ship by default (or via the [Visual C++ Redistributable](https://learn.microsoft.com/cpp/windows/latest-supported-vc-redist)).
- **Shared libraries ship beside the exe.** `prefer-dynamic` plus `bevy/dynamic_linking` give one shared `bevy_dylib` and one `renzora.dll`, plus a `std-<hash>.dll`. All of them, and the `plugins/` folder, must travel with `renzora.exe`.

### Icon and version metadata

`build.rs` embeds the executable icon and version resource automatically. On a Windows host it uses `winres` with `icon.ico` from the repo root; when cross-compiling from Linux it hand-writes a `.rc` file and compiles it with `llvm-rc`. The version string comes from `CARGO_PKG_VERSION`. The same build also emits `RENZORA_ENGINE_VERSION` and an FNV-1a `RENZORA_BUILD_HASH` (version + rustc + `bevy0.18`) used to reject ABI-incompatible dynamic plugins at load time.

## Packaging assets

The exported binary finds assets through the engine's VFS, which resolves each load path in this order:

1. `--rpak <path>` override
2. an `.rpak` archive embedded in the exe
3. an adjacent `renzora.rpak` (named after the exe stem)
4. an adjacent `assets/` directory
5. the current working directory

So to ship a game, place either an `assets/` folder or a packed `renzora.rpak` next to `renzora.exe`. `.rpak` is Renzora's own archive format (per-entry Stored or Zstd compression); see the asset and packaging docs for producing one.

## Compressing with UPX (optional)

The builder image ships UPX. To shrink the exe and all the shared libraries in a built directory:

```bash
docker/upx-compress.sh windows        # compresses dist/windows-x64/
docker/upx-compress.sh dist/windows-x64
```

It packs `renzora.exe`, `renzora.dll`, `renzora_editor.dll`, the hashed `bevy_dylib`, and everything in `plugins/` with `upx --brute` (slow, smallest output).

> Some antivirus engines flag UPX-packed executables. If that is a concern for distribution, skip compression or sign the binaries afterward.

## Code signing (optional)

Signing avoids "Unknown publisher" SmartScreen warnings. With a code-signing certificate and `signtool.exe` from the Windows SDK:

```bash
signtool sign /f cert.pfx /p <password> /tr http://timestamp.digicert.com /td sha256 /fd sha256 renzora.exe
```

## Distribution

Zip the `dist/windows-x64/` folder (with `renzora_editor.dll` removed for a shipped game) and distribute it — there is no installer to build, and the game is a single self-contained folder. Keep every `.dll`, the `plugins/` directory, and your assets together with `renzora.exe`.

## Troubleshooting

| Issue | Cause / fix |
|---|---|
| `bevy_dylib-*.dll` / `renzora.dll` / `std-*.dll` not found at launch | A shared library was separated from the exe. Keep the whole `dist/windows-x64/` contents (and `plugins/`) together. |
| The game opens the editor instead | `renzora_editor.dll` is still present. Delete it, or launch with `--no-editor`. |
| `VCRUNTIME140.dll` / `MSVCP140.dll` missing | Install the [Visual C++ Redistributable](https://learn.microsoft.com/cpp/windows/latest-supported-vc-redist) on the target machine. |
| Black screen on launch | No DX12/Vulkan-capable GPU or out-of-date drivers — Renzora renders through `wgpu`. Update GPU drivers. |
| Antivirus blocks the exe | Most common with UPX-packed builds; code-sign the executable or distribute uncompressed. |
| Slow first launch | Normal — shaders compile on first run and are cached for later launches. |

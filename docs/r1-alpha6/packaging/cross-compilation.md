# Cross-Compilation

Every Renzora target is cross-compiled inside one Docker image, so a single Linux container can produce Windows, macOS, iOS, Android, and WebAssembly builds — your host only needs Docker.

## One container, every target

Renzora does **not** use `cross`, `mingw`, or per-host linker juggling. All cross builds happen inside the engine's prebuilt image, **`ghcr.io/renzora/engine`** (`docker/Dockerfile`, `FROM rust:1.93.0-bookworm`). That image bundles every cross toolchain — the rustup targets, the linkers, and the platform SDKs — so the host only needs Docker installed. The GPU editor and game still run **natively** from the `dist/` output; only the *build* happens in the container.

The Dockerfile is also the **single source of truth for the Rust version** — there is no `rust-toolchain.toml` in the repo. Bumping the compiler means editing the `FROM rust:1.93.0-bookworm` line (which changes the image's content hash, so the external CLI re-pulls the new image).

> Ignore older guides that mention `cargo install cross`, `gcc-mingw-w64`, `aarch64-apple-ios-sim` via Xcode, or hand-editing `~/.cargo/config.toml` per target. None of that applies — the container already contains a configured linker and SDK for every supported triple.

## Supported targets

| Platform | Rust target | Toolchain | Linker |
|---|---|---|---|
| Linux x64 | `x86_64-unknown-linux-gnu` | native (container) | `clang` + `mold` |
| Windows x64 | `x86_64-pc-windows-msvc` | **xwin** (MSVC SDK + CRT) | `rust-lld` (LLVM `lld`) |
| macOS x64 | `x86_64-apple-darwin` | **osxcross** | osxcross darwin `clang` |
| macOS ARM64 | `aarch64-apple-darwin` | **osxcross** | osxcross darwin `clang` |
| iOS ARM64 | `aarch64-apple-ios` (+ `-sim`) | **osxcross** iPhoneOS SDK | `clang-19` wrapper |
| Android ARM64 | `aarch64-linux-android` | **Android NDK r27c** | NDK `android33-clang` |
| Android x86_64 | `x86_64-linux-android` | **Android NDK r27c** | NDK `android33-clang` |
| Web (WASM) | `wasm32-unknown-unknown` | `wasm-bindgen` + `binaryen` | (none — `wasm-bindgen`) |

> **tvOS / Apple TV is not a supported target.** The image installs no `aarch64-apple-tvos` rustup target and `docker/build-all.sh` has no tvOS lane. Orphan `cargo build-tvos` / `build-tvos-sim` aliases exist in `.cargo/config.toml`, but the container cannot build them — tvOS is aspirational, not shippable.

## The bundled toolchains

The image builds each toolchain at image-build time so it is fully self-contained. The C/C++ compiler triples (used by `cc`-style build scripts) are exported from `/etc/osxcross-env.sh`, which `docker/build-all.sh` sources before any build.

### Windows — xwin

Windows is built for the **MSVC ABI** (`x86_64-pc-windows-msvc`), not GNU/mingw. [`xwin`](https://github.com/Jake-Shadle/xwin) (v0.6.5) downloads the redistributable Visual Studio Build Tools CRT + Windows SDK into `/xwin` at image-build time. Combined with rustc's MSVC support, `clang-cl`, `lld-link`, and `llvm-lib` (all from LLVM 19), this gives a fully Linux→Windows-MSVC pipeline.

```toml
# container linker config (generated in the image)
[target.x86_64-pc-windows-msvc]
rustflags = [
    "-Lnative=/xwin/crt/lib/x86_64",
    "-Lnative=/xwin/sdk/lib/um/x86_64",
    "-Lnative=/xwin/sdk/lib/ucrt/x86_64",
]
```

> The repo pins the Windows linker to `rust-lld` (rustc's bundled `lld`) rather than MSVC `link.exe`, which hits `LNK1189` (the 65535-object limit) on `bevy_dylib` with `dynamic_linking` enabled. The MSVC build also links against `vcruntime140.dll` / `msvcp140.dll`, which Windows 10/11 ship by default.

### macOS & iOS — osxcross

macOS (`x86_64-apple-darwin` + `aarch64-apple-darwin`) and iOS (`aarch64-apple-ios`) are cross-compiled with [osxcross](https://github.com/tpoechtrager/osxcross). The image fetches the macOS SDK (`MacOSX26.1.sdk`) and the iPhoneOS SDK (`iPhoneOS15.5.sdk`), then builds osxcross's clang toolchain. Deployment targets default to macOS **11.0** and iOS **14.0**.

- **macOS** links with osxcross's `*-apple-darwin*-clang` and embeds an `@loader_path` rpath so the binary finds its sibling dylibs.
- **iOS** is compiled by a small `aarch64-apple-ios-clang` wrapper that invokes `clang-19 --target=arm64-apple-ios14.0 -isysroot /opt/iphoneos.sdk -fuse-ld=lld` (osxcross's macOS clang wrapper hardcodes `-mmacosx-version-min`, which conflicts with the iOS minimum).

> macOS lanes build **only if osxcross is present** in the image; otherwise `build-all.sh` prints a warning and skips them. The community SDK mirrors used by the Dockerfile are not license-clean — for production, regenerate the SDK tarballs from Xcode on a Mac and re-point the SDK URLs.

### Android — NDK r27c

Android (`aarch64-linux-android` + `x86_64-linux-android`) uses the Android **NDK r27c** at `/opt/android-ndk`, targeting **API level 33**. Each target links with the matching NDK clang (e.g. `aarch64-linux-android33-clang`). The Android build produces a `cdylib` from `renzora-android` whose library name is `main` → **`libmain.so`**.

### WebAssembly — wasm-bindgen + binaryen

The web target (`wasm32-unknown-unknown`) is built game-runtime-only, then post-processed:

1. `cargo build` emits `renzora.wasm`.
2. `wasm-bindgen` (v0.2.108) generates the JS glue: `renzora-runtime.js` + `renzora-runtime_bg.wasm` (`--target web`).
3. `wasm-opt` (from `binaryen`) shrinks the module with `-Oz` and the feature flags the runtime needs (`bulk-memory`, `sign-ext`, `reference-types`, `multivalue`, …).

> The web build is **runtime-only** — there is no WebAssembly editor (the binary has no compile-time `editor` feature, and the editor bundle is a desktop-only dlopen target). On `wasm32`, Lua is not compiled (only Rhai runs), and audio/DAW/mixer/networking compile to no-op stubs.

## Building with `docker/build-all.sh`

`docker/build-all.sh <output-dir> [platform ...]` runs inside the container and drives every cross build. Pass no platforms to build everything the image can produce, or a filtered list:

```bash
# Build specific platforms into ./dist
docker/build-all.sh dist windows linux

# Build everything the container can produce
docker/build-all.sh dist
```

### Platform tokens

| Token | Expands to | Output directory |
|---|---|---|
| `linux` | Linux x64 | `dist/linux-x64/` |
| `windows` | Windows x64 (MSVC) | `dist/windows-x64/` |
| `macos` | `macos-x64` + `macos-arm64` | `dist/macos-x64/`, `dist/macos-arm64/` |
| `macos-x64` | macOS x64 only | `dist/macos-x64/` |
| `macos-arm64` | macOS ARM64 only | `dist/macos-arm64/` |
| `wasm` | Web runtime | `dist/web-wasm32/runtime/` |
| `android` | `android-arm64` + `android-x86` | `dist/android-arm64/runtime/`, `dist/android-x86/runtime/` |
| `android-arm64` | Android ARM64 only | `dist/android-arm64/runtime/` |
| `android-x86` | Android x86_64 only | `dist/android-x86/runtime/` |
| `ios` | iOS ARM64 | `dist/ios-arm64/runtime/` |

Output directories are **arch-suffixed**, and the names do not match the README's flat `dist/<platform>/`. Desktop targets place the binary and its shared libraries **directly** in the platform dir; web and mobile targets nest their output under a `runtime/` subdirectory.

### Lane orchestration

Builds run as concurrent **lanes**, where the contention-free unit is the *feature* (each owns its own `--target-dir`), not the platform:

- The **desktop lane** builds the engine for every requested desktop platform into `target/editor/`. The desktop platforms inside that lane build sequentially (they share the target-dir and reuse host-side proc-macro/build-script artifacts).
- The **`wasm`**, **`android`**, and **`ios`** lanes each build into their own `target/{wasm,android,ios}/` and run concurrently with the desktop lane.

Concurrency is capped by `BUILD_JOBS` (env), defaulting to ~one lane per 4 GB of container RAM and clamped to the CPU count, because parallel Bevy builds are RAM-bound during codegen/link. On a memory-tight machine set `BUILD_JOBS=1`.

> The desktop lane is **required** (its failure fails the build); the `android` and `ios` lanes are **best-effort** — a failure there prints a `WARN` and is reported in the lane summary but does not fail the overall build. macOS is skipped entirely if osxcross is missing.

## Output layout

A desktop build is the engine binary plus the shared libraries it links by name. For example, `dist/windows-x64/` contains:

```
dist/windows-x64/
├── renzora.exe              # the engine binary (editor + runtime + server)
├── renzora.dll              # the SDK contract crate (shared TypeIds)
├── renzora_editor.dll       # the removable editor bundle (delete → shipped game)
├── bevy_dylib-<hash>.dll    # the exact bevy_dylib the binary imports
├── std-<hash>.dll           # the Rust std shared lib (prefer-dynamic)
└── plugins/                 # distribution-plugin cdylibs
```

On Linux the binary is `renzora` (no extension) with `librenzora.so` / `librenzora_editor.so` / `libbevy_dylib-*.so` / `libstd-*.so` beside it; on macOS the suffix is `.dylib`. The non-desktop lanes emit a single artifact each:

| Target | Artifact | Path |
|---|---|---|
| Web | `renzora-runtime.js` + `renzora-runtime_bg.wasm` | `dist/web-wasm32/runtime/` |
| Android | `libmain.so` | `dist/android-arm64/runtime/`, `dist/android-x86/runtime/` |
| iOS | `librenzora_ios.a` (staticlib) | `dist/ios-arm64/runtime/` |

On Linux, the editor output is additionally wrapped into an `AppDir` and packaged as `Renzora Engine-x86_64.AppImage` when `appimagetool` is available.

### Shared libraries travel beside the binary

Renzora's dynamic-plugin system requires the host binary, the dlopened editor bundle, and every distribution plugin to share **one** compiled copy of Bevy and of the `renzora` SDK so their `TypeId`s match across the dlopen boundary. The repo's `.cargo/config.toml` arranges this with `-C prefer-dynamic` + `bevy/dynamic_linking`, and embeds an rpath (`$ORIGIN` on Linux, `@loader_path` on macOS) so the binary finds those libraries next to itself.

Because of that, `build-all.sh` copies, per target:

- the **exact** `bevy_dylib-<hash>` the host binary imports (read from the binary itself — not just the newest by mtime, which could be a hash the binary doesn't link);
- `renzora.{dll,so,dylib}` and the `renzora_editor` bundle, beside the exe;
- every distribution-plugin `cdylib` into `plugins/`;
- the matching **Rust std** shared library (`std-*.dll` / `libstd-*.so` / `libstd-*.dylib`) — `prefer-dynamic` links std dynamically, so it must ship too.

> `crt-static` is intentionally **disabled** for the Windows target: it changes crate disambiguators, which would break `TypeId` equality across the dylib boundary and the whole dynamic-plugin system.

## `build.rs` and cross-compilation

The root `build.rs` adapts to whether it is building natively or cross-compiling to Windows:

- On a **Windows host** it embeds the icon/version resource via `winres`.
- When **cross-compiling Linux→Windows-MSVC** it hand-writes a `.rc` file and invokes `llvm-rc` (LLVM 19, in the image) to compile it.

It also emits two values used by the dynamic-plugin ABI guard, regardless of target:

- `RENZORA_ENGINE_VERSION` — the package version.
- `RENZORA_BUILD_HASH` — an FNV-1a hash of `"<version>-<rustc>-bevy0.18"`. The loader **rejects** any plugin whose hash differs, so a plugin built against a different compiler or engine version is refused rather than crashing. (This is why every build uses the same `dist` profile and the same containerized compiler.)

## Local single-target cross builds

For quick one-off cross builds you can use the `cargo` aliases in `.cargo/config.toml` directly, but you must provide the corresponding toolchain yourself — the container is the supported path:

```bash
cargo build-web          # wasm32 game runtime (needs wasm-bindgen + the wasm32 target)
cargo build-android      # aarch64-linux-android (needs the NDK + linker config)
cargo build-ios          # aarch64-apple-ios staticlib (macOS host / osxcross)
cargo build-ios-sim      # aarch64-apple-ios-sim
```

These build into a single `target/dist/` directory rather than the per-feature dirs `build-all.sh` uses, and they do **not** run the `wasm-bindgen` / AppImage / shared-lib-copy post-processing that `build-all.sh` does. For reproducible, shippable output use the container.

> `cargo build-tvos` / `build-tvos-sim` exist but cannot build — there is no tvOS target or SDK in the toolchain (see the note above).

## What's next?

- [Building from Source](/docs/r1-alpha5/setup/building-from-source) — the cargo aliases, runtime modes, and the one-binary/editor-as-cdylib model.
- [Building Export Templates](/docs/r1-alpha5/packaging/export-templates) — turning these builds into shippable game templates.
- [Asset Packing (rpak)](/docs/r1-alpha5/packaging/asset-packing) — how project assets are packed and shipped beside the binary.

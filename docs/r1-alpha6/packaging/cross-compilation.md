# Cross-Compilation

Every Renzora target is cross-compiled inside Docker, so your host only needs Docker to produce Windows, macOS, iOS, Android, and WebAssembly builds. The toolchain is split into a shared base image plus one image per platform, so you download only the toolchains you actually build.

## One image per platform, on a shared base

Renzora does **not** use `cross`, `mingw`, or per-host linker juggling. Cross builds happen inside the engine's prebuilt images, published under **`ghcr.io/renzora/*`**:

| Image | Adds on top of base |
|---|---|
| `base` (`docker/base/Dockerfile`, `FROM rust:1.93.0-bookworm`) | rust + Linux dev libs + LLVM-19 |
| `linux` | appimagetool + dual-arch cross-gcc + UPX |
| `windows` | xwin (MSVC SDK + CRT) |
| `macos` | osxcross + macOS SDK + rcodesign |
| `ios` | osxcross + iPhoneOS SDK |
| `android` | Android NDK |
| `wasm` | wasm-bindgen + binaryen |

Each platform image builds `FROM base`, so they share the base layer on pull (downloaded once, stored once) while a toolchain change to one platform never re-downloads the others. The `renzora` CLI pulls only what a command needs: `renzora run` pulls the host platform image; `renzora build` (no args) pulls all; `renzora build windows` pulls only Windows. The GPU editor and game still run **natively** from the `dist/` output; only the *build* happens in the container.

The base image is the **single source of truth for the Rust version** — there is no `rust-toolchain.toml` in the repo. Bumping the compiler means editing the `FROM rust:1.93.0-bookworm` line in `docker/base/Dockerfile`. Tags are content hashes (`baseTag = sha256(docker/base/Dockerfile)`, `<plat>Tag = sha256(baseTag + docker/<plat>/Dockerfile)`), so a base edit re-rolls **every** platform tag — the CLI re-pulls and CI rebuilds each platform on the new base — while a platform-only edit moves just that platform's tag.

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

## Building with `renzora build`

`renzora build [platform ...]` runs the cross build inside the container and writes into `dist/` (it wraps `docker/build-all.sh` under the hood). Pass no platforms to build everything the image can produce, or a filtered list:

```bash
# Build specific platforms into dist/
renzora build windows linux

# Build everything the container can produce
renzora build
```

### Platform tokens

| Token | Expands to | Output directory |
|---|---|---|
| `linux` | Linux x64 | `dist/linux-x64/` |
| `windows` | Windows x64 (MSVC) | `dist/windows-x64/` |
| `macos` | `macos-x64` + `macos-arm64` | `dist/macos-x64/`, `dist/macos-arm64/` |
| `macos-x64` | macOS x64 only | `dist/macos-x64/` |
| `macos-arm64` | macOS ARM64 only | `dist/macos-arm64/` |
| `wasm` | Web runtime | `dist/web-wasm32/` |
| `android` | `android-arm64` + `android-x86` | `dist/android-arm64/`, `dist/android-x86/` |
| `android-arm64` | Android ARM64 only | `dist/android-arm64/` |
| `android-x86` | Android x86_64 only | `dist/android-x86/` |
| `ios` | iOS ARM64 | `dist/ios-arm64/` |

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
| Web | `renzora-runtime.js` + `renzora-runtime_bg.wasm` | `dist/web-wasm32/` |
| Android | `libmain.so` | `dist/android-arm64/`, `dist/android-x86/` |
| iOS | `librenzora_ios.a` (staticlib) | `dist/ios-arm64/` |

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

## Single-target cross builds

To build just one target, pass its platform token to `renzora build` — everything still runs inside the container, so the host needs only Docker:

```bash
renzora build wasm           # wasm32 game runtime
renzora build android        # aarch64-linux-android
renzora build ios            # aarch64-apple-ios staticlib
```

These write into the arch-suffixed `dist/` layout and run the full `wasm-bindgen` / AppImage / shared-lib-copy post-processing.

> `cargo build-tvos` / `build-tvos-sim` exist but cannot build — there is no tvOS target or SDK in the toolchain (see the note above).

## What's next?

- [Building from Source](/docs/r1-alpha5/setup/building-from-source) — the cargo aliases, runtime modes, and the one-binary/editor-as-cdylib model.
- [Building Export Templates](/docs/r1-alpha5/packaging/export-templates) — turning these builds into shippable game templates.
- [Asset Packing (rpak)](/docs/r1-alpha5/packaging/asset-packing) — how project assets are packed and shipped beside the binary.

# Exporting to Other Platforms

Exporting for the machine you're sitting at needs nothing extra — the editor you're running *is* the desktop template. Shipping for a platform you don't own — a macOS or Android or web build from a Windows box — needs a **runtime template** for that target, and this page is about where those come from.

## Why a template is needed at all

An export doesn't recompile the engine (except in [lean mode](overview.md#lean-single-binary-compiled-from-source), which is host-only). It packs your project into a `.rpak` archive and combines it with a prebuilt runtime for the target platform:

- **Desktop** — the template is the `renzora` binary and its `plugins/`. The engine ships two executables (`renzora` = the game runtime, `renzora-editor` = the editor), so the runtime already sitting beside your editor *is* the template for your own platform.
- **Mobile and web** — the template is a container shell (an unsigned APK, an `.app` bundle, a wasm + JS bundle) that the export step injects `game.rpak` into.

So the only question a cross-platform export has to answer is: where does the target's runtime come from?

## Three ways to get a template

### 1. Download it from the editor (easiest)

If a platform's template isn't present in `dist/`, the export modal offers **Download from GitHub** — it fetches `renzora-runtime-<platform>.zip` from the [`renzora/engine`](https://github.com/renzora/engine/releases) release **matching your engine's version** and installs it under `~/.renzora/templates/<version>/<platform>/`. Pick the platform, download once, export as often as you like.

It asks for your version specifically, not for the newest release. The runtime and the editor are two halves of one version, so an `r1-alpha7` editor paired with an `r1-alpha6` runtime would produce a game that can't load the scene the editor just saved. If your exact version has no release yet — the usual case for a build from source — it falls back to the newest **nightly** for that version and labels it as such; it never falls back to an older version. The download is checksummed, and a mismatch installs nothing.

This is the path most projects want. You need no cross-compiler, no SDKs, and no container.

### 2. Install one from a file

**Install from file…** points the editor at a template you already have — one a teammate built, or one from a CI artifact. Same effect as the download, without the network.

### 3. Build the templates yourself

Building a runtime for a platform you're not on means cross-compiling, which means a matching linker and SDK for every target: the MSVC CRT for Windows, an Apple SDK for macOS/iOS, the Android NDK, `wasm-bindgen` for web. Assembling that by hand per host is the part nobody wants to maintain, so Renzora publishes a pinned container toolchain that already contains all of it, driven by a separate command-line tool.

That path is documented in full under Engine Internals:

- [Cross-Compilation](../packaging/cross-compilation.md) — the toolchain images, every supported target and its linker, the build lanes, and CI.
- [Building Export Templates](../packaging/export-templates.md) — turning those builds into the artifacts the export modal consumes.

You need this only if you're producing templates (engine contributors, release builds, a studio pinning its own runtime). Developing a game and shipping it to any supported platform works fine on options 1 and 2.

## Targets and where they land

| Platform | Template artifact | `dist/` directory |
|---|---|---|
| Windows (x64 / ARM64) | `renzora.exe` | `dist/windows-x64/`, `dist/windows-arm64/` |
| Linux (x64 / ARM64) | `renzora` | `dist/linux-x64/`, `dist/linux-arm64/` |
| macOS (x64 / ARM64) | `renzora` | `dist/macos-x64/`, `dist/macos-arm64/` |
| Android (ARM64 / x86_64) | `renzora-runtime-android-*.apk` | `dist/android-arm64/`, `dist/android-x86/` |
| iOS (ARM64) | `renzora-runtime-ios-arm64.zip` | `dist/ios-arm64/` |
| Web (WASM) | `renzora-runtime-web-wasm32.zip` | `dist/web-wasm32/` |

## Caveats worth knowing before you plan a release

- **Windows ARM64 is built natively, not cross-compiled.** The redistributable MSVC pieces a container may legally bake in stop short of what ARM64 needs, so that slice is built on a real Windows-on-ARM machine (CI has a runner for it). ARM64 users can also run the x64 binary under Windows 11's built-in emulation.
- **Apple targets need an SDK you supply.** The macOS and iOS lanes build only when an Apple SDK is present in the toolchain; the community SDK mirrors are not license-clean, so a production pipeline should regenerate them from Xcode on a Mac.
- **Android and iOS lanes are best-effort.** A failure there warns rather than failing the whole build, so check the lane summary before assuming a template exists.
- **Fire TV and Apple TV (tvOS) are listed but not shippable.** The export dialog shows them; there is no working toolchain lane for either, so no template is ever produced.
- **Lean single binary is host-only.** It compiles from source with native `cargo`, so it's offered only when the selected target matches your machine. Use a copy-based packaging mode for other platforms.

## What's next

- [Export Overview](overview.md) — packaging modes, feature stripping, and the export dialog itself.
- [Windows](windows.md) · [Linux](linux.md) · [macOS](macos.md) · [Android](android.md) · [iOS](ios.md) — per-platform notes, signing, and store submission.

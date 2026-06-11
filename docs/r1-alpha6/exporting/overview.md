# Export Overview

Exporting turns your project into a shippable game — the same `renzora` engine binary with the editor bundle removed, plus your packed assets.

## The shipped game is the engine without the editor

Renzora is one binary. The editor is **not** a compile-time build — it ships as a removable cdylib (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`) that sits **beside the exe**. The runtime shape is decided at launch:

- Bundle present → the binary runs as the **editor**.
- Delete that one file (or pass `--no-editor`) → the **same** binary is your **shipped game**.

So an "export" is really: take the already-built game binary for a target platform, leave out `renzora_editor.*`, and ship it next to your project's assets. There is no separate "game build" of the engine to compile.

> The export scanner only bundles **Runtime-scope** distribution plugins (single-plugin cdylibs from the editor's `plugins/` folder). It skips the editor bundle itself, so the editor can never be accidentally shipped inside a game.

## Exporting from the editor

Export is driven by the editor's `renzora_export` crate (`ExportPlugin`, editor-only). Triggering Export from the editor opens a modal overlay where you choose the target platform, packaging mode, and a handful of build settings:

| Setting | What it controls |
|---|---|
| **Platform** | Target platform (see the table below) |
| **Packaging mode** | Separate files vs. single self-contained binary |
| **Window mode / size** | Default `Windowed` / `Fullscreen` and resolution (e.g. `1280×720`) |
| **Icon** | Optional window/app icon path |
| **Compression level** | Zstd level used when packing the `.rpak` |
| **Console logging** | Whether the shipped build keeps a console/log |
| **Include server** | Also emit a dedicated-server bundle (desktop only) |
| **Mesh optimization** | Optional simplify / quantize / LOD generation while packing |
| **Plugins** | Which Runtime-scope distribution plugins to include |

The actual packing runs on a background thread; the modal polls its progress while open.

## Supported platforms

`renzora_export` produces builds for the targets the cross-compile toolchain can actually build (`docker/build-all.sh`):

| Platform | Output | Devices |
|---|---|---|
| **Windows (x64)** | `.exe` (+ shared libs) | Desktop PCs, laptops |
| **Linux (x64)** | ELF binary (+ shared libs) | Desktop PCs, Steam Deck |
| **macOS (x64)** | binary | Intel Macs |
| **macOS (ARM64)** | binary | Apple Silicon Macs |
| **Android (ARM64)** | `.apk` | Phones, tablets, Quest/Pico |
| **Android (x86_64)** | `.apk` | Android emulators |
| **iOS (ARM64)** | `.ipa` | iPhone, iPad |
| **Web (WASM)** | `.wasm` + `.js` + `game.rpak` + `index.html` | Modern browsers (WebGPU) |

> ⚠️ The export dialog also lists **Fire TV** and **Apple TV (tvOS)** entries, but there is **no working build toolchain** for them today — the Docker image installs no tvOS rustup target and `build-all.sh` has no Fire TV or tvOS lane, so no template is ever produced. Treat both as aspirational, not shippable.

## How assets ship — the `.rpak` archive

Your project's assets are packed into a single **`.rpak`** archive (Renzora's own v2 format): a 32-byte header, a data section of per-entry payloads each stored or Zstd-compressed independently, and a tail index. When the archive is appended to an executable it gains a 16-byte footer so the binary can find its own embedded data.

At launch the runtime's VFS looks for assets in this order:

1. An explicit `--rpak <path>` override
2. A `.rpak` **embedded in the executable**
3. An **adjacent** `<exe-stem>.rpak` beside the binary
4. Platform containers (Android APK asset, iOS app bundle, WASM-injected bytes)
5. The raw filesystem (loose `assets/`)

That ordering is what makes both packaging modes work without any code changes in your game.

## Packaging modes

| Mode | Layout | Use it for |
|---|---|---|
| **Separate files** (`SeparateFiles`) | game binary + a sibling `.rpak` | Development, quick re-packs, web/mobile (where the `.rpak` is injected into the container) |
| **Single binary** (`SingleBinary`) | one self-contained executable with the `.rpak` appended | Clean desktop distribution |

Per platform, packing produces:

- **Desktop** — the game binary (named after your project) with either a sibling `.rpak` or the `.rpak` appended, no `renzora_editor.*` beside it.
- **Android** — the template `.apk` with the project packed in as `assets/game.rpak`.
- **iOS** — the template `.app` bundle with `game.rpak` injected, re-zipped into an `.ipa`.
- **Web** — a zip containing `renzora-runtime.js`, `renzora-runtime_bg.wasm`, `game.rpak`, and a generated `index.html` that fetches the `.rpak` and starts the runtime.

## Where templates come from

For your **current desktop platform**, no download is needed — the editor's own binary *is* the game template, so export just copies what's already in `dist/<platform>/`.

For platforms you have **not built locally**, the editor can fetch a prebuilt runtime template from the engine's GitHub releases (`renzora/engine`) and cache it. Alternatively, build every target yourself with the container (see below).

### Building the templates yourself

Cross-platform templates are built in the engine's Docker image with `docker/build-all.sh <output-dir> [platforms...]`, which writes **arch-suffixed** output directories. Desktop binaries land directly in their dir; web and mobile nest under `runtime/`:

```bash
docker/build-all.sh dist windows linux wasm android ios
```

| Token | Output directory |
|---|---|
| `windows` | `dist/windows-x64/` |
| `linux` | `dist/linux-x64/` |
| `macos` (= `macos-x64` + `macos-arm64`) | `dist/macos-x64/`, `dist/macos-arm64/` |
| `wasm` | `dist/web-wasm32/runtime/` |
| `android` (= `android-arm64` + `android-x86`) | `dist/android-arm64/runtime/`, `dist/android-x86/runtime/` |
| `ios` | `dist/ios-arm64/runtime/` |

> macOS lanes build only when osxcross is present; the Android and iOS lanes are best-effort (a failure there does not fail the whole build). See [Installation → Cross-compiling with Docker](/docs/r1-alpha5/getting-started/installation) for toolchain details.

## Dedicated server export

Enabling **Include server** (desktop only) writes a small server bundle alongside the game export:

- `server.rpak` — the project assets stripped for server use (visual-only assets dropped).
- `server.bat` / `server.sh` — launchers that run the **same game binary** in server mode and point it at `server.rpak`.

There is **no separate server executable** — the dedicated server is the shipped game binary launched with `--server`:

```bash
renzora --server --rpak server.rpak --port 7636 --tick-rate 64 --max-clients 32
```

`--host` instead runs a windowed listen server (client + server in one process). See [Server setup](/docs/r1-alpha5/multiplayer/server-setup) for the full flag list and deployment notes.

> The current networking handshake is insecure (`Authentication::Manual` with a zero key) and the only working transport is native **UDP** — multiplayer exports are LAN/dev-grade today.

## Platform notes

### Web (WASM)

The web build is **game-runtime only** — there is no WebAssembly editor. It runs on **WebGPU**, and several native-only subsystems compile to no-ops in the browser:

- **Lua is not compiled** on `wasm32`; only **Rhai** (`.rhai`) scripts run. Author web-targeted logic in Rhai.
- Audio (Kira), the DAW, and the mixer are stubs (no `cpal` on web).
- Networking is a no-op stub (no native UDP), so multiplayer is unavailable on web.

### Android / iOS

Android and iOS export by injecting your `game.rpak` into a prebuilt template (`.apk` for Android, `.app`/`.ipa` for iOS). The runtime is a thin platform shim around the engine — `renzora-android` produces `libmain.so`; `renzora-ios` produces a `librenzora_ios.a` staticlib. Signing and store submission are handled with the platform's own tooling (a keystore for Android, an Apple Developer account + Xcode/TestFlight for iOS).

## What's next

- [Installation → Build from source](/docs/r1-alpha5/getting-started/installation) — the cargo aliases and Docker cross-compile setup behind these builds.
- [Multiplayer → Server setup](/docs/r1-alpha5/multiplayer/server-setup) — running the exported dedicated server.

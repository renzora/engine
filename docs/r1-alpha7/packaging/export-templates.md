# Building Export Templates

How to build the per-platform packaging templates that `renzora_export` injects your game's assets into.

## What an export template is

An export template is a **pre-built runtime artifact** for a target platform. When you export a project, the editor's `renzora_export` crate packs your assets into a `.rpak` archive and combines it with the template for the chosen platform — it does **not** recompile the engine.

There are two kinds of template, and the difference matters:

- **Desktop** (Windows / Linux / macOS): the template is the **`renzora` binary and its `plugins/`** — the game runtime, with no editor. The engine ships two executables (`renzora` = runtime, `renzora-editor` = editor), so "the desktop template" is the first of those plus the plugin libraries it loads. Nothing extra has to be compiled.
- **Mobile / web** (Android / iOS / WASM): the template is a **container shell** — an unsigned APK, an `.app` bundle, or a wasm + JS bundle — that the export step injects `game.rpak` into. These are produced by the per-platform build scripts under `templates/`.

You only need a template for a platform **other than the one you are running on** — your own platform's runtime already sits beside the editor.

> Templates are **packaging** templates, not project scaffolds: there's no per-template `template.toml` and no starter-project generator. (`renzora new` does exist — it clones the whole engine repo to set up a workspace, rather than instantiating a template.) The `templates/` directory holds only the Android/iOS/web container shells described below.

## The `templates/` directory

```
templates/
├── android/          # Gradle project — wraps libmain.so into an APK
│   ├── app/build.gradle.kts
│   ├── app/src/main/AndroidManifest.xml
│   └── build-template.sh / build-template.ps1
├── ios/              # Xcode project — wraps librenzora_ios.a into a .app
│   ├── RenzoraRuntime.xcodeproj/
│   ├── RenzoraRuntime/Info.plist
│   └── build-template.sh
└── web/              # Vite scaffold for the WASM runtime
    ├── index.html
    ├── package.json
    └── vite.config.js
```

There is **no `templates/windows/` (or linux/macos)** — desktop platforms use the `dist/<platform>/` binary directly.

## Where the editor looks for templates

`TemplateManager` (in `crates/renzora_export/src/templates.rs`) checks **two** locations per platform, in this order:

1. **`dist/<platform>/`** — a from-source checkout that ran `renzora build <platform>`. The `dist/` root is two levels above the running editor exe. Always preferred: if you built it, you meant to use it.
2. **`~/.renzora/templates/<version>/<platform>/`** — downloaded from the release matching this engine's version.

A local build in `dist/` wins over a download for the same platform; a platform is never listed twice.

`build-all.sh` nests each platform's local output differently, so the `dist/` scan resolves three layouts:

| Platform | `dist/` directory | Where the runtime actually is |
|---|---|---|
| Windows (x64 / ARM64) | `dist/windows-x64/`, `dist/windows-arm64/` | flat — `renzora.exe` |
| Linux (x64 / ARM64) | `dist/linux-x64/`, `dist/linux-arm64/` | `<name>.AppDir/renzora` |
| macOS (x64 / ARM64) | `dist/macos-x64/`, `dist/macos-arm64/` | `<name>.app/Contents/MacOS/renzora` |
| Android (ARM64) | `dist/android-arm64/` | `renzora-runtime-android-arm64.apk` |
| Android (x86_64) | `dist/android-x86/` | `renzora-runtime-android-x86_64.apk` |
| Fire TV (ARM64) | `dist/firetv-arm64/` | `renzora-runtime-firetv-arm64.apk` |
| iOS (ARM64) | `dist/ios-arm64/` | `renzora-runtime-ios-arm64.zip` |
| Web (WASM) | `dist/web-wasm32/` | `renzora-runtime-web-wasm32.zip` |

A **downloaded** template is always flat — the release packaging unwraps the AppImage/`.app` bundle so the install side needs no layout knowledge at all.

### Downloading a template

**Export → Packaging → Runtime template → Download from GitHub** fetches `renzora-runtime-<platform>.zip` from the release matching this engine and extracts it into `~/.renzora/templates/<version>/<platform>/`. The download is checksummed against the digest GitHub publishes for the asset; a mismatch aborts and installs nothing.

The store is **scoped by engine version** on purpose. The runtime and the editor are two halves of one version, so a `r1-alpha7` editor picking up a `r1-alpha6` runtime would produce a game that fails to load the scene the editor just saved. Separate directories make that impossible rather than merely unlikely.

**Install from file…** does the same thing from a template you built yourself, copying into the same per-version directory.

### Which release the editor asks for

Not `releases/latest` — *its own version's*. Resolution, in order:

1. **The tag this binary was published under**, when CI stamped one in. A release or nightly build never has to guess.
2. **A release tagged exactly `r1-alpha7`** — the normal case once the version has shipped.
3. **The newest nightly for this version** (`r1-alpha7-nightly-16aug26`) — the case for a build from source, whose version has no release yet. The export modal labels this "Nightly:" rather than "Release:" so the fallback is visible.

There is deliberately no fourth step: falling back to the previous *version* would reintroduce exactly the mismatch this ordering exists to prevent. If nothing matches, the editor says so and you build the template yourself with `renzora build <platform>`.

> ⚠️ The enum also defines an **Apple TV (tvOS)** template (`renzora-runtime-tvos-arm64.zip`), but the Docker toolchain installs no `aarch64-apple-tvos` rustup target and `docker/build-all.sh` has no tvOS lane, so no tvOS template is ever produced. Treat it as aspirational.

## Building the desktop templates

The desktop "template" is just the runtime binary. Build it via the renzora CLI (Docker):

```bash
# Builds the desktop binary + renzora_editor bundle + one shared bevy_dylib.
# Every editor build also produces the lean runtime binary.
renzora build
```

`renzora build` produces the `renzora`/`renzora.exe` binary plus its shared libraries (`bevy_dylib`, `renzora.dll`, `std-*`). That binary, with `renzora_editor.*` removed, is the desktop template.

For cross-platform output in one pass, pass the platform tokens — `renzora build` writes the arch-suffixed `dist/` layout the scanner expects (it runs `docker/build-all.sh` inside each platform's `ghcr.io/renzora/<platform>` container, pulling only the images those tokens need):

```bash
renzora build windows linux macos
```

> There is **no `renzora_runtime` binary package** to `cargo build --package`. The only workspace binary is `renzora_app` (`[[bin]] name = "renzora"`, `default-run = "renzora"`); the crate literally named `renzora` is the contracts *library*.

## Building the web template

The WASM runtime is **game-only** — there is no WebAssembly editor. Build it with `renzora build wasm`; under the hood that runs the `wasm` lane of `docker/build-all.sh`:

```bash
# from docker/build-all.sh — the WASM lane
cargo build --profile dist -p renzora_app \
    --no-default-features --features wasm \
    --target wasm32-unknown-unknown --target-dir target/wasm

wasm-bindgen --out-dir dist/web-wasm32 \
    --out-name renzora-runtime --target web \
    target/wasm/wasm32-unknown-unknown/dist/renzora.wasm

wasm-opt -Oz dist/web-wasm32/renzora-runtime_bg.wasm \
    -o dist/web-wasm32/renzora-runtime_bg.wasm
```

This produces `renzora-runtime.js` + `renzora-runtime_bg.wasm`. The web template the editor consumes is a zip of those two files; the export step adds `game.rpak` and a generated `index.html` (which `fetch`es `game.rpak`, calls `set_rpak`, then `start()`). The `templates/web/` Vite scaffold is for local previewing of that bundle.

> The `wasm` feature is the only build feature besides `runtime` (`[features]` in the root `Cargo.toml` is just `default = ["runtime"]`, `runtime`, and `wasm`). There are **no** `audio`/`physics`/`networking`/`scripting`/`blueprints` feature flags — those subsystems are always on, and on `wasm32` the native-only ones compile to no-op stubs automatically. Audio is not among them: it is a plugin, and a browser build needs a WebAudio backend. Scripting is in the same position for the same reason — a language backend is a plugin, and the browser cannot load one — so a web game has no text scripting today.

## Building the Android template

Android ships a thin shim crate, `renzora-android`, whose library name is `main`, so it compiles to **`libmain.so`**:

```rust
// crates/renzora_android/src/lib.rs
#[bevy_main]
fn main() {
    let mut app = renzora_runtime::build_runtime_app();
    app.run();
}
```

The container build produces just the native library:

```bash
# docker/build-all.sh android lane → dist/android-arm64/libmain.so
cargo build --profile dist -p renzora-android \
    --target aarch64-linux-android --target-dir target/android
```

To assemble a full APK shell you need a local Android SDK/NDK + cargo-ndk, then run the template script, which builds the `.so` and wraps it with Gradle:

```bash
./templates/android/build-template.sh             # arm64-v8a (default)
./templates/android/build-template.sh --x86_64    # x86_64 (emulator)
./templates/android/build-template.sh --firetv    # Fire TV / Android TV
./templates/android/build-template.sh --all        # all of the above
```

Each run emits an **unsigned** release APK named `renzora-runtime-android-arm64.apk` (etc.) into `target/templates/` and the per-user cache (`%APPDATA%/renzora/templates` on Windows, `~/.config/renzora/templates` elsewhere). The editor's export step injects your `.rpak` as `assets/game.rpak`, then signs the APK. See [Export: Android](/docs/r1-alpha7/exporting/android) for the full local-route walkthrough.

## Building the iOS / tvOS template

iOS ships the `renzora-ios` crate as a **staticlib** (`librenzora_ios.a`). The container build produces only that library:

```bash
# docker/build-all.sh ios lane → dist/ios-arm64/librenzora_ios.a
cargo build --profile dist -p renzora-ios \
    --target aarch64-apple-ios --target-dir target/ios
```

The full `.app` bundle requires **macOS with Xcode**; the template script cross-compiles the staticlib and builds the Xcode project:

```bash
./templates/ios/build-template.sh                  # iOS device (ARM64)
./templates/ios/build-template.sh --simulator      # iOS simulator
./templates/ios/build-template.sh --tvos           # Apple TV (toolchain not in container)
```

It zips `RenzoraRuntime.app` into `renzora-runtime-ios-arm64.zip`. Export extracts that zip, injects `game.rpak` into the app bundle root, and re-zips it as `<project>.ipa`. The `--tvos` flavor exists but depends on an `aarch64-apple-tvos` target the standard toolchain doesn't ship.

## How a template becomes a shipped game

The background export worker (`crates/renzora_export/src/overlay.rs`) packs assets, strips editor-only components, optionally optimizes meshes, rewrites `project.toml` with the chosen window/console settings, then combines with the template per platform:

| Platform | What export does with the template |
|---|---|
| Desktop — **separate files** | Copy the binary + write a sibling `<name>.rpak` |
| Desktop — **single binary** | Append the `.rpak` to a copy of the binary (one self-contained exe) |
| Android | Copy the template APK, add `assets/game.rpak` (stored, 16 KB-aligned), sign |
| iOS | Inject `game.rpak` into the `.app` bundle, re-zip as `.ipa` |
| Web | Zip `renzora-runtime.js` + `_bg.wasm` + `game.rpak` + generated `index.html` |

On desktop it also copies any shared libraries sitting beside the runtime — but **never** `renzora-editor`, so the export is a clean game. (Since Bevy went static there are usually none: the runtime is self-contained.)

### Plugin selection

Effects and other features live in distribution-plugin cdylibs. Export scans **the chosen platform's** `plugins/` directory with `renzora_plugin::host::loader::scan_plugins` (which lists each C-ABI plugin with its Editor/Runtime scope, and deliberately maps nothing to do it — see [architecture](../setup/architecture.md)), then **pre-selects just the plugins your scenes actually reference**: it matches each plugin's crate prefix (e.g. `renzora_matrix::`) against the serialized component type paths in the project's `.ron` files. Selected plugins are copied into `output/plugins/`. If no scenes can be read, it falls back to selecting everything; effects added purely from scripts aren't auto-detected, so you can tick those manually.

The directory scanned is the one the resolved template brought with it, not the editor's own. That distinction only started mattering when cross-platform templates began working: a Windows editor exporting a Linux game would otherwise offer its own `.dll`s, and the game would find nothing it could load — silently, since a plugin the host can't open is simply skipped. A template with no `plugins/` of its own falls back to the editor's, which is right for a same-platform export. Changing the target platform re-scans.

### Dedicated server

Checking **Include server** (desktop only) writes `server.rpak` (assets stripped for server use) plus a `server.bat`/`server.sh` launcher. There is no separate server binary — the launcher runs the **same game binary** with `--server`:

```bash
renzora --server --rpak server.rpak --port 7636 --tick-rate 64 --max-clients 32
```

## Versioning

Templates are matched to the engine **by version**, not by an ABI hash. The old `plugin_bevy_hash()` gate is gone along with the shared `bevy_dylib` it protected: a C-ABI plugin links no Bevy and negotiates compatibility through a version handshake plus `INTERFACE_PREFIX_HASHES` instead, so a plugin built by any rustc loads into any engine.

What still has to line up is the **runtime and the editor**, because they share the scene format and the project config. That is what the version-scoped template store enforces: `~/.renzora/templates/<version>/` can only ever hand this editor a runtime published for its own version.

The single source of that version is `renzora::version::ENGINE_VERSION` (`crates/renzora/src/version.rs`). It is what the About dialog shows, what the splash shows, what the release workflow tags with, and what the downloader asks GitHub for — bump it and the docs directory together.

CI stamps two further values into a published binary, read by `option_env!` when the contract crate compiles:

- `RENZORA_RELEASE_TAG` — the exact tag (`r1-alpha7`, or `r1-alpha7-nightly-16aug26`). Absent in a build from source, which is what makes it a *dev* build.
- `RENZORA_BUILD_COMMIT` — the commit the release was cut from.

## Releases and nightlies

Templates come from GitHub releases, published by the **Build Engine** workflow (`.github/workflows/build-engine.yml`):

| Trigger | Tag | Kind |
|---|---|---|
| Nightly schedule (02:00 UTC) | `r1-alpha7-nightly-16aug26` | Prerelease, one per night, 14 kept |
| Push of an `r1-alpha*` tag | `r1-alpha7` | Full release |
| Manual dispatch | either, or build without publishing | — |

Every release carries two assets per platform:

- `<platform>.zip` — the **engine**: editor and runtime together (Linux ships the `.AppImage`, macOS the `.app`).
- `renzora-runtime-<platform>.zip` — the **export template**: runtime and plugins, no editor.

plus `manifest.json` (every asset with its size and SHA-256) and `SHA256SUMS`. Nightlies are skipped on a day nothing landed on `main`.

## See also

- [Export Overview](/docs/r1-alpha7/exporting/overview) — the end-to-end export workflow and `.rpak`/VFS details.
- [Export: Android](/docs/r1-alpha7/exporting/android) — Android specifics (Gradle config, flavors, signing).
- [Building from source](/docs/r1-alpha7/setup/building-from-source) — cargo aliases and the Docker cross-compile image.

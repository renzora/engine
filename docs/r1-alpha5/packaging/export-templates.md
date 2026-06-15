# Building Export Templates

How to build the per-platform packaging templates that `renzora_export` injects your game's assets into.

## What an export template is

An export template is a **pre-built runtime artifact** for a target platform. When you export a project, the editor's `renzora_export` crate packs your assets into a `.rpak` archive and combines it with the template for the chosen platform — it does **not** recompile the engine.

There are two kinds of template, and the difference matters:

- **Desktop** (Windows / Linux / macOS): the template *is the `renzora` binary itself*. Because of Operation Merge, the same binary is the editor when `renzora_editor.{dll,so,dylib}` sits beside it and the shipped game when that file is absent. So "the desktop template" is just the already-built binary in `dist/<platform>/` — there is nothing extra to compile.
- **Mobile / web** (Android / iOS / WASM): the template is a **container shell** — an unsigned APK, an `.app` bundle, or a wasm + JS bundle — that the export step injects `game.rpak` into. These are produced by the per-platform build scripts under `templates/`.

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

## How `renzora_export` finds templates — the `dist/` scan

`TemplateManager` (in `crates/renzora_export/src/templates.rs`) scans `dist/<platform>/` once on startup. The `dist/` root is two levels above the running editor exe (`dist/<platform>/renzora.exe` → `dist/`). For each platform it looks for:

- **Desktop** → the game binary directly in `dist/<platform>/`.
- **Mobile / web** → the packaged artifact under `dist/<platform>/runtime/`.

| Platform | `dist/` directory | Template artifact the scanner expects |
|---|---|---|
| Windows (x64) | `dist/windows-x64/` | `renzora.exe` |
| Linux (x64) | `dist/linux-x64/` | `renzora` |
| macOS (x64) | `dist/macos-x64/` | `renzora` |
| macOS (ARM64) | `dist/macos-arm64/` | `renzora` |
| Android (ARM64) | `dist/android-arm64/runtime/` | `renzora-runtime-android-arm64.apk` |
| Android (x86_64) | `dist/android-x86/runtime/` | `renzora-runtime-android-x86_64.apk` |
| Fire TV (ARM64) | `dist/firetv-arm64/runtime/` | `renzora-runtime-firetv-arm64.apk` |
| iOS (ARM64) | `dist/ios-arm64/runtime/` | `renzora-runtime-ios-arm64.zip` |
| Web (WASM) | `dist/web-wasm32/runtime/` | `renzora-runtime-web-wasm32.zip` |

If a platform's template isn't present locally, the export modal can **Download from GitHub** (it fetches the matching `renzora-runtime-*` asset from the `renzora/engine` releases) or **Install from file…** (you point it at a template you built yourself). Both copy the artifact into the editor's runtime directory.

> ⚠️ The enum also defines an **Apple TV (tvOS)** template (`renzora-runtime-tvos-arm64.zip`), but the Docker toolchain installs no `aarch64-apple-tvos` rustup target and `docker/build-all.sh` has no tvOS lane, so no tvOS template is ever produced. Treat it as aspirational.

## Building the desktop templates

The desktop "template" is just the runtime binary. Build it via the renzora CLI (Docker):

```bash
# Builds the desktop binary + renzora_editor bundle + one shared bevy_dylib.
# Every editor build also produces the lean runtime binary.
renzora build
```

`renzora build` produces the `renzora`/`renzora.exe` binary plus its shared libraries (`bevy_dylib`, `renzora.dll`, `std-*`). That binary, with `renzora_editor.*` removed, is the desktop template.

For cross-platform output in one pass, pass the platform tokens — `renzora build` writes the arch-suffixed `dist/` layout the scanner expects (it runs `docker/build-all.sh` inside `ghcr.io/renzora/engine`, so the host only needs Docker):

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

wasm-bindgen --out-dir dist/web-wasm32/runtime \
    --out-name renzora-runtime --target web \
    target/wasm/wasm32-unknown-unknown/dist/renzora.wasm

wasm-opt -Oz dist/web-wasm32/runtime/renzora-runtime_bg.wasm \
    -o dist/web-wasm32/runtime/renzora-runtime_bg.wasm
```

This produces `renzora-runtime.js` + `renzora-runtime_bg.wasm`. The web template the editor consumes is a zip of those two files; the export step adds `game.rpak` and a generated `index.html` (which `fetch`es `game.rpak`, calls `set_rpak`, then `start()`). The `templates/web/` Vite scaffold is for local previewing of that bundle.

> The `wasm` feature is the only build feature besides `runtime` (`[features]` in the root `Cargo.toml` is just `default = ["runtime"]`, `runtime`, and `wasm`). There are **no** `audio`/`physics`/`networking`/`scripting_lua`/`scripting_rhai`/`blueprints` feature flags — those subsystems are always on, and on `wasm32` the native-only ones (Lua, Kira audio, Lightyear networking) compile to no-op stubs automatically. Web games script in **Rhai** only.

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
# docker/build-all.sh android lane → dist/android-arm64/runtime/libmain.so
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

Each run emits an **unsigned** release APK named `renzora-runtime-android-arm64.apk` (etc.) into `target/templates/` and the per-user cache (`%APPDATA%/renzora/templates` on Windows, `~/.config/renzora/templates` elsewhere). The editor's export step injects your `.rpak` as `assets/game.rpak`, then signs the APK. See [Export: Android](/docs/r1-alpha5/exporting/android) for the full local-route walkthrough.

## Building the iOS / tvOS template

iOS ships the `renzora-ios` crate as a **staticlib** (`librenzora_ios.a`). The container build produces only that library:

```bash
# docker/build-all.sh ios lane → dist/ios-arm64/runtime/librenzora_ios.a
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

On desktop it also copies the shared libraries that sit beside the binary — `bevy_dylib`, `renzora.dll`/`librenzora.*`, and the `std-*` dylib — but **never** `renzora_editor.*`, so the export is a clean game.

### Plugin selection

Effects and other features live in distribution-plugin cdylibs. Export scans the editor's `plugins/` directory with `dynamic_plugin_loader::scan_plugins` (which lists **Runtime-scope, single-plugin** cdylibs only — the editor bundle is skipped), then **pre-selects just the plugins your scenes actually reference**: it matches each plugin's crate prefix (e.g. `renzora_matrix::`) against the serialized component type paths in the project's `.ron` files. Selected plugins are copied into `output/plugins/`. If no scenes can be read, it falls back to selecting everything; effects added purely from scripts aren't auto-detected, so you can tick those manually.

### Dedicated server

Checking **Include server** (desktop only) writes `server.rpak` (assets stripped for server use) plus a `server.bat`/`server.sh` launcher. There is no separate server binary — the launcher runs the **same game binary** with `--server`:

```bash
renzora --server --rpak server.rpak --port 7636 --tick-rate 64 --max-clients 32
```

## Versioning and the ABI guard

There is no version manifest. Compatibility between a template and its plugins is enforced by an **ABI hash**: `build.rs` emits an FNV-1a `RENZORA_BUILD_HASH` (engine version + rustc + `bevy0.18`), and every dynamic plugin exports a `plugin_bevy_hash()` the loader compares against its own — a mismatch is rejected. So a template, its `bevy_dylib`/`renzora` shared libs, and the plugin cdylibs you ship beside it **must all come from the same engine build**. The export modal surfaces the latest GitHub release tag as "Latest" for reference, but it does not gate the export on a version string.

## See also

- [Export Overview](/docs/r1-alpha5/exporting/overview) — the end-to-end export workflow and `.rpak`/VFS details.
- [Export: Android](/docs/r1-alpha5/exporting/android) — Android specifics (Gradle config, flavors, signing).
- [Building from source](/docs/r1-alpha5/setup/building-from-source) — cargo aliases and the Docker cross-compile image.

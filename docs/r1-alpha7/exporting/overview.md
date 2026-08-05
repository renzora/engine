# Export Overview

Exporting turns your project into a shippable game — the same `renzora` engine binary with the editor bundle removed, plus your packed assets.

## The shipped game is the engine without the editor

Renzora is one binary. The editor is **not** a compile-time build — it ships as a removable cdylib (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`) that sits **beside the exe**. The runtime shape is decided at launch:

- Bundle present → the binary runs as the **editor**.
- Delete that one file (or pass `--no-editor`) → the **same** binary is your **shipped game**.

So an "export" is really: take the already-built game binary for a target platform, leave out `renzora_editor.*`, and ship it next to your project's assets. There is no separate "game build" of the engine to compile.

> The export scanner only bundles **Runtime-scope** distribution plugins (single-plugin cdylibs from the editor's `plugins/` folder). It skips the editor bundle itself, so the editor can never be accidentally shipped inside a game.

## Exporting from the editor

Export is driven by the editor's `renzora_export` crate (`ExportPlugin`, editor-only). Triggering Export from the editor opens a modal overlay with the target-platform list on the left and the build settings on the right, organized into horizontal tabs — **Output** (binary name, export directory, icon), **Packaging** (packaging mode + runtime template), **Features** (the lean engine-feature strip), **Plugins**, **Compression** (zstd level + mesh optimization), and **Options** (window + flags):

| Setting | What it controls |
|---|---|
| **Platform** | Target platform (see the table below) |
| **Packaging mode** | Separate files, single self-contained binary, or a **lean** recompiled-from-source single binary |
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
| **Single binary** (`SingleBinary`) | one self-contained executable (with sibling dylibs) and the `.rpak` appended | Clean desktop distribution, fast to produce |
| **Lean single binary** (`LeanSingleBinary`) | one **statically linked, stripped** executable with the `.rpak` appended — **no** sibling dylibs at all | Lean release builds (see below) |

The first two modes **copy** the already-built dev runtime, so they ship the engine as separate dylibs (`bevy_dylib`, `renzora`, a dynamic `std`) beside the exe — fast to produce, but bloated. The lean mode instead **recompiles** the game from source into a single static file. See the next section.

Per platform, packing produces:

- **Desktop** — the game binary (named after your project) with either a sibling `.rpak` or the `.rpak` appended, no `renzora_editor.*` beside it.
- **Android** — the template `.apk` with the project packed in as `assets/game.rpak`.
- **iOS** — the template `.app` bundle with `game.rpak` injected, re-zipped into an `.ipa`.
- **Web** — a zip containing `renzora-runtime.js`, `renzora-runtime_bg.wasm`, `game.rpak`, and a generated `index.html` that fetches the `.rpak` and starts the runtime.

## Lean single binary (compiled from source)

The two copy-based modes are great for development but ship the **whole engine** as
separate dynamic libraries next to the exe (a `bevy_dylib`, the `renzora` SDK
dylib, and a dynamically-linked `std`). That sharing is exactly what makes the
dev/editor build fast and keeps the plugin ABI stable — but it bloats a release.

**Lean single binary** mode produces a release build the right way: it
**recompiles your game's `renzora` binary from source**, statically, into one
self-contained file with the `.rpak` embedded — no sibling dylibs.

What it strips/changes versus the copy modes:

- **Static Bevy + static `std`** — `bevy_dylib` and the dynamic `std` are gone;
  everything is linked into the one executable (`--no-default-features --features
  runtime`, which drops the `dynamic_linking` feature).
- **Thin LTO + size optimisation (`opt-level = "s"`) + symbol strip** (the
  `dist-lean` cargo profile) — dead code is eliminated and the binary is built
  for size. Thin rather than fat because fat LTO merges the whole program into
  one link unit, which on a graph this size made the linker fail; `panic` stays
  at `unwind`, since `abort` can't be mixed with the precompiled `std` on stable.
- **Engine features you don't use are never compiled** — see
  [Engine features](#engine-features-the-features-tab) below.
- On Windows it also static-links the MSVC runtime (no `VCRUNTIME140.dll`
  dependency), which is safe here precisely because a lean binary has no dynamic
  plugin ABI to preserve.

### It needs a Rust toolchain — installed automatically

Because it compiles, lean mode needs `cargo`. If Rust is already on your `PATH`
it's used directly. If not — e.g. on a canonical editor release where the user has
no Rust — the editor **provisions `rustup` automatically** into a private cache
beside the editor (its own `CARGO_HOME`/`RUSTUP_HOME`, the pinned toolchain,
minimal profile). This **never touches your global environment** or any existing
Rust install, and the toolchain is reused on later exports. The first lean export
therefore does a one-time toolchain download plus a full from-scratch compile, so
it takes several minutes; subsequent ones are incremental.

### Host platform only (today)

Native `cargo` can only build for the **platform the editor is running on**, so
lean mode is offered only when the selected target matches your host. Building a
lean binary for a *different* OS is a hard Docker requirement (the canonical
cross-compile path), which is not yet wired into this mode — use the copy-based
modes, or build on the matching host, for other platforms in the meantime.

### Plugins

Engine plugins — the post-process effects, GI, cloth and the rest — are ordinary
crates linked into the binary, so a lean build simply doesn't compile the ones
you switch off in the Features tab. Nothing special happens at export time.

**C-ABI plugins** (`plugins/`, e.g. the Lua interpreter) are a different
mechanism: they're `dlopen`'d cdylibs, and a lean export copies them into a
`plugins/` directory beside the executable exactly as the other packaging modes
do. Compiling *those* into the binary instead — which a WASM export will require,
since there is no `dlopen` in a browser — is not implemented yet.

The whole lean build runs in an **isolated copy** of the engine source (synced
into the gitignored `target/export-src/`), so your dev tree is never touched —
`cargo renzora` and `renzora run` are completely unaffected. The copy is patched
freely (e.g. `renzora` is built rlib-only to dodge the Windows PE 65535-export
cap) because it's disposable; the first export copies the source and the rest are
incremental.

A lean build recompiles the **engine source** the editor was built from — your
project is just assets that ride along in the rpak — so it's available whenever
you run the editor from a source checkout. **Marketplace plugins** are C-ABI
cdylibs and need no source: they're copied beside the binary like any other.

## Engine features (the Features tab)

A lean export only compiles the engine your game actually uses. The **Features**
tab lists every strippable subsystem as a checkbox; unticking one removes its
Cargo features from the disposable source copy, so the code is never built rather
than built and dead-stripped. This is the single biggest lever on binary size —
much larger than LTO or symbol stripping.

Two kinds of default:

- **Structural subsystems** (physics, audio, animation, terrain, the post-process
  effects, UI, scripting, …) default to **on**. A game might reach them from a
  script the exporter can't see, so you untick the ones you know you don't use
  rather than risk silently losing something.
- **Safe leaves** (raytraced lighting, remote asset loading, debug gizmos, system
  diagnostics, editor conveniences) default to **off**, because nothing needs
  them unless you say so.

A few are **detected from your project**: image decoders follow the texture
formats actually present, `Scripting` follows whether the project contains `.lua`
files, `glTF model loading` whether it contains `.gltf`/`.glb` files,
`Blueprints` whether it contains graphs, and `Script HTTP` whether a script calls
`http_get`/`http_post`.

### Sections

The list is grouped so the two rendering pipelines sit next to each other — a
game is usually one or the other, and deciding which half to drop is the main
thing the tab is for:

**3D rendering** · **2D rendering** · **Post-processing** · **Sky & environment** ·
**Simulation** · **Systems & gameplay** · **Interface** · **Assets** ·
**Build & diagnostics**

Each header carries a **checkbox** that turns the whole section on or off at once
— nested children included, since a child is meaningless without its parent.
Clicking the **title** folds the section shut. Unticking 3D rendering and ticking
2D rendering is a complete 2D game in two clicks.

### Nested features

Some entries have children, shown indented. A child is always a strict subset of
its parent, so turning the parent off takes every child with it — leaving, say,
"advanced PBR texture maps" on while 3D rendering is off would pull the whole PBR
pipeline back in and undo the saving. Current groups:

| Parent | Children |
|---|---|
| **3D rendering** | graph materials, glTF loading, morph targets, advanced PBR texture maps, lighting lookup tables |
| **User interface** | Bevy's built-in font, Game UI (markup) |

**Graph materials** is the biggest of those — about 1 MiB for the `renzora_shader`
node-graph system that compiles `.material` assets into custom PBR shaders. A
game whose meshes all use plain StandardMaterial never touches it, so it's
detected from whether the project contains `.material` files.

### Dependencies between features

A few features need another one and will pull it back in automatically — Cargo
resolves this, so you can't produce a broken combination by unticking things:

- **Game UI** needs both **User interface** and **Scripting**.
- **Blueprints** and **Script HTTP** need **Scripting**.
- **3D text** needs **User interface** (its glyph outlines come from Bevy's text
  crate), so switching UI off switches 3D text off too.
- Turning **3D rendering** off also strips the subsystems built on it — terrain,
  water, splines, particles, the sky set and the post-process effects.

### Panic unwinding — the largest single lever

**Panic unwinding** is on by default and is not a subsystem: turning it off
builds with `panic = "abort"`. Measured on a cube-and-light project, that took
the binary from **60.9 MB to 46.7 MB — about 24%**. The saving is much larger
than the unwind tables alone, because dropping unwinding also removes the
landing pads and cleanup glue from the code section and the panic message and
source-location strings from the data section (`.text` −6.9 MB, `.rdata` −6.9 MB,
`.pdata` −1.1 MB).

**It has a real cost.** The engine wraps every call into a C-ABI plugin in
`catch_unwind`, including each script call, so that a panicking plugin or script
is caught and logged instead of killing the process. With `abort` nothing is
caught — one bad script takes the whole game down. Crash reporting still works,
because the panic hook runs before the abort. Ship it only once you've tested
your game's scripts and plugins.

(This is available to a lean export and not to the dev build for a concrete
reason: the dev build's `renzora` crate is a `dylib`, which links the precompiled
`std`'s unwinding runtime and cannot be mixed with `abort`. The export copy
builds it as an `rlib` only, so the restriction doesn't apply.)

### Where the size actually goes

Worth knowing before hunting for savings, measured on that same project:

| Section | Size | Share |
|---|---|---|
| `.text` (code) | 38.9 MiB | 64% |
| `.rdata` (read-only data) | 18.3 MiB | 30% |
| `.pdata` (unwind tables) | 2.3 MiB | 4% |
| everything else | 1.4 MiB | 2% |

**Roughly a third of the binary is data, not code** — embedded lookup tables
(the blue-noise texture alone is 1.57 MiB), shader source, and reflection type
information. That is why the LUT capabilities and `panic = "abort"` pay off out
of proportion to how small they look.

On the code side the largest crates are `bevy_pbr` (3.3 MiB), `std` (2.8),
`bevy_ecs` (2.6), `bevy_reflect` (2.2) and `naga` (2.1, the shader compiler wgpu
needs at runtime). Those are the engine itself; a 3D game needs them, so there is
no large saving left in code beyond switching off subsystems you don't use.

### What "User interface" off actually removes

Worth calling out because it's the largest single saving available: unticking it
drops `bevy_ui`, `bevy_ui_render`, `bevy_ui_widgets`, `bevy_text` and the
`renzora_ember` widget framework — and with `bevy_text` goes the entire text
shaping and font stack (parley, swash, harfrust, fontique, skrifa, read-fonts),
several MB that a game drawing no text has no use for. Only do it for a game with
genuinely no on-screen text or UI.

## Where templates come from

For your **current desktop platform**, no download is needed — the editor's own binary *is* the game template, so export just copies what's already in `dist/<platform>/`.

For platforms you have **not built locally**, the editor can fetch a prebuilt runtime template from the engine's GitHub releases (`renzora/engine`) and cache it. Alternatively, build every target yourself with the container (see below).

### Building the templates yourself

Cross-platform templates are built with `renzora build [platforms...]` (inside the engine's Docker image), which writes **arch-suffixed** output directories into `dist/`. Windows lands a flat exe; macOS/Linux wrap the binary in a `.app` / AppImage `.AppDir`; web and mobile drop their artifact directly in the platform dir:

```bash
renzora build windows linux wasm android ios
```

| Token | Output directory |
|---|---|
| `windows` | `dist/windows-x64/` |
| `linux` | `dist/linux-x64/` |
| `macos` (= `macos-x64` + `macos-arm64`) | `dist/macos-x64/`, `dist/macos-arm64/` |
| `wasm` | `dist/web-wasm32/` |
| `android` (= `android-arm64` + `android-x86`) | `dist/android-arm64/`, `dist/android-x86/` |
| `ios` | `dist/ios-arm64/` |

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

- [Installation → Working from a checkout](/docs/r1-alpha5/getting-started/installation) — the `renzora` CLI and Docker cross-compile setup behind these builds.
- [Multiplayer → Server setup](/docs/r1-alpha5/multiplayer/server-setup) — running the exported dedicated server.

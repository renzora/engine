# Export Overview

Exporting turns your project into a shippable game — the same `renzora` engine binary with the editor bundle removed, plus your packed assets.

## The shipped game is the engine without the editor

Renzora is one binary. The editor is **not** a compile-time build — it ships as a removable cdylib (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`) that sits **beside the exe**. The runtime shape is decided at launch:

- Bundle present → the binary runs as the **editor**.
- Delete that one file (or pass `--no-editor`) → the **same** binary is your **shipped game**.

So an "export" is really: take the already-built game binary for a target platform, leave out `renzora_editor.*`, and ship it next to your project's assets. There is no separate "game build" of the engine to compile.

> The export scanner only bundles **Runtime-scope** distribution plugins (single-plugin cdylibs from the editor's `plugins/` folder). It skips the editor bundle itself, so the editor can never be accidentally shipped inside a game.

## Exporting from the editor

Export is driven by the editor's `renzora_export` crate (`ExportPlugin`, editor-only). Triggering Export from the editor opens a modal overlay with the target-platform list on the left and the build settings on the right, organized into horizontal tabs — **Output** (binary name, export directory, icon), **Packaging** (packaging mode + runtime template), **Features** (the lean engine-feature strip), **Plugins**, **Compression** (zstd level, UPX binary packing + mesh optimization), and **Options** (window + flags):

| Setting | What it controls |
|---|---|
| **Platform** | Target platform (see the table below) |
| **Packaging mode** | Separate files, single self-contained binary, or a **lean** recompiled-from-source single binary |
| **Window mode / size** | Default `Windowed` / `Fullscreen` and resolution (e.g. `1280×720`) |
| **Icon** | Optional window/app icon path |
| **Compression level** | Zstd level used when packing the `.rpak` |
| **Compress binary with UPX** | Pack the shipped executable and libraries with UPX (see [below](#compressing-the-binary-with-upx)) |
| **Console logging** | Whether the shipped build keeps a console/log |
| **Include server** | Also emit a dedicated-server bundle (desktop only) |
| **Mesh optimization** | Optional simplify / quantize / LOD generation while packing |
| **Plugins** | Which Runtime-scope distribution plugins to include, and whether they ship as files or are linked into the binary |

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
  one link unit, which on a graph this size made the linker fail. Three of the
  profile's settings are yours to move per export — see
  [Build-profile knobs](#build-profile-knobs).
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
mechanism, and a lean export gives you a choice about them — see
[Plugin linking](#plugin-linking-the-plugins-tab) below.

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

## Plugin linking (the Plugins tab)

C-ABI plugins can reach the exported game two ways. The **Plugins** tab picks
which, and the plugin checkboxes below it pick *what* either way.

| Mode | What you ship | Works with |
|---|---|---|
| **Ship as files** (default) | A `plugins/` folder beside the executable, one library per plugin, loaded at startup | every packaging mode |
| **Link into the binary** | Nothing — the plugins are compiled into the executable | **Lean single binary** only |

Neither is more capable than the other: a linked-in plugin registers exactly the
same components, systems, panels and render passes as a loaded one, because the
C ABI never depended on there being a shared library. A plugin exports one
function and imports nothing — the interface is handed *in* as a table — so
whether the host got that function pointer from the OS loader or from its own
link table changes nothing downstream.

**Link them in when** you want one file to ship. A lean export is already a
single binary with its assets appended; a `plugins/` folder next to it puts you
back to a directory a player can break by deleting the wrong thing. It also
removes the startup directory scan and the per-plugin load.

**Ship files when** you want the set to stay open after release — mods, DLC
effects, a plugin you patch without reshipping the game — or when you're not
using lean mode.

### Why lean only

Linking a plugin in means *compiling* it, and lean mode is the only one that
compiles anything. The other two copy an already-built runtime binary; no amount
of packaging can put new code inside it. If you leave the toggle on **Link into
the binary** and switch packaging to a copy-based mode, the export says so and
ships the plugins as files instead of failing.

### What it needs, and what happens without it

A linked plugin is built from source, so the exporter looks for its crate under
the engine checkout's `plugins/` directory, matched by package name. A plugin
that has no source there — a **marketplace download**, which arrives as a
prebuilt library — is reported in the build log and shipped as a file beside the
binary. Mixing is fine and needs no thought: the game links in what it can and
still reads `plugins/` at startup for the rest.

### What you give up

**Hot reload.** The editor watches `plugins/` and swaps a rebuilt library in
without a restart; there is no file to watch inside a binary and no way to
replace code in a running one. This is why linking in is an export-time choice
and never how the editor itself runs — the editor always loads from files.

### Under the hood

The exporter writes a `renzora_static_plugins` crate into its disposable source
copy: one path dependency per plugin and a list pairing each plugin's `init`
function with the scope its library would have reported. The plugins are compiled
with `renzora_plugin`'s `static_link` feature, which drops the `#[no_mangle]`
from what `add!` emits — without that, two plugins each defining
`renzora_plugin_init` would fail to link. The host initialises them before it
scans `plugins/`, and Editor-scope plugins are skipped in a game exactly as they
are when loaded from disk.

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

> A game that makes any network request also needs `plugins/http` staged beside
> it — the engine carries no HTTP client. See
> [Network backends](../extending/network-backends.md). Leaving it out is how a
> fully offline game drops the TLS stack entirely.

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

### Build-profile knobs

Three toggles in the **Build & diagnostics** section aren't features at all —
nothing is stripped from the build when they're off. They edit the `dist-lean`
cargo profile for that one export. All three are phrased as the thing you *keep*,
so leaving them ticked reproduces exactly the profile that ships in the repo.

| Toggle | On (default) | Off |
|---|---|---|
| **Panic unwinding** | `panic = "unwind"` | `panic = "abort"` — see below |
| **Loop vectorization** | `opt-level = "s"` | `opt-level = "z"` |
| **Parallel code generation** | `codegen-units = 16` | `codegen-units = 1` |

**`opt-level = "z"`** is the same size-first optimisation as `"s"` with loop
vectorization switched off as well. It is *not* reliably smaller: a scalar loop
that runs more iterations can cost more in unrolled code than the vector version
saved, and hot per-vertex or per-pixel CPU work loses its SIMD widening either
way. Export once each way and compare the two files before shipping `z`.

**`codegen-units = 1`** gives LLVM one module per crate instead of 16, so thin
LTO sees whole crates at once and has fewer duplicated inline copies left to
merge. It's the classic size trick, but it overlaps with what LTO already does
here rather than adding to it, and it removes the parallelism that makes a lean
export take minutes instead of the better part of an hour. Measure before you
pay for it.

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

### Compressing the binary with UPX

**Compression → Compress binary with UPX** packs the shipped executable — and,
for the two copy-based modes, the sibling `bevy_dylib` / `renzora` / `std`
libraries and any plugins in `plugins/` — with [UPX](https://upx.github.io/).
Packed files carry a small decompressor stub and unpack themselves into memory at
launch, which cuts what your players download by more than half.

Measured on the engine's own Windows runtime (`dist/windows-x64/renzora.exe`,
already built with the stripped `dist` profile): **187.3 MB → 31.7 MB, an 83%
saving, in 82 seconds.** The packed binary boots normally — it was run headless
with `--server` and came up through the full plugin and scripting startup. Expect
a smaller ratio on a lean binary, which is already LTO'd and stripped of most of
what compresses best, but the same order of win.

Unlike everything else on this page, it is **post-build**: it changes nothing
about what was compiled, so it stacks on top of the feature strip and the profile
knobs, and it is the only size lever that helps the **Separate files** and
**Single binary** modes at all — those ship an already-built runtime that no
cargo setting can reach any more.

What to know before ticking it:

- **It needs `upx` on the machine.** Install it (`scoop install upx`,
  `apt install upx-ucl`, `brew install upx`) or point `RENZORA_UPX` at the
  executable. If it isn't found the export says so in the log and **continues
  uncompressed** — it never fails the export over this.
- **Windows and Linux exports only.** UPX supports Mach-O, but packing
  invalidates the code signature and Gatekeeper refuses the result, so macOS is
  excluded. Android ships a `.so` inside an already-compressed APK, iOS ships a
  static library, and `.wasm` isn't an executable format UPX knows — for the web,
  your server's gzip/brotli is the equivalent lever.
- **It costs a little startup time**, since the whole executable is decompressed
  before `main` runs, and it costs some memory (the unpacked image can't be paged
  from disk the way a normal executable's code is).
- **Some antivirus heuristics distrust packed executables.** Self-extracting
  binaries are a malware idiom as well as a legitimate one, so a packed game is
  more likely to be flagged, especially unsigned. If you ship signed builds or
  have had SmartScreen trouble, leave it off.

The engine's own release artefacts are compressed separately, by
`renzora upx [dist/<platform>]`, which uses the much slower `--brute`. An export
uses `--best --lzma`: within a few percent of brute force for a fraction of the
time, which matters when the input is a 50–200 MB game and someone is waiting.

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

> macOS lanes build only when osxcross is present; the Android and iOS lanes are best-effort (a failure there does not fail the whole build). See [Exporting to Other Platforms](cross-platform.md) for the three ways to get a template, and [Cross-Compilation](../packaging/cross-compilation.md) for toolchain details.

## Dedicated server export

Enabling **Include server** (desktop only) writes a small server bundle alongside the game export:

- `server.rpak` — the project assets stripped for server use (visual-only assets dropped).
- `server.bat` / `server.sh` — launchers that run the **same game binary** in server mode and point it at `server.rpak`.

There is **no separate server executable** — the dedicated server is the shipped game binary launched with `--server`:

```bash
renzora --server --rpak server.rpak --port 7636 --tick-rate 64 --max-clients 32
```

`--host` instead runs a windowed listen server (client + server in one process). See [Server setup](/docs/r1-alpha7/multiplayer/server-setup) for the full flag list and deployment notes.

> The current networking handshake is insecure (`Authentication::Manual` with a zero key) and the only working transport is native **UDP** — multiplayer exports are LAN/dev-grade today.

## Platform notes

### Web (WASM)

The web build is **game-runtime only** — there is no WebAssembly editor. It runs on **WebGPU**, and several native-only subsystems compile to no-ops in the browser:

- **Lua is not compiled** on `wasm32`; only **Rhai** (`.rhai`) scripts run. Author web-targeted logic in Rhai.
- The DAW and the mixer are editor-only. Audio itself needs a browser backend built against WebAudio — the bundled `plugins/audio` is native, because cpal cannot capture on the web. See [Audio backends](../extending/audio-backends.md).
- Networking is a no-op stub (no native UDP), so multiplayer is unavailable on web.

### Android / iOS

Android and iOS export by injecting your `game.rpak` into a prebuilt template (`.apk` for Android, `.app`/`.ipa` for iOS). The runtime is a thin platform shim around the engine — `renzora-android` produces `libmain.so`; `renzora-ios` produces a `librenzora_ios.a` staticlib. Signing and store submission are handled with the platform's own tooling (a keystore for Android, an Apple Developer account + Xcode/TestFlight for iOS).

## What's next

- [Installation → Working from a checkout](/docs/r1-alpha7/getting-started/installation) — the `renzora` CLI and Docker cross-compile setup behind these builds.
- [Multiplayer → Server setup](/docs/r1-alpha7/multiplayer/server-setup) — running the exported dedicated server.

# Export Overview

Exporting turns your project into a shippable game — the same `renzora` engine binary with the editor bundle removed, plus your packed assets.

## The shipped game is the engine without the editor

Renzora is one binary. The editor is **not** a compile-time build — it ships as a removable cdylib (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`) that sits **beside the exe**. The runtime shape is decided at launch:

- Bundle present → the binary runs as the **editor**.
- Delete that one file (or pass `--no-editor`) → the **same** binary is your **shipped game**.

So an "export" is really: take the already-built game binary for a target platform, leave out `renzora_editor.*`, and ship it next to your project's assets. There is no separate "game build" of the engine to compile.

> The export scanner only bundles **Runtime-scope** distribution plugins (single-plugin cdylibs from the editor's `plugins/` folder). It skips the editor bundle itself, so the editor can never be accidentally shipped inside a game.

## Exporting from the editor

Export is driven by the editor's `renzora_export` crate (`ExportPlugin`, editor-only). Triggering Export from the editor opens a modal overlay with the target-platform list on the left and the build settings on the right, organized into horizontal tabs — **Output** (binary name, export directory, icon), **Packaging** (packaging mode + runtime template), **Features** (the lean engine-feature strip), **Plugins**, **Files** (what goes into the archive), **Compression** (zstd level, UPX binary packing + mesh optimization), and **Options** (window + flags):

| Setting | What it controls |
|---|---|
| **Platform** | Target platform (see the table below) |
| **Packaging mode** | Separate files, single self-contained binary, or a **lean** recompiled-from-source single binary |
| **Window mode / size** | Default `Windowed` / `Fullscreen` and resolution (e.g. `1280×720`) |
| **Icon** | Optional icon for the window and (lean mode only) the executable — see [Icons](#icons) |
| **Compression level** | Zstd level used when packing the `.rpak` |
| **Compress binary with UPX** | Pack the shipped executable and libraries with UPX (see [below](#compressing-the-binary-with-upx)) |
| **Console logging** | Whether the shipped build keeps a console/log |
| **Include server** | Also emit a dedicated-server bundle (desktop only) |
| **Mesh optimization** | Optional simplify / quantize / LOD generation while packing |
| **Plugins** | Which Runtime-scope distribution plugins to include, and whether they ship as files or are linked into the binary |
| **Files** | Exactly which project files go into the `.rpak` — see [What goes in the archive](#what-goes-in-the-archive) |

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
- **Web** — a zip containing `renzora-runtime.js`, `renzora-runtime_bg.wasm`, `game.rpak`, and a generated `index.html` that fetches the `.rpak` and starts the runtime. The web offers two packaging modes rather than three — see [Web packaging](#web-packaging).

### Web packaging

The web's Packaging tab offers **Prebuilt template** and **Lean recompile**,
because the other distinction does not exist there: a `.wasm` module has nothing
to append an rpak to, so "separate files" and "single binary" would produce the
same zip.

**Prebuilt template** unzips the web runtime built by `cargo renzora wasm` (or by
CI) and adds your `game.rpak`. It is quick, and it ships the whole engine to
every player — the template was compiled once, for no game in particular.

**Lean recompile** compiles the module for this project, with the capabilities
your game does not use stripped out and the plugins you selected linked in. It
takes the same route as a desktop lean build, and needs no Docker: `wasm32-
unknown-unknown` is another architecture but not another OS, so rustc's own
linker handles it.

Three pieces are fetched for you if you don't have them: the `wasm32-unknown-
unknown` standard library (`rustup target add`), a `wasm-bindgen` CLI matching
the version in the engine's lockfile, and binaryen for `wasm-opt`. The first two
are required; **binaryen is not** — `-Oz` is a size pass, so if it cannot be
downloaded the export still succeeds and simply ships a bundle several times
larger, saying so in the log. A `wasm-opt` already on your `PATH` is preferred
over anything the exporter fetches.

> **Plugins on the web only work in lean mode.** A browser has no `dlopen`, so a
> `plugins/` folder beside the bundle is never read. A lean web export therefore
> always links the selected plugins into the module, whatever the Plugins tab's
> radio says; the template mode ships none, and tells you how many it left out.
> Plugins with a C build (`lua`, `tracy`) or a native-only dependency (`http`)
> cannot cross to the web at all.

## Icons

Pick one on the **Output** tab. It feeds two completely separate places, and it
is worth knowing which, because they do not both work in every packaging mode.

**The window icon** — what the running game shows in its title bar, the taskbar
button and Alt-Tab. The picked image is re-encoded to a 256×256 PNG, packed into
the `.rpak` as `assets/icon.png`, and applied at startup by the runtime. This
works in **every** packaging mode.

**The executable's icon** — what a file manager, a desktop shortcut and the
Windows Properties dialog show for the file itself. This is a Win32 resource
compiled *into* the binary, so it can only be set while the binary is being
built. That makes it **lean mode only**: the two copy-based modes ship a
pre-built template executable that already carries the engine's own icon, and
nothing rewrites it afterwards. A lean export also stamps your project's name
into the ProductName and FileDescription fields, which the copy-based modes leave
reading "Renzora Engine".

> If your game's exe must carry its own icon, use **Lean single binary**.

Any raster format the editor can read is accepted — `png`, `ico`, `jpg`, `jpeg`,
`bmp`, `webp`, `tga`. SVG is not supported. Non-square images are centred on a
transparent square rather than stretched, and the generated `.ico` carries the
full 16/32/48/64/128/256 set so Windows never has to rescale one for you.

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

### Where a lean build runs

Three shapes, decided by the target:

| Target | Runs | Needs |
|---|---|---|
| Your own platform | native `cargo`, no `--target` | Rust (provisioned if absent) |
| **Web** | native `cargo --target wasm32-unknown-unknown` | Rust + the wasm32 target + `wasm-bindgen` (all provisioned) |
| Another desktop OS | that platform's toolchain container | Docker |

The web is the case worth understanding, because it looks like a cross-build and
is not treated as one. A container is a **cross-linker** — the reason a Windows
host cannot produce a macOS binary is that it has no `ld64` and no Apple SDK.
`wasm32-unknown-unknown` has neither problem: rustc ships the linker and, once
`rustup target add` has run, the standard library. So the web needs `--target`
but no image, and sending it to Docker would mean demanding a container install
for a compile the host can already do.

Every lean build needs the **engine source**, which is what the mode recompiles.
A canonical editor download does not ship it; the Packaging tab offers a
**Download engine source** button when that is what is missing.

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

Those checkboxes are pre-ticked from the same project scan the Features tab uses,
so a fresh open lists the plugins your content actually references — a plugin's
components carry its crate name (`renzora_matrix::MatrixSettings`), and the scan
now looks in scripts and markup as well as scenes. A plugin nothing references is
left unticked rather than shipped by default; tick it if you load it some way the
scan can't see.

| Mode | What you ship | Works with |
|---|---|---|
| **Ship as files** (default) | A `plugins/` folder beside the executable, one library per plugin, loaded at startup | every packaging mode |
| **Link into the binary** | Nothing — the plugins are compiled into the executable | **Lean single binary** only; forced on the web |

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

### The web has no choice

A browser has no `dlopen`, so a `plugins/` folder beside the bundle is never
read — shipping files there means shipping nothing. A lean web export therefore
links the ticked plugins in whatever this tab says, and the prebuilt-template
mode reports how many it had to leave out rather than dropping them silently.

Four cannot cross at all, and no packaging mode changes that: `audio` (cpal, and
its entry point is native-only until a WebAudio backend exists), `lua` and
`tracy` (both compile C, and `wasm32-unknown-unknown` has no libc sysroot for
it), and `http` (a blocking socket client). Each says so in its own manifest —
see [Declaring a platform you can't build
for](../extending/standalone-plugins.md#declaring-a-platform-you-cant-build-for)
— so the Plugins tab names them for the selected platform and the export leaves
them out with a note, rather than failing minutes into the compile. Everything
else in `plugins/` is pure Rust and links in fine.

Note what `audio` being on that list means in practice: **a web export has no
sound today**, plugin or not — the engine's own audio runtime is compiled out on
wasm as well.

One consequence worth knowing: the host adopts a **linked-in script backend**
exactly as it would a loaded one, so scripting on the web is waiting on a
language backend written in pure Rust rather than on any missing plumbing.

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

### The tab opens on what your project actually uses

Every time you open the export dialog it reads the project — the scenes first,
then the scripts, the markup templates and the authored assets — and ticks the
features that content needs. A 2D game opens with the 3D pipeline already off; a
game with no terrain opens with terrain off; the plugin list beside it is
pre-ticked the same way.

It can be exact about this because a `.bsn` scene names every component by its
full Rust path, so `renzora_terrain::data::TerrainData` sitting in a scene is
proof that terrain is used, and its absence from every scene is proof that it
isn't. Subsystems a script can reach without naming a type are matched on the
script API instead — a Lua file calling `play_sound` keeps audio, one calling
`parkour_jump` keeps the traversal controller.

The scan runs on **every** fresh open, so editing a scene and re-opening the
dialog changes what it offers — saved presets included. A preset stores the
platform, packaging, output path, window settings and plugin choice; the feature
toggles are re-read from the project each time, because a preset's copy of them
is captured automatically when you close or export rather than chosen. (Switching
preset *while the dialog is open* does restore that preset's toggles, which is
the case where you did pick them.)

Three things it deliberately won't decide, because "no evidence" isn't the same
as "not used":

- **A project with no scenes** — everything falls back to the engine defaults, so
  a freshly created project doesn't export as an empty shell.
- **A project with no renderer in evidence** (a pure-UI menu, say) keeps *both*
  pipelines. A game a few MB larger beats a game that can't draw.
- **Which physics backend** — avian2d and avian3d share their components and
  their script API, so the scene genuinely doesn't record the answer. Physics
  follows the pipeline, which is what the engine does at runtime anyway.

Everything is still a toggle. If the scan gets a call wrong for your project —
you spawn a terrain from a script that names nothing, say — untick or tick it and
carry on, and save that as a preset if you want it to stick.

The two kinds of default still apply to whatever the scan can't speak to:

- **Structural subsystems** (localisation, gamepad input, …) default to **on**.
- **Safe leaves** (raytraced lighting, debug gizmos, system diagnostics, editor
  conveniences) default to **off**, because nothing needs them unless you say so.

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

### Three that need the right question asked

Some capabilities look used when they are not, because what a scene records is
not what the game does. These three had been on for every project:

**Picking** is not implied by a HUD. `bevy_ui` inserts `Pickable` and
`PickingInteraction` on every UI node as required components, so a project with a
menu saved hundreds of them into its scenes. What the toggle actually controls is
*world* raycasting, so it now follows a `Pointer<…>` observer or a picking
settings component instead. UI hit-testing is a separate bevy feature that rides
the UI capability and is unaffected.

**Tonemapping lookup tables** follow the curve your scene names. Five of Bevy's
eight tone curves need no tables at all; only AgX, TonyMcMapface and
BlenderFilmic sample them. If the tables turn out to be wanted after all, the
engine substitutes a table-free curve rather than failing — so the worst case
here is a slightly different picture, not a broken build.

**Translation packs** — about 2.4 MB of `languages/*.toml` compiled in as data —
follow a real `renzora::lang::t(…)` call in your project, or a `languages/`
folder of your own packs beside it. There is no project-level language field to
read: the active language is a *per-user* preference in
`~/.renzora/editor.toml`. Without the packs every `t()` returns its key's own
English text, which is what a build with no packs has always done.

### Nested features

Some entries have children, shown indented. A child is always a strict subset of
its parent, so turning the parent off takes every child with it — leaving, say,
"advanced PBR texture maps" on while 3D rendering is off would pull the whole PBR
pipeline back in and undo the saving. Current groups:

| Parent | Children |
|---|---|
| **3D rendering** | graph materials, glTF loading, morph targets, advanced PBR texture maps, lighting lookup tables |
| **Post-processing** | bloom, SSAO, SSR, depth of field, motion blur, distance fog, volumetric fog, lens distortion, order-independent transparency, anti-aliasing |
| **User interface** | Bevy's built-in font, Game UI (markup) |

Turning a parent off switches its children off **in the list**, not just at build
time. So does turning **3D rendering** off, which takes terrain, water, the sky
set, every post-process effect, Lumen, cloth, ragdolls, parkour, gaussian
splatting, forward decals and raytraced lighting with it — all of them are
`bevy_pbr` underneath, so keeping one is not a bigger binary, it is a build that
does not compile. The build has always enforced that; the list now shows it,
instead of leaving twenty green toggles for features about to be dropped.

The cascade only ever switches things **off**. Turning 3D rendering back on
grants permission rather than restoring a guess about what you wanted, so the
subsystems stay off until you tick the ones you need.

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
  water, splines, particles, wind, the sky set and the post-process effects — and
  the world-space UI, which draws a canvas onto a quad in the 3D scene. A
  fullscreen canvas is unaffected; a canvas whose render space is `world` simply
  has nothing to draw onto in a 2D build.
- Turning **Debug gizmos** off (the default) removes the immediate-mode drawing
  API *and* the engine code that calls it, such as the 2D-light selection
  outlines. Nothing a shipped game shows depends on it.

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

**Multiplayer is a strippable feature now.** The `Multiplayer (networking)` toggle
in the Features tab drops the UDP transport, replication, script RPC and both
server startup paths, and the scan turns it off for a project with no replicated
entities and no `rpc` / `net_*` calls in its scripts — which is most single-player
games. It was compiled into every export until this became a toggle. Ticking
**Include dedicated server** turns it back on, since a server with no transport
would be a file that cannot do anything; and a lean binary built without it says
so and starts as an ordinary client if you pass `--server`, rather than looking
like the flag was ignored.

## What goes in the archive

The `.rpak` does not hold your whole project. It holds what the game needs, found
by starting at `project.toml`'s `main_scene` and following every **quoted asset
path** outwards — a scene names its sprites, a sprite names its texture, a
markup template names its images — until nothing new turns up. Rust scripts are
read in place for the same references: they are compiled into the binary rather
than packed, so nothing else would ever have opened them.

That is a good default and it cannot be a complete one. A path a script builds at
runtime — `format!("levels/{n}.bsn")` — is not a literal anywhere, so nothing can
find it. A file the crawl misses is a game that runs and then logs
`Path not found: …` once a frame, which is a poor way to discover it.

### The Files tab

So the **Files** tab shows every file in the project as a folder tree, ticked as
the crawl would pack it, and lets you settle it yourself. Folders tick and untick
everything beneath them; a folder reads as ticked only when all of its files are.

From the moment you open the tab it is **authoritative** — the archive holds what
the list shows and nothing else. That is deliberate: a tab that merely *added* to
an invisible automatic set could not explain what it was going to ship, and could
not be used to leave something out.

- **Select all** / **Select none** for the broad strokes.
- **Reset to detected** goes back to the crawl's answer, re-running it — so it
  also picks up anything you have added since the dialog opened.

The ticks are worked out the first time you open the tab, not when the dialog
opens: it means reading the project, and that is not a cost to pay for a dialog
you opened to change the output path.

The selection is **per session**, not saved into a preset. A file list goes stale
in a way a feature toggle does not — you add and delete files as you work — and a
saved list would quietly stop shipping something you added. Every fresh open
starts from what the project says today.

## Platform notes

### Web (WASM)

The web build is **game-runtime only** — there is no WebAssembly editor. It runs on **WebGPU**, and several native-only subsystems compile to no-ops in the browser:

- **Lua does not run** on `wasm32`, so neither do blueprints (they compile to Lua and share its VM). The obstacle is no longer `dlopen`: a lean web export links its C-ABI plugins into the module and the host adopts a linked-in language backend exactly as it would a loaded one. It is `plugins/lua` itself — mlua builds Lua from **C**, and `wasm32-unknown-unknown` has no libc sysroot for that C to compile against. A language backend written in pure Rust would work on the web today. Until one exists, web-targeted logic has to live in Rust. (The `.rhai` backend that used to fill this gap has been removed.)
- The DAW and the mixer are editor-only. Audio itself needs a browser backend built against WebAudio — the bundled `plugins/audio` is native, because cpal cannot capture on the web. See [Audio backends](../extending/audio-backends.md).
- Networking is a no-op stub (no native UDP), so multiplayer is unavailable on web.

### Android / iOS

Android and iOS export by injecting your `game.rpak` into a prebuilt template (`.apk` for Android, `.app`/`.ipa` for iOS). The runtime is a thin platform shim around the engine — `renzora-android` produces `libmain.so`; `renzora-ios` produces a `librenzora_ios.a` staticlib. Signing and store submission are handled with the platform's own tooling (a keystore for Android, an Apple Developer account + Xcode/TestFlight for iOS).

## What's next

- [Installation → Working from a checkout](/docs/r1-alpha7/getting-started/installation) — the `renzora` CLI and Docker cross-compile setup behind these builds.
- [Multiplayer → Server setup](/docs/r1-alpha7/multiplayer/server-setup) — running the exported dedicated server.

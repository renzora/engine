# Native Plugins (full World)

A **native plugin** is an ordinary Bevy plugin, shipped as Rust source, compiled on the machine that installs it, and loaded by the editor at startup. It gets `&mut World`. Not a filtered view of it, not a command vocabulary — the same access an exclusive system has.

```rust
use bevy::prelude::*;

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, my_system);
    }
}

fn my_system(mut q: Query<&mut Transform>) { /* … */ }

renzora::plugin!(MyPlugin);
```

That last line is the only Renzora-specific thing about it. Everything above it is a Bevy plugin you could have written for any Bevy app.

## Which kind of plugin do I want?

Renzora has two plugin mechanisms and they are not competing — they serve different deployments.

| | **Native plugin** | **[Standalone C-ABI plugin](standalone-plugins.md)** |
|---|---|---|
| Crate type | `dylib` | `cdylib` |
| Links Bevy | yes, the engine's own | no |
| Access | full `&mut World`, any Bevy API | a fixed function table |
| Typical size | 100–500 KB | 20–100 KB |
| Ships as | source, compiled on install | a prebuilt binary |
| Runs in a shipped game | **no** | yes |
| Runs on wasm / mobile | **no** | yes |
| Needs the plugin SDK | yes | no |

Rule of thumb: **a native plugin extends the editor; a C-ABI plugin ships inside the game.** If your plugin adds a panel, a tool, a validator, an importer or a viewport gizmo, it is native. If it is a post-process effect, a script language or anything the player's copy of the game needs at runtime, it is C-ABI.

The reason is structural, not a missing feature. A native plugin links the shared `bevy_dylib` / `renzora_dylib` / `renzora_ember_dylib` images; a lean export is a fully static single binary with no shared images at all, and wasm and mobile have no dylibs to load. There is nothing for a native plugin to bind to there.

## Layout

Both kinds live in `plugins/`. A native plugin is a **directory**; a C-ABI plugin is a loose library file. The two loaders never collide.

```
<editor dir>/
  bevy_dylib-<hash>.dll     renzora_dylib.dll     renzora_ember_dylib.dll
  sdk/                                            the plugin SDK, optional
  plugins/
    grayscale.dll                                 C-ABI: prebuilt, 52 KB
    my-plugin/                                    native: shipped as source
      Cargo.toml
      src/lib.rs
      build/my_plugin.dll                         what rustc produced
      build/stamp.txt                             what it was built against
```

`src/lib.rs` is what makes a directory a plugin. Submodules work — `src/ui/panel.rs` and deeper are staged and watched.

## Building

From a source checkout, `cargo renzora` builds every native plugin under `plugins/` as part of staging. From a downloaded editor, there is nothing to run: the editor compiles a plugin the first time it sees one, and again whenever the engine moves under it.

**Do not run `cargo build` in a native plugin directory.** `plugins/` sits outside the engine workspace, so cargo would resolve it a fresh Bevy from crates.io — a different compilation with different `TypeId`s. The result builds cleanly, loads, and corrupts the World. The `bevy` and `renzora` entries in your `Cargo.toml` are there so rust-analyzer can resolve them while you author, and so Bevy's derive macros can read the manifest; the plugin itself is compiled by `rustc` against the SDK, and cargo is never pointed at those entries. (Your *other* dependencies are built by cargo — from a stripped manifest that mentions no Bevy. See [Crates from crates.io](#crates-from-cratesio).)

### The SDK

Compiling against the engine you already have needs what `rustc` reads at compile time — crate metadata, proc-macro dylibs, native import libraries. That is the **plugin SDK**.

It rides inside the engine download as a single compressed `sdk.tar.zst` (~444 MB) beside the executables, and is unpacked to `sdk/` once per update. Rust scripts need it as much as plugins do, so that unpack is part of setting the engine up rather than an optional extra. Bundling it rather than fetching it from somewhere is deliberate: there is no URL, no resume, no checksum and no offline case to handle, and a version mismatch is structurally impossible because the SDK in the folder is by construction the one that built the editor next to it.

zstd rather than xz, even though xz is 103 MB smaller: unpacking is on everyone's path now, so its cost is paid by every user while the download's is paid once. Measured, that trade buys a 29.8 s unpack down to ~2 s of decode, and removes a 1.9 GB temporary file along the way.

The metadata is staged as `.rmeta` files rather than the `.rlib` archives cargo produced. An `.rlib` holds two things — the crate's metadata and its compiled object code — and a plugin build consumes only the first: it typechecks against the metadata and takes the *code* from the three shared images (`bevy_dylib`, `renzora_dylib`, `renzora_ember_dylib`), which is what those exist for. Dropping the object half is what takes the extracted SDK from 3.3 GB to 1.5 GB, with byte-identical plugins out the other end.

This is safe because `rustc` will not link a crate's code from thin air. Under `-C prefer-dynamic` a crate whose objects are already inside a dylib being linked is taken from there; anything else demands its archive and says so — `error[E0461]: crate 'X' required to be available in rlib format`. There is no quiet failure. The one crate staged whole is the `bevy` facade itself: it lives inside `renzora_dylib` rather than `bevy_dylib`, so a plugin that imported `bevy` and touched nothing else would hit E0461 from metadata alone. That plugin could never load anyway — `renzora::plugin!` writes the symbol the loader looks for — but 42 KB buys the simpler rule, so `bevy` keeps its `.rlib`.

Without it, an already-built plugin still loads. What is lost is the ability to build or rebuild one.

The SDK is pinned to one exact `rustc` — Rust's crate metadata format is versioned and the compiler refuses another version's. The editor checks this before compiling and tells you which toolchain to install, rather than letting you find out from `error[E0514]`.

### Staleness and rebuilds

A plugin is bound to one engine build. Its metadata, its `TypeId`s and its imports all come from that build's artifacts, so a plugin built against a different engine is not "probably fine" — it is memory corruption with no diagnostic.

So each build records a **stamp**: a content hash of the shared images the plugin links, plus the rustc version. On load the editor compares. A mismatch rebuilds — about a second, and invisible — rather than loading.

This is the property that makes source-shipped plugins better than prebuilt ones. Update the engine and every installed plugin quietly rebuilds itself against the new one. Nothing has to be republished, and no plugin silently rots.

Editing `src/` rebuilds too, on the next launch. That is the whole iteration loop for someone working from a downloaded editor with no repository and no `cargo`.

## Turning plugins on and off

**Settings → Editor → Plugins** lists every plugin the engine found this launch — both kinds, in one list — with a switch each and a line saying what actually happened to it: *Active*, *Disabled*, a reason it was skipped, or the error it failed with.

The list is built from what the loaders reported, not from a directory scan, so it always matches what the engine really did. A plugin that failed to compile shows the first line of the error there and the whole thing in the Console.

**Changes take effect at the next launch.** That is structural rather than unfinished: a plugin adds systems, resources and function pointers to the `App` while it is being assembled, and Bevy cannot withdraw them. Unmapping the image is worse — a retired system is still *in* the schedule, merely returning early. So the switch records intent, and the loader acts on it at startup.

A disabled plugin costs nothing at all: it is skipped before the directory is touched, so there is no rebuild if its stamp is stale, no `dlopen`, and none of its static initializers run. That matters when you are disabling one to find out whether it is the plugin breaking your editor — half-running it would tell you nothing.

The list lives in `~/.renzora/editor.toml` under `disabled_plugins`, keyed by a native plugin's directory name or a standalone plugin's library stem (with any `lib` prefix stripped, so the same file is named the same thing on every platform). It is hand-editable if you have managed to disable the plugin that draws the settings panel.

## What you can reach

Three crates, and they are enough for almost anything:

- **`bevy`** — all of it. Queries, assets, schedules, render, UI, gizmos, `bsn!`.
- **`renzora`** — the contract crate. Every type that crosses a boundary in this engine lives here on purpose, so a plugin shares them rather than mirroring them: `EditorSelection`, `PlayModeState`, `SplashState`, `CurrentProject`, `lang::t()`, the Console buffer, the post-process framework, the shell registries.
- **`renzora_ember`** — the editor's UI framework. See **[Editor Panels from a Plugin](panels.md)**.

Anything else in the workspace is not reachable. If your plugin needs a type from another `renzora_*` crate, that type belongs in `renzora` — that is the rule the engine's own crates follow.

### Crates from crates.io

Add them to your plugin's `Cargo.toml` like any Rust project:

```toml
[dependencies]
bevy = "0.19"
renzora = { path = "../../crates/renzora" }

noise = "0.9"        # ← an ordinary crates.io dependency
```

That is all.

**How it can be that simple, given `cargo build` here would corrupt the World.** It would — and nothing here runs cargo on your manifest. The build reads your `[dependencies]`, strips `bevy` and every `renzora*`, and writes what is left into a **separate manifest that mentions no Bevy**. Cargo builds *that*, and the resulting rlibs are handed to your plugin's `rustc` as extra `--extern`s, alongside Bevy and the contract crate which still come from the SDK. Cargo resolves `noise`; cargo never resolves Bevy. The hazard isn't avoided by discipline, it is unreachable.

**A dependency that itself depends on Bevy is refused**, with a message naming it. The graph is resolved before anything compiles, so that costs seconds rather than a half-hour build of a Bevy that must not exist. If you need such a crate, use a [C-ABI plugin](standalone-plugins.md) — it shares no types with the engine and may depend on anything.

**A duplicate crate is harmless.** If you depend on `serde` and the engine already links its own, you get a second, privately linked copy. That matters only for crates holding process-global state — which is exactly why `renzora` and `renzora_ember` are shared images and not ordinary dependencies. And if such a type ever tried to cross into an engine API, the two copies are different types to the compiler: a compile error, not silent corruption.

Two things to know. Dependencies must be written **one per line** (`foo = { version = "1" }`); the `[dependencies.foo]` sub-table form is refused rather than silently ignored. And this is the one part of plugin building that needs a **network** — the SDK is otherwise entirely offline. A plugin that declares no third-party crates never invokes cargo at all and keeps the offline, one-second build.

## Things that will catch you once

**Spawn on `OnEnter(SplashState::Editor)`, not `Startup`.** The project-load teardown despawns everything carrying a `Name`, and it runs *after* `Startup`. A named entity spawned in `Startup` appears and then vanishes.

**But do give scene content a `Name`.** The hierarchy panel queries `(Entity, &Name)` — no name, no row, nothing to click, no selection and no gizmo. And the scene serializer only writes named entities, so an unnamed mesh is gone after a reload.

**`bsn!` parses component arguments as literal values.** `Mesh3d(mesh.clone())` fails with "Unexpected input after function name". Clone into a named binding first.

**The editor contract is glob-re-exported at the crate root.** Write `renzora::EditorSelection`, never `renzora::editor_contract::EditorSelection`.

**rustc's diagnostics name `renzora_dylib` and `renzora_ember_dylib`.** Those are the shared images' real crate names; the `--extern` alias hides them everywhere except error messages.

## Limits

- **Nothing is ever unloaded.** Every system a plugin registered is a function pointer into its image, and a Bevy schedule holds those for the life of the `App`. Unmapping the image turns them into dangling pointers, so a reload leaks the old one (a few hundred KB) and a restart reclaims it.
- **Loading is synchronous.** A stale plugin rebuilds during app assembly, holding startup for about a second each.
- **No undo integration yet.** A plugin can mutate the World freely but cannot push onto the editor's undo stack — that lives outside the contract crate.
- **A plugin can crash the editor.** A panic while constructing or during load is caught; a segfault is not. Installing a plugin runs its code, and the source is on disk to read.

## Examples

Reference plugins ship in `plugins/`, in increasing order of what they prove:

| | |
|---|---|
| `hello-native` | the boundary works — one exclusive-`&mut World` system, one `t()` call |
| `spinning-cube` | ordinary Bevy: a mesh, a material, a transform rotated each frame |
| `native-grayscale` | a post-process effect registered from a plugin |
| `orrery` | a `bsn!` hierarchy, run-time assets, and reading `EditorSelection` to draw gizmos |

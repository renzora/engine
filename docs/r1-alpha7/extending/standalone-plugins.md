# Standalone Plugins (C ABI)

Write a plugin as a self-contained `cdylib` that never links Bevy, build it with any Rust toolchain on any machine, and drop the resulting library into `<exe>/plugins/`.

> This is a **second, independent** plugin mechanism, not a replacement for the one in [Building Plugins](./plugins.md). Both exist because they solve different problems — see [Which one to use](#which-one-to-use).

## Why this exists

A [distribution plugin](./plugins.md) shares one compiled `bevy_dylib` with the host. That sharing is what makes it fast and complete — it gets *all* of Bevy — but it also means the plugin must be built in the same environment as the editor it loads into. Cargo names the shared library `bevy_dylib-<metadata>.dll`, where the metadata hashes the package id, feature set, profile, `RUSTFLAGS`, target and rustc. Build the plugin somewhere else and it imports a differently-named library that isn't beside the exe, and the OS loader fails it before any Renzora code runs.

A standalone plugin sidesteps that entirely:

- **It exports two symbols** — `renzora_plugin_init`, which does everything, and `renzora_plugin_scope`, which the host reads first to decide whether the plugin belongs in this binary at all — and **imports nothing** from the host.
- The host passes a `#[repr(C)]` function table *in* at load time.

There is no dynamic symbol to resolve against `renzora.exe`, so there is no filename to match, no `bevy_dylib-<hash>` to find, and no `TypeId` to line up. The only thing both sides must agree on is the layout of a handful of `#[repr(C)]` structs. That means a plugin built with rustc 1.90 loads into an editor built with rustc 1.95, and a plugin built in 2026 keeps loading into editors released later.

The price is that a standalone plugin reaches Bevy through a curated surface rather than all of it. That surface is designed to read *identically* to Bevy source — see [What it looks like](#what-it-looks-like). It covers components, resources, queries, systems, commands, assets, [generated geometry](#geometry) and [textures](#textures), [custom materials](#custom-materials), render passes, post-process effects, [animation](#animation), [physics](#physics), [HTTP](#http), scene serialization, and [editor panels](#editor-panels).

## Which one to use

| | [Distribution plugin](./plugins.md) | Standalone plugin |
|---|---|---|
| Links Bevy | yes, shares the host's `bevy_dylib` | no |
| Toolchain | must match the canonical build env | any |
| Bevy surface | all of it | the ABI surface |
| Editor panels | bevy_ui, in Rust | BSN + ember widgets |
| [Hot reload](#hot-reload) | no — restart the editor | yes, while it runs |
| Binary size | small (Bevy is shared) | ~210 KB (std linked statically) |
| Registers with | `renzora::add!` | `renzora_plugin::add!` |
| Breaks when | the editor's ABI moves | only on a MAJOR ABI bump |

Reach for a standalone plugin when you want to ship a prebuilt binary to people running editor versions you don't control, or when you'd rather not maintain a Docker toolchain to build a plugin. Reach for a distribution plugin when you need a part of Bevy the ABI doesn't expose yet.

In practice that line falls in a consistent place, and it is more useful than the table: **gameplay and geometry are well covered; rendering *integration* is not.** A plugin can simulate anything, generate any mesh, and put its own shader on it. As soon as it needs to know where the camera is, which way the light points, or what another pass wrote, it wants to be in-tree. See [Current limits](#current-limits).

## Traps

Everything a plugin *cannot* do fails at compile time, which costs you five minutes and a
lookup. This section is the other list: code that **compiles, runs, and does something other
than what the same source does in Bevy**, with no error and no warning.

Read it before you write anything. It is short on purpose, and it is the most expensive page
in this documentation to skip.

Three entries have been deleted from this list rather than reworded, which is the intended fate
of every one of them:

- A component with a `String`, `Vec` or `Box` field used to compile and then corrupt. It is now
  a compile error naming the fix.
- `Query::iter()` used to hand out `&mut` from a shared borrow, so nesting it inside a mutable
  loop aliased with no `unsafe` anywhere. `iter()` now yields the read-only projection and the
  borrow checker refuses the nested case.
- `Transform::rotate_x/y/z` used to be Bevy's `rotate_local_*`. They now match Bevy, and
  `rotate_local_*` exists for the other behaviour.

The reason this list exists at all is a deliberate design choice with a cost. The plugin API is
built to be *source-identical* to Bevy, so that porting is a change to the `use` line — which
means you are entitled to assume Bevy's semantics from Bevy's spelling. Every divergence below
is a place that promise is not kept, and the closer the surface reads like Bevy, the more each
one costs.

### Plugin systems run while you are editing

A plugin's `Update` systems have no play-state gate — they run in the editor viewport at all
times. Gameplay logic mutates the scene you are authoring, and those mutations are what gets
saved. If a system should only run during play, gate it yourself on your own resource.

### Anything you spawn without a name is not saved

Scene save collects only entities `With<Name>`. `commands.spawn((MyComp, Transform))` adds no
name, so it is silently absent from every saved scene. In BSN, the `#Key` prefix is what adds
one.

### `insert(bsn! { .. })` replaces, it does not patch

Inserting a component onto an existing entity builds the value from the type's `Default` and
applies only the fields you named. Setting `translation` therefore **resets `rotation` and
`scale`**. Safe on a marker component, destructive on a live `Camera` or `PointLight`.

Only the *first* top-level tree targets the entity — later ones become loose parentless roots.

### A mistyped type path matches nothing, forever

Host component names resolve by string at runtime. A typo in a `host_component!` path or a BSN
component name is a log line at most: the query compiles, matches zero entities, and keeps
doing so. There is no compile-time check and no plugin-visible error, because checking would
require the derive to know the engine's registry and a plugin links nothing.

Assert your mirrors at startup if you can, and check the log when a query is mysteriously empty.

### `remove::<T>()` for an unregistered type is a silent no-op

`insert` logs an error when the component was never registered; `remove` does not. Call
`app.register_component::<T>()` in `build()` for every type you insert *or* remove — including
host types.

### Several BSN constructs parse and are then dropped

These are accepted by the parser and discarded with at most a `warn!`, with no plugin-visible
result: field shorthand (`Comp { name }`), `on(|ev| …)` handlers, and a relationship target
other than `Children` (`MyRel [ … ]` spawns **children** regardless of the name). Loud failures,
by contrast, are `~Template`, `@SceneComponent` and `:"file.bsn"`.

### `Changed<T>` fires less often than it would in Bevy

A plugin's `&mut` write only marks a component changed **when the bytes actually change**. Bevy marks it the moment you take `&mut`, whether or not you wrote anything different; the host compares your staged bytes against a snapshot taken before your system ran and skips the write when they match, and skipping the write is what skips the tick.

So a `Changed<T>` watcher — another plugin system, or an ordinary engine system — fires less often for a plugin-written component than for the same feature written in-tree. There is no `set_changed()` in the ABI, so you cannot notify a downstream system without changing a byte, and the Bevy idiom `let _ = &mut *foo;` compiles here and does nothing.

The comparison is bitwise over the whole component, padding included. It errs loud rather than quiet: it can report a change that did not really happen (`-0.0` differs from `0.0`) but it cannot miss one that did.

This is deliberate and load-bearing. Unconditional write-back marked every matched component changed every frame, which does not merely cost time — it destroys change detection for the whole engine, because `Changed<Transform>` anywhere becomes true whenever any plugin so much as looks at a transform.

### `Added<T>` fires for the whole scene after a hot reload

Reloading keeps your components and resources — that is what makes hot reload tractable — but it registers *new systems*, and a Bevy system that has never run treats everything already in the world as freshly added.

Do not use `Added<T>` for one-time setup. Reload is the inner loop of plugin development, so you will hit this.

### `Added<T>` and `Changed<T>` cannot go inside `Or`

The host refuses the whole system at load, naming the term. A change-tick test is a per-row predicate and the host's query builder has no tick dimension, so the branch would come out empty — and an empty branch in an `Or` matches **every entity in the world**.

At the top level of a filter tuple they work normally: `(Changed<A>, With<B>)` and `(Changed<A>, Changed<B>)` are both fine, and mean "and", as in Bevy.

### Smaller ones worth knowing


- **`usize`, `i64` and `u32` fields are all edited as 32-bit** in the inspector, which
  reads and writes four bytes.
- **`#[derive(Component)]` on a tuple struct** registers with an empty field schema — only
  named fields are walked. Nothing appears in the inspector and nothing round-trips.
- **A field whose type the ABI cannot describe is dropped from the schema** silently, while
  still counting toward the component's size.
- **`PanelActionId` must sit on an entity that carries `Interaction`.** Bevy's `Button` does;
  `EmberButtonWidget` does not, because it builds its clickable box as a child. The pairing
  compiles, spawns, looks correct, and never dispatches.
- **Two plugin systems cannot be ordered against each other yet**, so a `Changed<T>` handoff between two of your own systems may be seen this frame or next, at the scheduler's choice. Bevy would let you write `.after(a)`. Do not build a pipeline out of `Changed<T>` between your own systems until the ABI has ordering — it will work most runs and jitter by a frame on others.
- **`Changed<T>` implies `With<T>` and takes a read borrow**, exactly as in Bevy: `Query<&Bar, Changed<Foo>>` matches only entities that have `Foo`, and will not run in parallel with anything writing `Foo`.
- **A layout change means a restart.** Adding or removing a component field, or changing its
  type, is refused by [hot reload](#hot-reload) — existing entities hold bytes for the old
  layout. Renames are free. This is the edit you will make most often while iterating.

## What it looks like

This is a complete, working plugin:

```rust
use renzora_plugin::prelude::*;

#[derive(Component)]
pub struct Spinner {
    pub speed: f32,
}

impl Default for Spinner {
    fn default() -> Self {
        Self { speed: 1.0 }
    }
}

fn spin(mut q: Query<(&mut Transform, &Spinner)>, time: Res<Time>) {
    for (t, s) in &mut q {
        t.rotate_y(s.speed * time.delta_secs());
    }
}

pub struct SpinnerPlugin;

impl Plugin for SpinnerPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<Spinner>()
            .add_systems(Update, spin);
    }
}

renzora_plugin::add!(SpinnerPlugin);
```

Apart from the import on the first line, that is Bevy source. The `Query`, `Res<Time>`, `Transform`, `Plugin` and `App` here are shims in `renzora_plugin::ecs` that mirror Bevy's API — but the code you write against them is the code you'd write against Bevy.

Under the hood `spin` is registered as a *dynamic* Bevy system built with `QueryParamBuilder`, so it carries real component access and **schedules in parallel** with the engine's own systems. It is not a callback on a side channel.

## Getting started

```toml
# Cargo.toml
[workspace]                      # see "Keep it out of the workspace" below

[package]
name = "my_plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
renzora_plugin = { path = "../../crates/renzora_plugin" }   # crates.io in a future release

[profile.dist]
inherits = "release"
opt-level = 2
strip = "symbols"
```

Build with `cargo build --release` and copy `target/release/my_plugin.dll` (`.so` / `.dylib` elsewhere) into `<exe-dir>/plugins/`. The editor loads everything in that directory at startup.

### Link std statically

Rust does this by default, so a plugin built anywhere outside an engine checkout is already correct and you can skip this section.

It matters because of what ends up in the plugin's **import table** — the list of libraries the OS must find before any Renzora code runs. Built correctly, a plugin's import table names nothing but the operating system:

```
KERNEL32.dll                        82 symbols
ntdll.dll                            2 symbols
api-ms-win-core-synch-l1-2-0.dll     3 symbols
```

With `-C prefer-dynamic`, it instead names `std-0cebe7c42cd80226.dll` — and that hash identifies one exact toolchain build. A plugin compiled with a different rustc names a different file, which isn't beside the executable, and the OS refuses to load it. That is the same trap as `bevy_dylib-<metadata>`, arriving by a different route, and it defeats the entire point of building without Bevy.

It's an easy one to miss because it fails late. Inside an engine checkout with the pinned toolchain, the matching `std` library is already staged beside the exe, so the plugin loads and everything looks fine — right up until someone with a different rustc tries the same binary.

If your plugin **does** live inside an engine checkout, you need to override it. Cargo discovers config by walking up from the working directory and does not stop at a workspace root, so your plugin inherits the engine's `.cargo/config.toml` even though it declares `[workspace]`. The engine sets `prefer-dynamic` so the executable, the editor bundle and distribution plugins can share one `bevy_dylib` — correct for them, wrong for you.

`plugins/.cargo/config.toml` in the engine repo already does this for the bundled examples:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "prefer-dynamic=no"]
```

Note the explicit `=no`. Cargo **merges** `rustflags` arrays across config files rather than replacing them, so omitting the flag achieves nothing — it has to be contradicted by a later entry.

**Not `+crt-static`.** It used to be there, and the reasoning was that statically linking the MSVC runtime drops `VCRUNTIME140.dll` and the `api-ms-win-crt-*` set, leaving nothing in the import table but the OS. That works, and it is not worth it — measured on a minimal plugin, release, symbols stripped:

| Config | Size | Imports |
|---|---|---|
| inherits the engine's `prefer-dynamic` | 16 KB | `std-<hash>.dll` — **broken**, see above |
| `prefer-dynamic=no` | 113 KB | `VCRUNTIME140` + the crt set |
| `prefer-dynamic=no` + `crt-static` | 219 KB | `KERNEL32` only |

About half the binary to remove imports the host already has: `renzora.exe` is itself an MSVC build and names `VCRUNTIME140.dll` and the whole `api-ms-win-crt-*` set, so every one of them is loaded into the process before a plugin is ever mapped. The dependency a plugin "avoids" is one the thing loading it already took.

The heap is the only substantive consideration, and it points the same way: without `crt-static` a plugin allocates from the **host's** CRT heap rather than a private one. That is fine, and marginally safer — the boundary never transfers ownership of an allocation, since plugin components are refused outright if they declare a destructor and command payloads are copied by the host.

### Drop std entirely (`no_std`)

Optional, off by default, and worth roughly 6× on size. A plugin that gives up the standard library builds to a fraction of one that keeps it:

| Build | `pulse.dll` |
|---|---|
| `std`, `dist` profile | 112,640 B |
| `std` + `panic = "abort"` | 108,032 B |
| `std` + fat LTO + `opt-level = "z"` + abort | 92,160 B |
| **`no_std`** | **17,408 B** |

Note the third row: this is not something optimisation flags get you. Every `dlopen`'d plugin carries its own copy of std, so ~90 KB is a floor per file.

**59 of the 63 plugins in this repo are built this way.** Measured across that set:

| | Total | Average |
|---|---|---|
| `std` | 6,608,896 B (6.30 MB) | 112,015 B |
| `no_std` | 1,091,584 B (1.04 MB) | 18,501 B |

— a 5.5 MB saving, 83.5%, for the same 59 plugins. The four that stay on `std` are `lua` (its `script` backend needs `std`, and mlua links libc regardless), `text3d` (reads font files with `std::fs`), `hair` (`HashMap` and a `Mutex` static) and `wobble` (its `noise` dependency pulls `num-traits` with default features, and cargo's feature unification is additive — a downstream crate cannot subtract `std` from a transitive dependency).

**What it costs is the panic firewall, and you should not skip past this.** Today a panicking plugin system is caught, reported, and the host disables *that system* while the editor keeps running. `catch_unwind` lives in std and has no `core` equivalent, so a `no_std` plugin cannot do that: a panic reaches an `extern "C"` frame and takes the whole process with it — the editor, and any unsaved scene work in it. `no_std` on stable also cannot unwind at all, which is why `panic = "abort"` below is mandatory rather than a suggestion.

So this suits a plugin whose failure modes you own — a post-process effect that is a shader plus a couple of arithmetic systems — and not a large gameplay plugin doing real indexing work, where the firewall is earning its 90 KB.

Three changes. In `Cargo.toml`:

```toml
[dependencies]
renzora_plugin = { path = "../../crates/renzora_plugin", default-features = false, features = ["libm"] }

[profile.dist]
inherits = "release"
opt-level = 2
strip = "symbols"
panic = "abort"        # mandatory — no_std on stable cannot unwind
```

And at the top of `src/lib.rs`:

```rust
#![no_std]
extern crate alloc;
renzora_plugin::no_std_runtime!();
```

`no_std_runtime!()` supplies the two lang items the standard library would have: a global allocator and a panic handler. The allocator is the **host process's own `malloc`/`free`**, which is deliberate — your plugin is loaded into a running engine that already has an initialised C runtime, so it shares one heap with everything else rather than carrying a second. That also keeps a buffer safe to free after it has crossed the boundary. The panic handler aborts, because there is nothing else it can do.

It also emits no-op stubs for `rust_eh_personality` and `_Unwind_Resume`. The precompiled `core`/`alloc` in the sysroot are built with unwinding, and their objects can leave those symbols undefined in the cdylib even with `panic = "abort"` in the profile. That is harmless at link time but fatal at load time — `dlopen` resolves eagerly and the host does not (and must not) export them — so without the stubs, every `no_std` plugin failed to load with `undefined symbol: rust_eh_personality`. With `panic = "abort"` neither stub can ever actually run; they exist purely so the dynamic linker finds a definition.

On macOS the crate also names `libSystem` for the link. Dropping `std` drops the only thing carrying the `#[link]` for libc, so rustc drives the linker with `-nodefaultlibs` and nothing supplies `malloc`, `free`, `abort` or `memcpy`. Linux never notices — `ld` allows a shared object to leave symbols undefined and lets the loader find them in the host process — but `ld64` resolves a dylib up front, so before this the build simply failed with a list of undefined C runtime symbols. Nothing about a plugin changes; the dependency was always there, it is just written down now.

The macro expands to nothing when emitting those items would be wrong — under `std`, and under `static_link` where the host binary already provides both — so it is safe to leave in place whatever way the plugin ends up being linked.

Everything else is unchanged: same `add!`, same exports (`renzora_plugin_init`, `renzora_plugin_scope`), same loader path. The engine cannot tell the difference.

Things to know:

- **`features = ["libm"]` is not optional.** `f32::sqrt`, `sin`, `acos`, `powf` and friends are std methods with no `core` equivalent — they lower to libm calls that `core` does not link — and `Vec3::length`, the `Quat` constructors and the colour conversions all need them. Leaving it out is a clear compile error rather than a mysterious one, but it is still a step you have to take. (`glam` and `nalgebra` use the same two-feature shape, so it may look familiar.)
- **Your own float maths keeps working.** The prelude exports `FloatExt` in a `no_std` build, which restores `sin`, `cos`, `powf`, `floor`, `fract` and the rest on `f32` with `std`'s exact signatures — so `x.sin()` compiles unchanged. It is only exported when `std` is off, so there is never a moment where a trait method and an inherent method both want to resolve the same call.
- **`Vec`, `String`, `Box`, `vec!` and `format!` are in the prelude too.** Bevy's prelude does not carry them because the std prelude does, but a `no_std` plugin has no std prelude. They come from `alloc`, and under `std` they are the identical items, so this changes nothing for a plugin that keeps it.
- **`script` and `host` require `std`.** A language backend parks its interpreter in a `std::sync::Mutex` and, in practice, links something like mlua that needs libc anyway — 90 KB is noise next to an interpreter. `host` is the engine side of the boundary and links Bevy.
- **A `no_std` plugin still links into a static export.** When the lean exporter compiles plugins into the binary, cargo unifies `renzora_plugin` back to `std` and `no_std_runtime!()` emits nothing, because the host already provides the allocator and panic handler. The plugin's own `#![no_std]` is untouched and irrelevant — a `no_std` crate links into a `std` binary perfectly happily.
- **What will not compile.** `std::collections::HashMap` (use `alloc::collections::BTreeMap`, or add `hashbrown`), `std::sync::Mutex`, anything under `std::fs`/`std::net`/`std::time`, and any dependency that itself needs `std`. Atomics are fine — they live in `core::sync::atomic`, which is where `ripple` gets them.

### Keep it out of the workspace

The empty `[workspace]` table is not optional if your plugin lives inside a checkout of the engine. Cargo's `exclude` key stops a directory being a workspace *member*, but it does not stop it resolving against the workspace — the plugin would still inherit the workspace's lockfile and feature unification, which quietly undoes the isolation the whole mechanism depends on. `[workspace]` makes the plugin its own workspace root.

## Turning it off

**Settings → Editor → Plugins** has a switch for every plugin the engine found, standalone ones included. Disabling takes effect at the next launch — see [Native Plugins](native-plugins.md#turning-plugins-on-and-off) for why, and for where the list is stored.

A disabled plugin is skipped before the file is opened, so none of its initializers run. It also stays disabled when its source is rebuilt: the hot-reload watcher below checks the same list, so saving a change to a plugin you turned off does not quietly bring it back.

## Hot reload

**Edit a `.rs` or `.wgsl` file under `plugins/` and save. The change is live in about a second, without restarting the editor.**

The editor watches plugin source, runs `cargo build` when it changes, stages the result, and swaps the new library in. Nothing to run, no second terminal:

```text
edit a file  →  cargo build  →  the new dll is staged  →  the running editor swaps it
```

That loop is only possible because a standalone plugin links no Bevy. One changed file rebuilds in well under a second; a plugin that shared the engine's Bevy would spend half a minute linking and the loop wouldn't be worth having.

**This is an editor feature, and only the editor pays for it.** To leave `plugins/<name>.dll` writable while it runs, the editor loads a private copy of it from `plugins/.reload/`; a shipped game — including the one **Play in Runtime Window** launches — never reloads anything, so it opens the real file directly. One consequence worth knowing during development: while a runtime window is open it holds `plugins/*.dll`, so a rebuild that stages plugins wants that window closed first.

### What survives a reload

**Your data.** Components and resources live in the host's ECS, keyed by name, so a swap never touches them — a counter keeps counting, entities keep their values, and nothing is serialised or restored. That is the whole reason this is tractable rather than a save-and-reload cycle.

Also reloaded: your **systems**, your **panel's BSN**, its title and icon, and your **shaders**. A shader is an asset, so replacing its source invalidates the pipeline and Bevy recompiles — no pipeline rebuild, no visible hitch.

### What a failure does: nothing

Every way a reload can fail leaves the running build untouched, and this is by design rather than luck.

- **Compile error** — nothing is staged, the editor never sees a change, cargo's full output goes to the log.
- **Init fails, wrong scope, ABI too old** — the previous build keeps running.
- **A component or resource changed layout** — refused, with the reason (`size 12 → 16`), because everything already holding that type was allocated for the old layout. **This is the one change that needs a restart.** Adding a field to a component is a restart; changing what a system does is not.

A shader whose uniform outgrew its settings struct is refused too, before it reaches the GPU — that particular mismatch is a device validation error, which is fatal rather than recoverable.

### Two things a reload cannot do yet

Adding a **new panel**, or a **new render pass / post-process effect**. Both need registration hooks that only exist while the app is being built. Editing an existing one is fine; adding one needs a restart, and the log says so.

### Doing it by hand

If you'd rather drive the build yourself — or the editor isn't running:

```bash
cargo renzora plugin <name>     # build one plugin and stage it
```

Use that rather than `cargo renzora`, which also stages `renzora.exe` and so needs the editor closed.

## Components

`#[derive(Component)]` generates everything the engine needs to store and edit a type it has no Rust definition for:

```rust
#[derive(Component, Default)]
#[repr(C)]
pub struct Orbit {
    pub radius: f32,
    pub speed: f32,
    pub height: f32,
    pub angle: f32,
}
```

- The **type path** (`module_path!() + "::Orbit"`) is the component's identity. Two plugins can each define a `Spinner` without colliding. Renaming the type or moving it between modules is a breaking change for saved data, exactly as renaming a Rust type would be.
- Every `f32`, `i32`, `bool`, `Vec3` and `Quat` field becomes an **editable inspector row**, and the component appears in **Add Component**. Fields whose type the editor can't draw are skipped — they still exist and still round-trip, they're just not editable. Fields prefixed with `_` are skipped deliberately, so GPU padding doesn't show up as a slider.
- `Default` is required. When you add the component in the editor the engine has to put *something* on the entity, and zeroed memory is a bad answer — a `speed: 0.0` component is present, correct, and doing nothing, which reads as a broken plugin.

Keep components plain data. Destructors are not supported yet, so no `String`, `Vec`, or `Box` fields.

`#[repr(C)]` is only strictly required when the struct is also a GPU uniform (see [Post-process effects](#post-process-effects)), but it costs nothing and makes the layout explicit.

### Text fields

A component can hold text with `Str256` — 252 bytes of inline UTF-8 plus a length:

```rust
use renzora_plugin::prelude::*;

#[derive(Component)]
#[repr(C)]
pub struct Label {
    pub text: Str256,
    pub font: Str256,
}

impl Default for Label {
    fn default() -> Self {
        Self { text: Str256::new("Label").unwrap_or(Str256::EMPTY), font: Str256::EMPTY }
    }
}
```

It draws as a text row in the inspector and round-trips through scenes like any other field. Read it with `as_str()`, write it with `Str256::new` (returns `None` if it does not fit) or `Str256::new_truncating`.

The fixed size is the whole point rather than a limitation to be lifted. Component storage is allocated by the host from a layout the plugin declares, and anything with a destructor is refused outright — a `String` would hand the host a pointer into the plugin's heap to free. 252 bytes covers a name, a label, or a path; a plugin needing more keeps it in its own memory keyed by entity, which is what `plugins/text3d` does for font *files* while the path itself lives on the component.

### Tuning how a field is edited

By default a numeric field gets an unbounded drag. `#[field(..)]` makes it a slider:

```rust
#[derive(Component, Default)]
#[component(name = "CRT")]
#[repr(C)]
pub struct Crt {
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub scanline_intensity: f32,
    #[field(min = 0.0, max = 1.0)]
    pub curvature: f32,
    #[field(skip)]
    pub internal_tuning: f32,
}
```

| | |
|---|---|
| `min` / `max` | Both or neither. Half a range has no sensible completion, and guessing one end quietly tunes a slider to something you didn't choose. |
| `speed` | Units per pixel of drag. Omit it and the engine uses a thousandth of the range, which keeps a `0..1` field and a `0..1000` field equally draggable. |
| `skip` | Keeps the field in the struct and out of the inspector — right for a value the code reads but nobody should drag, and required when the struct is a GPU uniform whose layout must not change. |
| `#[component(name = "..")]` | The inspector label. Without it the label is the type name, which turns `CRT` into `Crt`. |

An inverted range (`max` below `min`) is swapped rather than refused — a slider tuned backwards would sit dead at one end, and swapping is unambiguous where rejecting is a puzzle.

## Resources

`#[derive(Resource)]` gives a plugin global state:

```rust
#[derive(Resource)]
#[repr(C)]
pub struct FlockSettings {
    pub separation: f32,
    pub cohesion: f32,
    pub radius: f32,
    pub max_speed: f32,
}

impl Default for FlockSettings { /* … */ }

fn breathe(mut s: ResMut<FlockSettings>, time: Res<Time>) {
    s.cohesion = 0.8 + (time.elapsed_secs() * 0.4).sin() * 0.5;
}

fn flock(q: Query<&mut Transform>, s: Res<FlockSettings>) {
    // reads what `breathe` wrote
}
```

Register with `app.init_resource::<T>()` (inserts `Default`) or `app.insert_resource(value)`. Both are idempotent with respect to registration: two systems taking `ResMut<FlockSettings>` will not reset each other's value.

Resources are declared per-system, so two systems touching *different* resources still run in parallel — the same guarantee Bevy gives.

Unlike components, resources do **not** appear in Add Component (they're global; there's no entity for them to sit on) and have no inspector panel yet.

## Queries

```rust
fn example(
    a: Query<&Transform>,                          // read
    b: Query<&mut Transform>,                      // write
    c: Query<(&mut Transform, &Spinner)>,          // tuple, up to 3
    d: Query<Entity>,                              // the entity id
    e: Query<(&Transform, Option<&Boost>)>,        // optional data
    f: Query<&Transform, With<Spinner>>,           // filter
    g: Query<&Transform, Without<Spinner>>,
    h: Query<Entity, Or<(With<Spinner>, With<Orbit>)>>,
) {}
```

`Option<&T>` behaves as it does in Bevy: the entity matches whether or not it has the component, and you get `None` when it doesn't. That's what lets one query drive both the general case and a modified one:

```rust
fn flock(mut q: Query<(&mut Transform, &mut Boid, Option<&Leader>)>) {
    for (t, b, leader) in &mut q {
        let pull = leader.map_or(1.0, |l| 1.0 - l.bias);
        // leaders resist the flock; everything else is pulled in
    }
}
```

Iterate with `&q` / `&mut q` in a `for` loop, or `q.iter()` / `q.iter_mut()`. `q.len()` and `q.is_empty()` are available.

## Systems

A system is a plain function taking up to six parameters, in any combination:

| Parameter | Meaning |
|---|---|
| `Query<D, F>` | matched entities |

| `Res<T>` / `ResMut<T>` | a plugin resource |
| `Res<Time>` | the frame clock (`delta_secs()`, `elapsed_secs()`) |
| `Commands` | deferred structural changes |

Register into any of five schedules — `First`, `PreUpdate`, `Update`, `PostUpdate`, `Last`:

```rust
app.add_systems(Update, spin)
   .add_systems(PostUpdate, cleanup);
```

The function must be a plain `fn` or a non-capturing closure. A capturing closure has state the host has no way to own, so it's rejected at compile time.

### Panics are contained

A panic cannot unwind across an `extern "C"` boundary without aborting the process, so every system body is wrapped. If yours panics, the message is logged and **that system is disabled for the session** — the editor keeps running. A system that panics on frame one would otherwise emit thousands of identical errors and scroll the real one away.

## Input

```rust
fn walk(input: Res<Input>, mut q: Query<&mut Transform>) {
    for t in &mut q {
        if input.pressed(Key::W) {
            t.translation.z -= 0.1;
        }
        if input.just_pressed(Key::Space) {
            info("jump");
        }
        if input.mouse_pressed(MouseButton::Left) {
            let (dx, dy) = input.cursor_delta();
            t.rotate_y(dx * 0.01);
        }
    }
}
```

| | |
|---|---|
| `pressed` / `just_pressed` / `just_released` | Keyboard, by [`Key`](#keys) |
| `mouse_pressed` / `mouse_just_pressed` / `mouse_just_released` | `MouseButton::Left`, `Right`, `Middle`, `Back`, `Forward` |
| `cursor()` | Position in the primary window, `None` when the cursor is outside it |
| `cursor_delta()` | Movement since last frame — still reported while the cursor is locked, which is what a first-person camera needs |
| `scroll()` | Wheel movement this frame, in lines |

Reading input costs nothing across the boundary. The host flattens the whole frame into a bitset and sends it with the call, so `pressed` is a shift and a mask inside your plugin — not a call back into the engine.

`Res<Input>` is never absent. A host with no input at all (a dedicated server) reports everything as up rather than making you check.

### Keys

`Key::A`–`Key::Z`, `Key::Digit0`–`Digit9`, `Space`, `Enter`, `Escape`, `Tab`, `Backspace`, `Delete`, `Insert`, `Home`, `End`, `PageUp`, `PageDown`, the four `Arrow*`, the left/right `Shift`/`Control`/`Alt`/`Super`, `F1`–`F12`, and the punctuation row (`Minus`, `Equal`, `BracketLeft`/`Right`, `Backslash`, `Semicolon`, `Quote`, `Comma`, `Period`, `Slash`, `Backquote`, `CapsLock`).

Numpad, media and IME keys have no value yet. They read as never pressed rather than being given a number now that couldn't be changed later.

These are **not** Bevy's `KeyCode` discriminants, deliberately. That enum is `#[non_exhaustive]` and its values are an implementation detail, so a Bevy upgrade that inserted a variant would silently remap every plugin's key handling — W becomes E, and nothing fails to compile. These values are frozen.

## Animation

Animation is **not part of the ABI**. It is a feature-gated domain module you opt into:

```toml
renzora_plugin = { path = "../../crates/renzora_plugin", features = ["anim"] }
```

```rust
use renzora_plugin::prelude::*;
use renzora_plugin::anim::{AnimCommands, AnimState};

fn drive_gait(q: Query<(Entity, &Locomotion, &AnimState)>, mut cmds: Commands) {
    for (entity, loco, anim) in &q {
        let want = if loco.speed >= loco.run_at { "run" } else { "idle" };
        // Only switch when it actually changes — see below.
        if !anim.is_clip(want) {
            cmds.entity(entity).crossfade_animation(want, 0.2);
        }
    }
}
```

`AnimCommands` is an extension trait and has to be in scope — the boundary owns `EntityCommands` and has never heard of animation. That is the point: see [Domain modules](#domain-modules) for why, and for what adding audio or physics would look like.

### Driving it

Every operation hangs off `commands.entity(e)` and is deferred like any other command.

| | |
|---|---|
| `play_animation(name)` | Play looping at normal speed |
| `play_animation_with(name, speed, looping)` | The full form |
| `crossfade_animation(name, seconds)` | Blend into a clip |
| `stop_animation()` / `pause_animation()` / `resume_animation()` | |
| `set_animation_speed(mult)` / `seek_animation(seconds)` | |
| `set_anim_param(name, f32)` / `set_anim_bool(name, bool)` / `set_anim_trigger(name)` | State-machine parameters |
| `set_layer_weight(name, weight)` | Layer blend weight |
| `tween_position` / `tween_rotation` / `tween_scale` `(target, seconds, easing)` | Procedural tweens; rotation takes Euler degrees |

Easings are `Easing::Linear`, `In`, `Out`, `InOut`, and the `Quad`/`Cubic`/`Back`/`Elastic`/`Bounce` families (`Easing::OutBounce`, …).

Names cross **inline**, capped at 48 bytes. A longer one is dropped with a log line rather than truncated, because a shortened name matches no clip and reads as the animation system being broken.

### Reading it back

`AnimState` is a host component — `renzora_animation` maintains it — so query it like any other, and register it in `build()`:

```rust
app.register_component::<AnimState>();
```

| | |
|---|---|
| `is_clip(name)` / `is_state(name)` | Whether that clip / state-machine state is current |
| `is_playing()` | False while paused or stopped |
| `state_time` | Seconds in the current state |
| `time` | Property-animation playback time |

**Reading makes no FFI calls.** It arrives as an ordinary query cell, so a system checking animation state every frame does not call back into the engine — there is one call per system per frame regardless of how much it reads. It is not free, though; see [Cost](#cost).

**Read before you crossfade.** The mistake this exists to prevent is re-issuing a crossfade every frame the condition holds, which restarts the blend sixty times a second and never finishes. The fix is not for the plugin to remember what it last asked for — that goes wrong as soon as anything else drives the same animator — it is to ask the animator, as above. `plugins/locomotion` is the worked example.

### Why names are hashes

A plugin has no `String`, so `is_clip` compares a 64-bit FNV-1a of the name, folded at the call site. Two consequences worth knowing:

- A plugin can only ask *is it this one?* — it cannot enumerate or discover a clip name it wasn't already looking for.
- Nothing playing reads as `0`, which is deliberately not the hash of `""`, so an idle animator does not match `is_clip("")`.

### What is missing

Reading a **parameter** back. `set_anim_param` works, but params are an unbounded name→value map and don't fit a fixed-size mirror, so there is no `get_anim_param` yet. A plugin that needs the value can keep it in its own resource, which is where it usually came from.

A build with no animation crate — a dedicated server, a lean 2D export — accepts these commands and discards them each frame rather than growing a queue forever.

## Physics

Behind `features = ["physics"]`. Forces and impulses go out as commands; the body's state comes back as an ordinary query.

```rust
use renzora_plugin::physics::{PhysicsCommands, PhysicsState};

fn jump(mut q: Query<(Entity, &PhysicsState, &Jumper)>, input: Input, mut commands: Commands) {
    for (entity, state, jumper) in &mut q {
        if state.is_grounded() && input.just_pressed(Key::Space) {
            commands.entity(entity).apply_impulse(Vec3::new(0.0, jumper.power, 0.0));
        }
    }
}
```

| | |
|---|---|
| `apply_force(v)` | Continuous. Set it every frame for sustained thrust. |
| `apply_impulse(v)` | One-shot — a jump, a knockback, a launch. |
| `set_velocity(v)` | Outright, ignoring whatever it was. |
| `kinematic_slide(delta, max_slope_degrees)` | Move a kinematic body, sliding along anything steeper instead of walking up it. |

`PhysicsState` mirrors linear and angular velocity plus contact flags — `is_grounded()`, `is_colliding()`, `just_entered()`, `just_exited()`. Like `AnimState` it is a numeric-only mirror the bridge refreshes each frame, so it reads as a normal component with no service call involved.

What a plugin **cannot** do is create a body. `RigidBody`, `Collider` and the joint types are engine components with no plain-data mirror, so a plugin drives physics that something else set up — authored in the editor, or spawned as [BSN](#spawning-something-visible), which names host components as text and so can construct them.

## HTTP

Behind `features = ["http"]`. A request is fired by tag and collected later:

```rust
use renzora_plugin::http::{Http, HttpCommands};

const SCORES: u64 = 1;

fn fetch(mut commands: Commands) {
    commands.http_get(SCORES, "https://example.com/scores");
}

fn collect(http: Http) {
    if let Some(response) = http.poll(SCORES) {
        if response.is_ok() {
            info(&format!("got {}", response.body));
        }
    }
}
```

`poll` returning `None` is the normal state — a request takes many frames — and a response is delivered exactly once. The tag is yours to choose; it is how a plugin with several requests in flight tells them apart.

This exists because a plugin genuinely cannot do it itself. Nothing stops it from linking `reqwest`, but it would then own a runtime, a thread pool and a TLS stack per plugin, all of which the engine already has. Riding the engine's client also means a request goes through the same proxy and certificate configuration as everything else.

### Headers

`http_get`/`http_post` send only a JSON `Content-Type`. Anything needing auth goes through `http_with`:

```rust
use renzora_plugin::http::{HttpHeaders, HttpOp, HttpCommands};

commands.http_with(
    HttpOp::PostStream, TAG, &url, Some(&body),
    &HttpHeaders::new().bearer(&api_key),
);
```

`.bearer(t)` is `Authorization: Bearer t`; `.add(name, value)` is anything else — Anthropic, for instance, wants `x-api-key` plus a pinned `anthropic-version` rather than a bearer token.

- **An empty `HttpHeaders` falls through to the plain payload**, so a caller need not branch on whether it has any.
- **Your `Content-Type` wins.** The JSON default is only applied when you did not set one; sending both is two of the same header, which some servers reject and others resolve unpredictably.
- **Separators are filtered, not escaped.** A newline in a value or a colon in a name would re-split host-side and could smuggle in a second header, so both are dropped — a header name containing a newline is never legitimate.

This does not move `VERSION_MINOR`. The op carries a `WITH_HEADERS` flag bit that selects the payload shape, because the op is the one thing both sides agree on before either reads a byte — appending a field to the existing request header would have a new host misread an old plugin's shorter one.

### Streaming responses

`poll` waits for the whole body. For an endpoint that answers over a long-lived connection — an LLM streaming tokens as NDJSON or SSE, a progress feed, a tailed log — that defeats the point: you get everything at the end, or nothing.

`http_get_stream` / `http_post_stream` deliver the body in pieces instead:

```rust
use renzora_plugin::http::{Http, HttpCommands};

const REPLY: u64 = 2;

fn ask(mut commands: Commands) {
    commands.http_post_stream(REPLY, "http://localhost:11434/api/chat", r#"{"stream":true}"#);
}

fn collect(http: Http, mut state: ResMut<Reply>) {
    while let Some(chunk) = http.poll_stream(REPLY) {
        state.text.push_str(&chunk.data);
        if chunk.is_last() {
            state.done = true;
            break;
        }
    }
}
```

Four things to hold on to:

- **Poll in a `while` loop, not an `if`.** Several chunks usually land in one frame, and taking one per frame would make a fast reply arrive in slow motion.
- **Stop at `is_last()`.** Every stream ends with exactly one terminal chunk — `End` normally, `Error` if it failed after the response began, with the error text in `data`. It arrives whatever happens, so a plugin waiting for it will not wait forever. Polling past it just returns `None`.
- **Chunks are transport-sized, not message-sized.** A chunk is whatever came off the socket, so a JSON object can be split across two of them and two objects can share one. Accumulate and split on your API's own framing — that is why the host does not try to guess it.
- **`status` repeats on every chunk**, so a plugin that keeps only the latest still knows it. A `0` means the request never reached a response at all.

Streaming and whole-body requests share one queue but never cross: a chunk is invisible to `poll`, and a completed body is invisible to `poll_stream`. Mixing both in one plugin, on different tags, is fine.

## Diagnostics

Not behind a feature. The host's own measurements — frame time, FPS, entity count, per-render-pass GPU and CPU times, process CPU and memory — readable from any system:

```rust
use renzora_plugin::diagnostics::Diagnostics;

fn report(diags: Diagnostics) {
    if let Some(fps) = diags.get("fps") {
        info(&format!("{:.0} fps", fps.smoothed));
    }
}
```

`get` is a linear scan, which is right for the occasional lookup and wrong for reading twenty paths a frame — call `iter()` once and match against that instead.

Two things the host does not promise, and both bite quietly:

- **Which measurements exist.** An editor carries all of them; a shipped game usually carries none; a graphics backend without timestamp queries has the `render/*/elapsed_cpu` paths but not `elapsed_gpu`. That is why `get` returns `Option`, and why treating a missing diagnostic as zero is how you end up drawing a flat line and calling it data.
- **That a present measurement has a value.** A diagnostic registers before its first sample is taken, so `value` is `NaN` for the first frames. `Diagnostic::is_valid()` is the check. `NaN` propagates silently through a sum and poisons an average without any comparison ever being false.

Entry order is unspecified and not stable between calls — diagnostics are identified by path, and indexing them positionally reads FPS as GPU time the first time the host registers a new one.

This is a system param rather than an interface function because reading the store needs the world, and `SystemCall::host` is null while a system runs — the same constraint that shapes `Meshes` and `Http`. It is not a *domain* like HTTP or animation, so there is no feature to enable: the source is a field of `SystemCall` in every build.

`plugins/tracy` is the worked example — it streams every diagnostic to a Tracy profiler as a named plot. See [Profiling with Tracy](../editor-dev/profiling.md).

## File dialogs

Behind `features = ["dialog"]`. Native file and folder pickers, asked for by tag and collected later — the same fire-and-tag shape as HTTP, and for the same reason: the boundary has no callbacks, and a function pointer handed over would have to survive a hot reload.

```rust
use renzora_plugin::dialog::{DialogCommands, DialogFilter, Dialogs};

const PICK_DOCS: u64 = 1;

fn browse(mut commands: Commands) {
    commands.pick_folder(PICK_DOCS, "Choose a docs folder");
}

fn collect(dialogs: Dialogs, mut cfg: ResMut<Config>) {
    if let Some(outcome) = dialogs.poll(PICK_DOCS) {
        match outcome.path() {
            Some(path) => cfg.docs = path.into(),
            None => info("cancelled"),
        }
    }
}
```

`pick_file` and `pick_save_path` additionally take a `DialogFilter`:

```rust
let filter = DialogFilter::new()
    .add("Images", &["png", "jpg", "webp"])
    .add("All Files", &["*"]);
commands.pick_file(TAG, "Choose a texture", &filter);
```

- **Cancelling still replies.** It arrives as `DialogResult::Cancelled`, so a plugin that greyed out a button while the dialog was open hears back. `outcome.path()` collapses cancelled and unavailable to `None` when you don't care which.
- **`Unavailable` is distinct from cancelled** — no windowing, or a headless build. Worth handling separately if the plugin can fall back to typing a path.
- **The picker blocks the editor while it is open**, matching every other dialog in the editor. Your plugin still sees an async API: the answer lands in a queue and is collected on a later frame, so never assume it arrives in any particular one.
- **Why not link `rfd` yourself?** You'd own a second copy of the platform's dialog stack, opened from a thread the editor knows nothing about and parented to no window — which on Windows means a modal that can fall behind the editor with no way back to it.

Nothing in this domain touched the ABI. It was the first to ride the generic reply channel (`SystemCall::replies`), which is the return-path counterpart to `call_service` — so unlike meshes, images and HTTP before it, it needed no `VERSION_MINOR` bump of its own, and neither will the next domain.

## Domain modules

Animation, physics and HTTP all ride the same mechanism, and the shape matters more than any one of them.

`sys` — the commands, queries and interface table everything else rests on — deliberately does **not** know that animation exists. Instead it carries one generic command:

```rust
CommandKind::Service   // { service: u64, op: u32 } + opaque payload bytes
```

The host copies those bytes into a queue without reading them. `renzora_plugin::anim` is an ordinary *user* of that mechanism with no privileged access: it defines its own op numbering, encodes a plain-data payload, and tags it with `service_id("renzora.animation")`. On the engine side, `renzora_animation::plugin_bridge` takes the calls bearing that tag — and only those, so one domain can never eat another's — and turns them into real animation commands.

Two things follow, and they are why it is built this way:

- **Adding a domain does not move the ABI.** `sys::VERSION_MINOR` describes the boundary; a new module bumps the crate's own semver instead. A plugin that wants audio should not end up declaring a minimum ABI that also encodes animation history. Animation, physics and HTTP have all landed since 2.4 without moving it.
- **A plugin that doesn't use a domain pays nothing.** The module is behind a feature, and its types are plain data with no statics — measured, a plugin using animation is *smaller* than one that doesn't, because the difference is its own code, not the vocabulary.

Adding audio would be `src/audio.rs` behind an `audio` feature, plus a `plugin_bridge` module in whichever engine crate owns audio. Neither touches `sys`, and neither needs a new crate.

Two of the three needed one thing the generic channel doesn't provide, and it is worth knowing which: **a reply**. Commands are write-only, so `Http::poll` and `Meshes::read` are *system params* backed by their own pointer in the call struct, not service calls. Anything that answers a question rather than issuing an order takes that route, and that route does touch the ABI.

### What the plugin can and cannot reach

Worth being exact, because "shares data with Bevy without linking Bevy" invites the wrong conclusion.

A plugin never gets the `World` — the `host` pointer is null while a system runs, because the world is borrowed by the query. What it gets is three narrow things:

| | |
|---|---|
| **Query cells** | Pointers to just the components it *declared*, for just the entities that matched, valid only for that one call |
| **A command sink** | Write-only; applied after the system returns |
| **Service calls** | Opaque bytes parked for a bridge |

So it is a window the host opens per system per frame, not access. A plugin cannot iterate arbitrary entities, reach a resource it did not declare, or retain anything past the call.

The two paths are also asymmetric, deliberately. **Reads** go through the generic dispatcher — a real Bevy query, flattened into cells — and never touch the bridge. **Actions** go through the queue and the bridge; they are rare, so they can afford a translation step.

Both sides work from the same `#[repr(C)]` definitions compiled into each independently. Nothing is shared at link time: the layout is pinned by the C ABI, not by two rustc versions happening to agree.

### Cost

A plugin system is not free relative to a linked one, and it is worth knowing where the difference is before putting one on a hot path.

The FFI is **one call per system per frame**, not per entity and not per read — the host flattens every matching row into a pointer array first, and the plugin's loop then runs at native speed inside its own address space. That part scales fine.

What costs more than a linked system is getting the data there. The host copies every matched cell into a staging buffer it owns, and writable terms are copied a second time to form a change-detection baseline. A native Bevy system reads component storage in place and does neither. So a query over N entities with T terms pays on the order of 2–3×N×T copies, plus allocation, that a linked system would not.

The reason is soundness, not oversight: a pointer straight into component storage would require the plugin to assume a layout the host cannot guarantee. Handing out direct pointers for plugin-owned components — whose layouts *are* the plugin's own — is a known optimisation that needs an aliasing argument first.

Practically: at hundreds of entities this is noise. At tens of thousands, on a per-frame system, measure it. Cheap mitigations that need nothing from the engine — narrow the query with `With`/`Without` filters so fewer rows are gathered, and keep frequently-read state in a resource rather than on every entity.

## Commands

```rust
fn setup(mut commands: Commands) {
    let e = commands.spawn((Spinner { speed: 2.0 }, Orbit::default())).id();
    commands.entity(e).insert(Boost { amount: 5 });
    commands.entity(e).remove::<Orbit>();
    commands.entity(other).despawn();
}
```

Commands go through Bevy's own deferred queue, so spawning mid-iteration is exactly as safe as it is in a Rust system.

## Spawning something visible

Assets are created during `Plugin::build` and referenced by handle afterwards:

```rust
use core::sync::atomic::{AtomicU64, Ordering};
use renzora_plugin::sys::{AssetHandle, Primitive};

static MESH: AtomicU64 = AtomicU64::new(u64::MAX);
static MATERIAL: AtomicU64 = AtomicU64::new(u64::MAX);

impl Plugin for ScatterPlugin {
    fn build(&self, app: &mut App) {
        let mesh = app.add_mesh(Primitive::Cuboid, Vec3::splat(0.6));
        let material = app.add_material([0.3, 0.6, 0.9, 1.0]);
        MESH.store(mesh.0, Ordering::Relaxed);
        MATERIAL.store(material.0, Ordering::Relaxed);

        app.register_component::<Scatter>()
            .add_systems(Update, scatter);
    }
}

fn scatter(q: Query<&Scatter>, mut commands: Commands) {
    let mesh = AssetHandle(MESH.load(Ordering::Relaxed));
    let material = AssetHandle(MATERIAL.load(Ordering::Relaxed));
    commands.spawn_mesh(mesh, material, Transform::from_xyz(0.0, 1.0, 0.0));
}
```

The `static` is the one genuinely un-Bevy-like thing here, and it follows from systems having to be zero-sized: a handle created in `build` can't be captured by a closure the host has to own, so it's parked somewhere the system can read it. `Primitive` is `Cuboid`, `Sphere`, `Plane`, `Cylinder`, `Capsule` or `Torus`; `add_material_pbr` takes metallic and roughness alongside the colour.

## Geometry

Beyond the built-in primitives, a plugin can hand the engine its own vertices — which is what makes text meshes, procedural foliage, hair ribbons and water surfaces possible at all.

```rust
// A quad. Normals and UVs derived by the host.
let quad = app.add_mesh_data(
    &[Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0),
      Vec3::new(1.0, 0.0, 1.0),   Vec3::new(-1.0, 0.0, 1.0)],
    None, None,
    Some(&[0, 1, 2, 0, 2, 3]),
);
```

`normals` and `uvs` may be `None` — the host computes normals from the faces and zeroes the UVs. `indices` may be `None` for an unindexed triangle list, where every three positions form one face. Everything is copied before the call returns, so the slices can be locals.

Anything inconsistent — an index past the end, a normal count that doesn't match the vertices, a position count that isn't a whole number of triangles — is **refused**, returning an invalid handle and logging why. Padding or clamping it would produce a mesh that renders subtly wrong with nothing to point at.

### Rewriting a mesh each frame

`add_mesh_data` is init-only, like every asset constructor. Geometry that changes — a simulated strand, an edited string — is rewritten through the `Meshes` system param:

```rust
fn update(q: Query<&Ribbon>, meshes: Meshes) {
    for ribbon in &q {
        meshes.write(ribbon.handle(), &positions, Some(&normals), Some(&uvs), Some(&indices), None);
    }
}
```

The last argument is optional per-vertex colours. Vertex count and topology can change freely between writes — only the handle is fixed.

The practical consequence of init-only creation is a **pool**: every mesh the plugin will ever write has to exist by the end of `build`, so a plugin decides its own ceiling and hands slots out. `plugins/text3d` keeps 64 and `plugins/hair` keeps 16, both as a free stack. Seed each one with a degenerate triangle — a mesh with no positions is refused.

### Reading geometry already in the world

The counterpart: consuming what is already there, for scattering over a surface, growing from a scalp, or fitting to a wall.

```rust
fn scatter(q: Query<Entity, With<Scatter>>, meshes: Meshes) {
    for e in &q {
        let Some(mesh) = meshes.read(e) else { continue };  // still loading
        for [a, b, c] in mesh.triangles() { /* … */ }
    }
}
```

`read` copies into memory the plugin owns — the host never hands back a pointer into asset storage, which can move or be freed the moment the call returns. `None` means the entity has no mesh **or its asset hasn't loaded yet**, which is the normal state for the first few frames after a spawn, so poll rather than treating it as failure.

`triangles()` yields index triples whether or not the mesh was indexed, since almost nothing that walks a surface cares how the faces were stored.

## Textures

A plugin can upload its own pixels:

```rust
use renzora_plugin::sys::ImageFormat;

let tex = app.add_image(256, 256, ImageFormat::Rgba8UnormSrgb, &pixels);
```

`data` must be exactly `width * height * bytes_per_pixel`. A short buffer is refused rather than padded — uploading one as a full texture reads past the plugin's heap into a GPU transfer.

Init-only again, and rewritten from a system through `Images`:

```rust
fn step(images: Images) {
    images.write(handle, &pixels);
}
```

Dimensions and format are fixed at creation; only the contents change. A `data` length that doesn't match is refused and the previous pixels are left alone, so a bad frame shows the last good texture instead of garbage. The main reason a plugin wants a texture at all is a simulation it steps every frame — a heightfield, a flow map, a generated atlas — and that is exactly the shape this supports.

## Custom materials

A plugin can supply its own WGSL and drive it from one of its own components:

```rust
#[derive(Component, Default)]
#[repr(C)]
pub struct Ripple {
    #[field(min = 0.0, max = 8.0)]
    pub speed: f32,
    #[field(min = 0.0, max = 1.0)]
    pub amplitude: f32,
}

let mat = app.add_material_shader::<Ripple>("ripple", WGSL, AlphaMode::Blend, &[tex]);
```

`Ripple`'s bytes are uploaded as the uniform at `@group(3) @binding(0)`, so the parameters are described **once** — editable in the inspector, saved into scenes, readable by the plugin's own systems — rather than duplicated into a GPU-only struct that has to be kept in sync by hand. Any textures passed bind from `@binding(1)` upward, each as a texture/sampler pair.

The shader supplies a **fragment entry point only**; the vertex stage stays Bevy's, so skinning, morph targets and the instance-indexed model transform are handled for you.

```wgsl
#import bevy_pbr::forward_io::VertexOutput

struct Ripple { speed: f32, amplitude: f32 };
@group(3) @binding(0) var<uniform> ripple: Ripple;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let w = sin(in.world_position.x * 8.0) * ripple.amplitude;
    return vec4(0.1, 0.4 + w, 0.8, 1.0);
}
```

Note `@group(3)`, not `2` — Bevy 0.19 binds view data at 0, mesh data at 1 and 2. And note the import: unlike a [post-process shader](#the-shader-must-be-self-contained), a material is compiled through Bevy's normal pipeline, so **naga_oil imports work here**. `#import bevy_pbr::forward_io::VertexOutput` is in fact required, since that struct is what the vertex stage hands the fragment.

The component must be no larger than 256 bytes (`sys::MATERIAL_UNIFORM_CAP`). Over that is refused with a log line rather than clamped: the bind-group layout is fixed for the shared material type, and a uniform read past the end of its buffer is undefined on the GPU, not merely wrong.

### Shading geometry you didn't make

`spawn_mesh` and `make_renderable` set mesh, material and transform together, which is right when the plugin owns the geometry and wrong when it doesn't. To change only the material — an imported model, a shape the user authored, anything already in the scene:

```rust
fn shade(q: Query<Entity, (With<Glow>, With<Mesh3d>)>, mut commands: Commands) {
    for e in &q {
        commands.entity(e).set_material(material);
    }
}
```

Filter on `Mesh3d` as above unless you know the entity has one. A material on an entity with no mesh isn't an error and draws nothing — it just sits there, which is a confusing thing to debug.

This is the case worth having, and it's why `Mesh3d` staying opaque costs nothing. A plugin can't read the mesh handle back out, so before `set_material` existed there was no way to keep an entity's geometry and replace its shading: adding a custom material meant replacing the shape with one the plugin had made earlier, which limited plugin materials to objects the plugin spawned itself.

## Render passes

A plugin can put its own code inside Bevy's render graph:

```rust
impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_render_pass("my_pass", WGSL, RenderPhase::Overlay, 0.0, |pass| {
            pass.set_pipeline();
            pass.draw(0..3, 0..1);
        });
    }
}
```

Phases are `Gi`, `HdrPost`, `LdrPost` and `Overlay`; the `f32` is the order within the phase. The pipeline is built lazily against the view's actual colour format, so the pass works on HDR and LDR targets without you declaring which.

## Post-process effects

An effect is a component plus a shader. The component's fields become the shader's uniform:

```rust
#[derive(Component)]
#[repr(C)]
pub struct Tint {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub strength: f32,
}

app.add_post_process::<Tint>("tint", WGSL, RenderPhase::LdrPost, 0.0);
```

Add the component to **any** entity — it need not be the camera, and there is no routing table to configure — and the effect runs. Every field is an inspector row, with a slider wherever you gave a [range](#tuning-how-a-field-is-edited). Removing the component turns the effect off; there is no `enabled` flag to maintain. The effect is global, so a second entity carrying the same component does nothing.

Write **whatever fields the effect needs**, in any number. The engine rounds the uniform buffer to a 16-byte multiple for you, so no padding to a fixed size and no counting slots. Every one of the engine's 53 built-in effects is written this way.

`RenderPhase` picks where in the frame the pass runs — `Gi`, `HdrPost`, `LdrPost` or `Overlay` — and the `f32` sorts within it. [Post-Processing Effects](./post-processing.md) covers the phases and where the line falls between an effect you can write here and one that needs a Bevy-linked crate.

### The shader must be self-contained

The WGSL goes straight to naga, so `#import` — a naga_oil directive — is not available. Declare what you use, and take the fragment inputs explicitly.

This is specific to post-process, and worth not over-generalizing: a [custom material](#custom-materials) is compiled through Bevy's own pipeline, where imports do work. The difference is that a post-process effect is validated and handed to wgpu directly, with no preprocessor in the path.

```wgsl
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Tint {
    red: f32,
    green: f32,
    blue: f32,
    strength: f32,
};
@group(0) @binding(2) var<uniform> settings: Tint;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>)
    -> @location(0) vec4<f32> {
    let c = textureSample(screen_texture, texture_sampler, in_uv);
    return vec4<f32>(c.rgb * vec3(settings.red, settings.green, settings.blue), c.a);
}
```

Name the UV parameter something that can't collide with a local — `in_uv` rather than `uv`. A shader that computes its own `uv` (a curved CRT coordinate, say) will otherwise fail with `redefinition of uv`.

`include_str!("effect.wgsl")` keeps the shader in its own file, and editing it is a source change the [hot reload](#hot-reload) watcher already sees.

> **`#[repr(C)]` is mandatory here, and mind `vec3`.** WGSL aligns `vec3<f32>` to 16 bytes and Rust aligns `[f32; 3]` to 4, so the "same" struct is 32 bytes in the shader and 16 in Rust — a GPU validation panic, not a compile error. Pad with scalar `f32`s on both sides. The engine validates the shader with naga before it reaches wgpu and names this cause explicitly, so a mistake costs you a log line rather than the session.

## Runtime or editor

A plugin declares which it is as the second argument to `add!`:

```rust
renzora_plugin::add!(SpinnerPlugin);          // Runtime — the default
renzora_plugin::add!(WidgetsPlugin, Editor);  // Editor only
```

`Runtime` plugins load in the editor viewport **and** the shipped game. `Editor` plugins load only when the editor is present, so a panel-only plugin should say `Editor` — otherwise it defaults to `Runtime` and gets loaded alongside a game that has no editor for it to attach to.

The host reads this from a separate exported symbol before calling your init, so an out-of-scope plugin is never initialised at all.

## Shipping inside the game binary

A game exported in **Lean single binary** mode can compile its plugins *into* the
executable instead of shipping a `plugins/` folder next to it. The choice is the
Plugins tab of the export dialog — see
[Export → Plugin linking](../exporting/overview.md#plugin-linking-the-plugins-tab).

**Nothing in your plugin changes.** You do not opt in, add a feature, or write a
second entry point. Being linked in is a decision the person exporting the game
makes about your library, and the code path it takes is the one you already
wrote: the host calls the same `init` with the same interface table and your
plugin registers the same things. That works because the C ABI never actually
required a shared library — a plugin exports a function and imports nothing, so
where the host got the function pointer is not something the plugin can observe.

Two mechanical details, in case you meet them:

- The exporter builds your plugin from **source**, so it needs the crate in the
  engine checkout's `plugins/` directory. A prebuilt library with no source
  alongside it (anything downloaded from the marketplace) is shipped as a file
  instead, automatically and with a line in the build log.
- It compiles with `renzora_plugin`'s `static_link` feature on, which drops the
  `#[no_mangle]` from what `add!` emits so several plugins can coexist in one
  binary. Never enable that feature yourself: a `cdylib` built with it exports no
  init symbol and cannot be loaded from disk at all.

Hot reload is the one thing that does not survive — there is no file to watch and
no way to swap code inside a running binary. That costs a shipped game nothing;
it is why the editor always loads plugins from files.

## Editor panels

A plugin can add its own docked panel to the editor. The panel is described in BSN — the same syntax and the same parser a scene uses — and the host spawns it with real `renzora_ember` widgets:

```rust
impl Plugin for FlockPlugin {
    fn build(&self, app: &mut App) {
        app.add_panel(
            Panel::new("flock", "Flock", bsn! {
                Node { flex_direction: Column, row_gap: Px(6.0) }
                Children [
                    Text("Flocking"),
                    ( EmberButtonWidget { label: "Scatter" }
                      PanelActionId { action: 1 } ),
                ]
            })
            .icon("bird")
            .on_action(on_action),
        );
    }
}

fn on_action(action: Action) {
    if action.is("1") {
        info("scatter");
    }
}
```

`Panel::new` takes the id (used for the dock tab and layout persistence), the title shown on the tab, and the contents. `.icon()` takes a Phosphor icon name and defaults to `puzzle-piece`; `.category()` groups the panel in the add-panel menu; `.on_action()` attaches the click handler and can be omitted for a display-only panel.

Write the BSN inline with `bsn!` rather than parking it in a `const`. Combining the description with the registration is the point of the macro — a panel body in a `&str` constant somewhere else is the thing it replaces.

### Changing a panel after registration

`add_panel` takes the markup once, which covers a panel that is a fixed arrangement of widgets and nothing that has to *change* — a list that grows, a reply streaming in, a form whose fields depend on the selection. For that, a running system hands the host new BSN:

```rust
use renzora_plugin::panel::PanelCommands;

fn redraw(mut commands: Commands, log: Res<ChatLog>) {
    let mut markup = String::from("Node { flex_direction: Column } Children [");
    for line in &log.lines {
        markup.push_str(&format!("Text({:?}),", line));
    }
    markup.push(']');
    commands.set_panel_content("mychat", &markup);
}
```

It takes a `&str` rather than a `Scene`, because `Scene` holds a `&'static str` and the entire point here is content built at run time.

- **Call it unconditionally.** The host compares the markup before it parses anything, so sending a string the panel already has costs a string comparison and no redraw. You do not need to track dirtiness yourself.
- **The id must already be registered.** Creating a panel needs `&mut App`, which a system does not have, so an unknown id is a typo rather than an ordering problem — it is reported and ignored.
- **Malformed BSN keeps the old panel.** Same rule as hot reload: a half-built string is a normal intermediate state, and blanking the panel on every frame that does not parse would be worse than showing a stale one.
- **Action ids survive.** The redraw rebinds the handler to the current build, so `PanelActionId` numbers keep working across a content change.

This rides the generic service channel rather than adding an ABI function, so it does **not** move `VERSION_MINOR` — same as `anim`, `physics` and `http`.

### Panel content is not limited by the field kinds

Worth being precise about, because it is easy to assume otherwise. The panel body crosses the boundary as a **string**, and is parsed host-side. So it can contain anything BSN can express — nested lists, tuple structs, strings — regardless of the closed [`FieldKind`](#components) set that governs plugin *component* data.

What the field kinds still constrain is data a plugin's own systems read and write each frame. That is why `PanelActionId` carries a number instead of a name.

### Widgets

Ember widgets are components, so BSN can name them like any other. Fields are the same ones the underlying builder takes:

| Component | Fields |
|---|---|
| `EmberButtonWidget` | `label: String` |
| `EmberSliderWidget` | `value: f32`, `min: f32`, `max: f32` (default `0.0..1.0`) |
| `EmberToggle` | `on: bool` |
| `EmberCheckbox` | `checked: bool` |
| `EmberInput` | `placeholder: String`, `value: String` |
| `EmberDropdown` | `options: Vec<String>`, `selected: usize` |
| `EmberTabs` | `labels: Vec<String>` |
| `EmberProgress` | `value: f32` |
| `EmberTable` | `headers: Vec<String>`, `rows: Vec<Vec<String>>` (row-major) |
| `EmberTimeline` | `duration: f32`, `tracks: Vec<EmberTrack>` |

An `EmberTrack` is `name: String`, `color: (u8, u8, u8)`, `clips: Vec<EmberClip>`; an `EmberClip` is `start: f32`, `length: f32`, `label: String`. A track whose colour is left at `(0, 0, 0)` is assigned one from a cycling palette, so a handful of tracks read apart without you picking colours.

`EmberSliderWidget`'s `value` is in `min..=max`, not 0..1, and an inverted range (`max < min`) is allowed — it runs the track right-to-left.

A ragged `EmberTable` row is drawn as-is rather than padded — the widget lays out what it is given, because silently inventing cells would hide a mistake in whatever produced the data.

Anything Bevy's own UI offers works alongside them: `Node`, `Text`, `Children`, and any registered component. A partial `Node { flex_direction: Column }` means "default the rest".

### Binding a widget to a resource

A widget field written as `bind(Resource.field)` instead of a literal is wired **two-way** to that plugin resource: the widget shows the resource's current value, dragging it writes back, and a system that changes the value moves the widget.

```rust
( EmberSliderWidget { value: bind(FlockSettings.cohesion), min: 0.0, max: 2.0 } )
```

The point of this is what it *doesn't* do. Dragging that slider never calls into the plugin — the host already knows the resource's layout from `register_resource`, so it reads and writes the bytes itself. There is no per-frame FFI, no action handler, and no polling on either side.

- The target must be a **resource** the plugin registered. Components are not addressable this way; a panel has no entity to mean.
- `f32`, `i32` and `bool` fields can bind. `Vec3` and `Quat` cannot — there is no single-value widget for them, and the binding is refused with that reason rather than half-wired.
- `min`/`max` on the widget are in the **field's own units**, not normalised. `radius: 0.5..10.0` is written as `min: 0.5, max: 10.0`.
- Only top-level fields of a component body can bind. `bind` nested inside a value (`tracks: [ ( name: bind(…) ) ]`) is left as literal text, because a rebuilt list has nowhere stable to write back to.
- An unresolvable target — no such resource, no such field, or a name two loaded plugins both claim — is an error naming the candidates, not a silent no-op. Qualify an ambiguous one with the crate: `bind(flock::FlockSettings.cohesion)`.

### Actions

Put `PanelActionId { action: N }` on a widget and clicks reach `on_action`, where `Action::name()` is that number as a string and `Action::is("N")` is the usual test. `Action` also carries `value` (a toggle's 0 or 1, a slider's position, 0 for a button) and a `commands` queue — the same one a system gets, so a handler can spawn and despawn.

Do **not** set `PanelActionId`'s `panel` field. It indexes a list that spans every loaded plugin, so its correct value depends on what else is in `plugins/`; the host stamps it when the panel is spawned.

## Third-party crates

A standalone plugin is an ordinary Rust crate, so it can depend on anything on crates.io:

```toml
[dependencies]
renzora_plugin = "0.1"
noise = "0.9"
```

The dependency compiles into your `cdylib` and the engine never sees it — no version conflict is possible, because there's nothing to conflict with. The only crates you *can't* use are ones that take real Bevy types in their public API, since the ECS types here are shims rather than Bevy itself.

## Versioning

The ABI carries a `MAJOR.MINOR` version. A plugin loads into any host whose MAJOR matches and whose MINOR is at least the one it was built against.

- **MINOR** — a function appended to the end of the interface, or a field appended to the end of a struct. Older plugins never touch it and keep working.
- **MAJOR** — anything else. Every existing plugin is refused, which is the point.

A plugin built against a newer MINOR than the host provides is refused with a message naming the versions, rather than being allowed to call a function the host doesn't have.

**The current ABI is 4.8.** The 4.x MINORs:

- **4.1** — `RemovedComponents`
- **4.2** — `Added` and `Changed` query filters
- **4.3** — `add_script_backend`
- **4.4** — [streaming HTTP responses](#streaming-responses)
- **4.5** — `SystemCall::replies`, the generic host-to-plugin answer channel
- **4.6** — `PanelAction::text`, so a panel's text inputs reach the plugin
- **4.7** — [`add_settings_section`](#editor-panels)
- **4.8** — the [`Diagnostics`](#diagnostics) system param

4.8 breaks 4.5's stated intent of being the last per-domain source, and the reason is worth knowing before adding a ninth: replies are *answers to a call the plugin made*, delivered a frame later. Diagnostics are the opposite shape — a reader wants this frame's numbers during this frame, and a round trip would hand every consumer values one frame stale. A profiler plotting last frame's frame time against this frame's marker is not a profiler. If a new domain can tolerate a frame of latency, it should still ride `ReplySource` and cost no bump at all.

The 2.x MINORs below are kept as history, because two of them broke the additive guarantee and that is why 3.0 exists:

- **2.1** — editor panels
- **2.2** — [input](#input)
- **2.3** — [field editing ranges](#tuning-how-a-field-is-edited)
- **2.4** — `CommandKind::Service`, the generic channel [domain modules](#domain-modules) ride on
- **2.5** — [`add_mesh_data`](#geometry): geometry from the plugin's own vertices
- **2.6** — [`Str256`](#text-fields), and the `Str` field kind that draws it
- **2.7** — [`Meshes::read`](#reading-geometry-already-in-the-world)
- **2.8** — the [`Http`](#http) system param
- **2.9** — [`add_material_shader`](#custom-materials)
- **2.10** — [`Meshes::write`](#rewriting-a-mesh-each-frame)
- **2.11** — [`add_image`](#textures), `Images::write`, and material textures
- **2.12** — [`set_material`](#shading-geometry-you-didnt-make): change an entity's material without touching its mesh
- **3.13** — the interface table now carries a hash of its own shape, so a mismatch is a refused load rather than a wrong call (below)

Note what is *not* in that list: animation, physics and HTTP *commands*. They ship alongside but are [domain modules](#domain-modules), not boundary surface, so they moved the crate's version and not the ABI's. What did land as MINORs — `Http::poll`, `Meshes::read` — are the parts that hand data *back*, which the generic channel cannot do. Audio will follow the same split.

The run from 2.5 to 2.11 is what porting real plugins cost. Each one was a capability an actual plugin was blocked on and nothing else would substitute for: `plugins/text3d` needed strings and then mesh writes; `plugins/hair` needed to read a scalp mesh before it could grow anything from it. None of them was foreseeable from the outside, which is the argument for porting a plugin before declaring the surface complete.

**MAJOR went to 3 to repair the interface table**, and it is worth reading why, because it is the failure mode this whole scheme is built to avoid.

The interface is a struct of function pointers, so a plugin calls a function by its *offset*. Appending is safe; anything else is not. `add_material_shader` (2.9) and `add_image` (2.11) were each **inserted into the middle** of the struct and recorded in the changelog as appended. A plugin built at 2.5–2.10 would therefore have called the slot it compiled against and landed in a different function — handing a mesh descriptor to something that reads it as an image descriptor, or running an unchecked UTF-8 conversion over vertex positions. That is a segfault, and the panic guard around plugin calls catches panics, not that.

No reordering fixes it: 2.5–2.8 expects `add_mesh_data` in the slot 2.9–2.10 expects `add_material_shader` in, so one of them is always wrong. Rejecting them all by name is the only honest repair. The fields are now in true append order, and `crates/renzora_plugin/tests/abi_order.rs` pins that order so the next insertion fails CI rather than shipping.

The lesson generalises past this ABI: **inserting a field next to its relatives reads as tidier than appending it three screens away**, which is exactly why it happened twice without anyone noticing in review. Order-is-ABI has to be enforced by a test, not by intent.

A test only protects this repository, though, and the whole point of the ABI is a plugin built somewhere else. So since **3.13** the table carries a hash of its own shape: entry *n* covers the first *n* fields' name and written type, and a plugin compares the entry for its own field count against the one it compiled with. Appending leaves every earlier entry untouched — which is precisely what the append-only rule promises — while inserting, reordering or retyping moves everything from that point on, and the plugin is refused with a message saying to rebuild.

It is deliberately conservative: it hashes the source text, so re-spelling a type or renaming a parameter refuses prebuilt plugins even though neither changes the ABI. That is the right way to be wrong — a false positive costs a rebuild, a false negative costs a call landing in a different function. Treat cosmetic edits to that struct as ABI edits.

MAJOR went to 2 when panels landed, so a plugin built against a 1.x ABI is refused and needs a rebuild — no source change, in most cases. The two changes that were not additive:

- Scope moved to its own exported symbol, `renzora_plugin_scope`, so the host can read whether a plugin is `Runtime` or `Editor` **before** running its init. An editor-only panel is now never initialised inside a shipped game, rather than being initialised and then ignored.
- Every enum a plugin writes is `#[repr(transparent)]` with associated constants where it previously had variants. Wire-identical, and your `Schedule::Update` still compiles — but a value outside the known range, read out of plugin memory, is no longer undefined behaviour. Values from a newer ABI now arrive as an unknown number the host can reject.

## Worked examples

The engine ships several, each under `plugins/`:

| Plugin | Demonstrates |
|---|---|
| `drift` | **hot reload you can see** — change a constant, rebuild, watch entities change course |
| `flock` | a resource shared across systems, `Option<&T>`, and a panel with bound sliders |
| `magnet` | `Or` filters and optional write access |
| `forge` | assets, and spawning renderable entities from a panel |
| `locomotion` | **reading animation state**, and why that beats tracking it plugin-side |
| `pulse` | a post-process effect driven by a system |
| `wobble` | pulling in a third-party crate (`noise`) |
| `widgets` | every panel widget, and nothing else |
| `ripple` | a [custom material](#custom-materials) and a [texture](#textures) regenerated each frame |
| `hair` | [reading a mesh](#reading-geometry-already-in-the-world) and rewriting one each frame |

Most are under 100 lines, with no Bevy anywhere in their dependency tree and nothing but the OS in their import table.

`hair` is the exception at ~710 lines, and it is the one to read if you are porting something real rather than starting fresh. It was an engine crate that linked Bevy directly, and what the boundary cost it is mostly two things: `Assets<T>`, worked around with a mesh pool, and `RemovedComponents`, worked around with a liveness sweep each frame. It ported completely and its crate is gone.

There used to be a `text3d` here too, and its fate is the more instructive story. Mesh mode ported; **flat mode did not**, because it needs a real Bevy `Material` and a runtime-sized SDF atlas, and a function table can carry neither. So the engine shipped two incomplete halves — a C-ABI plugin with mesh mode and a fixed entity cap, and a statically linked crate with both modes — until [native plugins](native-plugins.md) made the choice unnecessary. It is one plugin now, with both modes and no cap. When a port keeps running into the boundary rather than around it, that is the signal to reach for a native plugin instead.

Alongside them sit the **post-process effects** — every screen-space effect the engine ships except the handful wired into the render graph itself (bloom, SSAO, motion blur, the tonemapper). Each is a struct, a `Default`, one `add_post_process` line and a `.wgsl` file, which makes them the best set to read before writing your own: pick the one closest to what you want and follow its shape. `plugins/crt` is the one to open first — its module doc is where the reasoning about padding and `enabled` flags lives.

Two of the examples are worth singling out.

**`drift` exists to be broken.** Rebuild it while the editor runs and the change takes effect without a restart. Then try to break the reload: introduce a compile error and nothing happens, because the previous build keeps running; add a field to `Drift` and the reload is refused with the reason, because entities already carrying it were allocated for the old layout. Both are the intended behaviour, not bugs to report.

**`ripple` is built to fail legibly.** A material is the one part of the surface where a mistake gives you a black quad rather than a compile error — the uniform bound at the wrong group, a texture that never uploaded, a per-frame refresh that isn't running. So it puts all four in one entity and makes each failure a *different* picture; its module doc has the table. Add **Ripple** to any entity and read what you get.

It also shows the one thing about material components that reads oddly. `Ripple` carries two `_pad` fields, and one of them holds plugin state — because the component *is* the uniform block, byte for byte, so a field added for bookkeeping would shift every member after it and quietly hand the shader the wrong bytes. Padding the WGSL side already demands is the only space there is.

**`widgets` has no systems, no assets and nothing to spawn.** It is one panel containing every widget the BSN path can reach, so if something in it renders wrong the fault is in the parser, the widget's component front-end, or the widget — there is nothing else it could be. Open it from the add-panel menu after a build.

## Scene serialization

Plugin components survive save and load, even though they have no Rust type and no `TypeRegistration` for reflection to work from. The host mirrors each plugin's registered component schemas into a plain-data registry, and the scene format reads that instead of the type registry.

The interesting case is a scene saved with a plugin that is no longer loaded. Those components are **not** dropped — they are kept as raw data and re-attached by field name if the plugin comes back. So removing a plugin, opening a scene and saving it does not silently destroy the data it owned.

Field *names* are what re-attach, so renaming a field in a plugin orphans that field's saved values while the rest of the component still loads. Adding and removing fields is safe.

## Exposing an engine crate to plugins

*For engine developers.* If you write an in-tree Bevy plugin and want standalone plugins to drive it, you add a `plugin_bridge` module to **your own crate** — there is no separate bridge crate, and `renzora_plugin` never learns your name. The working examples are `renzora_animation`, `renzora_physics`, `renzora_scripting` (which owns HTTP) and `renzora_postprocess`.

How much you have to write depends on what you are exposing.

**Reading your state — nothing but a type path.** If the component is `#[repr(C)]` plain data, a plugin can query it directly: host components marshal as a straight byte copy, so the plugin declares a matching mirror and names your type.

```rust
// In the plugin:
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Health { pub current: f32, pub max: f32 }

renzora_plugin::host_component!(Health, "my_game::health::Health");
```

Field order and types must match exactly — a mismatch is a wrong-offset read, not a compile error. And the component must be plain data: a `String`, `Vec` or `Handle` in the mirror would hand the plugin a pointer into the engine's heap.

**You have to say it may be read.** Naming a type is not enough — a plugin that asks to read an engine component as *data* is refused unless the crate owning that type has opted in:

```rust
// In your crate's plugin_bridge::install:
renzora_plugin::host::expose_component_data::<Health>(app);
```

Filtering (`With<Health>` / `Without<Health>`) needs none of this and works for anything the engine registered for reflection. Data is the restricted direction, because it hands over raw bytes: without the gate a plugin could name `bevy_window::window::Window`, which owns a `String`, and follow a pointer into the engine's heap. Types whose layout is not stable enough to mirror should simply never be exposed — `GlobalTransform` wraps a `glam::Affine3A`, whose representation changes with the SIMD backend the engine was built with.

Note what the gate does **not** do: nothing can check that a plugin's mirror actually matches your type, because the plugin sends a name and nothing else. Exposing a type is a promise that its layout is stable and documented; mirroring it correctly is still the plugin author's job.

If your real component *can't* be plain data (`AnimatorReadState` is `String` + `HashMap`), synthesize a second numeric-only mirror and keep it in sync each frame. That is the bulk of what `renzora_animation::plugin_bridge` does.

**Constructing your components — already free.** `spawn_bsn` sends text that names components, resolved host-side through the reflection registry, so a plugin can construct any registered engine component — including yours — with neither side knowing the other exists.

**Triggering behaviour — a domain module, and nothing in the contract.** "Play this clip" isn't a field write, and there is no generic "call this function" across the boundary. But since 2.4 there is a generic *channel*: you write `src/<domain>.rs` in `renzora_plugin` behind a feature, defining your own ops and payload structs and tagging them with `service_id("renzora.<domain>")`. The host parks the bytes without reading them and your bridge drains the ones bearing your tag. `sys` never learns your domain exists, so this costs **no** [MINOR bump](#versioning) — see [Domain modules](#domain-modules).

**Answering a question — this one does touch the contract.** Commands are write-only, so anything that hands data *back* on demand needs a pointer in `SystemCall` and a system param to read it through. `Http::poll` and `Meshes::read` both took a MINOR for exactly this reason. Before reaching for one, check whether a numeric mirror the bridge refreshes each frame would do instead — `AnimState` and `PhysicsState` answer their questions that way and cost nothing.

### How they find each other

Reads are resolved **by string, at runtime** — `AppTypeRegistry::get_with_type_path` → `TypeId` → `ComponentId`. Nothing links. This is Bevy's own late binding, the same mechanism that lets a scene file name a component it doesn't import; the ABI just uses it from the far side of a `dlopen`.

The cost is that it's untyped. Rename your component and both sides still compile, the plugin gets `INVALID`, and its queries match nothing — silently, forever. So assert it at startup, where a mismatch is a panic naming both halves:

```rust
assert_eq!(
    <PluginAnimState as bevy::reflect::TypePath>::type_path(),
    <renzora_plugin::ecs::AnimState as renzora_plugin::ecs::Component>::TYPE_PATH,
);
```

Your crate can import `renzora_plugin`, so it can compare the two strings directly. Do this — it is the only thing standing between a rename and a class of bug with no error message.

### Where the module goes

In the crate that owns the domain, as long as that crate may depend on `renzora_plugin` (with the `host` feature). The dependency only ever runs that way: `renzora_plugin` must stay publishable to crates.io so a third-party author can `cargo add` it, and a published crate cannot have path dependencies.

The one exception is `renzora` itself, which is capped at Bevy + serialization by policy so a feature crate can never introduce a cycle. That is why the render bridge lives in `renzora_postprocess` rather than beside `RenderComposition` in `renzora` — one crate out, in an existing neighbour that owns the same domain.

### Editing the contract itself

*For engine developers.* If you are about to change anything in `crates/renzora_plugin/src/sys.rs`, read that file's module documentation first — it carries the full rule. The short version, because getting it wrong is silent:

Both sides compile their own copy of that file from separate source trees. There is no link step, no symbol to mismatch, and no version that moves on its own. **The layout is the entire contract.**

- **A table read by offset may be appended to.** `Interface`, `SystemCall`, `FrameCtx`, `CommandSink`, `MeshSource`, `ImageSource`, `HttpSource`, `PanelAction`. A field at the very end is invisible to a build compiled against the older layout. Append it, update the golden list in `tests/abi_order.rs`, bump `VERSION_MINOR` in the same commit.
- **Everything else is frozen — not even appendable.** Anything crossing through a pointer, by value inside another struct, or as opaque payload bytes is not read prefix-first: the reader dereferences every field, walks an array at its own `size_of`, or memcpys the whole thing. There is no prefix to preserve, so appending is exactly as fatal as reordering. **Mint a new type beside the old one and a new function that takes it.**
- **Grep for a type as a *field type* before touching it.** `StrRef` is embedded in seven boundary structs; `Vec3` in six, plus four pointer arrays. Growing either breaks structs declared elsewhere, with every golden list and every `size_of` unchanged and both sides compiling.
- **Some contracts are numbering, not shape.** A `Key`'s value *is* its bit index into `InputState`; each domain's `SERVICE` id is compiled into every plugin ever shipped. No layout test can see these.

The guard is `cargo test -p renzora_plugin --test abi_order`, which pins 60 types and 203 fields. It runs natively — this crate links no Bevy, so it does not need the container.

That guard exists because the append-only rule was broken **three times** without anyone noticing in review: `SystemEntry` gained a return value under a MINOR and killed the process with no diagnostic, and two functions were inserted into the middle of `Interface` and recorded in the changelog as appended. Inserting a field next to its relatives reads as tidier than appending it three screens away, which is exactly why it kept happening.

## Current limits

The full ledger is **[Plugin API status](./plugin-api-status.md)** — 215 entries, one row per
thing a Bevy developer might reach for. The summary:

| | |
|---|---|
| **22** | work identically — the source is character-for-character Bevy |
| **95** | differ — usable, but you write something else or it behaves differently |
| **60** | missing — no blocker, nobody built it |
| **38** | never — structurally blocked |

That shape is the useful part. **Most of what is absent is a backlog, not a boundary.** Of the
38 that will never work, almost all are one of four things:

- **A Bevy generic that would have to be instantiated** — `Assets<T>`, `Handle<T>`,
  `MeshMaterial3d<M>`, `EventReader<E>`, `ButtonInput<T>`. Monomorphization happens at compile
  time against a Bevy the plugin does not link. The dodge is a fixed-shape surface the host
  pre-monomorphized, which is why you write `app.add_mesh(...)` and not `meshes.add(...)`.
- **A capturing closure** — `Commands::queue`, `add_observer`, `entity.observe`, `on(|ev| …)`
  in BSN. A boxed closure is a destructor crossing the boundary. (The per-system token that
  would allow this already exists in the ABI, unused — so this one may move.)
- **A type with a destructor as component data** — `String`, `Vec`, `Box`, `Handle`, and
  therefore `&Children` and `&Name` as query data.
- **A method that is compiled code in the host binary** — `camera.world_to_viewport(..)`,
  `material.base_color = ..`, `Circle::new(50.0).mesh()`.

Everything else on the page is work nobody has done yet, and the order it gets worked in is the
[roadmap](./plugin-api-status.md#roadmap) at the bottom of it.

### The gaps most likely to matter to you

- **Scheduling vocabulary.** No `.run_if`, `.before`, `.after`, `.chain`, `.in_set`, no system
  sets, no states. `add_systems(Update, (a, b))` does not compile — one system per call. Two
  plugin systems in the same schedule have no defined order relative to each other.
- **`Startup` and `FixedUpdate`.** Five main-loop schedules only. An unknown one is re-homed to
  `Update` with a warning rather than dropped.
- **Change detection and removal.** No `Added<T>`, `Changed<T>`, `Ref<T>`, `RemovedComponents`.
  This is why `plugins/hair` and `plugins/text3d` both hand-roll a per-frame liveness sweep and
  a signature hash — read them before writing your own.
- **`Local<T>`, `ParamSet`, `Single`, messages, observers, component hooks.**
- **Assets by path.** No `AssetServer`. Meshes and images can be [created](#geometry) and
  [rewritten](#textures), but only ones the plugin made. Asset creation is init-only, which is
  why real plugins allocate a fixed pool in `build()` and hand out slots.
- **Gamepad.** Keyboard, mouse and cursor are covered.
- **Audio and navigation** have no domain module yet; they follow the shape of
  [animation, physics and HTTP](#domain-modules) when they land.
- **Adding** a panel, render pass or post-process effect during a [reload](#hot-reload).
  Editing an existing one works; adding one needs a restart.

### What is *not* a limit, contrary to earlier versions of this page

Two entries were wrong here for months, in opposite directions, and both are worth stating
plainly because they change what you would build.

**A plugin *can* configure Bevy's own rendering.** `commands.entity(e).insert(bsn! { Bloom })`
goes through reflection host-side and reaches **every** engine component that derives
`Reflect` — `DepthPrepass`, `Bloom`, `Tonemapping`, `Msaa`, SSAO, lights, camera settings. No
`#[repr(C)]` mirror, no `ComponentId`, no layout knowledge. See
[Shading geometry you didn't make](#shading-geometry-you-didnt-make) for the sibling case, and
mind that BSN insert **replaces rather than patches** — safe on a marker, destructive on a live
`Camera`.

**A plugin *can* find the camera and the lights.** Not with Bevy-identical source, but with
three lines:

```rust
pub struct Camera3d(());
renzora_plugin::host_component!(Camera3d, "bevy_camera::components::Camera3d");
```

Then `With<Camera3d>` compiles and matches. This works for any host component registered for
reflection. **Filter-only** — never use such a mirror as query *data*, because a mirror of a
non-plain-data host component hands you a pointer into the engine's heap.

`pool_water` and `text3d` were the two the C-ABI boundary could not hold, for reasons narrower
than "rendering integration": the first displaces vertices in a *vertex* shader, and a plugin
material supplies a fragment only; the second rasterizes glyphs into an SDF atlas sized at
runtime, and plugin image creation is init-only. Both are [native plugins](native-plugins.md)
now, which is the right answer to that shape of limit — a native plugin links the real Bevy, so
neither constraint applies to it.

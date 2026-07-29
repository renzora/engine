# Standalone Plugins (C ABI)

Write a plugin as a self-contained `cdylib` that never links Bevy, build it with any Rust toolchain on any machine, and drop the resulting library into `<exe>/plugins/`.

> This is a **second, independent** plugin mechanism, not a replacement for the one in [Building Plugins](./plugins.md). Both exist because they solve different problems — see [Which one to use](#which-one-to-use).

## Why this exists

A [distribution plugin](./plugins.md) shares one compiled `bevy_dylib` with the host. That sharing is what makes it fast and complete — it gets *all* of Bevy — but it also means the plugin must be built in the same environment as the editor it loads into. Cargo names the shared library `bevy_dylib-<metadata>.dll`, where the metadata hashes the package id, feature set, profile, `RUSTFLAGS`, target and rustc. Build the plugin somewhere else and it imports a differently-named library that isn't beside the exe, and the OS loader fails it before any Renzora code runs.

A standalone plugin sidesteps that entirely:

- **It exports exactly one symbol** (`renzora_plugin_init`) and **imports nothing** from the host.
- The host passes a `#[repr(C)]` function table *in* at load time.

There is no dynamic symbol to resolve against `renzora.exe`, so there is no filename to match, no `bevy_dylib-<hash>` to find, and no `TypeId` to line up. The only thing both sides must agree on is the layout of a handful of `#[repr(C)]` structs. That means a plugin built with rustc 1.90 loads into an editor built with rustc 1.95, and a plugin built in 2026 keeps loading into editors released later.

The price is that a standalone plugin reaches Bevy through a curated surface rather than all of it. That surface is designed to read *identically* to Bevy source — see [What it looks like](#what-it-looks-like).

## Which one to use

| | [Distribution plugin](./plugins.md) | Standalone plugin |
|---|---|---|
| Links Bevy | yes, shares the host's `bevy_dylib` | no |
| Toolchain | must match the canonical build env | any |
| Bevy surface | all of it | the ABI surface |
| Binary size | small (Bevy is shared) | ~210 KB (std linked statically) |
| Registers with | `renzora::add!` | `renzora_plugin::add!` |
| Breaks when | the editor's ABI moves | only on a MAJOR ABI bump |

Reach for a standalone plugin when you want to ship a prebuilt binary to people running editor versions you don't control, or when you'd rather not maintain a Docker toolchain to build a plugin. Reach for a distribution plugin when you need a part of Bevy the ABI doesn't expose yet.

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
rustflags = ["-C", "prefer-dynamic=no", "-C", "target-feature=+crt-static"]
```

Note the explicit `=no`. Cargo **merges** `rustflags` arrays across config files rather than replacing them, so omitting the flag achieves nothing — it has to be contradicted by a later entry. `crt-static` additionally drops `VCRUNTIME140.dll` and the `api-ms-win-crt-*` set; the engine itself can't use it (it moves crate disambiguators, which shifts the `TypeId`s the shared-dylib plugin ABI depends on) but a standalone plugin shares no types with anyone, so nothing there applies.

The cost is size: roughly 20 KB dynamically linked against ~210 KB statically. Take the 210 KB.

### Keep it out of the workspace

The empty `[workspace]` table is not optional if your plugin lives inside a checkout of the engine. Cargo's `exclude` key stops a directory being a workspace *member*, but it does not stop it resolving against the workspace — the plugin would still inherit the workspace's lockfile and feature unification, which quietly undoes the isolation the whole mechanism depends on. `[workspace]` makes the plugin its own workspace root.

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

Add the component to a camera and the effect runs; every field is an inspector slider.

> **`#[repr(C)]` and scalar padding are mandatory here.** WGSL aligns `vec3<f32>` to 16 bytes and Rust aligns `[f32; 3]` to 4, so the "same" struct is 32 bytes in the shader and 16 in Rust — which shows up as a GPU validation panic, not a compile error. Pad with scalar `f32`s on both sides. The engine validates the shader with naga at load and names this cause explicitly if it catches it.

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

## Worked examples

The engine ships several, each under `plugins/`:

| Plugin | Demonstrates |
|---|---|
| `spinner` | the minimum viable plugin |
| `orbit` | reading and writing a plugin component each frame |
| `tint` | a post-process effect with no systems |
| `pulse` | a post-process effect driven by a system |
| `wobble` | pulling in a third-party crate (`noise`) |
| `scatter` | assets and spawning renderable entities |
| `flock` | a resource shared across systems, and `Option<&T>` |

Every one is under 100 lines, with no Bevy anywhere in its dependency tree and nothing but the OS in its import table.

## Current limits

The surface is deliberately incremental. Not yet available:

- `Startup` and `FixedUpdate` schedules — systems run in the five main-loop schedules only
- Input (`ButtonInput<KeyCode>`, mouse, gamepad)
- Hierarchy (`ChildOf` / `Children`, `with_children`)
- Change detection (`Added<T>`, `Changed<T>`, `Ref<T>`)
- Messages, observers, and component hooks
- `Local<T>`, `ParamSet`, run conditions, system ordering
- The `Assets<T>` / `AssetServer` idiom, and loading assets by path
- Host components beyond `Transform`, `GlobalTransform`, `Visibility`, `Name` and `Mesh3d`
- **Scene serialization** — plugin components do not yet survive save and load

That last one is the significant one for shipping a real project, and it's the next thing being built.

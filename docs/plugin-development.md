# Plugin Development

How to extend Renzora with plugins, components, and scripting. This is for people building *on* the engine's Rust API — if you just want to install and use the editor, see the [README](../README.md).

## Plugin SDK

Plugins are Rust crates that get full Bevy ECS access — `Commands`, `Query`, `Res`, `ResMut`, `Assets`, everything. No FFI wrappers or translation layers.

The SDK (`renzora` crate) connects plugins to Bevy. It provides the `add!()` macro plus the shared editor contracts and types (e.g. `FieldDef`, the inspectable registries, and — under the `editor` feature — the `AppEditorExt` trait that adds `register_inspectable()`). It does not re-export engine internals: editor-framework traits live in their own crates a plugin can depend on directly (`EditorPanel` in `renzora_ui`, `ThemeManager` in `renzora_theme`). Otherwise plugins interact with engine systems through the ECS.

### Scaffolding

```bash
renzora add cool_fx              # plugin for the editor and exported games
renzora add my_panel --editor    # editor-only plugin
renzora add my_effect --dylib    # distributable plugin (drop-in .dll/.so/.dylib)
renzora remove cool_fx           # delete one
```

This creates `crates/renzora_<name>/` with a working skeleton and wires it into the build automatically — no manual edits to any other crate.

### Plugin Crate

The scaffold writes this for you:

```rust
use bevy::prelude::*;

#[derive(Default)]
pub struct CoolFxPlugin;

impl Plugin for CoolFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(PointLight::default());
}

renzora::add!(CoolFxPlugin);
```

```toml
[package]
name = "renzora_cool_fx"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false }
```

No `[lib]` section needed — the crate defaults to `rlib`, gets statically linked into the host binary, and self-registers via an `inventory` ctor at process startup. Same on desktop, iOS staticlib, Android cdylib, and WASM.

### Plugin Scope

Control when your plugin loads:

```rust
renzora::add!(MyPlugin);                          // Runtime (default)
renzora::add!(MyPlugin, Editor);                  // editor-only tooling
renzora::add!(MyPlugin, Runtime);                 // explicit (same as default)
renzora::add!(MyFoundation, Runtime, priority = -100); // load earlier
```

Scopes are **exclusive** — a plugin is either `Runtime` or `Editor`, there is no "both". `Runtime` plugins run in the game (and, for engine/statically-linked plugins, in the editor viewport that hosts the runtime); `Editor` plugins are editor-only tooling (panels, gizmos). A feature that needs editor tooling on top of runtime behaviour ships **two** plugins — one of each scope (e.g. `GameUiPlugin` + `GameUiEditorPlugin`).

The macro emits an `inventory::submit!` block; the runtime iterates the registry once at startup and calls `app.add_plugins(...)` on every entry whose scope matches the host. No central enumeration, no manual `add_plugins` to keep in sync.

### Distribution Plugins

To ship a plugin as a standalone file users can drop into `<renzora>/plugins/`, scaffold it with `--dylib`:

```sh
renzora add cool_fx --dylib
```

Build it and drop the resulting `.dll`/`.so`/`.dylib` into the engine's `plugins/` directory — the engine finds and loads it at startup. It has to be built against the same engine version, or it's rejected at load time.

## Creating Components

Components are data types that attach to entities and show up in the editor inspector with editable fields.

```rust
use bevy::prelude::*;
use renzora::prelude::*;

#[derive(Component, Default, Reflect, Inspectable)]
#[inspectable(name = "Health", icon = "HEART", category = "gameplay")]
pub struct Health {
    #[field(speed = 1.0, min = 0.0, max = 10000.0)]
    pub current: f32,
    #[field(speed = 1.0, min = 1.0, max = 10000.0)]
    pub max: f32,
    #[field(name = "Shield")]
    pub has_shield: bool,
}

#[derive(Default)]
pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<Health>();
    }
}

renzora::add!(HealthPlugin);
```

Field types are inferred: `f32` renders as a drag slider, `bool` as a checkbox, `String` as a text input, `Vec3` as XYZ fields.

`#[field(...)]` attributes: `speed`, `min`, `max`, `name`, `skip`, `readonly`

`#[inspectable(...)]` attributes: `name`, `icon` (Phosphor icon name), `category`, `type_id`

## Scripting

The engine supports Rhai (all platforms) and Lua (native only) scripting. Components registered with `register_inspectable()` are automatically available to scripts — no extra setup needed. The component is added to Bevy's ECS and reflection system, so scripts can read and write any field.

```lua
-- get a component field on self
local hp = get("Health.current")

-- set a component field on self
set("Health.current", 50.0)

-- read/write a field on a named entity
local boss_hp = get_on("Boss", "Health.current")
set_on("Boss", "Health.current", 50.0)
```

This works for any component from any plugin. If someone publishes a `Sun` plugin with a `SunLight` component, scripts can immediately do `set("SunLight.intensity", 2.0)` (or `set_on("World Environment", "SunLight.intensity", 2.0)`) without the plugin author writing any scripting glue.

## Workspaces and Stable ABI

Rust has no stable ABI, so the engine and its plugins must be compiled together to share types safely. Each target builds with `--workspace` into its own directory (`target/{editor,runtime,server}/`), compiling Bevy once into a shared `bevy_dylib` that everything links against — giving identical `TypeId`s. Separate directories also mean switching targets doesn't invalidate the others' caches.

Distribution plugins are checked at load time: each exports a `plugin_bevy_hash()`, and the loader rejects any whose hash doesn't match the engine's (i.e. built with a different compiler, Bevy version, or profile).

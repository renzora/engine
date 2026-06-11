# Creating Components

Define your own data components, make them survive scene save/load, and expose their fields in the editor's Inspector.

This page assumes you already know the basics of Bevy components, queries, and the `renzora::add!` plugin macro — see [ECS & Bevy](/docs/r1-alpha5/engine-core/ecs) first. Everything here is about the two Renzora-specific layers on top of a plain component: **scene serialization** (`register_type`) and the **`#[derive(Inspectable)]`** editor card.

## A plain component

Any Rust struct or enum can be a component. Derive `Component` and attach it to an entity:

```rust
use bevy::prelude::*;

#[derive(Component)]
pub struct Inventory {
    pub items: Vec<String>,
    pub max_slots: usize,
}

fn spawn_chest(mut commands: Commands) {
    commands.spawn((
        Name::new("Chest"),
        Transform::default(),
        Inventory { items: vec![], max_slots: 20 },
    ));
}
```

A plain component like this works at runtime, but it is **invisible to scene files and to the editor** until you add reflection (below). Nothing else is required to use it in systems and queries.

## Making a component persist in scenes

Renzora scenes are RON files produced by `renzora_engine::scene_io`. Saving serializes the live ECS world through Bevy's reflection (`DynamicSceneBuilder`), so a component is only written to (and read back from) a scene if its type is **registered for reflection** and is **reflectable + serde-serializable**.

The canonical form for a serializable gameplay component is:

```rust
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Reflect, Serialize, Deserialize, Default, Clone, Debug)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DayNightCycle {
    /// 0.0 = midnight, 0.5 = noon
    pub time_of_day: f32,
    /// hours of in-game time per real second
    pub cycle_speed: f32,
    pub sun_color: [f32; 3],
}
```

What each piece does:

| Derive / attribute | Why it's needed |
|---|---|
| `Component` | Makes it an ECS component. |
| `Reflect` | Lets Bevy walk the type at runtime — required for any scene I/O. |
| `Serialize` + `Deserialize` (serde) | The serializers used when serde reflection is enabled. |
| `#[reflect(Component, ...)]` | Registers the `ReflectComponent` data so the scene loader can **insert** the type onto an entity, plus `Serialize`/`Deserialize` so round-tripping uses your serde impls. |
| `Default` | Required by the loader to construct the value, and by `#[derive(Inspectable)]` (below). |

> A **marker** component (zero fields) only needs `#[derive(Component, Reflect, Default)]` + `#[reflect(Component)]` — there is nothing to serialize, so the serde derives are optional.

### Register the type

Deriving `Reflect` is not enough on its own — the type must be added to the app's type registry with `register_type`. Do this in a plugin's `build`:

```rust
use bevy::prelude::*;

pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DayNightCycle>();
        app.add_systems(Update, advance_day_night);
    }
}

// Self-register in the global plugin inventory (Runtime scope by default).
renzora::add!(DayNightPlugin);
```

> **Register from a `Runtime`-scoped plugin (the `add!` default), not an editor-only one.** Editor plugins ship only inside the removable `renzora_editor` bundle. If a type is registered *only* by an editor plugin, the shipped game (no bundle) never registers it, and the scene loader silently drops it on load — see the lossy-load note below.

### How save and load behave

Knowing the rules avoids surprises:

- **Only entities with a `Name` are saved.** `save_scene` queries `With<Name>`, so anonymous entities are skipped. (Naming an entity also auto-attaches a `ScriptComponent`.)
- **Resources are never saved.** The builder calls `deny_all_resources()`; scenes carry entities + components only. Use resources for transient global state, not save data.
- **Runtime/editor-only components are stripped.** Meshes, materials, cameras, `avian3d` physics bodies, animation runtime state, `bevy_ui` camera plumbing, and networking markers are explicitly denied — they get regenerated on load from their serializable companions (e.g. `MeshInstanceData`, `PhysicsBodyData`). Anything that fails to serialize is dropped too.
- **Loading is lossy.** If a scene names a type that isn't registered, `load_scene` strips just that entry, keeps loading the rest, logs a warning, and fires a `SceneLoadedWithSkippedTypes` event. A scene from the editor that references a component your runtime build never registered will load with that component missing rather than failing.

## Making a component editable in the Inspector

To give a component an editable card in the editor's Inspector, derive `renzora::Inspectable` and annotate the fields. This is the same `Health`/`Movement` pattern shipped in the `renzora_test_component` example crate:

```rust
use bevy::prelude::*;
use renzora::{AppEditorExt, Inspectable};

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

pub struct HealthEditorPlugin;

impl Plugin for HealthEditorPlugin {
    fn build(&self, app: &mut App) {
        // Registers the type AND the Inspector card in one call.
        app.register_inspectable::<Health>();
    }
}

renzora::add!(HealthEditorPlugin, Editor);
```

`register_inspectable::<T>()` (from the `AppEditorExt` trait) calls `register_type::<T>()` for you and then registers the generated `InspectorEntry`. `T` must implement `Component + Default + Reflect` — the derive does **not** add those, so list them yourself.

> `Inspectable` does not derive `#[reflect(Component)]`. If the same component must also round-trip through scene files in the **shipped game**, follow the dual-plugin pattern in the next section so the type is registered (and serialized) at runtime, independently of the editor.

### Enable the `editor` feature

`Inspectable`, `AppEditorExt`, `register_inspectable`, and the `#[field(...)]` / `#[inspectable(...)]` attributes are all gated behind the `renzora` crate's `editor` feature (default features are empty). Your crate's `Cargo.toml` must opt in:

```toml
[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false, features = ["editor"] }
```

> Import from the crate root: `use renzora::{AppEditorExt, Inspectable};` or `use renzora::*;`. There is **no `renzora::prelude`** module.

### `#[inspectable(...)]` — struct attributes

| Attribute | Effect | Default |
|---|---|---|
| `name = "..."` | Card title in the Inspector. | Title-cased struct name |
| `icon = "..."` | Phosphor icon constant (`SCREAMING_SNAKE_CASE`, e.g. `HEART`, `SNEAKER_MOVE`); lowercased to a kebab-case glyph name. | `"CUBE"` |
| `category = "..."` | Grouping bucket in the Add-Component overlay. | `"component"` |
| `type_id = "..."` | Stable string id for the entry. | snake_case of the struct name |

### `#[field(...)]` — field attributes

| Attribute | Effect |
|---|---|
| `name = "..."` | Override the field's label (default: title-cased field name). |
| `speed = <f32>` | Drag sensitivity for `Float`/`Vec3` widgets. |
| `min = <f32>` / `max = <f32>` | Clamp range for `Float` widgets. |
| `skip` | Exclude the field from the Inspector entirely. |
| `readonly` | Show the field as a non-editable debug string. |

### Field-type mapping

The widget is inferred from the field's Rust type — there is no `type =` attribute:

| Rust type | Inspector widget |
|---|---|
| `f32`, `f64` | `Float` drag (honours `speed`/`min`/`max`) |
| `bool` | checkbox |
| `Vec3` | three-component drag (honours `speed`) |
| `String` | single-line text input |
| `Color` | color picker |
| anything else | read-only debug string |

> Fields named `enabled` and any field starting with `_p` are skipped automatically — they are reserved for the post-process effect macro's padding/toggle convention.

## The dual-plugin pattern (data + editor)

Runtime plugins (rlibs, registered through `add!`'s inventory) ship in every build. Editor plugins ship only in the `renzora_editor` bundle and are filtered by `PluginScope` — there is **no "both" scope**, so a feature that needs runtime data *and* an editor card ships **two plugins**:

```rust
use bevy::prelude::*;

// Always shipped: registers the type so it round-trips in editor AND game.
pub struct HealthPlugin;
impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Health>();
        app.add_systems(Update, tick_health);
    }
}
renzora::add!(HealthPlugin); // Runtime scope

// Editor-only: adds the Inspector card (and re-registers the type for the editor).
#[cfg(feature = "editor")]
pub struct HealthEditorPlugin;
#[cfg(feature = "editor")]
impl Plugin for HealthEditorPlugin {
    fn build(&self, app: &mut App) {
        use renzora::AppEditorExt;
        app.register_inspectable::<Health>();
    }
}
#[cfg(feature = "editor")]
renzora::add!(HealthEditorPlugin, Editor);
```

This is how Renzora's own dual-mode crates are split (e.g. the runtime `renzora_<name>` crate plus a `renzora_<name>/editor` subcrate). The runtime half guarantees the component is registered and serializable in the shipped game; the editor half adds the authoring UI.

## Components vs. resources

| | Component | Resource |
|---|---|---|
| Attached to | an entity | the world (one global instance) |
| Accessed via | `Query<&MyComponent>` | `Res<MyResource>` / `ResMut<MyResource>` |
| Use for | per-entity data | global state, config |
| Saved in scenes | yes (if reflected + registered + on a named entity) | **no** (`deny_all_resources`) |

For resources, message/observer events, systems, and scheduling, see [ECS & Bevy](/docs/r1-alpha5/engine-core/ecs).

# Custom Inspector Fields

Make your own components editable in the Inspector with a `#[derive(Inspectable)]` and one `register_inspectable` call — no egui, no closures, just declarative field metadata.

## The model: declarative `FieldDef`s, not egui closures

The Inspector is data-driven. Each component type registers an `InspectorEntry` — a name, icon, category, and a `Vec<FieldDef>` — into the `renzora::InspectorRegistry` resource. A `FieldDef` is just a label, a `FieldType` (which widget to show), and a pair of `fn(&World, Entity)` get/set pointers. The editor's native **bevy_ui** inspector walks the registry, reads each field through `get_fn`, draws the matching widget, and writes edits back through `set_fn`. You almost never build a `FieldDef` by hand — the `#[derive(Inspectable)]` macro generates the whole entry from your struct.

These types live in the `renzora` crate (the shared `renzora.dll` contract), gated behind its **`editor`** feature so non-editor builds carry zero editor surface.

> ⚠️ **egui is gone.** There is no `InspectorRegistry::register::<T>(|ui, comp| { ui.add(egui::Slider …) })` closure API, no `egui::DragValue`/`ComboBox`/`CollapsingHeader`, and no `renzora_editor::InspectorRegistry` import. Any example showing an `egui::*` widget or a `|ui, comp|` body is from a dead API — ignore it. Declarative `FieldDef`s drive everything; for genuinely custom UI you register a **bevy_ui** drawer (last section), not an egui closure.

## The fast path: `#[derive(Inspectable)]`

Annotate a component struct, then register it from your plugin. This is the complete `renzora_test_component` example that ships in the engine:

```rust
use bevy::prelude::*;
use renzora::{AppEditorExt, Inspectable};

/// Health component with current/max HP and a shield flag.
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
pub struct TestComponentPlugin;

impl Plugin for TestComponentPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<Health>();
    }
}

// Editor-scope: replayed into the app by the renzora_editor bundle.
renzora::add!(TestComponentPlugin, Editor);
```

`register_inspectable::<T>()` (an `AppEditorExt` method on `App`) does two things: it calls `app.register_type::<T>()` (so scripts and reflection can reach the component) and inserts `T::inspector_entry()` into the `InspectorRegistry`. Select an entity carrying `Health` and the component appears with a heart icon, two clamped drag values, and a "Shield" checkbox.

### Cargo dependencies

`Inspectable`, `AppEditorExt`, `FieldDef`, and friends are behind `renzora`'s `editor` feature, which is **off by default**. A crate that derives `Inspectable` must turn it on:

```toml
[dependencies]
bevy = { workspace = true }
renzora = { path = "../renzora", default-features = false, features = ["editor"] }
renzora_macros = { path = "../renzora_macros" }
```

> There is **no `renzora::prelude`**. Import what you need directly — `use renzora::{AppEditorExt, Inspectable};` — or glob with `use renzora::*;`.

### `#[inspectable(...)]` — struct attributes

All optional; each has a sensible default derived from the struct name.

| Attribute | Default | Effect |
|---|---|---|
| `name = "..."` | title-cased struct name | Section header shown in the Inspector |
| `icon = "..."` | `"CUBE"` | Phosphor icon **constant** (`SCREAMING_SNAKE_CASE`); emitted as a kebab-case name (`HEART` → `heart`, `SNEAKER_MOVE` → `sneaker-move`) that the native inspector resolves to a glyph |
| `category = "..."` | `"component"` | Grouping key (e.g. `"gameplay"`) |
| `type_id = "..."` | snake_case struct name | Stable string key for the entry in the registry |

### `#[field(...)]` — field attributes

| Attribute | Applies to | Effect |
|---|---|---|
| `speed = <num>` | `Float` (default `0.01`), `Vec3` (default `0.1`) | Drag sensitivity |
| `min = <num>` | `Float` (default `f32::MIN`) | Lower clamp |
| `max = <num>` | `Float` (default `f32::MAX`) | Upper clamp |
| `name = "..."` | any field | Override the label (default = title-cased field name) |
| `skip` | any field | Omit the field from the Inspector entirely |
| `readonly` | any field | Force a non-editable `{:?}` debug display |
| `default = <num>` | (see note) | Parsed but **ignored** by `#[derive(Inspectable)]` |

> ⚠️ `#[field(default = ...)]` is recognised by the attribute parser but **has no effect under `#[derive(Inspectable)]`** — initial values come from your type's own `Default` impl (the generated "Add Component" action inserts `T::default()`). `default` is only consumed by the `#[post_process]` attribute macro, which uses it to build the effect's `Default`. Don't rely on it to seed inspector fields.

### Supported field types

The derive infers the widget from each field's Rust type. Only these map to an editable widget; **anything else becomes a read-only debug string**:

| Rust field type | `FieldType` | Widget |
|---|---|---|
| `f32`, `f64` | `Float { speed, min, max }` | Drag value / slider |
| `bool` | `Bool` | Checkbox / toggle |
| `Vec3` | `Vec3 { speed }` | XYZ drag values |
| `String` | `String` | Text input |
| `Color` (bevy) | `Color` | RGB color picker |
| *everything else* | `ReadOnly` | Non-editable `{:?}` text |

> ⚠️ The derive does **not** auto-handle `i32`/`u32` and other integers, `Vec2`, `Entity`, enums, or `Handle<_>` — they all silently fall through to `ReadOnly`. To make those editable, use the manual field macros or a native drawer below. (This is the big departure from older docs, which claimed automatic integer drags, `Vec2`, entity pickers, and enum dropdowns from the derive.)

### Requirements

A struct deriving `Inspectable` must:

- Be a **struct with named fields** (tuple/unit structs and enums are rejected at compile time).
- Derive `Component` and `Default` — `register_inspectable::<T>()` requires `T: Component + Default`, and the generated add action inserts `T::default()`.
- Derive `Reflect` — `register_inspectable` calls `register_type::<T>()`, which needs `T: GetTypeRegistration`.

Fields named `enabled` or starting with `_p` are skipped automatically (a convenience for the post-process layout convention).

## Editor-only registration in dual-mode plugins

The `TestComponentPlugin` above is **editor-scope** (`add!(_, Editor)`), so its whole crate is always built with the `editor` feature and the `register_inspectable` call needs no guard.

A plugin that must run in **both** the editor and the shipped game (for example, a post-process effect or any runtime component that also wants an inspector) is different: its `register_inspectable` call must be compiled out when the `editor` feature is off. The crate declares its own `editor` feature that forwards to `renzora/editor`, and gates the call. This is exactly what every effect crate (e.g. `renzora_ascii`) does:

```toml
[features]
default = ["editor", "dlopen"]
dlopen = []
editor = ["renzora/editor"]
```

```rust
use bevy::prelude::*;
#[cfg(feature = "editor")]
use renzora::AppEditorExt;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MySettings>();
        // ... runtime systems / plugins that run everywhere ...

        #[cfg(feature = "editor")]
        app.register_inspectable::<MySettings>();
    }
}

renzora::add!(MyPlugin);
```

When the game is built without the editor feature, the derive's `InspectableComponent` impl and the registration both vanish, leaving only the plain runtime component.

## Beyond the derive: manual `FieldDef`s

For fields the derive maps to `ReadOnly` (integers, enum-as-`u32`, RGBA-with-alpha, `Vec3`-stored colors), build the `FieldDef`s yourself with the field-builder macros and hand the entry to `register_inspector`. The macros are exported at the `renzora` crate root (under the `editor` feature):

| Macro | Field shape | Widget |
|---|---|---|
| `renzora::float_field!(name, Comp, field, speed, min, max)` | `f32` | Drag value |
| `renzora::int_field!(name, Comp, field, ty, speed, min, max)` | `u32`/`i32`/… | Drag value, cast on write |
| `renzora::bool_field!(name, Comp, field)` | `bool` | Checkbox |
| `renzora::string_field!(name, Comp, field)` | `String` | Text input |
| `renzora::color_rgba_field!(name, Comp, field)` | `Color` | RGBA picker (editable alpha) |
| `renzora::vec3_color_field!(name, Comp, field)` | `Vec3` (RGB) | Color picker |
| `renzora::tuple_color_field!(name, Comp, field)` | `(f32, f32, f32)` | Color picker |
| `renzora::enum_u32_field!(name, Comp, field, ["A", "B", ...])` | `u32` index | Dropdown |

```rust
use bevy::prelude::*;
use renzora::{
    AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry,
    int_field, enum_u32_field,
};

#[derive(Component, Default, Reflect)]
pub struct SpawnConfig {
    pub max_entities: u32, // integer → not handled by the derive
    pub mode: u32,         // enum-as-index → dropdown
    pub spawn_rate: f32,
}

fn register_spawn_config(app: &mut App) {
    app.register_type::<SpawnConfig>();
    app.register_inspector(InspectorEntry {
        type_id: "spawn_config",
        display_name: "Spawn Config",
        icon: "sparkle", // already kebab-case here, not a constant
        category: "gameplay",
        has_fn: |w, e| w.get::<SpawnConfig>(e).is_some(),
        add_fn: Some(|w, e| { w.entity_mut(e).insert(SpawnConfig::default()); }),
        remove_fn: Some(|w, e| { w.entity_mut(e).remove::<SpawnConfig>(); }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            int_field!("Max Entities", SpawnConfig, max_entities, u32, 1.0, 0.0, 10000.0),
            enum_u32_field!("Mode", SpawnConfig, mode, ["Burst", "Stream", "Trickle"]),
            FieldDef {
                name: "Spawn Rate",
                field_type: FieldType::Float { speed: 0.1, min: 0.0, max: 100.0 },
                get_fn: |w, e| w.get::<SpawnConfig>(e).map(|c| FieldValue::Float(c.spawn_rate)),
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(f), Some(mut c)) = (v, w.get_mut::<SpawnConfig>(e)) {
                        c.spawn_rate = f;
                    }
                },
            },
        ],
    });
}
```

> Inspector entries sort with `type_id == "name"` first, `"transform"` second, `"material_ref"` third, and everything else appended in registration order. `add_fn`/`remove_fn` being `Some` is what makes a component show up in the **Add Component** overlay and gain a trash button; leave them `None` to register a read-only, non-removable card.

## Custom bevy_ui drawers

When declarative fields aren't enough — conditional rows, buttons, validation warnings, a bespoke layout — register a native drawer that builds an arbitrary `bevy_ui` subtree for the component. This is the modern replacement for the old egui closure:

```rust
use bevy::prelude::*;
use renzora::AppEditorExt;

// NativeInspectorDrawer = fn(&mut World, Entity) -> Entity
fn draw_spawn_config(world: &mut World, entity: Entity) -> Entity {
    let mut q = bevy::ecs::world::CommandQueue::default();
    let root;
    {
        let mut c = Commands::new(&mut q, world);
        root = c.spawn(Node { /* ... */ ..default() }).id();
        // build + bind ember widgets, conditional rows, buttons, etc.
    }
    q.apply(world);
    root
}

fn register(app: &mut App) {
    app.register_native_inspector_ui("spawn_config", draw_spawn_config);
}
```

The drawer takes `&mut World` and the selected `Entity`, builds its UI (typically via a local `CommandQueue` so it can use ember widgets and two-way bindings), and returns the **root entity**; the inspector parents that under the component's section header. Read the world through bindings and write it from your own systems or interaction callbacks — see *Building Editor Panels* for the reactive helpers.

## Reference

### `FieldType`

```rust
pub enum FieldType {
    Float { speed: f32, min: f32, max: f32 },
    Vec3  { speed: f32 },
    Bool,
    Color,
    ColorRgba,                              // RGBA with editable alpha
    String,
    ReadOnly,
    Asset { extensions: Vec<String> },      // drag-drop from the asset browser
    Enum  { options: &'static [&'static str] },
}
```

### `FieldValue`

```rust
pub enum FieldValue {
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
    Color([f32; 3]),
    ColorRgba([f32; 4]),
    String(String),
    ReadOnly(String),
    Asset(Option<String>),
    Enum(String),
}
```

### `InspectorEntry`

```rust
pub struct InspectorEntry {
    pub type_id: &'static str,
    pub display_name: &'static str,
    pub icon: &'static str,                 // kebab-case Phosphor name
    pub category: &'static str,
    pub has_fn: fn(&World, Entity) -> bool,
    pub add_fn: Option<fn(&mut World, Entity)>,
    pub remove_fn: Option<fn(&mut World, Entity)>,
    pub is_enabled_fn: Option<fn(&World, Entity) -> bool>,
    pub set_enabled_fn: Option<fn(&mut World, Entity, bool)>,
    pub fields: Vec<FieldDef>,
}
```

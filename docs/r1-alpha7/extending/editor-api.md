# Editor Features from Code

Everything the editor does that a plugin can hook into, and the call that hooks into it.

[Panels](panels.md) covers *drawing* — this page covers *behaviour*: the registries a [native plugin](native-plugins.md) writes into to add inspector sections, spawn menu entries, keyboard shortcuts, viewport tools, hierarchy icons and console output, plus the shared state every plugin reads.

All of it lives in the **contract crate**, `renzora`, and is glob-re-exported at the crate root. Write `renzora::EditorSelection`, never `renzora::editor_contract::EditorSelection`. Most of the registrations are methods on `App` from one of two traits:

```rust
use renzora::editor_contract::AppEditorExt;   // inspector, presets, tools, shortcuts, icons
use renzora::core::RenzoraShellExt;           // panels, status bar
```

A registration is a build-time act. Call it from your plugin's `build(&self, app: &mut App)`; there is no runtime registration, and nothing is ever unregistered (see [Limits](native-plugins.md#limits)).

## Selection

`EditorSelection` is the resource everything in the editor agrees on. It uses interior mutability, so `Res<EditorSelection>` is enough to change it — a system that only *sets* selection needs no exclusive access and no `ResMut`.

```rust
use renzora::EditorSelection;

fn my_system(selection: Res<EditorSelection>, q: Query<&Name>) {
    for e in selection.get_all() {
        if let Ok(name) = q.get(e) { info!("selected: {name}"); }
    }
}
```

| Method | |
|---|---|
| `get() -> Option<Entity>` | the primary (last-clicked) selection |
| `get_all() -> Vec<Entity>` | everything selected, in click order |
| `set(Option<Entity>)` | replace the selection |
| `set_multiple(Vec<Entity>)` | replace with many |
| `toggle(Entity)` | add/remove one (ctrl-click semantics) |
| `select_range(&visible_order, anchor, target)` | shift-click semantics; you supply the display order |
| `is_selected(Entity) -> bool` | |
| `has_multi_selection() -> bool` | |
| `clear()` | |
| `version() -> u64` | bumps on every change — a cheap dirty check for a `keyed_list_tokened` token |

`version()` is the right dependency for a panel that must rebuild on selection change: comparing it is O(1), where diffing `get_all()` is not.

## Play mode

`PlayModeState` tells you what the editor is doing. The distinctions matter and are easy to get wrong:

| Method | True when |
|---|---|
| `is_editing()` | stopped |
| `is_playing()` / `is_paused()` | full play mode |
| `is_simulating()` | in-editor Simulate — the scene runs, editor chrome stays live |
| `is_in_play_mode()` | Playing or Paused — **deliberately excludes Simulate** |
| `is_scripts_running()` | Playing or Simulating — scripts, physics and animation are ticking |

Use `is_in_play_mode()` to decide whether to hide editor chrome or swap to the game camera; Simulate must read as "not in play mode" there or all that tooling switches off. Use `is_scripts_running()` to decide whether gameplay logic should tick.

Two run conditions ship ready-made: `not_in_play_mode` for editor systems that must stop during play, and `in_three_view` / `in_two_view` for systems whose visuals only make sense through a 3D or 2D camera.

```rust
app.add_systems(Update, my_gizmo_system.run_if(renzora::core::not_in_play_mode));
```

## The inspector

### Declarative fields

Register a component and the inspector builds its section: a header with your icon, one row per field, drag-values and colour pickers and asset slots chosen from the field types.

```rust
use renzora::editor_contract::{
    AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry,
};

app.register_inspector(InspectorEntry {
    type_id: "my_glow",
    display_name: "Glow",
    icon: "sun",
    category: "Rendering",
    has_fn: |world, e| world.get::<Glow>(e).is_some(),
    add_fn: Some(|world, e| { world.entity_mut(e).insert(Glow::default()); }),
    remove_fn: Some(|world, e| { world.entity_mut(e).remove::<Glow>(); }),
    is_enabled_fn: None,
    set_enabled_fn: None,
    fields: vec![FieldDef {
        name: "Intensity",
        field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 10.0 },
        get_fn: |world, e| world.get::<Glow>(e).map(|g| FieldValue::Float(g.intensity)),
        set_fn: |world, e, v| {
            if let (Some(mut g), FieldValue::Float(f)) = (world.get_mut::<Glow>(e), v) {
                g.intensity = f;
            }
        },
    }],
});
```

`add_fn: None` keeps the component out of the **Add Component** overlay; `remove_fn: None` hides the section's toggle and trash controls. `is_enabled_fn` + `set_enabled_fn` together give the section a toggle switch.

The field types available: `Float { speed, min, max }`, `Int { min, max }`, `Vec3 { speed }`, `Bool`, `Color`, `ColorRgba`, `String`, `ReadOnly`, `Asset { extensions }`, `AssetCreatable { extensions, create_fn }`, `Enum { options }`, `DynamicEnum { options }`, and `Button { icon }`.

Three of those have traps worth naming. **`Int` is not cosmetic** — a `set_fn` that rounds into an integer field *must* pair with `FieldType::Int`, or the widget's fractional drag model and your rounded re-read fight each other and the value visibly stutters mid-drag. **`Button` has no value**: its `get_fn` should return `None`, and a click calls `set_fn` with `FieldValue::Bool(true)` as the press signal. **`DynamicEnum` speaks indices**, not labels — `FieldValue::Float(idx)` — which is what makes it keyframeable.

`register_inspectable::<T>()` is the shortcut when your component derives `Inspectable`: it registers the type for reflection *and* the generated entry in one call. See [Custom Inspector Fields](../editor-dev/inspector-fields.md) for the derive attributes.

### Native drawers

When declarative fields are not enough — conditional rows, a custom widget, a button that depends on other state — register a drawer instead. It gets `&mut World` and returns the root entity of a `bevy_ui` subtree, so the whole of [ember](panels.md) is available:

```rust
app.register_native_inspector_ui("my_glow", |world, entity| { /* -> Entity */ });
```

The `type_id` must match the `InspectorEntry` you registered. A drawer replaces the field rows; the section header stays.

## Spawning

### Entity presets

An entry in the hierarchy's spawn overlay:

```rust
use renzora::editor_contract::EntityPreset;

app.register_entity_preset(EntityPreset {
    id: "my_plugin.beacon",
    display_name: "Beacon",
    icon: "lighthouse",
    category: "Lights",
    spawn_fn: |world| {
        world.spawn((Name::new("Beacon"), Transform::default(), Beacon)).id()
    },
});
```

Give what you spawn a `Name`. The hierarchy panel queries `(Entity, &Name)`, so an unnamed entity has no row, cannot be clicked, cannot be selected and gets no gizmo — and the scene serializer only writes named entities, so it also vanishes on reload.

### Scene starters

The cards on the hierarchy's empty-state picker, for filling a blank scene in one click:

```rust
use renzora::editor_contract::SceneStarter;

app.register_scene_starter(SceneStarter {
    id: "my_plugin.arena",
    title: "Physics Arena",
    description: "A floor, four walls and a camera.",
    icon: "cube",
    spawn_fn: |world| { /* spawn as much as you like */ },
});
```

`spawn_fn` may spawn any number of entities, insert resources, or switch workspace. Re-registering an existing id is ignored rather than replacing.

### Hierarchy icons

So the tree shows your component with its own glyph instead of the generic one:

```rust
use renzora::editor_contract::ComponentIconEntry;

app.register_component_icon(ComponentIconEntry {
    type_id: std::any::TypeId::of::<Beacon>(),
    name: "Beacon",
    icon: "lighthouse",
    color: [255, 200, 120],
    priority: 10,
    dynamic_icon_fn: None,
});
```

`priority` breaks ties when an entity has several registered components — higher wins, which is how a camera outranks the mesh it also carries. `name` is what the hierarchy's filter-by-type control shows. `dynamic_icon_fn` lets the icon depend on entity state (an on/off light, a playing/stopped emitter).

## Keyboard shortcuts

```rust
use bevy::input::keyboard::KeyCode;
use renzora::core::keybindings::KeyBinding;
use renzora::editor_contract::ShortcutEntry;

app.register_shortcut(ShortcutEntry::new(
    "my_plugin.snap_all",
    "Snap Selection to Ground",
    "Transform",
    KeyBinding::new(KeyCode::KeyG).ctrl().shift(),
    |world| { /* &mut World */ },
));
```

The binding you pass is a *default*. It is seeded into `KeyBindings::plugin_bindings` under your id, appears in **Settings → Shortcuts** under your category, and the user can rebind it — after which your default is never consulted again. So the id must stay stable across releases; changing it orphans everyone's rebind.

Use an existing category (`"Camera"`, `"Tools"`, `"Transform"`, …) to group with the built-ins, or any string for a category of your own.

## Viewport tools

A tool is a button on the viewport toolbar with predicates for visibility and active state:

```rust
use renzora::editor_contract::{ToolEntry, ToolSection};

app.register_tool(
    ToolEntry::new("my_plugin.paint", "paint-brush", "Paint (B)", ToolSection::Shelf("myplugin.a-paint"))
        .order(10)
        .visible_if(|world| world.get_resource::<MyMode>().is_some())
        .active_if(|world| world.resource::<MyMode>().painting)
        .on_activate(|world| { world.resource_mut::<MyMode>().painting = true; }),
);
```

Sections decide where it lands, and the choice is about space:

| `ToolSection` | Where |
|---|---|
| `Transform` | with select / move / rotate / scale |
| `Terrain` | the context-sensitive terrain group |
| `Custom(key)` | a new group on the horizontal strip |
| `Shelf(group)` | the vertical two-column shelf on the viewport's left edge |

The horizontal strip runs out of room after a few buttons and wraps into a second row, pushing Play and the view controls down with it. So the strip is for the few buttons that *choose a mode*, and the shelf is for what a mode reveals — its brushes, its select modes, its ops — because the shelf grows downwards into empty space.

Shelf groups render **alphabetically by group string**, globally across all crates. A toolset whose groups must hold a fixed order encodes it in the id, the way terrain does: `terrain.a-sculpt` → `terrain.b-paint` → `terrain.c-foliage-brush`.

`on_activate` runs as a deferred editor command, so it gets `&mut World` safely from a click handler that is `&World`-only.

This is the *contract-crate* way to add a toolbar button. The [ember toolbar functions](panels.md#viewport-toolbar-and-strips) are the other way, and they differ: `ToolEntry` gives you a standard icon button with the editor's own predicates and arrangement; `register_viewport_tool_group` gives you a blank slate to build any widget into. Use `ToolEntry` unless you need a control the button shape cannot express.

## Console output

The Console panel reads a process-global buffer, so a plugin logs into it with a plain function call — no resource, no system param:

```rust
use renzora::core::console_log::{console_error, console_info, console_success, console_warn};

console_info("my-plugin", "scanned 412 meshes");
console_warn("my-plugin", format!("{n} meshes have no UVs"));
```

The first argument is a category, used for the Console's filter chips — use one stable string per plugin. The `clog_info!` / `clog_success!` / `clog_warn!` / `clog_error!` macros take format arguments directly.

Bevy's own `info!` / `warn!` / `error!` also reach the console through the tracing layer, so an ordinary Bevy log is not lost — but it lands with the module path as its category rather than a name you chose.

### Content problems

The Problems panel is fed by the `ContentProblems` resource, keyed by asset path. A validator plugin — an importer that checks what it imported, a lint that runs over the scene — publishes into it:

```rust
use renzora::content_problems::{ContentProblem, ContentProblems, ProblemSeverity};

fn report(mut problems: ResMut<ContentProblems>) {
    problems.set("models/tree.glb", vec![ContentProblem { /* … */ }]);
}
```

`set(path, problems)` replaces everything recorded for that path, `clear_path(path)` removes it, and `error_count()` / `warning_count()` drive the panel's badge. Replacing rather than appending is deliberate: a re-validation should not stack duplicates of the problems it just re-found.

## Localization

Every user-visible string should go through the contract crate's translation table:

```rust
use renzora::lang::{t, t_args, t_or};

t("menu.file")                                   // -> the active language's text
t_or("my_plugin.title", "My Plugin")             // -> fallback if the key is missing
t_args("my_plugin.count", &[("n", "12")])        // -> interpolated
```

A plugin ships its own keys by registering a pack at build time:

```rust
renzora::lang::register_pack_str(include_str!("../locales/en.toml"))?;
```

**A missing translation is not an error** — `t()` returns the key itself. That is why a plugin that accidentally links its own copy of `renzora` renders raw keys (`menu.file`, `common.settings`) everywhere with nothing logged: it got a private, empty translation table. If you ever see that, the shared image is the thing to check. See [Localization](localization.md).

## Project and lifecycle

| | |
|---|---|
| `CurrentProject` | the open project — paths, settings, name |
| `SplashState` | the boot state machine; `OnEnter(SplashState::Editor)` is where a plugin spawns scene content |
| `SaveSceneRequested` | trigger it to ask the editor to save; how the autosave plugin works |
| `GameEvent` | the broadcast event bus shared with scripts — `app.add_observer(\|t: On<GameEvent>\| …)` |

**Spawn on `OnEnter(SplashState::Editor)`, not `Startup`.** The project-load teardown despawns everything carrying a `Name` and it runs *after* `Startup`, so a named entity spawned in `Startup` appears and then vanishes.

## GPU pass attribution

If your plugin adds its own render pass, tell the GPU Pass Breakdown what drives it, so a user profiling a slow frame sees *your entities* rather than an unexplained pass name:

```rust
app.register_gpu_pass_source::<MyEffect>("my_effect_", "effect");
```

Every pass whose name starts with `my_effect_` is then attributed to the live entities carrying `MyEffect`, labelled with that noun. See [Profiling](../editor-dev/profiling.md).

## Post-process effects

`add_post_process::<T>()` does the whole job for an effect component — registers the type for reflection, adds the render plugin, and registers the inspector entry generated by `#[post_process]`:

```rust
app.add_post_process::<MyEffect>();
```

That is the native-plugin route. A post-process effect that must run in a **shipped game** wants a [C-ABI plugin](standalone-plugins.md) instead — a native plugin cannot load into a lean export — and that is what almost every effect in `plugins/` is. `plugins/grayscale` is the smallest of them, a 52-line `#![no_std]` template. See [Post-Processing Effects](post-processing.md).

## What a plugin still cannot do

- **Push onto the undo stack.** Undo lives in `renzora_undo`, which is not one of the three crates a plugin can link. A plugin can mutate the world freely; the user cannot undo it. This is the largest gap in the plugin API today.
- **Unregister anything.** Registries are append-only for the life of the `App`, because Bevy cannot withdraw a system and unmapping a plugin's image would dangle every function pointer it left behind.
- **Reach another `renzora_*` crate.** Only `bevy`, `renzora` and `renzora_ember` are linkable. If you need a type from elsewhere, that type belongs in the contract crate — which is the rule the engine's own crates follow, and the reason this list is as short as it is.

## See also

- [Editor Panels from a Plugin](panels.md) — the UI half
- [Native Plugins](native-plugins.md) — how a plugin is built and loaded
- [Custom Inspector Fields](../editor-dev/inspector-fields.md) — the `Inspectable` derive
- [Making Edits Undoable](../editor-dev/undo.md) — for in-workspace crates, which can
- [Plugin API Status](plugin-api-status.md) — what is stable and what is not

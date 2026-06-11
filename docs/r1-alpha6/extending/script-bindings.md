# Script API Bindings

Add your own Lua and Rhai functions — and per-entity context — to the scripting runtime from any crate, using the `ScriptExtension` trait.

## How extensions fit in

The scripting core (`renzora_scripting`) is language-agnostic: a `ScriptEngine` resource holds a list of `ScriptBackend`s, and each script dispatches to a backend by file extension (`.lua` → Lua, `.rhai` → Rhai). The base API — roughly 70 Lua / 45 Rhai functions plus all the context globals — is registered by the scripting crate itself.

Anything *beyond* that base set is contributed by domain crates through one trait: **`renzora_scripting::extension::ScriptExtension`**. The engine's own `renzora_physics`, `renzora_navmesh`, and `renzora_animation` crates use it to add their helpers; your gameplay crate uses the exact same path.

> This page is about **adding** functions. For the functions that already exist, see the [Lua](/docs/r1-alpha5/scripting/lua) and [Rhai](/docs/r1-alpha5/scripting/rhai) references and the [API catalog](/docs/r1-alpha5/api/scripting).

## The `ScriptExtension` trait

This is the real signature, from `crates/renzora_scripting/src/extension.rs`:

```rust
pub trait ScriptExtension: Send + Sync + 'static {
    /// Human-readable name, used only for logging.
    fn name(&self) -> &str;

    /// Fill `ExtensionData` for one entity before its scripts run.
    /// Called per-entity each frame with read-only world access.
    fn populate_context(&self, world: &World, entity: Entity, data: &mut ExtensionData);

    /// Register custom Lua functions. Runs once per Lua VM creation.
    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, _lua: &mlua::Lua) {}

    /// Push per-frame Lua globals from the data `populate_context` produced.
    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn setup_lua_context(&self, _lua: &mlua::Lua, _data: &ExtensionData) {}

    /// Register custom Rhai functions. Runs once per Rhai engine creation.
    #[cfg(feature = "rhai")]
    fn register_rhai_functions(&self, _engine: &mut rhai::Engine) {}

    /// Push per-frame Rhai scope vars from the data `populate_context` produced.
    #[cfg(feature = "rhai")]
    fn setup_rhai_scope(&self, _scope: &mut rhai::Scope, _data: &ExtensionData) {}
}
```

Only `name` and `populate_context` are required; the four backend methods have default no-op bodies, so you implement just the ones you need.

> ⚠️ Earlier docs showed a three-method trait with `populate_context`, `register_rhai_functions`, and `register_lua_functions` only. The real trait also has `name`, `setup_lua_context`, and `setup_rhai_scope`, and the four language methods are **feature-gated** — `register_lua_functions`/`setup_lua_context` behind `#[cfg(all(feature = "lua", not(target_arch = "wasm32")))]`, and the Rhai pair behind `#[cfg(feature = "rhai")]`. Lua is native-only; Rhai runs everywhere including WASM.

### Two phases: register once, set up each frame

The split matters for performance:

| Phase | Method | When it runs |
|---|---|---|
| **Register** | `register_lua_functions` | Once, when a Lua VM is built (one VM is cached per `(entity, script_path)`). |
| **Register** | `register_rhai_functions` | Once, lazily on first execution — the Rhai engine is shared across scripts. |
| **Per-frame** | `populate_context` | Every frame, per scripted entity, with `&World`. Fills `ExtensionData`. |
| **Per-frame** | `setup_lua_context` / `setup_rhai_scope` | Every frame, before each hook, reading that `ExtensionData` back into the script's globals/scope. |

Register functions in the register phase; never re-register every frame. Push *values* in the per-frame phase.

## `ExtensionData` — typed per-entity data

`ExtensionData` is a type-keyed bag (keyed by `TypeId`, **not** by string). You `insert` a typed value in `populate_context` and `get` it back in `setup_lua_context` / `setup_rhai_scope`:

```rust
impl ExtensionData {
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T);
    pub fn get<T: 'static>(&self) -> Option<&T>;
}
```

> ⚠️ There is **no** `data.set("name", value)` string-keyed API — that was invented by an old draft. Insert a concrete type and read it back by type.

## Registering an extension

There is **no `app.register_script_extension(...)` method**. You insert your extension into the `ScriptExtensions` resource from your plugin's `build`, creating the resource if it doesn't exist yet:

```rust
use bevy::prelude::*;
use renzora_scripting::extension::ScriptExtensions;

impl Plugin for MyGameplayPlugin {
    fn build(&self, app: &mut App) {
        // ... your systems, observers, resources ...

        let mut extensions = app
            .world_mut()
            .get_resource_or_insert_with(ScriptExtensions::default);
        extensions.register(MyScriptExtension);
    }
}
```

This is exactly how `renzora_physics`, `renzora_navmesh`, and `renzora_animation` register theirs. `ScriptExtensions::register` takes `impl ScriptExtension` by value.

### Cargo setup

Your crate references `mlua::Lua` / `rhai::Engine` in the trait impl, so it needs the matching features and deps. Mirror the engine crates: declare your own `lua`/`rhai` features that forward to `renzora_scripting`, and pull the backends in as optional deps (Lua native-only):

```toml
[features]
default = ["lua", "rhai"]
lua = ["renzora_scripting/lua", "dep:mlua"]
rhai = ["renzora_scripting/rhai", "dep:rhai"]

[dependencies]
renzora = { path = "../renzora", default-features = false }
renzora_scripting = { path = "../renzora_scripting", default-features = false }
rhai = { version = "1.21", features = ["sync"], optional = true }

# Lua types referenced in the trait impl — native targets only.
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
mlua = { version = "0.10", features = ["lua54"], optional = true }
```

## Adding functions: the `action` pattern

A script function shouldn't mutate the world directly — scripts run in a backend thread/VM, not a Bevy system. Instead, the engine's extensions push a **command** that a normal Bevy observer drains and applies. The recommended verb is `ScriptCommand::Action`, which fires the same `ScriptAction` event the bare `action(name, args)` function uses, so you can react to it with `app.add_observer(...)`.

`push_command` is the public entry point; `ScriptActionValue` carries the args:

```rust
// renzora_scripting::backends::push_command
pub fn push_command(cmd: ScriptCommand);

// renzora::ScriptActionValue
pub enum ScriptActionValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
}
```

Here is the navmesh extension verbatim — a complete, real example of registering Lua functions that emit actions:

```rust
use renzora_scripting::extension::{ExtensionData, ScriptExtension};

pub struct NavScriptExtension;

impl ScriptExtension for NavScriptExtension {
    fn name(&self) -> &str { "navigation" }

    fn populate_context(&self, _world: &World, _entity: Entity, _data: &mut ExtensionData) {
        // Per-entity reads go through `get("NavReadState.*")` (see below).
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        use renzora::ScriptActionValue;
        use renzora_scripting::backends::push_command;
        use renzora_scripting::ScriptCommand;
        use std::collections::HashMap;

        let globals = lua.globals();

        fn push_nav_action(name: &'static str, args: HashMap<String, ScriptActionValue>) {
            push_command(ScriptCommand::Action {
                name: name.into(),
                target_entity: None, // None = the running script's entity
                args,
            });
        }

        let _ = globals.set(
            "nav_set_destination",
            lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
                let mut args = HashMap::new();
                args.insert("target".into(), ScriptActionValue::Vec3([x, y, z]));
                push_nav_action("nav_set_destination", args);
                Ok(())
            }).unwrap(),
        );

        let _ = globals.set(
            "nav_stop",
            lua.create_function(|_, ()| {
                push_nav_action("nav_clear_destination", HashMap::new());
                Ok(())
            }).unwrap(),
        );
    }
}
```

The matching observer (registered in the same plugin's `build`) does the real work:

```rust
app.add_observer(handle_nav_script_actions); // reacts to ScriptAction { name, .. }
```

`renzora_physics` follows the same shape for `move_controller`, `apply_force`, `apply_impulse`, and `set_linear_velocity` (all routed through `kinematic_slide` / `apply_force` / `set_velocity` actions); `renzora_animation` does it for `set_anim_param`, `set_anim_bool`, and `set_anim_trigger`.

## Reading subsystem state

Mutations go out as actions; reads come back through reflection. The engine auto-mirrors per-entity state into reflectable components (`PhysicsReadState`, `NavReadState`, `AnimatorReadState`), which scripts query with the base `get(...)` function — no extra binding needed:

```lua
function on_update()
    if get("PhysicsReadState.grounded") == true then
        apply_impulse(0, 8, 0)
    end
    if get("NavReadState.has_path") then
        -- ...
    end
end
```

If an extension function needs to *return* a computed value, call the reflection dispatcher directly. This is how animation's `get_animation_length` works:

```rust
let result = renzora_scripting::get_handler::call_get(
    None,                       // None = the self entity
    "AnimatorReadState",        // component
    &format!("clip_lengths.{}", name),
);
let seconds = match result {
    Some(renzora::PropertyValue::Float(f)) => f,
    _ => 0.0,
};
```

## Per-entity globals via `populate_context`

When you want a script to *read* a custom value as a plain global (not via `get`), use the two-phase data path. Fill `ExtensionData` per entity in `populate_context`, then push it into the VM in `setup_lua_context`:

```rust
use bevy::prelude::*;
use renzora_scripting::extension::{ExtensionData, ScriptExtension};

#[derive(Component)]
struct Inventory { items: Vec<String>, max_slots: usize }

// Plain typed payload stored in ExtensionData (keyed by its TypeId).
struct InvData { size: i64, full: bool }

pub struct InventoryScriptExtension;

impl ScriptExtension for InventoryScriptExtension {
    fn name(&self) -> &str { "inventory" }

    fn populate_context(&self, world: &World, entity: Entity, data: &mut ExtensionData) {
        if let Some(inv) = world.get::<Inventory>(entity) {
            data.insert(InvData {
                size: inv.items.len() as i64,
                full: inv.items.len() >= inv.max_slots,
            });
        }
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn setup_lua_context(&self, lua: &mlua::Lua, data: &ExtensionData) {
        if let Some(inv) = data.get::<InvData>() {
            let g = lua.globals();
            let _ = g.set("inventory_size", inv.size);
            let _ = g.set("inventory_full", inv.full);
        }
    }

    #[cfg(feature = "rhai")]
    fn setup_rhai_scope(&self, scope: &mut rhai::Scope, data: &ExtensionData) {
        if let Some(inv) = data.get::<InvData>() {
            scope.push("inventory_size", inv.size);
            scope.push("inventory_full", inv.full);
        }
    }
}
```

Scripts then read `inventory_size` / `inventory_full` like any other context global.

## Registration order and shadowing

Extension functions are registered **after** the base API (`register_lua_functions` runs after the scripting crate's `register_api`). Because a Lua global set later overwrites the earlier one, **an extension can shadow a base function** by registering the same name.

The engine relies on this on purpose: the base `apply_force`/`apply_impulse`/`set_velocity` and `set_anim_param`/`set_anim_bool` exist in the core API, and the physics/animation extensions **re-register them** so the calls route through their `ScriptAction` observers instead of the core stubs. At runtime only the extension version runs. If you register a name that collides with a base function, yours wins — so namespace your verbs (e.g. `inventory_add`, not `add`) unless shadowing is what you want.

## Lua vs Rhai when extending

The base API is already lopsided — Rhai is a deliberate subset of Lua (see [Rhai limitations](/docs/r1-alpha5/scripting/rhai#what-rhai-cant-do)). Extensions inherit the same asymmetry, and it is opt-in per method:

- Implement `register_lua_functions` → the function exists in `.lua` scripts (native only).
- Implement `register_rhai_functions` → the function exists in `.rhai` scripts (all platforms, incl. WASM).

The built-in `renzora_physics` / `renzora_navmesh` / `renzora_animation` extensions implement **only the Lua methods**, which is why their helpers (`move_controller`, `nav_set_destination`, `set_anim_trigger`, …) are absent from Rhai. If you want your binding available in WASM exports, you must implement the Rhai pair too:

```rust
#[cfg(feature = "rhai")]
fn register_rhai_functions(&self, engine: &mut rhai::Engine) {
    engine.register_fn("inventory_count", |item: String| -> i64 {
        // Rhai engine is shared; this registers once globally.
        0
    });
}
```

## Type mapping

Argument and return types map across the boundary as follows:

| Rust | Lua (mlua 0.10) | Rhai (1.21) |
|---|---|---|
| `f32` / `f64` | `number` | `FLOAT` (f64) |
| `i64` / `i32` | `integer` | `INT` (i64) |
| `bool` | `boolean` | `bool` |
| `String` / `&str` | `string` | `String` / `ImmutableString` |
| `()` | `nil` | `()` (unit) |
| `Vec<T>` | `table` (sequence) | `Array` |
| `HashMap` / object | `table` | `Map` (object map `#{}`) |

> Lua functions are built with `lua.create_function(|_, args| { ... Ok(ret) })`; Rhai functions with `engine.register_fn("name", |args| ret)`. Keep argument types primitive — the `ScriptActionValue` enum your actions carry is `Float`/`Int`/`Bool`/`String`/`Vec3` only.

## Testing an extension

There is no `ScriptTestHarness` type. Test an extension the way the engine does — build a minimal Bevy `App`, add `ScriptingPlugin` and your plugin, attach a script, step the app, and assert on the world your observer mutated:

```rust
#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use renzora_scripting::ScriptingPlugin;

    #[test]
    fn inventory_binding_runs() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ScriptingPlugin)
            .add_plugins(MyGameplayPlugin); // registers InventoryScriptExtension

        // Spawn a Named entity (auto-gets a ScriptComponent), point it at a
        // .lua/.rhai file that calls your function, then step:
        app.update();

        // assert on the component your ScriptAction observer changed
    }
}
```

## Related

- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — how scripts attach, the lifecycle hooks, the `action()` bus
- [Lua](/docs/r1-alpha5/scripting/lua) / [Rhai](/docs/r1-alpha5/scripting/rhai) — the base function surface you're extending
- [Visual Blueprints](/docs/r1-alpha5/scripting/blueprints) — the separate node-graph system, interpreted at runtime
- [Engine Architecture](/docs/r1-alpha5/setup/architecture) — plugin scopes and the cdylib/ABI model your extension crate lives in

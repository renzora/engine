# Script API Bindings

Add your own script functions from any crate by **declaring** them. You write no
interpreter code, and every scripting language the engine can load picks your
functions up for free.

## How extensions fit in

The scripting core (`renzora_scripting`) is language-agnostic and statically
linked: a `ScriptEngine` resource holds a list of `ScriptBackend`s and dispatches
each script to one by file extension. The interpreters themselves are plugins —
`plugins/lua` supplies Lua — so "which language can I script in" is answered by
which plugin is present, not by how the engine was compiled. See
[Script Backends](/docs/r1-alpha8/extending/script-backends) if you want to add a
language.

The base API — the ~70 functions plus the context globals — comes from the
language plugin. Anything *beyond* it is contributed by domain crates through one
trait: **`renzora_scripting::extension::ScriptExtension`**. The engine's own
`renzora_physics`, `renzora_navmesh`, `renzora_animation`, `renzora_ragdoll` and
`renzora_lang` use it; your gameplay crate uses the same path.

> This page is about **adding** functions. For the functions that already exist,
> see the [Lua reference](/docs/r1-alpha8/scripting/lua) and the
> [API catalog](/docs/r1-alpha8/api/scripting).

## Declare, don't write

```rust
pub trait ScriptExtension: Send + Sync + 'static {
    /// For logs.
    fn name(&self) -> &str;

    /// The functions this crate contributes.
    fn bindings(&self) -> Vec<Binding>;
}
```

That is the whole trait. A `Binding` says what a function is called, what it
does, and what arguments it takes — not how to build one, because the language
plugin knows that and your crate should not have to.

### Why it works this way

Every binding in the engine turned out to be the same four lines: read the
arguments, pack them into a `ScriptCommand::Action`, push it. Writing that by
hand meant `renzora_physics` compiled a Lua interpreter in order to say
"`apply_force` takes three floats", and a second language would have meant a
second copy of every binding.

Declaring it instead means your crate links no interpreter, and a Wren plugin
would expose `apply_force` without `renzora_physics` knowing Wren exists.

## Writing one

```rust
use renzora_scripting::extension::{Bind, Binding, ParamKind, ScriptExtension};

pub struct CombatScriptExtension;

impl ScriptExtension for CombatScriptExtension {
    fn name(&self) -> &str {
        "combat"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            // deal_damage(amount) -> fires ScriptAction "deal_damage"
            Bind::action("deal_damage", "deal_damage")
                .arg("amount", ParamKind::Float)
                .doc("Damage the script's own entity.")
                .build(),

            // knockback(x, y, z) -> three separate float args named x, y, z
            Bind::action("knockback", "knockback")
                .xyz()
                .doc("Push the entity along a world-space vector.")
                .build(),

            // aim_at(x, y, z) -> ONE Vec3 arg named "target"
            Bind::action("aim_at", "aim_at")
                .vec3("target")
                .doc("Turn the entity to face a world position.")
                .build(),

            // get_stamina() -> reads a reflected field and returns it
            Bind::read("get_stamina", "CombatReadState", "stamina")
                .doc("Current stamina, 0 if the entity has no combat state.")
                .build(),
        ]
    }
}
```

Register it from your plugin's `build()`:

```rust
use bevy::prelude::*;
use renzora_scripting::extension::ScriptExtensions;

impl Plugin for MyGameplayPlugin {
    fn build(&self, app: &mut App) {
        let mut extensions = app
            .world_mut()
            .get_resource_or_insert_with(ScriptExtensions::default);
        extensions.register(CombatScriptExtension);
    }
}
```

There is no `app.register_script_extension(...)`; this is the path the engine's
own crates use.

### Cargo setup

Nothing special. Your crate needs `renzora_scripting` for the trait and nothing
else — no interpreter dependency, no feature forwarding, no target gating:

```toml
[dependencies]
renzora_scripting = { path = "../renzora_scripting", default-features = false }
```

## The three kinds

| Constructor | What the script gets | What happens |
|---|---|---|
| `Bind::action(name, action)` | a function returning nothing | fires a `ScriptAction` your crate observes |
| `Bind::read(name, component, field)` | a function returning a value | reads a reflected field through the `get` path |
| `Bind::translate(name)` | a function returning a string | looks the argument up in the localization table |

Three, because that is what the engine's five extensions needed. A fourth kind is
a change here *and* in every language plugin, so it should earn its place.

## Parameters

| `ParamKind` | Script arguments consumed | Argument produced |
|---|---|---|
| `Float`, `Int`, `Bool`, `Str` | 1 | one value of that type |
| `Vec3` | **3** | one `Vec3` |

`Vec3` consuming three arguments is not a convenience — both shapes exist in the
engine. `apply_force(x, y, z)` sends three separate float args because its
handler reads `x`/`y`/`z`; `nav_set_destination(x, y, z)` sends one `Vec3` named
`target` because its handler reads `target`. Match whatever your `ScriptAction`
observer expects.

`.xyz()` is shorthand for three `Float` params named `x`, `y`, `z`.

## Placeholders in a read path

`Bind::read` accepts `{0}`, `{1}` … in the component or field path, substituted
with the call's arguments:

```rust
Bind::read("get_animation_length", "AnimatorReadState", "clip_lengths.{0}")
    .arg("name", ParamKind::Str)
    .build()
```

`get_animation_length("run")` then reads `AnimatorReadState.clip_lengths.run`.

A placeholder with no matching argument is left as written, so the path visibly
fails to resolve rather than silently reading a different field.

## Applying an action

`Bind::action` only *emits* the action. Your crate handles it with an observer:

```rust
app.add_observer(|trigger: On<renzora::ScriptAction>, mut q: Query<&mut Health>| {
    let action = trigger.event();
    if action.name != "deal_damage" {
        return;
    }
    let Some(renzora::ScriptActionValue::Float(amount)) = action.args.get("amount") else {
        return;
    };
    if let Ok(mut health) = q.get_mut(action.entity) {
        health.current -= amount;
    }
});
```

Scripts never mutate the world directly — they run inside a plugin, not a Bevy
system — so every mutation goes through this queue-then-apply path.

## Name collisions

Two extensions declaring the same function name is refused, not shadowed: the
first registration wins and the second is logged and dropped. Shadowing would
make which crate won depend on registration order, so the same project could
behave differently on two machines.

## What was removed

Earlier versions of this trait also had `populate_context`, `setup_lua_context`
and a type-erased `ExtensionData` bag carried per-entity through the script
context. Every implementation of both methods was an empty stub and nothing ever
read the bag, so the mechanism was being allocated and threaded through the
per-entity loop to hold nothing. If you need per-entity data in a script, mirror
it onto a reflected `*ReadState` component and read it with `Bind::read` or
`get("MyReadState.field")` — which is what the engine's own crates already did.

The Rhai methods are gone too; the Rhai backend was removed before this change.

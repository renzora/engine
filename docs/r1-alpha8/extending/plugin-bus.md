# Talking to Other Plugins (bus)

Every native plugin runs in the same `World`, but each one is compiled on its own. A struct defined in one plugin is a type the others have never seen, so they cannot query it, import it or send it as a Bevy `Message`.

`renzora::bus` is how they talk anyway. A plugin **emits events** and **provides services** under a string name, and the data travels as a `BusValue`, a type from the contract that every plugin already has. Neither side needs the other installed to compile, load or run.

```rust
use renzora::bus::prelude::*;
```

## Events

An event is fire and forget: one plugin says something happened, and any number of others react.

```rust
// combat plugin
world.emit(
    "combat.damaged",
    BusValue::map().with("entity", target).with("amount", 25.0),
);

// hud plugin: never depends on the combat plugin
app.on_event("combat.damaged", |world, ev| {
    let entity: Entity = ev.field("entity")?;
    let amount: f32 = ev.field("amount")?;
    // flash the health bar ...
    Ok(())
});
```

Nobody listening is not an error. The emitter cannot know which plugins are installed and does not need to.

From an ordinary system, which has no `&mut World`, emit through `Commands`:

```rust
fn hit(mut commands: Commands) {
    commands.emit("combat.damaged", BusValue::map().with("amount", 10.0));
}
```

## Services

A service is a request with an answer. Exactly one plugin provides each name.

```rust
// inventory plugin
app.provide(
    ServiceDecl::new("inventory.add_item")
        .describe("Adds items to the player's inventory")
        .arg("id", BusKind::String)
        .optional_arg("count", BusKind::Int)
        .returns(BusKind::Bool),
    |world, args| {
        let id: String = args.field("id")?;
        let count = args.opt_field::<i64>("count")?.unwrap_or(1);
        // ...
        Ok(true.into())
    },
);

// any other plugin
match world.call("inventory.add_item", BusValue::map().with("id", "sword")) {
    Ok(added) => { /* ... */ }
    Err(BusError::NotProvided(_)) => { /* the inventory plugin is not installed */ }
    Err(err) => warn!("{err}"),
}
```

From a system, `commands.call_then(name, args, |world, answer| ...)` makes the call when commands are applied and hands you the answer with `&mut World`.

To find out whether an optional companion plugin is there before offering a feature, ask `world.resource::<Bus>().has_service("inventory.add_item")`.

A second plugin providing a name that is already taken is refused with a warning, and the first one keeps answering. Which plugin builds first is load order, and a service that changed hands with load order would be a bug nobody could reproduce.

## Payloads

`BusValue` is a small JSON-like tree with the engine's own types as leaves:

| Variant | Rust |
|---|---|
| `Null`, `Bool`, `Int`, `Float`, `String` | `()`, `bool`, `i32`/`i64`/`u32`, `f32`/`f64`, `&str`/`String` |
| `Entity`, `Vec2`, `Vec3`, `Vec4` | the Bevy types (a colour travels as linear RGBA in a `Vec4`) |
| `List`, `Map` | `Vec<T>`, `BTreeMap<String, BusValue>` |

Everything above converts with `.into()`. Build a map with `BusValue::map().with(key, value)`, and read one with:

- `field::<T>(key)`: the value, or a `BadArgs` error naming the field. Written for `?` in a handler, so a malformed payload becomes an answer instead of a panic.
- `opt_field::<T>(key)`: `None` when the key is missing or `Null`.
- `to::<T>()`: for a payload that is a bare value, not a map.
- `get`, `as_float`, `as_entity`, ... for looking without converting.

An `Int` is accepted wherever a `Float` is expected, so a caller who writes `25` instead of `25.0` is not refused.

An `Entity` means the same thing to every plugin, because there is only one `World`.

## Declarations

Declaring what you emit and provide is optional, but worth doing:

```rust
app.declare_event(
    EventDecl::new("combat.damaged")
        .describe("Something took damage")
        .field("entity", BusKind::Entity)
        .field("amount", BusKind::Float)
        .optional_field("source", BusKind::Entity),
);
```

- **A service's declared arguments are checked before its handler runs.** A missing or mistyped argument comes back to the caller as `BadArgs` with the field named, rather than failing somewhere inside code the caller did not write.
- **A declared event's payload is checked when it is emitted.** A mismatch is logged once and the event is still delivered, so one plugin's mistake does not turn into missing behaviour in another.
- **Keys nobody declared are allowed.** A provider can add a field without breaking callers that already send it.
- **The bus lists them.** `Bus::events()` and `Bus::services()` return every declaration sorted by name, with its description and fields, for a panel or a plugin's own documentation.

## Naming and versioning

There is one namespace for every installed plugin, so **name things `<plugin>.<thing>`**: `combat.damaged`, `inventory.add_item`. The prefix is what keeps two plugins' `damaged` apart.

To change a payload incompatibly, give it a new name (`inventory.add_item@2`) and keep answering the old one for a while. Adding an optional field needs no new name.

## How delivery works

- **Delivery is immediate.** `world.emit` runs every listener before it returns, and `world.call` returns the handler's answer. `Commands::emit` and `call_then` do the same at the point the system's commands are applied.
- **Listeners and handlers get `&mut World`**, so they can query, spawn, emit and call like any exclusive system.
- **Nesting is bounded.** A listener may emit and a service may call another, up to 32 deep (`MAX_DEPTH`). Two plugins that answer each other's event in a loop stop there with a warning, instead of overflowing the stack and taking the editor down with no message.
- **A listener that returns an error** is logged once per event and error; the other listeners still run.
- `world.subscribe(event, listener)` is the `World` form of `on_event`, and returns a `ListenerId` for `world.unsubscribe(id)`.
- `app.on_any_event` / `world.subscribe_all` hear every event, with its name. They are for bridges and tools (the scripting bridge is one); a gameplay plugin that reacts to everything is coupled to everything. They run before the named listeners.

## From scripts

Scripts are on the bus too, through the event functions they already have. There is one event space for plugins and scripts together.

**A plugin's event reaches scripts.** Anything emitted on the bus arrives in every script's `on_event(name, args)` (Lua) or as `ScriptHook::Event` (Rust) on the next frame:

```lua
function on_event(name, args)
    if name == "combat.damaged" then
        print("hit for " .. args.amount)
    end
end
```

**A script's event reaches plugins.** `emit(name, args)` from a script is emitted on the bus under the same name, so a plugin's `on_event` hears it. The arguments arrive as a map.

```lua
emit("quest.started", { id = "dragon" })
```

**A script calls a service with `action`.** Name the service as the action, and name the event you want the answer delivered as in `callback`:

```lua
action("inventory.add_item", { id = "sword", count = 1, callback = "on_added" })

function on_event(name, args)
    if name == "on_added" then
        if args.ok then print("added") else print(args.error) end
    end
end
```

The answer event carries the service's answer (a map's keys become the event's arguments; any other value arrives as `value`), plus `ok`, and `error` when the call failed. A script cannot wait for an answer while its hook is running, which is why it comes back as an event. `callback` is taken off the arguments before the service sees them. Without a `callback`, a failed call is reported in the Console.

Script arguments are flat, so a few values change type on the way to a script:

| On the bus | In a script |
|---|---|
| `Entity` | its id, an integer (what every script function takes) |
| `Vec2` | a `Vec3` with `z = 0` |
| `Vec4`, `List`, nested `Map` | a JSON string |
| `Null` | left out |

In the other direction, a script's integer entity id is accepted wherever a service or event declares `BusKind::Entity`, and `field::<Entity>` reads it.

Events go to scripts only while scripts are running (Play, or a previewed script), so a plugin that emits every frame in the editor does not leave a backlog to arrive all at once when Play starts.

A Rust script holds `&mut World`, so it can also use the bus directly, and get a service's answer on the spot:

```rust
use renzora::bus::prelude::*;

let added = ctx.world().call("inventory.add_item", BusValue::map().with("id", "sword"));
```

## This is not Bevy's `Event` or `Message`

Bevy's events and messages are keyed by a Rust type, which is exactly what two separately built plugins cannot agree on. Inside one plugin, keep using Bevy's: they are typed and checked at compile time. Use the bus at the edge, for what another plugin needs to hear or ask.

The cost of crossing that edge is that names and fields are checked at run time, not by the compiler. Declarations are what make those failures say what went wrong.

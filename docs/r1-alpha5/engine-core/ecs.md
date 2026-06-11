# ECS & Bevy

Renzora is built on Bevy 0.18's Entity Component System, where game state is plain data and logic is just functions over that data.

## Why ECS?

Bevy is a data-driven engine. Instead of deep inheritance hierarchies, Renzora composes behaviour from four primitives:

- **Entities** — unique `Entity` IDs. Just a handle; they hold no data themselves.
- **Components** — data structs attached to entities. They define what an entity *is*.
- **Systems** — functions that query entities by their components. They define what *happens*.
- **Resources** — global singletons not attached to any entity.

This layout keeps related data contiguous in memory (cache-friendly), makes composition trivial, and lets Bevy run non-conflicting systems in parallel automatically.

> Everything on this page is stock Bevy 0.18 — the same API used inside the engine crates and inside dynamic plugins. The Renzora-specific layering is covered in the last section.

## Entities

Create entities with `Commands`, spawning a tuple of components as a bundle:

```rust
use bevy::prelude::*;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Health { current: f32, max: f32 }

fn spawn_player(mut commands: Commands) {
    let entity = commands.spawn((
        Transform::default(),
        Player,
        Health { current: 100.0, max: 100.0 },
    )).id();
}
```

Despawn with `.despawn()`. In Bevy 0.18 a despawn is **recursive by default** — it removes the entity and all of its related children, so there is no separate `despawn_recursive()`:

```rust
fn cleanup(mut commands: Commands, query: Query<Entity, With<Dead>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
```

## Components

Any Rust struct or enum can be a component — just derive `Component`:

```rust
#[derive(Component)]
struct Health { current: f32, max: f32 }

#[derive(Component)]
struct Player; // marker component — zero size, used purely for filtering
```

Insert or remove components at runtime through `Commands`:

```rust
commands.entity(entity).insert(Poisoned { duration: 5.0 });
commands.entity(entity).remove::<Poisoned>();
```

## Systems

Systems are ordinary functions. Bevy injects their parameters by type:

```rust
fn regenerate_health(
    time: Res<Time>,
    mut query: Query<&mut Health, With<Player>>,
) {
    for mut health in &mut query {
        health.current = (health.current + 5.0 * time.delta_secs()).min(health.max);
    }
}
```

Register a system on a schedule from inside a plugin's `build`:

```rust
app.add_systems(Update, regenerate_health);
```

## Queries

A `Query` selects entities by component combination and access mode:

```rust
// Read Transform and Health together
Query<(&Transform, &Health)>

// Mutable Health, only on entities tagged Player
Query<&mut Health, With<Player>>

// Health, but only on entities that do NOT have Player
Query<&Health, Without<Player>>

// Only entities whose Health changed this frame
Query<&Health, Changed<Health>>

// Only entities that gained a Health component since last run
Query<&Health, Added<Health>>
```

Fetch a single entity directly. `get` / `get_mut` return a `Result`, and `single` / `single_mut` return a `Result` for queries you expect to match exactly one entity:

```rust
fn poke(query: Query<&Health>, entity: Entity) {
    if let Ok(health) = query.get(entity) {
        // ...
    }
}
```

## Resources

Resources are global state any system can read or write. Derive `Resource`:

```rust
#[derive(Resource, Default)]
struct GameScore { points: u32 }

// Read-only
fn display_score(score: Res<GameScore>) {
    info!("Score: {}", score.points);
}

// Mutable
fn add_points(mut score: ResMut<GameScore>) {
    score.points += 10;
}
```

Install a resource when building the app:

```rust
app.init_resource::<GameScore>();          // uses Default
app.insert_resource(GameScore { points: 0 }); // explicit value
```

## Messages and events

Bevy 0.18 splits "one-shot communication" into **two distinct mechanisms**. Use the right one for the job.

### Buffered messages

A message is written to a buffer and read by polling systems later in the frame. This is the pattern that older Bevy called "events" — the types were renamed. Derive `Message`, register it with `add_message`, write with `MessageWriter::write`, and drain with `MessageReader::read`:

```rust
#[derive(Message)]
struct EnemyDied { entity: Entity, xp_value: u32 }

// Producer
fn check_death(
    query: Query<(Entity, &Health, &XpValue)>,
    mut deaths: MessageWriter<EnemyDied>,
) {
    for (entity, health, xp) in &query {
        if health.current <= 0.0 {
            deaths.write(EnemyDied { entity, xp_value: xp.0 });
        }
    }
}

// Consumer
fn award_xp(mut deaths: MessageReader<EnemyDied>, mut score: ResMut<GameScore>) {
    for ev in deaths.read() {
        score.points += ev.xp_value;
    }
}
```

```rust
app.add_message::<EnemyDied>();
```

> Heads-up for anyone porting older Bevy code: `EventWriter`/`EventReader`/`send` became `MessageWriter`/`MessageReader`/`write`, and `add_event` became `add_message`. Built-in input streams follow this too — e.g. `MessageReader<KeyboardInput>`.

### Observers

An observer reacts **immediately** when an event is triggered, rather than being polled. Derive `Event`, register a handler with `add_observer`, and read the payload with `trigger.event()`:

```rust
#[derive(Event)]
struct Explosion { radius: f32 }

fn on_explosion(trigger: On<Explosion>, mut commands: Commands) {
    let blast = trigger.event();
    // react right now...
}

// in build():
app.add_observer(on_explosion);
// trigger it from anywhere with a &mut World / Commands:
commands.trigger(Explosion { radius: 5.0 });
```

Observers also fire on **component lifecycle hooks**. `On<Insert, C>`, `On<Replace, C>`, and `On<Remove, C>` run when a component is added/changed/removed; the affected entity is on `trigger.entity`:

```rust
fn on_health_added(trigger: On<Insert, Health>, query: Query<&Health>) {
    let entity = trigger.entity;
    if let Ok(health) = query.get(entity) {
        info!("entity {entity} spawned with {} HP", health.current);
    }
}

// in build():
app.add_observer(on_health_added);
```

## System scheduling

Bevy runs systems in parallel whenever their data access doesn't conflict. Constrain ordering explicitly when you need it:

```rust
app.add_systems(Update, (
    read_input,
    move_player.after(read_input),
    update_camera.after(move_player),
));

// Or force a strict sequence with .chain()
app.add_systems(Update, (read_input, move_player, update_camera).chain());
```

### Schedule labels

| Schedule | When it runs |
|----------|-------------|
| `Startup` | Once, at app launch |
| `First` | Very start of every frame |
| `PreUpdate` | Before `Update`, every frame |
| `Update` | Main game logic, every frame |
| `PostUpdate` | After `Update` (transform propagation, render prep) |
| `Last` | End of every frame |
| `FixedUpdate` | Fixed timestep (default 64 Hz) — use for physics and other rate-sensitive logic |

> `FixedUpdate` can run zero or many times per frame depending on frame time. Read `Res<Time<Fixed>>` (or just `Res<Time>`, which reports fixed time inside that schedule) rather than wall-clock delta there.

## How Renzora builds on Bevy

Renzora is **one binary** (`renzora`) that is always runtime-shaped. The editor is not a compile-time feature — it ships as a removable `renzora_editor` cdylib bundle that sits beside the executable. Delete that one file and the same binary becomes the shipped game. (The full picture lives in the engine-core architecture and plugin pages.)

Almost every engine feature is its own Bevy `Plugin`. `renzora_runtime::add_engine_plugins(app, is_editor)` installs an ordered foundation, then fans out every other runtime-scope plugin from a global registry:

| Plugin | Crate | Role |
|--------|-------|------|
| `RuntimePlugin` | `renzora_engine` | VFS, asset reader, scene I/O, autoload |
| `GlobalsPlugin` | `renzora_globals` | shared global state |
| `InputPlugin` | `renzora_input` | input mapping |
| `ScriptingPlugin` | `renzora_scripting` | Lua + Rhai backends |
| `PhysicsPlugin` | `renzora_physics` | physics integration + script bindings |
| `ViewportStretchPlugin` | `renzora_runtime` | pixel-art scaling (game builds only) |

Everything else — rendering effects, UI, networking, audio, and so on — registers itself through the inventory macro rather than being hand-added:

```rust
use renzora::*; // NOTE: there is no `renzora::prelude` — import from the crate root

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, my_system);
    }
}

// Registers the plugin in the global inventory (Runtime scope by default).
renzora::add!(MyPlugin);
```

Editor-only plugins (panels, inspectors, gizmos) are not added by `add_engine_plugins` at all — they arrive through the editor bundle and run only when the editor session is active. The editor UI is built on `bevy_ui` and `renzora_ember`; there is **no egui** anywhere in the engine.

> Import from the crate root with `use renzora::*;` or pull specific items (`use renzora::Inspectable;`). `renzora::prelude` does not exist. Bevy's own `use bevy::prelude::*;` is the standard way to reach `Component`, `Query`, `Commands`, `Plugin`, and friends.

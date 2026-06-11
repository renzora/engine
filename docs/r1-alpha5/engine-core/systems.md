# Writing Systems

Systems hold all the logic in a Renzora game; they are plain Bevy 0.18 functions that a plugin registers on a schedule.

## Systems live in plugins

Renzora never hand-wires systems in a `main()`. Every feature is a Bevy `Plugin`, the plugin's `build(&mut App)` calls `add_systems`, and the plugin self-registers with `renzora::add!`. At startup the engine walks a global registry and installs each matching plugin, so a feature drops into the build just by existing.

```rust
use bevy::prelude::*; // Component, Query, Commands, Plugin, Res, Time, ...

#[derive(Resource, Default)]
struct Score { points: u32 }

#[derive(Default)]
pub struct ScoreboardPlugin;

impl Plugin for ScoreboardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .add_systems(Startup, setup_scoreboard)
            .add_systems(Update, (award_points, refresh_label).chain());
    }
}

fn setup_scoreboard(/* ... */) { /* spawn UI once */ }
fn award_points(mut score: ResMut<Score>) { score.points += 1; }
fn refresh_label(score: Res<Score>) { info!("score: {}", score.points); }

// Register the plugin in the global inventory. Runtime scope by default.
renzora::add!(ScoreboardPlugin);
```

That single `renzora::add!(ScoreboardPlugin)` line is the whole registration. There is no central list of plugins to edit.

> Import Bevy's ECS types from `use bevy::prelude::*;` and the registration macro from `renzora`. There is **no `renzora::prelude`** — reach engine items with `use renzora::*;` or by path (`use renzora::Inspectable;`). The macro path `renzora::add!` works regardless of imports.

## Runtime vs Editor scope

The second argument to `renzora::add!` decides **which session a plugin's systems run in**. It is the most important scheduling switch Renzora adds on top of Bevy.

```rust
renzora::add!(MyGameplay);                       // Runtime (the default)
renzora::add!(MyGameplay, Runtime);              // Runtime, stated explicitly
renzora::add!(MyEditorTool, Editor);             // Editor only
renzora::add!(MyFoundation, Runtime, priority = -100); // earlier in the fan-out
```

`PluginScope` is exactly `{ Editor, Runtime }`, and matching is **exact equality** — there is no "both" scope.

| Scope | Where its systems run | Use for |
|-------|-----------------------|---------|
| `Runtime` (default) | The editor viewport **and** the exported game | Gameplay, rendering effects, physics, UI, audio — anything the shipped game needs |
| `Editor` | The editor session only (the removable `renzora_editor` bundle) | Panels, inspectors, gizmos, authoring tools — things that must never ship in a game |

Because a `Runtime` plugin's systems run inside the editor viewport too, your gameplay is live while you edit. `Editor` plugins live in the `renzora_editor` cdylib bundle that sits beside the executable; delete that one file and the same binary becomes the shipped game, with every `Editor`-scope system gone.

> A feature that needs editor tooling **on top of** runtime behaviour ships **two** plugins, one of each scope — for example `GameUiPlugin` (Runtime) plus `GameUiEditorPlugin` (Editor). Do not try to give one plugin both scopes; it cannot.

> Put anything the game depends on in `Runtime` scope. An `Editor`-scope system will run while you author but will silently vanish from the exported build.

## When your systems start running

`renzora_runtime::add_engine_plugins(app, is_editor)` builds the runtime side of every session. It installs an **ordered foundation** first, then fans out every other `Runtime`-scope plugin from the global registry:

| Order | Plugin | Crate | Role |
|-------|--------|-------|------|
| 1 | `RuntimePlugin` | `renzora_engine` | VFS, asset reader, scene I/O, autoload |
| 2 | `GlobalsPlugin` | `renzora_globals` | shared global state |
| 3 | `InputPlugin` | `renzora_input` | input mapping |
| 4 | `ScriptingPlugin` | `renzora_scripting` | Lua + Rhai backends |
| 5 | `PhysicsPlugin` | `renzora_physics` | physics integration + script bindings |
| 6 | `ViewportStretchPlugin` | `renzora_runtime` | pixel-art scaling — **game builds only** (`!is_editor`) |
| 7+ | every `Runtime`-scope `add!` plugin | various | fanned out via `for_each_static_plugin(Runtime)` |

`Editor`-scope plugins are **not** installed here. They arrive only through the editor bundle's `plugin_install_scope` entry point, called with `host_scope = Editor` when an editor session is detected. There is no compile-time `editor` feature.

The fan-out (step 7) visits plugins in ascending `priority` (default `0`), but that ordering controls only **when each plugin's `build` runs**, not when its systems execute each frame.

> Do **not** rely on plugin `priority` to order systems across plugins. Reach for a non-zero `priority` only when a plugin's `build` must initialise a resource another plugin reads in its own `build`. To order the systems themselves, use Bevy's `.after()` / `.before()` / `.chain()` and system sets, described below.

> `Startup` systems fire once, before any dynamically hot-loaded plugin joins the session. A plugin dropped into `plugins/` at runtime should do its one-time setup from an `Update` system that self-guards (run once, then early-return), because its `Startup` schedule has already passed.

## System function signatures

A system is any function whose parameters all implement `SystemParam`. Bevy injects each one by type:

```rust
fn my_system(
    time: Res<Time>,                          // read a resource
    mut score: ResMut<Score>,                 // write a resource
    query: Query<(&Transform, &Health)>,      // read entity data
    mut commands: Commands,                    // spawn/despawn/insert/remove
    mut deaths: MessageWriter<EnemyDied>,      // write buffered messages
    incoming: MessageReader<SpawnRequest>,     // read buffered messages
) {
    // ...
}
```

Register it on a schedule from inside a plugin's `build`:

```rust
app.add_systems(Update, my_system);
```

> Bevy 0.18 renamed buffered "events" to **messages**: `EventWriter`/`EventReader`/`send`/`add_event` are now `MessageWriter`/`MessageReader`/`write`/`add_message`. Observer-style `Event` + `On<...>` is a separate mechanism. Both are covered on the ECS page.

## Schedules

Pick the schedule that matches when the logic should run:

| Schedule | When it runs |
|----------|--------------|
| `Startup` | Once, at session launch |
| `First` | Very start of every frame |
| `PreUpdate` | Before `Update`, every frame |
| `Update` | Main game logic, every frame |
| `PostUpdate` | After `Update` (transform propagation, render prep) |
| `Last` | End of every frame |
| `FixedUpdate` | Fixed timestep (default 64 Hz) — physics and other rate-sensitive logic |

Most gameplay lives in `Update`. Anything that must be frame-rate independent belongs in `FixedUpdate`.

## Ordering systems

Within a schedule Bevy runs systems in parallel whenever their data access does not conflict. Constrain the order only where it matters:

```rust
// Explicit relative ordering
app.add_systems(Update, (
    gather_input,
    process_movement.after(gather_input),
    apply_damage.after(process_movement),
));

// Strict sequence
app.add_systems(Update, (
    gather_input,
    process_movement,
    apply_damage,
).chain());

// Run before another system
app.add_systems(Update, cleanup.before(spawn_enemies));
```

## System sets

Group systems under a shared label so you can order and gate many at once:

```rust
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum GameSet {
    Input,
    Logic,
    Render,
}

app.configure_sets(Update, (
    GameSet::Input,
    GameSet::Logic.after(GameSet::Input),
    GameSet::Render.after(GameSet::Logic),
));

app.add_systems(Update, gather_input.in_set(GameSet::Input));
app.add_systems(Update, move_player.in_set(GameSet::Logic));
```

Sets are the right tool for cross-plugin ordering: one plugin can define a public `SystemSet`, and others slot their systems into it without depending on individual function names.

## Run conditions

Attach a condition so a system only runs when it returns `true`:

```rust
#[derive(Resource, PartialEq)]
enum GameState { Playing, Paused }

app.add_systems(Update, pause_menu.run_if(resource_equals(GameState::Paused)));
app.add_systems(Update, game_logic.run_if(not(resource_equals(GameState::Paused))));

// Custom condition — any system returning bool works
fn is_playing(state: Res<GameState>) -> bool {
    *state == GameState::Playing
}
app.add_systems(Update, spawn_waves.run_if(is_playing));
```

Bevy ships common conditions such as `resource_exists::<T>`, `resource_changed::<T>`, `resource_equals`, `any_with_component::<T>`, and `on_timer(..)`, and they combine with `not`, `.and`, and `.or`.

## Fixed timestep

Use `FixedUpdate` for physics and deterministic logic. It runs at a constant rate regardless of frame rate — zero times on a fast frame, several times on a slow one:

```rust
app.add_systems(FixedUpdate, physics_step);
app.insert_resource(Time::<Fixed>::from_hz(60.0)); // 60 ticks per second (default is 64)
```

Inside a `FixedUpdate` system, `Res<Time>` already reports the fixed delta, so `time.delta_secs()` is your constant tick length (`1/60` here). Read `Res<Time<Fixed>>` explicitly if you need the fixed clock from elsewhere.

## One-shot systems

Register a system to run on demand rather than every frame:

```rust
let system_id = app.register_system(|mut commands: Commands| {
    commands.spawn((Transform::default(), Name::new("spawned-on-demand")));
});

// Later, from any system with Commands:
fn trigger(mut commands: Commands) {
    commands.run_system(system_id);
}
```

## Exclusive systems

When you need full mutable `World` access — no other system runs in parallel — take `&mut World`:

```rust
fn reset_all_health(world: &mut World) {
    let mut query = world.query::<&mut Health>();
    for mut health in query.iter_mut(world) {
        health.current = health.max;
    }
}
```

Reach for this rarely; it serialises the whole schedule around your system.

## Worked examples

Health regeneration, clamped to the maximum:

```rust
fn regenerate_health(
    time: Res<Time>,
    mut query: Query<&mut Health, (With<Player>, Without<Dead>)>,
) {
    for mut health in &mut query {
        if health.current < health.max {
            health.current = (health.current + 2.0 * time.delta_secs()).min(health.max);
        }
    }
}
```

Despawn entities after a timer. In Bevy 0.18 `despawn()` is **recursive by default** — it removes the entity and its children, so there is no separate `despawn_recursive()`:

```rust
#[derive(Component)]
struct DespawnTimer(Timer);

fn tick_despawn_timers(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DespawnTimer)>,
) {
    for (entity, mut timer) in &mut query {
        timer.0.tick(time.delta());
        if timer.0.finished() {
            commands.entity(entity).despawn();
        }
    }
}
```

## Performance tips

- **Use `Changed<T>`** to skip entities whose data has not changed since last run.
- **Use `With<T>` / `Without<T>`** for filtering instead of fetching an `Option<&T>` and branching in the loop.
- **Avoid iterating one `Query` inside another** — it is O(n²). Communicate through messages or resources instead.
- **Prefer `FixedUpdate`** for physics and anything that must not depend on frame rate.
- Systems run in parallel automatically when their access does not conflict. Add ordering constraints only where correctness demands them — do not hand-roll threading.

> For entities, components, queries, messages, and observers, see the ECS & Bevy page. For how plugins are loaded, scoped, and hot-reloaded, see the plugin architecture page.

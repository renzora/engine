# Scripting API

The functions a script can call, by area. Engine-wide primitives (moving
entities, playing sounds, reading and writing components) come with the
scripting language; the rest are declared by the engine feature that owns
them, so every language gets them alike (see
[Script bindings](../extending/script-bindings.md)). Each area's page has the
details and examples:

| Area | Page |
|---|---|
| Keyboard, mouse and gamepad | [Input Handling](../scripting/input.md) |
| Forces, velocities, character movement | [Physics](../scripting/physics.md) |
| Climbing, vaulting, wall runs | [Parkour & Traversal](../scripting/parkour.md) |
| Ragdolls, partial ragdolls, get-up blends | [Ragdoll Physics](../scripting/ragdoll.md) |
| IK targets, look-at, foot IK, spring bones | [IK, Constraints & Spring Bones](../scripting/ik-constraints.md) |
| Particle effects | [Particle Effects](../scripting/particles.md) |
| On-screen UI | [Game UI](../scripting/game-ui.md) |
| Rust scripts (`&mut World`) | [Rust Scripts](../scripting/rust-scripts.md) |
| Sounds, music and the mixer | [Audio](../editor/audio.md) |

## Game settings (`set_setting`, `get_setting`)

Every game setting can be read and changed while the game runs, which is what
an in-game options menu needs. A setting is named by its path in
`project.toml`: `window.vsync`, `window.max_fps`, `window.mode`,
`rendering.render_scale`, `rendering.graphics_quality`,
`audio.buses.Master.volume` (a bus by its key). The change is applied by the
running game straight away (the next frame), and is **not** written to
`project.toml`: it lasts until the game quits. To keep a player's choices,
save them yourself and set them again on start.

| Function | Arguments | |
|---|---|---|
| `set_setting` | `path` (string), `value` | Change a setting. `value` is converted to the setting's own type, so `"false"`, `false`, `"60"` and `60` all work. |
| `get_setting` | `path` (string) | The setting as text (`"High"`, `"60"`, `"true"`), or nil if there is none. |
| `get_setting_number` | `path` (string) | A numeric setting as a number. |
| `get_setting_bool` | `path` (string) | A true/false setting as a boolean. |

Two more families of path are not in `project.toml`:

| Path | Value | |
|---|---|---|
| `ui.scale` | number | The player's UI size, multiplied onto the canvas fit: `1.25` is 25% larger. Clamped to 0.25 to 4. See [Game UI](../scripting/game-ui.md#players-ui-size). |
| `input.<action>.<slot>` | binding text | One binding of the action map, such as `input.jump.0` = `"Key:KeyF"`. An empty value clears the slot; one past the last slot adds a binding. See [Input Handling](../scripting/input.md#rebinding-while-the-game-runs). |

```lua
-- An options menu's "Apply" button.
set_setting("window.vsync", vsync_checked and "true" or "false")
set_setting("window.max_fps", "60")       -- 0 is no limit
set_setting("rendering.graphics_quality", "Low")
set_setting("ui.scale", "1.25")

print(get_setting("rendering.graphics_quality"))   -- "Low", next frame
```

A value that does not fit is refused with a warning in the Console and nothing
changes: `set_setting("window.vsync", "sometimes")`, an unknown quality name, a
negative frame rate, or a path that names no setting. A few keys are not
writable at all (`profiling_key` and the editor's own `editor_*` fields).

A write lands when the script's commands are applied, so a `get_setting`
straight after a `set_setting` in the same hook still returns the old value.

Scripts change settings only in a game (the shipped one, and Play). In the
editor process the project's settings are the ones the editor saves, so a
script cannot change them there.

**Rust scripts** do not need these functions: they have the world, and the
settings live in `renzora::CurrentProject`'s `config`, which they change
directly; the runtime applies a change there while the game runs. Two player
preferences that are not project settings live in their own resources:
`renzora::GameUiScale` (what `ui.scale` sets) and the action map,
`renzora::InputMap`.

```rust
let world = ctx.world();
let mut project = world.resource_mut::<renzora::CurrentProject>();
project.config.window.vsync = false;
project.config.window.max_fps = 60;
world.resource_mut::<renzora::GameUiScale>().set(1.25);
```

## Mixer (`set_*_volume`)

Each changes the game's audio settings live, the same values the Mixer
panel shows. Volumes are linear: `1` is normal, `0` silent, `1.5` the most.
Bus keys match without regard to case.

| Function | Arguments | |
|---|---|---|
| `set_master_volume` | `volume` | Set the Master bus. |
| `set_bus_volume` | `bus` (string), `volume` | Set a bus by its key (`"Sfx"`, `"Music"`, `"Ambient"`, or a custom bus). |
| `set_bus_muted` | `bus` (string), `muted` (bool) | Mute or unmute a bus. |

## Particle effects (`vfx_*`)

Each acts on the script's own entity's `.vfx` effect, or its direct children's
when it has none; an `_on` form names another entity instead.

| Function | Arguments | |
|---|---|---|
| `vfx_play` | | Play, or resume after a pause. |
| `vfx_pause` | | Freeze where it is. |
| `vfx_stop` | | Clear every particle and stop. |
| `vfx_restart` | | Clear and play from the beginning. |
| `vfx_burst` | `count` (int) | Spawn extra particles from every emitter now. |
| `vfx_set_time_scale` | `scale` | Play faster or slower. |
| `vfx_set_param` | `name` (string), `value` | Set the effect's Parameter nodes called `name`. |
| `vfx_set_param_vec3` | `name`, `x`, `y`, `z` | Set them to a vector. |
| `vfx_set_param_color` | `name`, `r`, `g`, `b`, `a` | Set them to a linear RGBA colour. |
| `vfx_clear_param` | `name` | Give them back their own value. |
| `vfx_set_param_on` | `entity` (string), `name`, `value` | `vfx_set_param` on the named entity. |
| `vfx_set_param_vec3_on` | `entity`, `name`, `x`, `y`, `z` | `vfx_set_param_vec3` on the named entity. |
| `vfx_set_param_color_on` | `entity`, `name`, `r`, `g`, `b`, `a` | `vfx_set_param_color` on the named entity. |

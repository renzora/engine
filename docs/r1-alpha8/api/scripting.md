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

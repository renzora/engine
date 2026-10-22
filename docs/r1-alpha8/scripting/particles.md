# Particle Effects

Scripts drive a `.vfx` [particle effect](../rendering/particle-system.md)
with the `vfx_*` functions: play it, stop it, burst it, and turn the knobs its
author exposed as **Parameter** nodes. They are declared by the particle
system itself, so every scripting language has them.

## Which effect

A call acts on the script's own entity when it has a Particle System, and
otherwise on its direct children that do: a script on a character can fire the
dust effect parented under its feet without looking it up. The `_on` forms take
the name of another entity (as the Hierarchy shows it) and do the same there.

## Playback

| Function | |
|---|---|
| `vfx_play()` | Play, or resume after a pause. |
| `vfx_pause()` | Freeze where it is. |
| `vfx_stop()` | Clear every particle and stop. |
| `vfx_restart()` | Clear and play from the beginning. |
| `vfx_burst(count)` | Spawn `count` extra particles from every emitter now. |
| `vfx_set_time_scale(scale)` | Play faster (above 1) or slower (below 1). |

## Parameters

An effect exposes a value to scripts with a Parameter node in its graph: give
the node a **Name** in the effect editor, wire it where the value goes (a spawn
rate, a velocity, a colour), and set it by that name. Until a script sets it,
the node is its own value. See
[Parameters](../rendering/particle-system.md#parameters) for how it behaves.

| Function | |
|---|---|
| `vfx_set_param(name, value)` | Set the Parameter nodes called `name` to a number. |
| `vfx_set_param_vec3(name, x, y, z)` | Set them to a vector. |
| `vfx_set_param_color(name, r, g, b, a)` | Set them to a linear RGBA colour. |
| `vfx_clear_param(name)` | Give them back their own value. |
| `vfx_set_param_on(entity, name, value)` | `vfx_set_param` on the named entity's effect. |
| `vfx_set_param_vec3_on(entity, name, x, y, z)` | `vfx_set_param_vec3` on the named entity's effect. |
| `vfx_set_param_color_on(entity, name, r, g, b, a)` | `vfx_set_param_color` on the named entity's effect. |

Every node with the name answers, in every emitter of the effect. A value
holds until it is set again or cleared, survives the effect file being edited
and reloaded, and is not saved with the scene.

```lua
local heat = 0

function on_update(dt)
    -- A fire that builds while E is held and dies down when it is not.
    if is_key_pressed("E") then
        heat = math.min(heat + dt, 1)
    else
        heat = math.max(heat - dt * 0.5, 0)
    end
    vfx_set_param("intensity", 5 + 60 * heat)
    vfx_set_param_color("tint", 1.0, 0.3 + 0.5 * heat, 0.1, 1.0)

    if is_key_just_pressed("Space") then
        -- Aim the sparks effect on another entity upward, then burst this
        -- entity's own effect.
        vfx_set_param_vec3_on("Sparks", "launch", 0, 8, 0)
        vfx_burst(40)
    end
end
```

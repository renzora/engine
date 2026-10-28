# Scripting Overview

A scene describes what exists. A script describes what happens.

You attach a script to an entity, and from then on that entity has behaviour: it moves, reacts to input, takes damage, opens a door.

## Attaching a script

Every entity can carry a **Scripts** component. Drag a script file from the **Assets** panel onto an entity, or select the entity and click **Add Script** in the Inspector.

<!-- screenshot: script_component.png - the Scripts component in the Inspector with a script attached -->

One entity can carry several scripts. They run in the order they are listed.

## Languages

Renzora routes a script to a language by its file extension.

**Rust** (`.rs`) is built in. The editor compiles a Rust script when you save it and calls it once per frame per entity, with full access to the world. See [Rust Scripts](/docs/r1-alpha8/scripting/rust-scripts).

**Other languages** arrive as backends from the [Marketplace](/docs/r1-alpha8/marketplace/browsing). Install one and the engine starts routing that extension to it. Two languages can coexist in one project, and on one entity.

Which languages a project can be written in is decided by which backends are installed, not by how the engine was compiled. Each backend brings its own reference.

## When scripts run

Nothing runs while you are arranging the scene. Scripts run in the game you start with **Play**, or in the editor when you switch on one script's play button in the Inspector.

See [Playing and Simulating](/docs/r1-alpha8/editor/play-mode).

## Every frame, or every fixed step

A script has two places to put work that repeats:

| Hook | Runs | Use it for |
|---|---|---|
| `on_update` | Once per rendered frame. `delta` is the frame time, so it changes with the frame rate. | Input edges (`is_key_just_pressed`), UI, camera, anything visual. |
| `on_fixed_update` | Once per fixed step (64 times a second by default), so zero, one or several times in a frame. `delta` is always the same. | Forces, velocities and any movement that must behave the same at 30 fps and 240 fps. |

Physics steps on the same fixed clock, and what `on_fixed_update` asks of it (a force, a velocity) is applied before the step that follows it. A force pushed from `on_update` lasts until the next frame; one pushed from `on_fixed_update` lasts until the end of its frame, and is renewed by the next tick that pushes it.

Input is read once per frame, so a fixed step sees the frame's input. Held keys, buttons and axes are reliable there. A "just pressed" edge can show up in two fixed steps of one frame, or in none on a frame that had no step, so read edges in `on_update`.

`on_fixed_update` starts the frame after `on_ready` has run, never before.

## Related pages

- [Input](/docs/r1-alpha8/scripting/input) for reading the keyboard, mouse and gamepads
- [Physics](/docs/r1-alpha8/scripting/physics) for bodies, colliders and forces
- [Game UI](/docs/r1-alpha8/scripting/game-ui) for menus and HUDs
- [Script Backends](/docs/r1-alpha8/extending/script-backends) for adding a language

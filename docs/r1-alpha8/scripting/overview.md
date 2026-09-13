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

Nothing runs while you are arranging the scene. Scripts run in **Play**, in **Simulate**, or when you switch on one script's play button in the Inspector.

See [Playing and Simulating](/docs/r1-alpha8/editor/play-mode).

## Related pages

- [Input](/docs/r1-alpha8/scripting/input) for reading the keyboard, mouse and gamepads
- [Physics](/docs/r1-alpha8/scripting/physics) for bodies, colliders and forces
- [Game UI](/docs/r1-alpha8/scripting/game-ui) for menus and HUDs
- [Script Backends](/docs/r1-alpha8/extending/script-backends) for adding a language

# Playing and Simulating

The Play controls sit in the top bar. They are how you run your game without leaving the editor.

<!-- screenshot: play_controls.png - the Play, Pause and Stop buttons with the play target dropdown open -->

## Play

**Play** (or `F5`) runs your game. The editor swaps to your game camera, hides its own gizmos, grid and overlays, and from that point you are playing rather than editing.

**Pause** freezes it with everything still on screen. **Stop** ends the run and puts the scene back exactly as it was.

Nothing you do while playing is saved into your scene. Entering Play takes a snapshot, and Stop restores it. Move something in a running game and it snaps back when you stop.

## Simulate

**Simulate** runs scripts, physics and animation while leaving the editor fully live. You keep your editor camera, your gizmos, your selection and the Inspector. You can select something mid-simulation and watch its values change.

It is the right choice when you want to see behaviour rather than play the game: watching a ragdoll settle, checking a physics stack, tuning a script's numbers while it runs.

Like Play, entering Simulate snapshots the scene and Stop restores it.

| | Play | Simulate |
|---|---|---|
| Camera | Your game camera | The editor camera |
| Editor gizmos and grid | Hidden | Visible |
| Selection and Inspector | Unavailable | Live |
| Scripts, physics, animation | Running | Running |
| Scene restored on Stop | Yes | Yes |

## Where the game runs

The dropdown beside Play picks the target.

| Target | What happens |
|---|---|
| Viewport | The game plays inside the viewport panel. The default, and the fastest to get in and out of. |
| Window | The game opens in its own window, at the size your project's Window settings specify. |
| VR Headset | The game runs on a connected headset. |

The choice is remembered between sessions.

Playing in a **Window** is the closest thing to what a player gets, because the window is the one your project is configured to open. Playing in the **Viewport** is quicker and keeps the editor's panels around it.

## Scripts only run while playing

A script attached to an entity does nothing while you are arranging the scene. It runs in Play, in Simulate, or when you switch on that specific script's play button in the Inspector.

That last one is how you test one script without running everything. Without it, dropping a script that spawns things onto an entity would start spawning them the moment you attached it.

## Testing multiplayer

Running a server and a client at once needs two processes, so it does not happen inside one editor. See [Server Setup](/docs/r1-alpha8/multiplayer/server-setup).

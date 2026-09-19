# Playing

The Play controls sit in the top bar. They are how you run your game without leaving the editor.

<!-- screenshot: play_controls.png - the Play and Stop buttons with the play target dropdown open -->

## Play

**Play** (or `F5`) runs your game. **Stop** ends the run.

The game runs in **its own process**, not in the editor's. That is the whole design: the editor's world is for editing, so nothing the running game does can reach it. There is no snapshot to take and nothing to restore, because the thing you were editing was never the thing that ran. Stop leaves no debris.

It also means the game runs what is **on disk**. Save before you play, or you will be playing the last saved version.

## Where the game runs

The dropdown beside Play picks the target.

| Target | What happens |
|---|---|
| Window | The game opens in its own window, at the size your project's Window settings specify. |
| VR Headset | The game runs on a connected headset. Only offered when the editor was started VR-capable. |

The choice is remembered between sessions.

**Viewport** and **Scripts** used to be here. Viewport played into the editor's own viewport panel and Scripts ticked scripts and physics with the editor left live; both shared the world you were editing, which is the thing this design exists to prevent. A play panel that shows the running process inside the editor is the intended replacement.

## Scripts only run while playing

A script attached to an entity does nothing while you are arranging the scene. It runs in Play, or when you switch on that specific script's play button in the Inspector.

That last one is how you test one script without running everything. Without it, dropping a script that spawns things onto an entity would start spawning them the moment you attached it.

## Testing multiplayer

Running a server and a client at once needs two processes, so it does not happen inside one editor. See [Server Setup](/docs/r1-alpha8/multiplayer/server-setup).

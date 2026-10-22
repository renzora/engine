# Playing Your Game

The **Play** button sits at the right of the top bar, beside the Marketplace. It runs your game without leaving the editor.

<!-- screenshot: play_controls.png - the Play button in the top bar, and the Game panel showing a running game -->

## Play

**Play** (or `F5`) saves the scene and starts your game. Press it again, now reading **Stop**, to end the game.

The game runs as **its own program**, separate from the editor. That has three consequences worth knowing:

- **A slow game doesn't slow the editor.** The game renders at its own frame rate, and the editor stays fully usable while it runs. You can keep working while it plays. The two still share one graphics card, so a game that keeps it fully busy makes the editor's drawing slower too.
- **A crash in the game doesn't take the editor down.** The game's window closes, the Console shows what went wrong, and you carry on.
- **Nothing the game does changes your scene.** The game loads the scene from the saved file, so moving something in a running game never touches what you are editing. That is also why Play saves first.

Everything the game logs appears in the editor's **Console**, under **Runtime**.

## The Game panel

With **Embed Game in Editor** on (the default), the running game appears in the **Game** panel. Pressing Play brings the panel to the front, adding it next to the viewport if your layout doesn't have one.

It is the game's real window placed inside the editor, so click into it and the keyboard and mouse go to the game. Click anywhere else in the editor to take them back.

Turn the setting off in **Settings > Scripting > Embed Game in Editor** and the game opens in a window of its own instead, at the size your project's Window settings specify. That is the closest thing to what a player gets.

A few limits:

- **Windows only for now.** On other platforms the game always opens in its own window.
- **Docked panels only.** If you float the Game panel out into a window of its own, the game hides until you dock the panel again.
- **Menus over the panel.** The game's window is drawn above the editor in that area, so a menu or popup the editor opens over the Game panel appears behind the running game.

## Editing doesn't play anything

While you are arranging the scene, nothing in it moves by itself. Physics is paused, scripts don't run, animated models hold their pose, cloth hangs still, and navigation agents stay where you put them. What you see is exactly what the scene file says.

Particles are the exception: emitters keep running so you can see an effect without pressing Play.

The other exception is a script's own play button in the Inspector. Switching it on runs that one script while you edit, which is how you test a script without running the whole game.

## VR

Playing into a VR headset from the editor has been removed. A game can still run in a headset with the `--vr` flag; see [VR](/docs/r1-alpha8/editor/vr).

## Testing multiplayer

Running a server and a client at once needs two processes. See [Server Setup](/docs/r1-alpha8/multiplayer/server-setup).

# Playing Your Game

The **Play** button sits at the right of the top bar, beside the Marketplace. It runs your game without leaving the editor.

<!-- screenshot: play_controls.png - the Play button in the top bar, and the Game panel showing a running game -->

## Play

**Play** (or `F5`) saves the scene and starts your game. Press it again, now reading **Stop**, to end the game.

The game runs as **its own program**, separate from the editor. That has three consequences worth knowing:

- **A slow game doesn't slow the editor.** The game renders at its own frame rate, and the editor stays fully usable while it runs. You can keep working while it plays. The two still share one graphics card, so a game that keeps it fully busy makes the editor's drawing slower too.
- **A crash in the game doesn't take the editor down.** The game's window closes, the Console shows what went wrong, and you carry on.
- **Nothing the game does changes your scene.** The game loads the scene from the saved file, so moving something in a running game never touches what you are editing. That is also why Play saves first.

The game never outlives the editor. Closing the editor ends it, and so does the editor crashing or being ended from Task Manager.

Everything the game logs appears in the editor's **Console**, under **Runtime**.

## The Game panel

With **Embed Game in Editor** on (the default), the running game appears in the **Game** panel. Pressing Play brings the panel to the front, adding it next to the viewport if your layout doesn't have one.

It is the game's real window placed inside the editor, so click into it and the keyboard and mouse go to the game. Click anywhere else in the editor to take them back.

Turn the setting off in **Settings > Scripting > Embed Game in Editor** and the game opens in a window of its own instead, at the size your project's Window settings specify. That is the closest thing to what a player gets.

A few limits:

- **Windows only for now.** On other platforms the game always opens in its own window.
- **Docked panels only.** If you float the Game panel out into a window of its own, the game hides until you dock the panel again.
- **Menus over the panel.** The game's window is drawn above the editor in that area, so a menu or popup the editor opens over the Game panel appears behind the running game.

## The editor follows the game

A second or two after you press Play, the Console says **Game connected**. From then until you press Stop, the editor and the game stay in step.

**You see the game in the editor.** The Hierarchy, the Inspector and the viewport show the game as it runs: the player's position as they walk, a crate tumbling, a bullet the game spawned appearing in the Hierarchy and disappearing when it hits. Select the player and the Inspector shows its live values.

**Your edits reach the game.** Change a component in the Inspector (a light's colour, a material, a collider's size, a value on one of your own components), move something with the gizmo, add or remove a component, add or delete an object, and the running game picks it up a moment later.

**Stop puts the game's changes back and keeps yours.** When the game ends, everything it did to the scene is undone: the player returns to the start, spawned objects go, removed ones come back. Everything you changed while it ran stays. If you edited something the game was also moving, your value wins.

**Keep changes** appears beside Stop while a game is connected. Press it to adopt what the game has done so far, for example to let physics drop a pile of rocks into place and keep where they landed. Stop then puts back only what the game does after that.

**Saving waits for Stop.** While the editor is following a game, the scene holds a moment of gameplay, so saving it then would write the player's mid-run position into your level. Saves (`Ctrl+S`, auto-save) are held and run as soon as you press Stop. The Console says so.

A few limits:

- **Most components, not all.** Anything the scene file stores through reflection travels. A few plugin-owned components are stored another way and are not sent yet.
- **Parenting is set when an object appears.** Moving an object to a different parent while the game runs changes only the editor's copy.
- **Only scene objects.** Objects the game makes without a name, and the generated insides of models, are not followed, just as they are not saved.

## Testing the game with an agent

An AI agent connected through the **MCP Server** plugin (from the Marketplace) can play the running game as well as edit it. Because the editor follows the game, its usual tools already see the game's live state. Two more reach the game itself:

- **game_screenshot** returns a picture of the game's own window.
- **game_input** presses keys and gamepad controls in the game, each held for as long as asked. Gamepad input arrives on a virtual gamepad the game sees as a real one, so a game that reads gamepads needs nothing special.

## Editing doesn't play anything

While you are arranging the scene, nothing in it moves by itself. Physics is paused, scripts don't run, animated models hold their pose, cloth hangs still, and navigation agents stay where you put them. What you see is exactly what the scene file says.

Particles are the exception: emitters keep running so you can see an effect without pressing Play.

The other exception is a script's own play button in the Inspector. Switching it on runs that one script while you edit, which is how you test a script without running the whole game.

## VR

Playing into a VR headset from the editor has been removed. A game can still run in a headset with the `--vr` flag; see [VR](/docs/r1-alpha8/editor/vr).

## Testing multiplayer

Running a server and a client at once needs two processes. See [Server Setup](/docs/r1-alpha8/multiplayer/server-setup).

# Renzora Engine `r1-alpha8`

## 2026-10-19 01:55
- feat(materials): the Material Preview shows a Sprite material live on a sample sprite instead of a sphere
- feat(materials): the add-node menu only offers nodes the material's domain can use, and a node it cannot use is marked red and takes no wires
- fix(materials): the Sprite Output's Color and Alpha pins no longer show a value box that would override the sprite
- feat(assets): a Sprite material's thumbnail is the sample sprite wearing it, tagged 2D
- feat(editor): dragging a material over something that cannot wear it shows a "not allowed" cursor and the drop does nothing
- feat(viewport): a Sprite material dropped on a sprite in the 2D view is applied to it
- feat(editor): dragged nodes in every node graph snap to the grid dots; hold Alt to place freely

## 2026-10-19 00:37
- fix(editor): the viewport stats, node-graph wires and grid, UI canvas rulers and scrollbars, 2D overlay, scale HUD and menu-bar logo spawn again instead of failing, which left the stats text piled in the window's corner
- feat(engine): `RENZORA_SHUFFLE_SEED=<n>` shuffles the order systems run in, to expose ordering bugs

## 2026-10-18 03:03
- refactor(ui): the editor and game UI track hover and press with Bevy's picking instead of the deprecated `Interaction`; plugins use `renzora_ember::UiInteraction` and `interaction::BLOCK` / `PASS_THROUGH`
- feat(ui): a row stays highlighted while the pointer is on a button inside it
- change(game-ui): a game's UI panel only stops a click reaching a 3D object behind it when it blocks

## 2026-10-18 02:24
- feat(materials): sprites can wear material graphs: Add ▸ Sprite Material in the Assets panel, then drag it onto a sprite in the Hierarchy
- feat(materials): a Sprite Color node reads a sprite's own texture at any UV, for waves, wobble and dissolves
- fix(materials): a Sprite material dropped on a 3D mesh, or a 3D material on a sprite, is refused with a Console line instead of failing to draw

## 2026-10-18 02:05
- refactor(materials): the material graph writes its shaders in WESL instead of the old dialect that had to be translated on every load
- fix(shaders): a `.wgsl` file written in WESL loads instead of failing as plain WGSL

## 2026-10-18 01:49
- feat(editor): a plugin or script system that panics is logged to the Console and the editor keeps running, instead of crashing

## 2026-10-17 19:03
- feat(editor): the window title reads "Renzora Engine" with the version and the open project's folder, replacing the version in the status bar
- feat(editor): the editor, launcher and setup windows show the Renzora icon in their title bar and taskbar entry
- fix(runtime): an exported game shows the window icon set in its project again

## 2026-10-17 17:21
- feat(editor): the editor and launcher windows use the operating system's own frame again, so they move, snap and resize like any other window

## 2026-10-17 16:08
- refactor(rendering): Lumen and screen-space GI are removed, leaving Solari as the engine's global illumination

## 2026-10-16 18:28
- feat(viewport): dropping a model that brought its own camera adds a default scene camera that rides it with the source's field of view, so Play shows the imported shot
- fix(import): FBX materials that share a name each keep their own material instead of rendering with the other's

## 2026-10-16 16:30
- feat(import): an FBX animated on its objects (doors, machines, a fly-through camera) imports with its moving parts intact and a clip that plays them
- feat(animation): a Follow Node component keeps an entity on a named node, so a scene camera can ride an imported camera

## 2026-10-16 13:35
- perf(viewport): the viewport renders only when the scene changes, so a heavy scene no longer slows the rest of the editor while nothing moves; Display > Render > Realtime renders every frame

## 2026-10-16 12:39
- fix(materials): imported materials render as plain PBR materials again instead of compiling a shader each, which also puts imported models back into Solari's ray tracing

## 2026-10-16 12:18
- feat(solari): `cargo renzora` builds in NVIDIA DLSS Ray Reconstruction whenever the DLSS SDK is set up, which denoises Solari on an RTX card under Vulkan
- fix(solari): switching Solari off and on again brings the ray-traced lighting back, and no longer switches a Deferred project to forward rendering

## 2026-10-16 11:32
- feat(solari): the Solari component has a Quality preset (Low, Medium, High, Ultra) that trades frame rate against noise
- fix(solari): Solari denoises again with ReSTIR, which Bevy 0.20 had switched off by default
- fix(solari): the Console warns when Solari has no sky light because the Environment Map is off
- docs(solari): the Solari page explains the sky light Bevy 0.20 added and the new Quality presets

## 2026-10-16 10:22
- fix(rendering): resizing the viewport with screen-space GI on no longer leaves the sky black until the camera moves

## 2026-10-15 21:36
- fix(scripting): opening a project no longer drops the editor to a few frames a second while its Rust scripts build
- fix(scripting): a Rust script that has not changed since the last launch loads without being recompiled

## 2026-10-15 21:12
- fix(rendering): sky lighting is no longer rebuilt every frame, which cost frame rate and left black patches in the environment

## 2026-10-15 20:15
- fix(editor): the status bar is back at the bottom of the window, and a floating dock window no longer grows past its own edge
- fix(materials): opening a project no longer freezes the editor while its materials are checked for shader errors

## 2026-10-15 14:53
- feat(engine): the engine runs on Bevy 0.20 and Rust 1.97.1
- feat(shaders): shaders are written in WESL, and `.wesl` files can be opened, edited, previewed and dropped onto meshes like `.wgsl`
- feat(shaders): a `.wesl` shader can import functions from another `.wesl` file in the project
- feat(shaders): shaders written for earlier versions (`#import`, `#ifdef`) keep working, including those in saved materials, themes and plugins
- fix(rendering): glass, water and Scene Color materials still refract the scene behind them
- fix(rendering): the "None" tonemapping mode still keeps colour grading and dithering

## 2026-10-15 13:35
- feat(viewport): a `.wgsl` shader can be dragged onto a mesh in the viewport or the Hierarchy to apply it

## 2026-10-15 06:31
- fix(assets): moving or renaming an asset updates the paths stored in the project's materials, scenes, particles and UI templates, so a moved model keeps its textures
- feat(materials): a `.material` changed on disk outside the Material Editor reloads in the viewport

## 2026-10-15 04:31
- feat(editor): a project opens in a centered 1600x900 window instead of maximizing
- feat(settings): Launcher Effects turns off the launcher cinematic and loading-screen haze, automatically on ultrawide and larger-than-4K displays
- feat(viewport): the log names the active cameras and the environment maps being filtered whenever either changes

## 2026-10-15 03:14
- perf(rendering): environment maps are only re-filtered while their sky, image or camera is changing, instead of every frame
- perf(viewport): the debug visualization and infinite grid no longer re-upload their GPU data every frame
- perf(ui): theme surface materials are only re-uploaded when their colors, image or animation actually change
- perf(editor): the material, shader, studio, particle and camera preview cameras stop rendering when their panel is not the visible tab

## 2026-10-15 02:22
- fix(settings): the renderer backend chosen in Settings now takes effect on restart instead of always falling back to Vulkan

## 2026-09-25 11:11
- fix(scripting): a Rust script that panics no longer brings up the crash dialog, and the Console names the script, the message and the line

## 2026-09-22 18:26
- docs(plugin): the editor API said a plugin cannot make its edits undoable, which stopped being true when the undo core moved to the contract crate

## 2026-09-21 19:17
- feat(camera): a plugin can move the viewport camera, framing an entity or looking from a point at a target

## 2026-09-18 16:27
- fix(game-ui): an `expand` canvas previewed over the viewport lays out like the shipped game, so edge-anchored UI reaches the edge
- fix(game-ui): a canvas root no longer keeps a preview scale that was saved into a scene, which shrank the UI in an exported game

## 2026-09-15 13:52
- fix(linux): the editor window carries an app id, so a dock can show its icon and pinning it works

## 2026-09-15 13:38
- feat(editor): asset context menus can copy project-relative paths

## 2026-09-15 13:16
- fix(plugins): deleting a plugin that is currently loaded works instead of failing on Windows, and the folder is reclaimed at the next start

## 2026-09-15 12:37
- fix(marketplace): updating an installed plugin works while the editor is open, instead of failing with "the directory is not empty"

## 2026-09-15 12:14
- feat(marketplace): an Updates view lists the installed plugins with a newer version published, and updates them
- feat(marketplace): a card says what you already have: grey Installed with a tick, or an amber Update when there is a newer version
- feat(update): the software update dialog lists plugin updates under the engine's own versions
- feat(settings): Settings ▸ Editor ▸ Plugins marks plugins with an update and counts them above the grid
- feat(settings): a Plugin Update Reminders switch decides whether the editor volunteers that a plugin is out of date
- feat(export): the exporter's Plugins tab marks any plugin that would ship out of date
- fix(marketplace): a category's card grid fills the width instead of leaving most of a card's worth of gap at the end of every row

## 2026-09-15 11:46
- feat(marketplace): a plugin states the engine each release was built for, so you are offered the newest version your editor can actually run instead of one that will not load
- feat(marketplace): the plugin store hides listings with no release for your engine, and shows the version you would get rather than the newest one published
- feat(marketplace): the update check now says when a newer version exists but needs a newer editor, instead of showing nothing
- feat(plugins): a plugin can add its own entry to the Assets and Attach create menus
- fix(assets): the Lua Script entry no longer appears when no Lua interpreter is installed
- fix(scripting): a plugin can implement a scripting language again, which fixes Lua
- fix(plugins): a plugin's artwork shows up in Settings and the exporter again, whatever image format it ships
- build(ci): every nightly now checks that the published plugins still compile against the engine it just built, on all six desktop platforms
- build: the macOS and Linux build artifacts no longer carry the application twice, halving them from 1.16 GB to about 660 MB
- feat(sdk): a plugin or Rust script that fails to build now writes the compiler's diagnostic to a `build.log` in its own build directory, so it can be read after the window that showed it has closed
- feat(inspector): a component the graphics quality tier has switched off now shows an amber warning in its header saying so, instead of looking broken
- fix(marketplace): installing a free asset without signing in now counts towards the creator's downloads, which it never did
- fix(marketplace): installing a multi-file asset counts one download instead of two
- feat(marketplace): opening an asset in the editor's store now counts a view, as opening its page on the website already did
- fix(particles): removing a particle effect, or pointing it at a different file, now works instead of leaving the old effect drawing or frozen
- fix(particles): clearing the effect file leaves the emitter empty rather than swapping in a default effect that keeps emitting
- fix(particles): editing a `.particle`'s spawn rate, burst count or timing now takes effect, which only its colours did before
- fix(particles): an effect's light is removed when its effect no longer has one
- fix(scene): a particle emitter no longer saves its live spawner timing into the scene file
- feat(scene): choose what happens when a scene is edited outside the editor while you have unsaved changes: ask, reload, or keep yours (Settings → Editor → Scenes)
- feat(scene): deleting a file the open scene still uses now warns and names the entities using it, instead of the mesh quietly vanishing
- feat(particles): a `.particle` edited outside the editor updates every entity using it
- feat(ui): a `.html` template edited outside the editor rebuilds the canvases using it
- fix(assets): a file saved by an editor that writes to a temporary name and renames it now hot-reloads, which covers most editors and previously reloaded nothing
- feat(engine): the editor watches the whole project, so a file edited outside it is picked up as soon as it is saved
- fix(engine): a file being saved is no longer reported while it is still half-written under a temporary name
- fix(assets): a texture, model or sound edited outside the editor reloads on save, which it never did before
- perf(scripting): a Rust script rebuilds when you save it rather than up to half a second later, and an idle project costs nothing to watch
- fix(scripting): opening a project no longer compiles every Rust script twice
- perf(editor): the Assets panel, the script picker and the material picker no longer re-read the project on a timer to notice files changed outside the editor
- perf(editor): the script and material pickers update from the file that changed instead of re-reading the project, so their cost no longer grows with project size
- perf(editor): the Scenes panel no longer reads the scenes folder off disk on every frame it is open
- perf(editor): the editor no longer stats the active theme's shader files every frame to notice a theme edit
- perf(editor): project fonts and language packs are picked up when they change rather than by re-reading their folders on a timer
- perf(editor): a folder tile in the Assets panel walks its contents once instead of every five seconds for as long as it is on screen
- feat(scene): a scene edited outside the editor reloads in the viewport, and says so instead of reloading when you have unsaved changes
- fix(scripting): the editor no longer stutters twice a second on a project with a lot of files, which the Rust script watcher was walking on the main thread
- fix(editor): the VR on/off line at startup is printed again, having been written before the logger existed and silently dropped
- feat(editor): every editor launch logs whether VR is on or off, and whether pipelined rendering is on
- feat(editor): the console names each viewport camera as it switches on or off, and says why
- fix(editor): the editor no longer loses a third of its frame rate just because an OpenXR runtime is installed; VR editing is opt-in with `--xr`
- feat(plugin): a plugin can use the engine's physics types, so physics plugins no longer carry their own copy
- feat(plugin): a plugin can declare script functions, which only engine crates could do before
- refactor(plugin): Solari, lens distortion, wind and water ship as installed plugins rather than in the binary
- refactor(plugin): there is one kind of installable plugin, and it is an ordinary Bevy plugin shipped as source
- refactor(scripting): a language backend is a plain Rust trait a plugin registers, rather than a C-ABI boundary
- feat(settings): Reset to Defaults no longer offers to clear plugin settings, which plugins now keep as ordinary resources
- refactor(net): the HTTP client is a plain Rust trait rather than a C-ABI boundary
- refactor(audio): the mixer is a plain Rust trait rather than a C-ABI boundary
- fix(net): a backend that panics now fails the requests already handed to it, instead of leaving them parked until their own timeouts
- perf(plugin): plugins build several at a time instead of one after another
- perf(plugin): the plugins that take longest to build start first, so one no longer trails after the rest have finished
- fix(plugin): a plugin's cargo build tree is deleted once it has been built, which was most of what an install kept on disk
- fix(plugin): the setup progress bar follows plugins finishing rather than starting, so it no longer jumps ahead and then stalls
- fix(plugin): a disabled plugin that has never been built now appears in Settings, so it can be turned back on
- fix(ci): the Windows build no longer fails its own MSVC runtime audit when there is nothing to report
- feat(ci): a manual build can target a single arch, such as `windows-x64`, instead of a whole operating system
- fix(windows): the engine, its plugins and exported games link the Visual C++ runtime in, so they start on a machine that has never installed the redistributable
- fix(macos): the editor no longer opens as "damaged" — the `.app` ships signed again
- feat(ci): macOS releases can be Developer ID signed and notarized when the signing secrets are set

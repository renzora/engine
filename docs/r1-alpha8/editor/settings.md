# Settings

`Ctrl+,` or the gear button beside the menu opens Settings: a search box and a category list on the left, the settings themselves on the right.

<!-- screenshot: settings_interface.png - the Settings overlay on the Interface page, category rail on the left -->

Each category is a page holding one or more collapsible sections. Changes apply as you make them. There is no Apply button.

## The pages

| Group | Page | Sections |
|---|---|---|
| Project | Project | Project, Global Scenes |
| | Window | Window, Render Resolution |
| | Rendering | 3D Rendering, 2D Rendering |
| Appearance | Interface | Fonts, Language, Display, Hierarchy, Inspector, UI Workspace |
| | Theme | Active Theme, Semantic Colors, Surfaces, Text, Widgets, Panels, Syntax Tokens, Editor Chrome, Widget Styles |
| Editor | General | Developer, Import, Scenes |
| | Renderer | Renderer |
| | Auto-Save | Auto-Save |
| | Viewport | Grid, Labels |
| | Camera | Camera |
| | Gizmos | Gizmos |
| | Scripting | Scripting, Code Editor |
| Controls | Input | Input actions and their bindings |
| | Shortcuts | One section per shortcut category |
| Plugins | one page per plugin | Whatever that plugin registers |

Everything under **Project** is stored in the project's `project.toml` and travels with the project. Everything else is yours, and follows you between projects.

The editor writes `project.toml` only when one of these settings changes (or a scene it names is renamed). Opening, closing and switching scene tabs is remembered in your own settings instead, so a change you make to `project.toml` by hand while the editor is open is kept. Change the same setting in both places at once and the editor's write wins. A theme added to `themes/`, or a font added anywhere under `fonts/`, appears in the Game UI Theme and Game UI Font lists without reopening the project, even with Settings open. Game UI Font saves a project font by its path (`fonts/isometra/Isometra-Regular.ttf`), which is what lets an exported game load it.

## Project

<!-- screenshot: settings_project.png - the Project settings page -->

**Project** names your game, sets its version, and picks the boot scene, which is the scene that loads when the game starts. **Global Scenes** lists scenes that load before the boot scene and stay loaded across scene changes, which is where a persistent HUD or audio manager goes.

**Physics** picks the physics engine for the 3D world and the 2D world separately. **Default** is the engine's own avian; a physics engine installed as a plugin appears beside it in whichever world it simulates. The editor moves the open scene onto the new engine straight away, and the game uses it from its next start. The Console says which engine runs each world once a project loads. See [Physics Backends](/docs/r1-alpha8/extending/physics-backends).

**Window** is the window your shipped game opens: its size, whether it is resizable, windowed, fullscreen or borderless, **VSync**, and the **Frame Rate Limit**. VSync follows the monitor (240 frames a second on a 240 Hz screen); the Frame Rate Limit holds the game to a chosen rate on any screen, such as a steady 30 or 60, and the two combine. The limit applies to the game and to Play, not to the editor's own window. See [Frame-rate limit](../rendering/pipeline.md#frame-rate-limit). A running game applies a change at once, so a settings menu in the game can offer these too; it changes only what was changed, so a player's resized window keeps its size when, say, VSync is switched. Play opens the game in its own window, so all of this applies there exactly as in an export.

The editor is your game with editor chrome on top, so a setting the game has is not repeated for the editor. **VSync** here and **Graphics Quality** under Rendering apply to the editor as well as the game: turn VSync off and the editor uncaps too, and the viewport renders at the same quality tier the game will ship with.

**Render Resolution** is what the camera renders at before being scaled onto that window. It only takes effect once **Stretch Mode** is set to Viewport. Leave Stretch Mode off and the two are the same thing. Turn it on and set the resolution to 320x180 and you get chunky pixel art upscaled to a 1080p window. Both apply while the game runs.

**Rendering** picks the renderer and the graphics backend, sets **Graphics Quality** and the game's **Render Scale**, and holds the 2D settings. The game's graphics backend is `[rendering] backend` in `project.toml` (`auto`, `dx12`, `vulkan`, `metal` or `gl`): the exported game and Play use it, and it takes effect the next time the game starts, because the GPU backend is chosen once at launch. A backend the player's platform cannot run (DirectX 12 on Linux, say) falls back to Automatic, with a warning in the log. The editor's own window keeps the backend under [Renderer](#renderer). See [Graphics backend](../rendering/pipeline.md#graphics-backend). The 2D **Image Filter** applies to sprites already on screen as well as new ones.

## Interface

**Fonts** picks the UI font, the code font and a base size. The UI font defaults to **Inter**; **Noto Sans**, the earlier default, is still in the list.

**Display** holds **UI Scale**, from 75% to 300%, applied on top of your operating system's own scaling. `Ctrl+0` snaps back to 100% if you pick something awkward. **Scroll Speed** is here too.

**Language** picks the interface language from the built-in packs plus any you have added.

**Hierarchy** and **Inspector** hold how those two panels behave: whether parents stack, whether clicking a row toggles it, how much of the Inspector is expanded by default.

**UI Workspace** covers what happens after you drag a panel out of the bottom panel, and whether new files start with boilerplate.

## Renderer

**Renderer** holds the **Graphics Backend** the editor starts with, which takes effect after a restart and is yours rather than the game's (a game uses its project's backend, under Project > Rendering), and **Launcher Effects**: the launcher's 3D cinematic and the animated haze behind the loading screen. Both fill the window, so their cost grows with the display. **Auto** turns them off on ultrawide and larger-than-4K displays and leaves them on elsewhere; **Full** and **Reduced** force one or the other.

**Throttle When Unfocused** (on by default) holds the editor to about 60 frames a second while another window has focus, to save power and heat. Turn it off to keep the editor at full speed in the background, for example while profiling it from another window: with it on, every measurement taken that way stops at about 16 ms a frame whatever the editor's real cost.

**Render Thumbnails** (under Import, on by default) renders previews for models and materials that have none on disk yet, in the [companion](/docs/r1-alpha8/editor/companion)'s render worker or in the editor. Turn it off to show only thumbnails already rendered: the companion drops its backlog and stops the render worker straight away, giving back the memory and GPU it held. Useful in a project with thousands of imported materials, such as a city. Thumbnails that were skipped while it was off render after it is turned back on and the project is reopened.

## Viewport, Camera and Gizmos

<!-- screenshot: settings_viewport.png - the Viewport settings page showing the Grid section -->

**Viewport** holds the grid and entity labels. VSync and Graphics Quality are not here: the editor follows the game's, under Project. **Realtime** is switched from the viewport's **Display > Render** menu. **Camera** holds move speed and the look, orbit, pan and zoom sensitivities. **Gizmos** picks which gizmos draw and how large they are.

These are covered in context in [Viewport and Camera](/docs/r1-alpha8/editor/viewport).

## Shortcuts

<!-- screenshot: settings_shortcuts.png - the Shortcuts page with a rebind in progress -->

Every action the editor knows, grouped by category. Click a binding and press the keys you want. Only the shortcuts you actually change are saved, so a later release that moves a default still moves it for you.

See [Keyboard Shortcuts](/docs/r1-alpha8/editor/shortcuts) for the shipped defaults.

## Input

<!-- screenshot: settings_input.png - the Input page showing input actions and their bindings -->

Input actions are your game's controls, not the editor's. You name an action such as `jump` or `move_forward`, bind keys, mouse buttons or gamepad inputs to it, and read the action by name from a script. Rebinding later does not touch the script.

See [Input](/docs/r1-alpha8/scripting/input).

## Theme

<!-- screenshot: settings_theme.png - the Theme page with the colour sections open -->

Pick a theme, or edit one. See [Themes](/docs/r1-alpha8/editor/themes).

## Plugins

<!-- screenshot: settings_plugins.png - the Plugins page listing installed plugins with their toggles -->

Every installed plugin, with a toggle and, for one you installed yourself, a delete button. Both take effect on the next start: a plugin's code is mapped into the running editor and cannot be withdrawn, so deleting one removes it from the plugins folder now and it stops loading next time. A plugin that has its own settings gets its own page under this group.

A plugin with a newer version published is marked twice: an amber arrow on its card's artwork, and **Update to 1.2.0** in place of its status line. An amber line above the grid counts them and takes you straight to the marketplace's **Updates** view, which is where the update is installed from. One that needs a newer editor than you are running says so instead, and is fixed by **Help ▸ Check for Updates**.

**Plugin Update Reminders** decides whether the editor volunteers any of this. On, it shows one toast at startup when something is out of date. Off, it says nothing and waits to be asked: this page, the updater and the marketplace's Updates view all still answer.

New plugins are installed from the [Marketplace](/docs/r1-alpha8/marketplace/browsing) or from the dashboard.

## Resetting

**View > Reset to Defaults** is the "I have made a mess of this" button. It asks what to reset, and each part can be left alone.

| | What goes |
|---|---|
| Workspaces and panels | Every workspace layout, the floating windows and the bottom panel |
| Editor settings | Every preference on these pages, and the theme |
| Viewport | Camera speed, snapping, the grid, gizmos |
| Keyboard shortcuts | Every shortcut back to its shipped key |
| Plugin settings | Everything installed plugins have saved |
| Tutorial progress | Which chapters are done, so the tutorial offers itself again |

Plugin settings and Tutorial progress start unticked. The other four are on.

Your language, which plugins are installed, and your projects are never touched. Everything takes effect immediately, with no restart.

Three narrower resets sit in the same menu: **Reset Layout** for the active workspace, **Reset Workspace** for the whole ribbon, and **Reset Global Docks** for the bottom panel.

## Where your settings live

Your preferences are in `~/.renzora/settings.toml`, in sections.

| Section | Holds |
|---|---|
| `[app]` | Language, disabled plugins, update channel, tutorial progress, auto-save |
| `[editor]` | Everything on the Settings pages above |
| `[viewport]` | Camera sensitivities, the grid, gizmos and snapping |
| `[keybindings]` | Only the shortcuts you have rebound |
| `[projects."<path>"]` | Per project: the scene you had open and your open documents |
| `[plugins]` | One entry per plugin that saves settings |

It is plain TOML and safe to read or hand-edit. The editor rewrites a section when something in it changes and leaves the others alone.

Three things deliberately live elsewhere. `project.toml` holds only what a shipped game needs, so nothing about you travels with a project you publish. The dock layout is a tree rather than a list of preferences, so it lives in `~/.renzora/layout.json`. Themes are their own files, because a theme is something you share.

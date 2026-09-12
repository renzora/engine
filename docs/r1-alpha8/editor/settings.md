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
| Editor | General | Developer, Renderer, Import |
| | Auto-Save | Auto-Save |
| | Viewport | Grid, Labels, Performance |
| | Camera | Camera |
| | Gizmos | Gizmos |
| | Scripting | Scripting, Code Editor |
| Controls | Input | Input actions and their bindings |
| | Shortcuts | One section per shortcut category |
| Plugins | one page per plugin | Whatever that plugin registers |

Everything under **Project** is stored in the project's `project.toml` and travels with the project. Everything else is yours, and follows you between projects.

## Project

<!-- screenshot: settings_project.png - the Project settings page -->

**Project** names your game, sets its version, and picks the boot scene, which is the scene that loads when the game starts. **Global Scenes** lists scenes that load before the boot scene and stay loaded across scene changes, which is where a persistent HUD or audio manager goes.

**Window** is the window your shipped game opens: its size, whether it is resizable, and windowed, fullscreen or borderless.

**Render Resolution** is what the camera renders at before being scaled onto that window. It only takes effect once **Stretch Mode** is set to Viewport. Leave Stretch Mode off and the two are the same thing. Turn it on and set the resolution to 320x180 and you get chunky pixel art upscaled to a 1080p window.

**Rendering** picks the renderer and the graphics backend, and holds the 2D settings.

## Interface

**Fonts** picks the UI font, the code font and a base size.

**Display** holds **UI Scale**, from 75% to 300%, applied on top of your operating system's own scaling. `Ctrl+0` snaps back to 100% if you pick something awkward. **Scroll Speed** is here too.

**Language** picks the interface language from the built-in packs plus any you have added.

**Hierarchy** and **Inspector** hold how those two panels behave: whether parents stack, whether clicking a row toggles it, how much of the Inspector is expanded by default.

**UI Workspace** covers document tabs, what happens after you drag a panel out of the bottom panel, and whether new files start with boilerplate.

## Viewport, Camera and Gizmos

<!-- screenshot: settings_viewport.png - the Viewport settings page showing the Grid section -->

**Viewport** holds the grid, entity labels and the performance options. **Camera** holds move speed and the look, orbit, pan and zoom sensitivities. **Gizmos** picks which gizmos draw and how large they are.

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

Every installed plugin, with a toggle. Turning one off takes effect on the next start. A plugin that has its own settings gets its own page under this group.

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
| `[projects."<path>"]` | Per project: the scene you had open and your document tabs |
| `[plugins]` | One entry per plugin that saves settings |

It is plain TOML and safe to read or hand-edit. The editor rewrites a section when something in it changes and leaves the others alone.

Three things deliberately live elsewhere. `project.toml` holds only what a shipped game needs, so nothing about you travels with a project you publish. The dock layout is a tree rather than a list of preferences, so it lives in `~/.renzora/layout.json`. Themes are their own files, because a theme is something you share.

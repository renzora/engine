# Keyboard Shortcuts

Every default editor shortcut, the command palette, and how to rebind keys to your liking.

Shortcuts are an **editor-only** feature. They live in the `renzora_keybindings` crate (`KeybindingsPlugin`), and the action/binding types are defined in `renzora::core::keybindings` (`EditorAction`, `KeyBinding`, `KeyBindings`) so other editor plugins can dispatch them. Because the whole editor ships as the removable `renzora_editor` bundle, none of these bindings exist in an exported game.

> The fastest way to find any command is the **command palette** — press `Ctrl+P` and start typing. It lists every tool, action, panel, and layout with its current keybinding, so you rarely need to memorize the tables below.

## Command palette

Press `Ctrl+P` to open the command palette (`renzora_command_palette`). It is a fuzzy-searchable modal that aggregates, with zero per-plugin wiring:

- Registered **tools** from the toolbar (only those visible in the current context).
- **Plugin shortcuts** registered via `register_shortcut`, each showing its current binding.
- Every built-in **editor action** (the `EditorAction` enum), shown with its key and dispatched exactly as a real key press.
- **Layouts** — `Switch to <Workspace>`.
- **Panels** — `Open <Panel>` (focuses it if already docked).
- **Settings** tabs, **File** menu commands, and **Documentation** links.

**Scope tabs.** A tab strip under the search box narrows what you're searching:

| Tab | Searches | Picking a result |
|---|---|---|
| Commands | Everything above (the default) | Runs it |
| Entities | Named entities in the open scene | Selects the entity |
| Settings | Settings tabs | Opens Settings on that tab |
| Docs | renzora.com documentation (live search) | Opens the page in your browser |
| Forum | Forum threads | Opens the thread in the Forum panel |
| Users | Community members | Opens their profile |
| Feed | Recent community-feed posts | Opens the post, comments expanded |
| Courses | Learning courses | Opens the Docs panel |
| Marketplace | Store assets (server-side search) | Opens the Marketplace panel |

The remote tabs (Docs → Marketplace) query renzora.com as you type, debounced; Docs/Forum/Users want at least two characters, while Feed/Courses/Marketplace list their latest content even with an empty query.

| Key | Action |
|---|---|
| `Ctrl+P` | Open / close the palette |
| Type | Filter by label or category |
| `Up` / `Down` | Move selection |
| `Enter` | Run the selected command |
| `Esc` | Dismiss |

## Camera

Movement keys are active **only while you hold the right mouse button** to fly. The same `W` `E` `Q` letters switch gizmo tools when you are not flying (see below) — they never conflict because flying gates them.

| Key | Action |
|---|---|
| `W` `A` `S` `D` | Fly forward / left / back / right (hold right-click) |
| `E` / `Q` | Fly up / down (hold right-click) |
| `Left Shift` | Fly faster (hold) |
| `F` | Focus selected |
| `Home` | Reset camera |
| `A` | Frame all |
| `End` | Move camera to cursor |
| `]` / `[` | Camera speed up / down |
| `L` | Toggle pivot lock |

### View angles

Blender-style numpad views; the `Ctrl` modifier gives the opposite view.

| Key | Action |
|---|---|
| `Numpad 1` / `Ctrl+Numpad 1` | Front / Back |
| `Numpad 3` / `Ctrl+Numpad 3` | Right / Left |
| `Numpad 7` / `Ctrl+Numpad 7` | Top / Bottom |
| `Numpad 5` | Toggle perspective / orthographic |

## Tools

These set the persistent gizmo handle (`ActiveTool`). They fire only when the right mouse button is not held and no modal transform is in progress.

| Key | Action |
|---|---|
| `Q` | Select |
| `W` | Translate (move) |
| `E` | Rotate |
| `R` | Scale |

### Modal transforms (Blender-style)

With at least one entity selected (and the viewport in Scene mode), these start a real-time modal transform driven by mouse movement.

| Key | Action |
|---|---|
| `G` | Grab (move) |
| `R` | Rotate |
| `S` | Scale |

While a modal transform is active:

| Key | Action |
|---|---|
| `X` / `Y` / `Z` | Constrain to that axis |
| `Shift+X` / `Shift+Y` / `Shift+Z` | Constrain to the opposite plane |
| Type digits / `.` / `-` | Enter a precise value |
| `Enter` or left-click | Confirm |
| `Esc` or right-click | Cancel |

## Selection

Mouse picking happens in the viewport; the keyboard handles the rest.

| Input | Action |
|---|---|
| Left-click | Select (replace) |
| `Ctrl+Click` | Toggle selection |
| `Shift+Click` | Add to selection |
| Left-drag | Box (marquee) select |
| `Esc` | Deselect all |
| `Ctrl+A` | Select all |
| `X` | Select under cursor |
| `V` | Move selection to cursor |
| `Ctrl+D` | Duplicate |
| `Alt+D` | Duplicate and move (starts a modal grab) |
| `Delete` | Delete |
| `F2` | Rename |
| `H` / `Shift+H` | Hide selected / isolate selected |

## Edit

| Key | Action |
|---|---|
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `Ctrl+C` / `Ctrl+V` | Copy / Paste |
| `Ctrl+A` | Create node |

> Undo/redo are command-based (`renzora_undo`). Redo defaults to `Ctrl+Y` — there is no `Ctrl+Shift+Z` binding out of the box. History is per-context (scene, material graph, blueprint, …), depth-capped at 500, and exposed in the History panel.

## File

| Key | Action |
|---|---|
| `Ctrl+N` | New scene |
| `Ctrl+O` | Open scene |
| `Ctrl+S` | Save scene |
| `Ctrl+Shift+S` | Save scene as |
| `Ctrl+,` | Open Settings |

## View

| Key | Action |
|---|---|
| `Alt+Z` | Toggle wireframe |
| `Alt+Shift+Z` | Toggle lighting |
| `Ctrl+G` | Toggle grid |
| `Ctrl+Space` | Toggle bottom panel |
| `T` | Toggle snap |
| `Shift+T` | Toggle edge snap |
| `Alt+T` | Toggle scale-from-bottom |
| `Ctrl+0` | Reset UI scale to 100% |

> Wireframe is `Alt+Z` (not bare `Z`) and lighting is `Alt+Shift+Z`. Plain `Z` was dropped because it clashed with `Ctrl+Z` and the gizmo tool keys.

## Play

| Key | Action |
|---|---|
| `F5` | Play / Stop |

## Code editor

These apply when the **Code** panel has keyboard focus (category "Code Editor"). Several reuse chords that mean something else in the viewport (`Ctrl+S`, `Ctrl+G`, `Ctrl+D`, `Ctrl+Space`) — focus decides which action fires.

| Key | Action |
|---|---|
| `Ctrl+S` / `Ctrl+Shift+S` | Save file / save all |
| `Ctrl+W` | Close tab |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |
| `Ctrl+F` / `Ctrl+H` | Find / replace |
| `Ctrl+G` | Go to line |
| `Ctrl+/` / `Ctrl+Shift+/` | Toggle line / block comment |
| `Ctrl+Space` | Trigger autocomplete |
| `Ctrl+D` | Select next occurrence |
| `Ctrl+Shift+D` | Duplicate line |
| `Ctrl+Shift+K` | Delete line |
| `Alt+Up` / `Alt+Down` | Move line up / down |
| `Ctrl+Alt+Up` / `Ctrl+Alt+Down` | Add cursor above / below |
| `Shift+Esc` | Clear extra cursors |
| `F12` | Go to definition |
| `Ctrl+Shift+F` | Format document |
| `Ctrl+Alt+D` | Show diff vs saved |
| `Ctrl+Shift+[` | Toggle fold |
| `Ctrl+\` | Split editor right |

## Customizing shortcuts

Open **Settings → Shortcuts** (or `Ctrl+,` then the *Shortcuts* tab) to rebind any action. Each binding is a key plus the `Ctrl` / `Shift` / `Alt` modifiers, and modifier matching is exact — `Ctrl+S` will not fire if `Shift` is also held. Rebound keys are respected everywhere, including the command palette and programmatic dispatches.

> Some defaults intentionally overlap: `A` is both *Frame All* and the fly-left key, and `Ctrl+A` is both *Select All* and *Create Node*. Context (whether you are flying, what panel is focused, what is selected) decides which one runs. If a chord feels ambiguous, rebind it here.

## Plugin shortcuts

Plugins add their own commands through the editor SDK, and they appear in the command palette and the Shortcuts settings automatically. Register a `ShortcutEntry` with a default `KeyBinding`:

```rust
use bevy::prelude::*;
use renzora::core::keybindings::KeyBinding;
use renzora_editor_framework::{AppEditorExt, ShortcutEntry};

fn build(app: &mut App) {
    app.register_shortcut(ShortcutEntry::new(
        "my_plugin.do_thing",          // stable id
        "Do The Thing",                // display name
        "My Plugin",                   // category
        KeyBinding::new(KeyCode::KeyP).ctrl().shift(),
        |world: &mut World| {
            // handler runs with full &mut World access
        },
    ));
}
```

The id is stable so user-customized bindings survive plugin reloads. This is exactly how the command palette itself registers `Ctrl+P` (`command_palette.toggle`).

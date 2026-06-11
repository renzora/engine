# Theming System

Renzora has two independent theming layers — the editor's ember widgets and the in-game `renzora_game_ui` HUD — each a swappable resource painted from color tokens, with no egui anywhere.

## Two layers

Theming is split by *where the UI runs*. The editor and the shipped game use entirely separate types, so a game's HUD theme never inherits the editor's chrome colors.

| Layer | Where | Types | Crate | Repaint |
|---|---|---|---|---|
| Editor widgets & chrome | The editor session | `style::Theme` + `theme::Palette`/`StyleSheet` | `renzora_ember` | `Styled` + `apply_theme` (live), or re-spawn for the `Palette` |
| In-game UI (HUD, menus) | The shipped game | `UiTheme` + `UiThemed` | `renzora_game_ui` | `ui_theme_system` (live) |

The editor's *theme files, picker, and live color editing* are owned by a third piece, `renzora_theme::ThemeManager`, which feeds the ember layer through a bridge (below).

> ⚠️ `egui`/`bevy_egui` were removed from the engine. There is **no `EditorTheme` resource, no `Color32`/`FontId`, no `register_theme`, no `ThemeManager::cycle_next`/`set_theme`.** Any doc or example using those is from a dead API. All colors are bevy `Color` (or the hex-serialized `Rgba`/`ThemeColor` wrappers), and custom themes live in the project's `themes/` directory (not `editor_themes/`).

---

## Editor theming (ember)

The editor uses two cooperating mechanisms in `renzora_ember`. Both default to the built-in dark look and both are runtime-safe (no egui), so an exported game that pulls in ember widgets renders with the same colors.

### The runtime palette — `renzora_ember::theme`

`Palette` is a **process-wide** set of base colors held in a `LazyLock<RwLock<…>>`. Spawn functions only have `&mut Commands` (not the world), so they resolve the *current* colors through lowercase accessor functions instead of threading a resource through every signature:

```rust
use renzora_ember::theme::{accent, panel_bg, rgb, text_primary, text_muted};

// Resolve colors at spawn time.
commands.spawn((
    Text::new("Hello"),
    TextColor(rgb(text_primary())),       // rgb((u8,u8,u8)) -> bevy Color
    BackgroundColor(rgb(panel_bg())),
));
```

A representative slice of the accessors (see `renzora_ember::theme` for all 24):

| Accessor | Default (dark) | Role |
|---|---|---|
| `window_bg()` | `(24, 24, 30)` | Chrome: top bar, doc tabs, status bar, dock gaps |
| `panel_bg()` | `(26, 26, 31)` | Panel leaf content |
| `header_bg()` | `(30, 30, 36)` | Tab headers |
| `accent()` | `(80, 140, 255)` | Buttons, active underline, selection |
| `text_primary()` | `(230, 230, 240)` | Main text |
| `text_muted()` | `(148, 148, 160)` | Labels, hints |
| `border()` | `(60, 60, 74)` | Input / panel borders |
| `selection()` | `(50, 54, 66)` | Selected rows |
| `play_green()` / `warn_amber()` | `(89,191,115)` / `(224,170,72)` | Semantic success / warning |

Because these are read **at spawn**, changing the palette does not retroactively recolor already-spawned widgets — the shell *re-spawns* the chrome when the active theme switches (see the bridge). `renzora_ember::theme` also carries a parallel process-wide `StyleSheet` (per-widget-type geometry + typography, resolved with `style(Role)`), set the same way.

```rust
use renzora_ember::theme::{set_palette, palette, Palette};

// Replace the live palette (the theme bridge does this; you rarely call it directly).
set_palette(Palette { accent: (0, 255, 200), ..palette() });
```

### The per-widget `Theme` resource — `renzora_ember::style`

`style::Theme` is a `Reflect` + Serde **resource** holding one `StyleToken` per widget `Role`. Unlike the palette, this path repaints **live**: any widget carrying a `Styled` component is repainted by the `apply_theme` system whenever the `Theme` resource (or the widget's own `Styled`) changes — no re-spawn. Both are registered by `style::ThemePlugin`, which is part of `EmberPlugin`.

```rust
use renzora_ember::style::{Styled, Role, WidgetState};

// A themeable button: paints from theme.token(Role::Button) for its state.
commands.spawn((
    Node { padding: UiRect::all(Val::Px(8.0)), ..default() },
    BackgroundColor::default(),     // apply_theme fills this
    BorderColor::all(Color::NONE),  // and this
    Styled::new(Role::Button),
));

// Change the widget's state and apply_theme repaints it next frame.
fn on_hover(mut q: Query<&mut Styled>) {
    for mut s in &mut q {
        s.state = WidgetState::Hover;
    }
}
```

`apply_theme` writes the token's `bg_for(state)` into `BackgroundColor`, sets `Node` border width / radius / padding, and (if present) `BorderColor` via `border_for(state)`.

**`WidgetState`:** `Normal`, `Hover`, `Pressed`, `Active` (selected/on/focused), `Disabled`.

**`Role`** (each maps to a `StyleToken` via `Theme::token`): `Button`, `ButtonAccent`, `IconButton`, `Input`, `Checkbox`, `Segment`, `Toggle`, `Card`, `Badge`, `Alert`, `Toast`, `Tab`, `Panel`, `Menu`.

Each `StyleToken` is a box style with per-state fills plus geometry and text:

| Field | Type | Meaning |
|---|---|---|
| `bg`, `bg_hover`, `bg_pressed`, `bg_active`, `bg_disabled` | `Rgba` | Fill per `WidgetState` |
| `border`, `border_active` | `Rgba` | Border (active = focus color) |
| `border_width`, `radius` | `f32` | Border thickness, corner radius (px) |
| `pad_x`, `pad_y` | `f32` | Inner padding (px) |
| `text`, `text_muted` | `Rgba` | Foreground colors |

Beyond the per-`Role` tokens, `Theme` also has **bespoke multi-element styles** as direct fields — `node_graph` (`NodeGraphStyle`), `asset_tile` (`AssetTileStyle`), `dock` (`DockStyle`: leaf/tab-bar/divider/shadow chrome), `top_bar` / `doc_tabs` / `status_bar` (`BarStyle`), and `timeline` (`TimelineStyle`). These let one element (e.g. a node-graph cable) be retargeted without smearing across the rest of the widget.

> `Rgba` is the theme color type: sRGBA bytes serialized to/from `#RRGGBB` / `#RRGGBBAA`. `Rgba::rgb((r,g,b))` builds an opaque color; `.color()` converts to a bevy `Color`.

### Theme files — `themes/*.toml`

Custom editor themes are TOML files under the project's `themes/` directory. A single `themes/<name>.toml` can hold **two kinds of sections**, and each loader reads only its own sections and ignores the rest:

1. **Editor color sections** (`[meta]`, `[semantic]`, `[surfaces]`, `[text]`, `[widgets]`, `[panels]`, `[categories]`, `[material]`, `[viewport]`) — parsed into `renzora_theme::Theme` by `ThemeManager`. The bridge maps these into the ember `Palette`.
2. **Ember per-widget style sections** (`[button]`, `[input]`, `[dock]`, `[node_graph]`, …) — parsed into `renzora_ember::style::Theme` by `Theme::from_toml`.

`Theme::from_toml` **cascades**: it deep-merges your file over the palette-derived defaults, so a theme only needs to specify the elements it overrides (a whole `[button]` table, or just `button.bg`):

```toml
# themes/cyberpunk.toml

# ── Editor color sections (renzora_theme::Theme) → ember Palette ──
[meta]
name = "Cyberpunk"

[semantic]
accent  = "#00FFC8"
success = "#00FF64"
warning = "#FFC800"
error   = "#FF3250"

[surfaces]
window = "#0A0514"   # → window_bg()
panel  = "#140A23"   # → panel_bg()
popup  = "#1E1432"   # → popup_bg()

[text]
primary = "#DCDCFF"
muted   = "#9696C8"

# ── Ember per-widget overrides (renzora_ember::style::Theme) ──
[button]
bg         = "#1E1432"
bg_hover   = "#2A1E46"
bg_pressed = "#00FFC8"
radius     = 6.0

[input]
border        = "#321E50"
border_active = "#00FFC8"

[dock]
leaf_radius = 8.0
shadow      = true
```

> Colors are hex strings. `#RRGGBB` is opaque; add a fourth byte (`#RRGGBBAA`) for alpha. Numbers (`radius`, `pad_x`, …) are plain floats in logical px. Any section you omit keeps its built-in default.

### Switching & editing themes — `renzora_theme::ThemeManager`

`ThemeManager` (an editor-only resource, initialized by `renzora_editor_framework`) owns theme discovery, the active theme, and persistence. The **Settings overlay → Theme tab** edits `active_theme` live with color pickers; the status-bar theme switcher calls `load_theme` to swap. Its real API:

| Method / field | Purpose |
|---|---|
| `active_theme: Theme` | The live `renzora_theme::Theme` (color sections) |
| `active_theme_name: String` | Name of the active theme |
| `available_themes: Vec<String>` | `"Dark"`, `"Light"`, plus custom file stems |
| `set_project_path(&Path)` | Point at the project; scans `themes/*.toml` |
| `scan_themes()` | Refresh `available_themes` |
| `load_theme(name) -> bool` | Activate a built-in or a `themes/<name>.toml` |
| `save_theme(name) -> Option<PathBuf>` | Serialize `active_theme` to `themes/<name>.toml` |
| `duplicate_theme(new_name) -> bool` | Copy the active theme to a new file |
| `delete_theme(name) -> bool` | Delete a custom theme (built-ins refuse) |
| `is_builtin(name) -> bool` | `true` for `"Dark"` / `"Light"` |
| `mark_modified()` / `has_unsaved_changes` | Dirty tracking for the editor |

> The only built-in **editor** themes are **Dark** and **Light**. (High Contrast is a *game-UI* built-in — see below.) File-based custom themes are native-only; on WASM `ThemeManager` exposes just the two built-ins.

### The bridge — how it all connects

`renzora_shell`'s `theme_bridge` system glues the layers together each frame:

- It maps `ThemeManager.active_theme` (a `renzora_theme::Theme`) into an ember `Palette` via `palette_from_theme` and pushes it with `set_palette` — e.g. `surfaces.window → window_bg`, `surfaces.panel → panel_bg`, `surfaces.extreme → header_bg`, `semantic.accent → accent`, `panels.tab_active → tab_active`.
- On a theme **switch**, it rebuilds the ember `style::Theme` with `build_ember_theme` (which calls `Theme::from_toml` on the same `themes/<name>.toml`) and `insert_resource`s it, then despawns and re-spawns the chrome so palette-derived widgets pick up the new colors. `apply_theme` then repaints every `Styled` widget live.
- Individual color edits update the palette but do **not** rebuild (that would close the color picker every frame).

---

## Game UI theming (`renzora_game_ui`)

The shipped game's HUD and menus use a completely separate, self-contained theme: the `UiTheme` resource plus the `UiThemed` marker. This layer is registered by `GameUiPlugin` (runtime scope), which inserts a default `UiTheme` and runs `ui_theme_system`.

`UiTheme` is a flat resource of **semantic tokens** (a `Reflect` + Serde resource, so it serializes and can be saved in a scene):

```rust
use bevy::prelude::*;
use renzora_game_ui::components::{UiTheme, UiThemed};

// Swap the whole theme — every UiThemed widget re-syncs next frame.
fn use_light_ui(mut commands: Commands) {
    commands.insert_resource(UiTheme::light());
}

// Or build a custom one from the dark base.
fn use_custom_ui(mut commands: Commands) {
    commands.insert_resource(UiTheme {
        accent: Color::srgb(0.0, 0.85, 0.6),
        health_fill: Color::srgb(0.2, 0.8, 0.2),
        health_low: Color::srgb(0.9, 0.2, 0.2),
        ..UiTheme::dark()
    });
}

// Mark a widget so its colors follow the active UiTheme.
fn spawn_button(mut commands: Commands) {
    commands.spawn((Node::default(), UiThemed));
}
```

Built-in constructors: `UiTheme::dark()` (the default), `UiTheme::light()`, `UiTheme::high_contrast()`.

Key tokens (see the struct for the full set): surfaces (`surface`, `surface_raised`, `surface_overlay`), text (`text_primary`, `text_secondary`, `text_muted`, `text_on_accent`), interactive (`accent`, `accent_hovered`, `accent_pressed`), semantic (`success`, `warning`, `error`, `info`), widget tokens (`border`, `track`, `thumb`, `progress_fill`, `health_fill`, `health_low`, `toggle_on`, `toggle_off`, `scrollbar`, `tooltip_bg`, `modal_backdrop`, `title_bar`), typography (`font_size_sm`/`md`/`lg`/`xl`), and geometry (`border_radius`, `border_width`, `spacing`).

When the `UiTheme` resource changes, `ui_theme_system` re-derives each `UiThemed` widget's style from `theme.widget_style(widget_type)` and `theme.interaction_style()`, updating its style components (`UiFill`, `UiStroke`, `UiBorderRadius`, `UiTextStyle`, `UiInteractionStyle`, …) and the per-widget data colors (slider track/fill/thumb, checkbox, toggle, tooltip, modal backdrop, window title bar).

> Unlike the editor layer, `UiTheme` is set in code (or carried in a scene) — there is no `ThemeManager`/`themes/*.toml` picker for the game HUD. Insert or replace the resource and marked widgets follow it.

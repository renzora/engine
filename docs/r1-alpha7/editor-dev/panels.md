# Building Editor Panels

Add panels to the Renzora editor with three small `App` extension methods — no egui, no traits to implement, just bevy_ui.

## The model: native bevy_ui, not a panel trait

The editor shell is **bevy_ui-native** (`renzora_shell::ShellPlugin`). The reusable dock model — `DockTree`, splits, tabs, drag-docking, drop zones — lives in `renzora_ember::dock`; the shell supplies the per-workspace layouts and chrome (menu bar, ribbon, document-tab strip, status bar). A panel is therefore just a tree of ordinary `bevy_ui` entities (`Node`, `Text`, `BackgroundColor`, …) built into a dock leaf, plus a little metadata so the dock and the Add-Panel picker know about it.

Panels are contributed by **editor-scope plugins**. The editor itself is the removable `renzora_editor` cdylib bundle that loads beside the engine binary; an editor plugin registers itself with `renzora::add!(MyPlugin, Editor)` and is replayed into the app when the bundle is installed. (See *Plugins & ABI* for how scopes and the bundle work.)

> ⚠️ There is **no `EditorPanel` trait, no `register_panel`/`register_panel_with_persistence`, and no `EditorCommands`.** egui and `bevy_egui` were removed from the engine entirely. Any doc or example showing `impl EditorPanel`, `egui::Window`, or a `&egui::Context` is from a dead API — ignore it. Panels are plain bevy_ui, and you mutate the world from systems and reactive closures with normal `&mut World` / `Commands` access.

## The three registration APIs

| Method | Trait | What it does |
|---|---|---|
| `register_shell_panel(id, title, icon, category)` | `renzora::RenzoraShellExt` | Registers panel **metadata** in `renzora::ShellPanelRegistry`, populating the dock tab label/icon and the Add-Panel `+` picker. |
| `register_panel_content(id, scroll, build_fn)` | `renzora_ember::panel::RegisterPanelContent` | Registers the **content builder** (real bevy_ui entities) and marks the id in `renzora::NativePanelIds` so the shell skips its placeholder. |
| `register_shell_status_item(item)` | `renzora::RenzoraShellExt` | Adds a per-frame **status-bar segment** to `renzora::ShellStatusRegistry`. |

A normal panel uses the first two together: one call for metadata, one for content. Status items are independent of panels.

> The shell pre-seeds metadata for ~55 built-in panels from its internal `PANEL_META` table, so most engine panels only call `register_panel_content`. A plugin that calls `register_shell_panel` for an id **wins** over the seeded default — that is how you contribute a brand-new panel.

### `register_shell_panel` — metadata

```rust
fn register_shell_panel(
    &mut self,
    id: impl Into<String>,
    title: impl Into<String>,   // shown on the dock tab + picker
    icon: impl Into<String>,    // kebab-case Phosphor icon name (e.g. "sparkle")
    category: impl Into<String>,// groups the entry in the Add-Panel picker
) -> &mut Self;
```

`icon` is a Phosphor glyph **name** (resolved via `renzora_ember::font::icon_glyph`), not a glyph or a path. `category` is a free-form string ("Scene", "Editing", "Debug", "Tutorial", …) used only to group the picker.

### `register_panel_content` — content

```rust
fn register_panel_content<F>(&mut self, id: &'static str, scroll: bool, build: F) -> PanelScope<'_>
where
    F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static;
```

- `scroll` — `true` wraps your content in a scroll view; pass `false` if the panel scrolls itself.
- `build` — returns the **root entity** of your content. It runs when the panel's tab becomes active. Everything while it *stays* active is driven by the reactive layer (next section), so you do not rebuild every frame.
- `EmberFonts` carries the three editor fonts: `fonts.ui`, `fonts.phosphor`, `fonts.mono`.
- The return value is a [`PanelScope`](#keeping-a-hidden-panel-cheap--panelscope) — chain `.systems(..)` off it to register systems that only run while the panel is visible.

Calling this also registers the id with `NativePanelIds`, so the shell stops drawing its generic placeholder for that id and lets your build own the dock leaf's `content` entity.

> **`build` re-runs on every tab activation — don't keep state in the entities.**
> A dock leaf keeps only its **active** tab's content alive; switching tabs despawns
> the pane you left and rebuilds it when you come back. That is deliberate: hidden
> panels used to accumulate in the tree, and `bevy_ui` walks the *whole* tree three
> times a frame with no skip for hidden subtrees, so their layout cost was paid every
> frame forever. Removing them was worth ~3.5 ms of main-world time in a
> representative editor layout — the single largest main-world win in the r1-alpha7
> performance pass. The cost is that per-panel view state (scroll offset, which
> section was expanded) does **not** survive a tab switch unless you store it in a
> `Resource` and read it back in `build`. The inspector's
> `InspectorSectionsOpen` is the reference example.

### `register_shell_status_item` — status bar

```rust
pub struct ShellStatusItem {
    pub id: &'static str,
    pub align: ShellStatusAlign,            // Left | Right
    pub order: i32,                         // sort within the side
    pub render: fn(&World) -> Vec<ShellStatusSegment>,
}

impl ShellStatusSegment {
    pub fn new(icon: impl Into<String>, text: impl Into<String>, color: [u8; 3]) -> Self;
}
```

`render` runs **every frame** with `&World`, so live metrics update without re-registering. Each `ShellStatusSegment` is an optional Phosphor icon name + text + an RGB color.

## A complete panel

A custom panel is one editor-scope plugin that makes both calls in `build()`. This example shows an "Entity Count" panel that displays a live count, kept in sync with `bind_text`.

```rust
use bevy::prelude::*;
use renzora::RenzoraShellExt;                       // register_shell_panel
use renzora_ember::panel::RegisterPanelContent;     // register_panel_content
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::bind_text;
use renzora_ember::theme::{accent, rgb, text_muted, text_primary};

const PANEL_ID: &str = "entity_count";

#[derive(Default)]
pub struct EntityCountPanelPlugin;

impl Plugin for EntityCountPanelPlugin {
    fn build(&self, app: &mut App) {
        // 1. Metadata → dock tab + Add-Panel picker.
        app.register_shell_panel(PANEL_ID, "Entity Count", "list-numbers", "Debug");
        // 2. Content → built once when first shown.
        app.register_panel_content(PANEL_ID, true, build_content);
    }
}

fn build_content(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        })
        .id();

    let icon = icon_text(commands, &fonts.phosphor, "list-numbers", accent(), 18.0);

    let label = commands
        .spawn((
            Text::new("Entities:"),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_muted())),
        ))
        .id();

    // This Text entity is rebound every frame by bind_text below.
    let value = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 13.0),
            TextColor(rgb(text_primary())),
        ))
        .id();

    bind_text(commands, value, |world: &World| {
        format!("{}", world.entities().len())
    });

    commands.entity(root).add_children(&[icon, label, value]);
    root
}

// Editor-scope: this plugin is replayed into the app by the renzora_editor bundle.
renzora::add!(EntityCountPanelPlugin, Editor);
```

Note `use renzora::RenzoraShellExt;` — import the trait directly. There is **no `renzora::prelude`**; use `use renzora::*;` or import individual items.

## Reactive content

Because `build_content` runs only once, you wire dynamic parts with helpers from `renzora_ember::reactive`. Each takes the target entity and a `Fn(&World) -> _` closure that the reactive layer evaluates each frame and applies only on change:

| Helper | Closure returns | Effect |
|---|---|---|
| `bind_text(commands, entity, f)` | `String` | Updates the entity's `Text` |
| `bind_text_color(commands, entity, f)` | `Color` | Updates its `TextColor` |
| `bind_display(commands, entity, f)` | `bool` | Shows/hides the entity (`Node` display) |
| `keyed_list(commands, container, f)` | `KeyedSnapshot` | Diff-rebuilds a dynamic child list |

For variable-length content (a list of items), return a `KeyedSnapshot`: a stable key + content hash per row plus a per-index `build` closure. The reactive layer only respawns rows whose key/hash changed.

```rust
use renzora_ember::reactive::{keyed_list, KeyedSnapshot};

// `list` is a column Node spawned in build_content.
keyed_list(commands, list, |world: &World| {
    let names: Vec<String> = collect_entity_names(world);
    let items = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(name, &mut h);
            (i as u64, std::hash::Hasher::finish(&h)) // (stable key, content hash)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, fonts, i| {
            c.spawn((
                Text::new(names[i].clone()),
                renzora_ember::font::ui_font(&fonts.ui, 12.0),
            ))
            .id()
        }),
    }
});
```

> To **mutate** the world from a panel (spawn, despawn, change a selection), do it from your plugin's own systems or from an interaction callback that receives `&mut World` — not from the build closure, which only constructs UI. Bindings read the world; systems write it.

### Escape hatch — `react` and `react_anchored`

When a widget needs to do something the `bind_*` table doesn't cover, `react` runs an arbitrary `FnMut(&mut World) -> bool` each frame (returning `false` retires it):

```rust
use renzora_ember::reactive::{react, react_anchored};

react_anchored(commands, my_widget, move |world: &mut World| {
    // arbitrary per-frame work for this widget
    true
});
```

**Prefer `react_anchored` for anything that belongs to a widget.** The anchor entity does two things: it opts the reaction into the hidden-pane skip, and it becomes the liveness handle — the reaction is dropped when the entity despawns, exactly like a `bind_*`. Plain `react` has no anchor, so it can't be skipped and shows up as `"(world)"` in the reactivity debug panel. Text inputs and colour pickers were each cloning several `String`s per frame for panes the user couldn't see before they were anchored.

Do **not** anchor work that must keep running while its panel is backgrounded — export progress, background loads. Anchoring those silently pauses them.

### Virtualized lists — `virtual_scroll`

A `keyed_list` builds one UI entity per item the snapshot emits. For a long list (hundreds–thousands of rows) that tanks the frame rate — every off-screen row still costs layout, change-detection and render. Wrap the same snapshot in `virtual_scroll` instead and only the rows in (or near) the viewport are built; two empty spacer nodes stand in for the rest so the scrollbar and scroll height stay correct.

```rust
use renzora_ember::virtual_scroll::virtual_scroll;

// `list` is the content node you'd otherwise pass to keyed_list, wrapped in a
// scroll_view. `snapshot` is unchanged — it still returns the FULL item list;
// virtual_scroll windows it. `6` is the overscan (extra rows above/below).
virtual_scroll(commands, list, 6, my_snapshot);
let scroll = renzora_ember::widgets::scroll_view(commands, list);
```

It's **self-measuring**: the row height and column count are read from the laid-out children each frame, so it adapts to variable item sizes (e.g. a zoom slider), grid wrapping and DPI with no per-panel constants. The hierarchy and the asset browser both build on it — prefer it over hand-rolling windowing.

#### `virtual_scroll_versioned` — when the snapshot itself is expensive

`virtual_scroll` still calls your snapshot every frame; it's the *rows* that are windowed, not the snapshot. If building the item list is itself costly (a large directory listing, a material index), pass a cheap version function and it's skipped entirely on frames where nothing changed:

```rust
use renzora_ember::virtual_scroll::virtual_scroll_versioned;

virtual_scroll_versioned(
    commands,
    list,
    6,
    |w| w.resource::<MaterialIndex>().generation,  // bumps when the data changes
    picker_snapshot,
);
```

Scroll position and viewport size are folded into the version automatically, so the window still rebuilds while scrolling and resizing — you only have to account for **data** changes.

Pick the version so it changes when the *rendered row content* changes, and no more often. The material picker hashes `(absolute path, is_current)` per row and deliberately **not** the thumbnail handle: thumbnails stream in asynchronously, so including them would invalidate the list on almost every frame during a load and give back nothing. Converting that one picker from a full rebuild to `virtual_scroll_versioned` took `text_system`'s worst frame from 14.54 ms to 2.58 ms and removed every frame over 25 ms.

### Keeping a hidden panel cheap — `PanelScope`

Reactive bindings and `keyed_list`/`virtual_scroll` snapshots are **automatically skipped while a panel is a hidden background tab**, and the panel's entities are despawned outright. Plain `Update` *systems*, though, run regardless of visibility. Register them by chaining off `register_panel_content` and they inherit the gate:

```rust
app.register_panel_content("my_panel", true, build_content)
    .systems(Update, (refresh_thumbnails, relayout_tiles));
```

`.systems(..)` applies `panel_active("my_panel")` for you. Because the id comes from the registration it is written **once** and can't drift — the old failure mode was a panel renamed on one line and left stale on another, silently un-gating itself. You can still stack your own conditions; they compose:

```rust
app.register_panel_content("my_panel", true, build_content)
    .systems(
        Update,
        refresh
            .run_if(in_state(renzora::SplashState::Editor))
            .run_if(on_timer(Duration::from_millis(250))),
    );
```

For work that must continue while the panel is hidden, use `.always(..)` on the same chain, or `.app()` to drop back to the raw `&mut App`.

Gate only **view** systems. Leave always-on work ungated — a console that must keep capturing logs while hidden, an async poll that has to drain in-flight requests, or a flag another panel reads each frame.

#### The regression guard

`crates/renzora_ember/tests/panel_systems_gated.rs` fails the build if a file that calls `register_panel_content` also uses a bare `app.add_systems` without gating. It checks for *gating*, not for a particular style — a per-system `.run_if(panel_active(..))` still passes, so correctly-gated older panels aren't churned.

This is a test rather than a convention because the convention lost: a survey found 1283 of 1535 per-frame system registrations running unconditionally in an idle editor. Nobody skipped the gate on purpose — most authors never knew it existed.

If your systems genuinely must run while the panel is hidden, put the exemption **at the call site**:

```rust
// panel-systems-ungated: poll_store drains in-flight async requests
app.add_systems(Update, (poll_store, ..));
```

Deliberately not a path list in the test: a list rots the moment a file is renamed, and it puts the justification where nobody looks while editing the code it excuses. At the call site the reason travels with the code and can't go stale — delete the systems and the marker goes with them.

## A status-bar item

Status items don't need a panel. Register one `ShellStatusItem` whose `render` returns the current segments:

```rust
use bevy::prelude::*;
use renzora::{RenzoraShellExt, ShellStatusAlign, ShellStatusItem, ShellStatusSegment};

#[derive(Default)]
pub struct FpsStatusPlugin;

impl Plugin for FpsStatusPlugin {
    fn build(&self, app: &mut App) {
        app.register_shell_status_item(ShellStatusItem {
            id: "fps_status",
            align: ShellStatusAlign::Right,
            order: 0,
            render: fps_segments,
        });
    }
}

fn fps_segments(world: &World) -> Vec<ShellStatusSegment> {
    let fps = world
        .get_resource::<bevy::diagnostic::DiagnosticsStore>()
        .and_then(|d| d.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS))
        .and_then(|f| f.average())
        .unwrap_or(0.0);

    let color = if fps >= 55.0 { [100, 200, 100] }
        else if fps >= 30.0 { [220, 180, 50] }
        else { [220, 80, 80] };

    vec![ShellStatusSegment::new("speedometer", format!("{fps:.0} FPS"), color)]
}

renzora::add!(FpsStatusPlugin, Editor);
```

## Where panels appear

Registering a panel does **not** force it into a layout. The metadata makes it available in the dock tab strip's **+** (Add-Panel) picker, grouped by `category`; the user docks it where they like. Built-in workspaces (Scene, Blueprints, Scripting, Animation, Materials, Particles, Debug, Gallery) are eight separate `DockTree`s the shell ships and the user can reorder, rename, and add to. The live layout persists per workspace, and the whole set (every workspace's tree + the active index) is serialized to `~/.renzora/layout.json` so split sizes, panel placement, and active tabs survive a restart. On launch the shell restores that file and appends any built-in workspace the saved set predates; deleting the file resets to the shipped defaults.

If you want a panel docked by default, add it to a workspace layout rather than relying on the picker; otherwise the **+** picker is how users bring it in (this is exactly what the tutorial's throwaway "Demo Panel" does — registered but deliberately not pre-docked).

> Editor panels only exist in the editor session. They live in editor-scope plugins linked into the `renzora_editor` bundle (or shipped as a `--editor` distribution plugin). When the bundle is absent — the shipped game — none of this code runs, because `PluginScope::Editor` plugins are never installed into a runtime-only binary.

## Panel toolbars — the shared strip below the top bar

There is **one** toolbar strip, mounted by the shell just under the top bar, and its contents follow the **active panel**. A panel (or any plugin) registers toolbar items keyed by a dock panel id; the strip shows a panel's items only while that panel is the active (visible) tab in its leaf — so the toolbar swaps automatically as you move between the viewport, the code editor, the material graph, etc. Nothing is keyed to a *workspace*; it's purely which panels are on screen.

The whole strip hides while the game is playing, so the running game gets a clean view.

There is a second, separate toolbar: the strip **overlaid on the viewport itself**. It carries Select / Move / Rotate / Scale, the session actions, the snap pills and the display dropdowns — everything that used to be the viewport's group in the shared strip — plus the per-view controls at its right end. `renzora_ember::toolbar::register_viewport_tool_trailing(build)` appends a widget between those two groups, right-aligned and ahead of the view-angle dropdown, primary viewport only; it's how the shell mounts the **Play** control there. That registry is a static rather than a resource because the in-viewport strip is built from a panel-content closure that receives only `Commands` + fonts, with no `World` in scope. The strip's *tools* hide during play; the trailing widgets do not, because Stop is the one control that has to outlive the toolbar it sits on.

That strip is an `arrange_row` (see *Custom Widgets*): it wraps to a second line when a viewport is too narrow for everything, and each group carries a grip you can drag to reorder the bar.

The API lives on `App` (trait `renzora_ember::toolbar::PanelToolbarExt`):

```rust
use renzora_ember::toolbar::PanelToolbarExt;

// Simplest: an icon button. `on_click` runs (deferred) with `&mut World`.
app.register_panel_toolbar_button(
    "material_graph", "floppy-disk", "Save material",
    |w| { /* save the open material */ },
);

// Full control: a builder closure with `&mut Commands` + `&EmberFonts`, so you
// can spawn ANY ember widget — dropdown, slider, input, checkbox, toggle
// switch, color picker — and wire reactivity with the `bind_*` helpers
// (e.g. read `EditorSelection` to react when a mesh is picked).
app.register_panel_toolbar("blueprint_graph", |commands, fonts| {
    let row = commands.spawn(Node { /* a Row */ ..default() }).id();
    // … add_node / auto_layout / apply buttons …
    row // the item's root entity
});

// Show one item's group for SEVERAL panels (any-active). The main viewport
// toolbar uses this so it shows for every viewport slot:
app.register_panel_toolbar_multi(
    &["viewport", "viewport-2", "viewport-3", "viewport-4"],
    |commands, fonts| build_viewport_header(commands, fonts),
);
```

Notes:
- The strip is **centered** by the host. If several panels are visible at once, their groups concatenate (in registration order) into that one centered cluster.
- Built-in users of the strip: the **viewport header** (view/mode/snap/display/camera + gizmo tools + the 3D/2D/UI selector — shown for any of the 4 viewport slots), the **material graph** (Add Node / Apply / Fit / Center / Zoom + material picker), and the **blueprint graph** (Add Node / Auto Layout / Apply).
- Items are built **once** when the shell spawns (and on a theme rebuild). Register at plugin-build time, before the chrome mounts.
- A builder that needs a resource snapshot *at build time* (e.g. `font_picker`, which snapshots the `FontRegistry`) can't read it from this signature yet — bind reactively instead, or open an issue to widen the builder.

### Or just put it in your panel

A toolbar is only UI, so you don't *have* to use the strip — an editor can build the same ember widgets directly inside its own panel. The **code editor** (its font-size + Minimap/Whitespace bar, below the tab strip) and the **UI canvas** (its align/grid/snap/zoom bar) do exactly that. The widgets and their click systems behave identically either way — systems query by marker component, not by tree position. Use the strip when the toolbar belongs in the shared chrome below the tabs; build it in-panel when it belongs to that panel's own layout.

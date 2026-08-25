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

Pick the version so it changes when the *rendered row content* changes, and no more often. The material picker hashes `(absolute path, is_current)` per row and deliberately **not** the thumbnail handle: thumbnails stream in asynchronously, so including them would invalidate the list on almost every frame during a load and give back nothing. Converting that picker from a full rebuild to a versioned snapshot took `text_system`'s worst frame from 14.54 ms to 2.58 ms and removed every frame over 25 ms.

> The material picker itself has since dropped `virtual_scroll` for a plain `keyed_list_tokened` — it caps at twelve tiles and has no scroll area of its own, so there is no window to compute. The versioned token is the part that mattered; virtualization only pays once a list is longer than the screen.

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

"Visible" spans every dock area, not just the workspace one: `panel_active` counts a panel as active if it is the live tab in the primary [`Dock`], in the global bottom panel ([`FixedDock`]), or in any floating dock window. A panel dragged into the bottom panel keeps updating, as it must — it is on screen.

#### Dock areas

The editor renders three kinds of dock area, all driven by the same [`DockTree`] model and reconciler:

| Area | Tree | Notes |
|---|---|---|
| Primary | `Dock::tree` | The active workspace's layout. Swapped wholesale on a workspace switch. |
| Global bottom panel | `FixedDock::tree` | One tree shared by every workspace, held outside the workspace layouts so a switch can't disturb it. Occupies the bottom of the dock region either overlaid on the primary area (default) or in-flow beneath it — the user's choice, see below. |
| Floating window | `DockWindowState::tree` | One per torn-off OS window. |

An area can be declared **non-movable**, which the bottom panel is. That drops the whole-leaf drag grip from its tab bars — the leaf is pinned, though individual tabs still drag in and out — and marks the tab bar's filler [`FixedAreaHeader`], which the consumer can use as a drag surface of its own (the shell resizes the bottom panel from it).

The bottom panel's **Overlay / Layout** mode (`BottomDockMode`, persisted in `layout.json`) is a property of the shell's chrome, not of the dock model: both modes keep the panel occupying the bottom `height` px of the dock wrapper, and only the panel node changes — `PositionType::Absolute` pinned to the wrapper's bottom edge, or `Relative` as an in-flow row of the wrapper's column, where the dock area's `flex_grow` hands it the remainder. That is why the absolutely-placed resize band and corner buttons need no mode-specific arithmetic, and why nothing about the tree, the reconciler or `panel_active` cares which mode is on.

> A dock area overlaid on another needs a `GlobalZIndex` tier, not merely a later sibling. `GlobalZIndex` lifts a node into the **root** stacking order, and the node-graph widget puts its canvas and nodes on one — so a graph panel will paint straight over an untiered overlay regardless of sibling order. The bottom panel sits at 100: above panel content, below the dock's drop overlay (200), modals and dropdowns (500), menus (700) and drag ghosts (1000).

#### The regression guard

`crates/renzora_ember/tests/panel_systems_gated.rs` fails the build if a file that calls `register_panel_content` also uses a bare `app.add_systems` without gating. It checks for *gating*, not for a particular style — a per-system `.run_if(panel_active(..))` still passes, so correctly-gated older panels aren't churned.

This is a test rather than a convention because the convention lost: a survey found 1283 of 1535 per-frame system registrations running unconditionally in an idle editor. Nobody skipped the gate on purpose — most authors never knew it existed.

If your systems genuinely must run while the panel is hidden, put the exemption **at the call site**:

```rust
// panel-systems-ungated: poll_store drains in-flight async requests
app.add_systems(Update, (poll_store, ..));
```

Deliberately not a path list in the test: a list rots the moment a file is renamed, and it puts the justification where nobody looks while editing the code it excuses. At the call site the reason travels with the code and can't go stale — delete the systems and the marker goes with them.

## Press guards — what else sits over your content

A panel that acts on a press in its *empty* space — clear the selection, start a rubber-band sweep, begin a drag — usually decides "was this press mine?" geometrically, by testing `RelativeCursorPosition::cursor_over` on its content node. That test is pure geometry: it knows the cursor is inside your rect, not whether something was drawn on top of it. Two editor widgets are drawn on top of every panel's rect, so a press on either one reads as a press on your empty content unless you say otherwise:

| Resource | Set while | Import |
|---|---|---|
| `ScrollbarBusy` | pressing a visible scroll track, or mid-thumb-drag | `renzora_ember::widgets::ScrollbarBusy` |
| `ResizeBusy` | the button is held after a press on any `ResizeHandle` — a dock divider, the global bottom panel's grip band, a floating dock window's edge zone, the shell window's edge grips | `renzora_ember::resize::ResizeBusy` |

The divider is the surprising one. Its visible line is 1px but its grab strip is 11px, so the handle deliberately **overhangs about 5px into the panes on either side** — which is inside your content rect. The bottom panel's grip is the same shape one tier up: it straddles the panel's own top edge, so it hangs over the workspace above — which is how dragging the panel upward used to arm the 3D viewport's selection box. Take both flags and bail on a press when either is set:

```rust
fn my_empty_press(
    mouse: Res<ButtonInput<MouseButton>>,
    content: Query<&RelativeCursorPosition, With<MyContentArea>>,
    scrollbar: Res<ScrollbarBusy>,
    resizing: Res<ResizeBusy>,
) {
    if !mouse.just_pressed(MouseButton::Left) || scrollbar.active() || resizing.active() {
        return;
    }
    // ... your press action
}
```

Both are refreshed in `PreUpdate` after the pointer state settles, so they're already correct when your `Update` system reads them, whatever the system order. `ResizeBusy` stays set for the whole gesture, not just the press frame — a resize drag continues after the cursor has left the handle, and an OS window resize takes the pointer away entirely.

Read `ResizeBusy` itself, not a downstream mirror of it. The viewport's "is the pointer over me" flag, for instance, is recomputed in `Update` and can be a frame behind — and the frame it is behind for is the press frame, the only one that matters.

The flag is defined in the contract crate, so crates outside the UI stack can obey it without linking `renzora_ember` — `renzora_gizmo`'s scene picking is one, which is why dragging a dock seam no longer starts a selection box or grabs a transform handle. Import it from `renzora::core::resize` there, and take it as `Option<Res<ResizeBusy>>`: it only exists once ember's plugin has registered it, so a runtime without the editor UI must still run. Two helpers make that shape readable — `resize_in_flight(&resizing)` for a mid-system guard, and the `not_resizing` run condition for a system that does nothing but act on a press (or one already sitting on Bevy's 16-parameter ceiling, where one more `Res` would stop it being a system at all):

```rust
use renzora::core::resize::not_resizing;

app.add_systems(Update, my_press_system.run_if(not_resizing));
```

A panel that hit-tests with `Interaction` is safe from resize handles specifically, because `ResizeHandle` also forces `FocusPolicy::Block` onto the node it marks, so the press stops there. Don't assume that's the default: in Bevy 0.19 `FocusPolicy` defaults to `Pass`, and an unblocked node marks *every* node under the cursor `Pressed`, not just the front one. Scroll tracks carry no `Interaction` at all and so block nothing — `ScrollbarBusy` is the guard there either way. If you build a widget that owns a press and sits over other content, give it `FocusPolicy::Block` (and `ResizeHandle`, if it resizes something).

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

Registering a panel does **not** force it into a layout. The metadata makes it available in the dock tab strip's **+** (Add-Panel) picker, grouped by `category`; the user docks it where they like. Built-in workspaces (Scene, Scripting, Blueprints, Animation, Materials, Particles, Debug, Gallery) are eight separate `DockTree`s the shell ships and the user can reorder, rename, and add to. The live layout persists per workspace, and the whole set (every workspace's tree + the active index) is serialized to `~/.renzora/layout.json` so split sizes, panel placement, and active tabs survive a restart. On launch the shell restores that file and appends any built-in workspace the saved set predates; deleting the file resets to the shipped defaults.

If you want a panel docked by default, add it to a workspace layout rather than relying on the picker; otherwise the **+** picker is how users bring it in (this is exactly what the tutorial's throwaway "Demo Panel" does — registered but deliberately not pre-docked).

> Editor panels only exist in the editor session. They live in editor-scope plugins linked into the `renzora_editor` bundle (or shipped as a `--editor` distribution plugin). When the bundle is absent — the shipped game — none of this code runs, because `PluginScope::Editor` plugins are never installed into a runtime-only binary.

## Panel toolbars — a panel's tools live *in* that panel

There is no shared toolbar strip. There used to be: one row under the top bar
whose contents followed whichever panel was the active dock tab, fed by
`register_panel_toolbar*`. It's gone, along with `PanelToolbars`,
`PanelToolbarExt` and `build_toolbar_host`.

**Build your toolbar inside your panel's content.** That's what the code editor
always did — its tab strip and its font-size / minimap / whitespace row are both
children of the `code_editor` panel — and it's where the material and blueprint
graphs' toolbars moved. A row of controls for one panel, rendered somewhere else,
has to answer "is my panel the visible tab?" every frame, and answers it in a bar
that sits the same distance from every panel, the one it acts on included:

```rust
fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands.spawn(Node {
        flex_direction: FlexDirection::Column, ..default()
    }).id();

    let toolbar = build_toolbar(commands, fonts);   // your own row
    let canvas = node_graph_view(commands, fonts);
    commands.entity(root).add_children(&[toolbar, canvas.viewport]);
    root
}
```

Nothing else is needed: the panel is only built when it's docked, and only
rendered when it's the visible tab, so visibility comes free.

### The viewport is the exception, and why

The viewport carries two mount points other crates can push widgets into, both
in `renzora_ember::toolbar`:

| Registry | Where it mounts |
|---|---|
| `register_viewport_tool_trailing(build)` | The right-hand end of the in-viewport tool strip — the one with Select / Move / Rotate / Scale on it |
| `register_viewport_top_strip(build)` | A full-width bar inside the primary viewport panel, under that tool strip |

These exist for a dependency reason, not a layout one. The editor **shell** needs
to put things into the viewport — the scene tabs, historically Play — and it
cannot call the viewport to do it: `renzora_shell` depends on `renzora_viewport`,
not the other way round. The shell registers a builder; the viewport panel builds
whatever it finds; no edge points the wrong way.

Both are `static` registries rather than resources because the viewport panel is
built from a panel-content closure that receives only `Commands` + `EmberFonts`,
with no `World` in scope to read a resource from. Register at plugin-build time,
before the chrome mounts.

The in-viewport tool strip is an `arrange_row` (see *Custom Widgets*): it wraps
to a second line when a viewport is too narrow for everything, and each group
carries a grip you can drag to reorder the bar. Its *tools* hide during play; the
trailing widgets do not, because Stop is the one control that has to outlive the
toolbar it sits on.

### Or just put it in your panel

A toolbar is only UI, so you don't *have* to use the strip — an editor can build the same ember widgets directly inside its own panel. The **code editor** (its font-size + Minimap/Whitespace bar, below the tab strip) and the **UI canvas** (its align/grid/snap/zoom bar) do exactly that. The widgets and their click systems behave identically either way — systems query by marker component, not by tree position. Use the strip when the toolbar belongs in the shared chrome below the tabs; build it in-panel when it belongs to that panel's own layout.

## If your panel renders a 3D preview: claim a render layer

Plenty of panels show a little 3D scene of their own — the material and shader
previews, the particle preview, the animation studio, the import dialog's model
view, the asset browser's thumbnail captures. Each of those is a real camera, its
own lights, and often a floor or a backdrop, all spawned into the **same `World`**
as the actual scene and as every other preview.

The only thing keeping them apart is a bevy `RenderLayers` index. Every rig puts
its camera and all of its content on one layer, so a camera sees its own scene
and nothing else.

Those indices are handed out by a registry in the contract crate,
`renzora::core::viewport_types` — `MATERIAL_PREVIEW_LAYER`,
`PARTICLE_PREVIEW_LAYER`, `MODEL_THUMBNAIL_LAYER`, and so on, plus a comment
listing which numbers are still free. **Take your layer from there and add a
constant for your rig; don't define a private one next to your camera.** A rig
can only tell that a layer is free by checking every other rig, and the crates
that own them don't depend on each other — a private constant has nothing to
check against.

Two layers ended up double-booked exactly that way. The particle preview and the
`.material` thumbnail capture both sat on 7, so the particle preview's
checkerboard floor and its directional light rendered into every material
thumbnail in the asset browser; the material preview and the model thumbnail
capture both sat on 8. Sharing a layer never errors — it silently draws one rig's
contents into the other's, and because most rigs park their geometry at the world
origin, that shows up as another preview's mesh embedded in yours.

```rust
use renzora::core::viewport_types::MY_PREVIEW_LAYER;

commands.spawn((
    Camera3d::default(),
    Camera { target: RenderTarget::Image(image.into()), is_active: false, ..default() },
    RenderLayers::layer(MY_PREVIEW_LAYER),
    // Skip the editor's environment/skybox takeover systems, which would
    // otherwise attach a scene env-map to your offscreen camera.
    IsolatedCamera,
    HideInHierarchy,
    EditorLocked,
));
```

`IsolatedCamera`, `HideInHierarchy` and `EditorLocked` belong on every entity in
the rig: they keep it out of the hierarchy panel, out of selection, and out of
the systems that push scene lighting onto cameras.

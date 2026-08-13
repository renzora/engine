# Custom Widgets

Build reusable UI components for editor panels and game HUDs with `renzora_ember` — plain bevy_ui builder functions, WGSL `UiMaterial` widgets, and the markup attribute kernel.

## Two ways to build a widget

Renzora's UI lives in one crate, `renzora_ember`, used by both the editor and exported games. There are two complementary ways to produce a widget:

1. **Rust builder functions** (`renzora_ember::widgets`) — a function that spawns one or more `bevy_ui` entities and returns the root `Entity`. This is how editor panels and Rust-driven HUDs are built. Registered by `WidgetsPlugin` (part of `EmberPlugin`).
2. **Markup** — author widgets declaratively in hot-reloadable `.html` files; the markup loader (`MarkupPlugin`) spawns the same `bevy_ui` entities for you, plus an attribute **interaction kernel** (`toggle=`, `drag_value=`, `fill=`, `vector=`). See *Markup & Templates* for the authoring format; this page covers the kernel attributes and how they relate to the Rust widgets.

Both paths produce ordinary `bevy_ui` entities (`Node`, `Text`, `BackgroundColor`, …). There is no retained widget VM and no per-frame re-layout pass beyond Bevy's own.

> ⚠️ **egui is gone.** `egui`/`bevy_egui` were removed from the engine entirely. There is no `egui::Ui`, no `egui::Widget` trait, no `ui.add(...)`, and no `EditorTheme`. Any example showing `impl egui::Widget` or `ui.label(...)` is a dead API — ignore it. Widgets are bevy_ui entities built from `Commands`.

## The builder-function pattern

A widget is a `pub fn` that takes `&mut Commands` (and, when it needs text, fonts), spawns the entity tree, and returns the root. The canonical signature:

```rust
use bevy::prelude::*;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{accent, border, rgb, text_primary};

/// A labelled pill that shows a status color.
pub fn status_pill(commands: &mut Commands, fonts: &EmberFonts, label: &str, ok: bool) -> Entity {
    let color = if ok { accent() } else { (200, 80, 80) };
    let pill = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BorderColor::all(rgb(border())),
            Name::new("status-pill"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(color)),
        ))
        .id();
    commands.entity(pill).add_child(text);
    pill
}
```

Key conventions, matching every built-in widget:

- Colors come from the theme palette in `renzora_ember::theme`: accessor functions like `accent()`, `border()`, `card_bg()`, `window_bg()`, `text_primary()`, `text_muted()`, `tab_active()` each return an `(u8, u8, u8)` triple; `rgb(...)` turns one into a bevy `Color`. Don't hard-code hex.
- Text needs a font handle. `EmberFonts` carries three: `fonts.ui`, `fonts.phosphor` (icons), `fonts.mono`. `ui_font(&handle, size)` builds a `TextFont`; `icon_text(commands, &fonts.phosphor, name, color, size)` spawns a Phosphor glyph.
- Give every node a `Name` — it's how markup bindings, the dock, and debug tooling find it.
- For interactivity, add `Interaction::default()` and either a marker component your own system reads, or one of ember's existing markers (e.g. `EmberButton`).

To call a builder from an editor panel, use the `build` closure of `register_panel_content` (see *Building Editor Panels*), which hands you exactly `(&mut Commands, &EmberFonts)`:

```rust
app.register_panel_content("my_panel", true, |commands, fonts| {
    let root = commands.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    }).id();

    let ok  = status_pill(commands, fonts, "Connected", true);
    let bad = status_pill(commands, fonts, "Offline", false);
    commands.entity(root).add_children(&[ok, bad]);
    root
});
```

### Blocking the pointer

**A widget that owns a press needs an explicit `FocusPolicy::Block`.** Since Bevy 0.19, `Node` *requires* `FocusPolicy`, and that component's `Default` is `Pass` — so a node you never gave a policy to does not capture the pointer. bevy_ui walks every node under the cursor front-to-back and marks them all `Interaction::Pressed` until it meets one that blocks, which means a plain button hands its press to whatever sits behind it, **its own ancestors included**:

```rust
commands.spawn((
    Node { /* … */ },
    Interaction::default(),
    FocusPolicy::Block,   // ← without this, the press also lands on the row,
    MyButton,             //   card or panel this button is sitting inside
));
```

This is the opposite of the pre-0.19 default, so it reads as "extra ceremony" and is easy to leave off. What it costs when you do: the splash launcher's ✕ removed a recent project *and* opened it, because the row behind the ✕ is the "open this project" hit-box (GH #82); a press on any splash button also reached the whole-window drag handle underneath it; the tutorial card's Skip button also started dragging the card.

Two things that are **not** affected, and shouldn't be "fixed" with `Block`:

- **Layout scaffolding.** Wrappers, spacers, text and icons inside a button want `Pass` (their default) — that's how a click on a button's label reaches the button. Marking them `Block` breaks the widget.
- **`RelativeCursorPosition`.** Bevy fills `cursor_over` for *every* node containing the pointer, whoever captures the press. Hover *visuals* on a container that holds its own interactive children should read `cursor_over`, not `Interaction` — otherwise the container flattens out the moment the cursor crosses onto a child that blocks. (Ember's `correct_pointer_state` already clears `cursor_over` for anything clipped by a scroll area or covered by an overlay, so it stays trustworthy.)

## The built-in widget library

`renzora_ember::widgets` ships ~80 widget modules, each a builder fn (or several) plus the interaction system that animates its state. They are registered by `WidgetsPlugin`. Import them with `use renzora_ember::widgets::*;`. A representative selection:

| Category | Builder fns |
|---|---|
| Buttons | `button`, `icon_button`, `icon_label_button` |
| Toggles | `checkbox`, `toggle`, `toggle_switch`, `radio`, `segmented` |
| Numeric | `slider`, `drag_value`, `spin_slider`, `stepper`, `knob`, `fader`, `range`, `xy_pad` |
| Selection | `dropdown`, `dropdown_compact`, `dropdown_with_icons`, `multi_select`, `search`, `tags_input` |
| Text entry | `text_input`, `textarea`, `floating_label`, `input_group`, `validation` |
| Color / curves | `color_picker`, `gradient` editor, `curve` editor |
| Data viz | `gauge`, `line_chart`, `bar_chart`, `sparkline`, `line_chart_live`, `waveform`, `vu_meter`, `mixer` |
| Containers | `card`, `section`, `accordion`, `collapsible`, `tabs`, `divider`, `scroll_area` |
| Data display | `table`, `tree`, `grid`, `avatar`, `chip`, `badge`, `list_group`, `timeline_view` |
| Pickers | `folder_picker`, `font_picker`, `asset_slot` |
| Overlays | `modal`, `popover`, `tooltip`, `popup`, `menu`, `screen_menu`, `menu_submenu`, `context_menu`, `toast`, `alert` |
| Navigation | `navbar`, `breadcrumb`, `pagination` |
| Editors | `node_graph`, `code_editor`, `property_row`, `vec3_edit` |
| Feedback | `progress`, `spinner`, `skeleton` |

Most take `&mut Commands` plus their initial value or fonts. Examples:

```rust
let b = button(commands, &fonts.ui, "Apply");          // -> Entity
let c = checkbox(commands, true);                       // initial checked
let s = slider(commands, 0.5);                          // value in 0..1
let k = knob(commands, 0.25);
let g = gauge(commands, fonts, 0.8);                    // circular dial + % label
let chart = line_chart(commands, fonts, &samples);
let w = waveform(commands, &amplitudes);
let ng = node_graph(commands, fonts);
```

> The **Gallery** workspace in the editor is a living catalog of this widget set — open it to see every widget rendered live with its current theme.

### Faders and VU meters run either way

`fader` and the `vu_meter*` builders are vertical, which is what a mixing desk wants. Each also has a horizontal twin — `fader_horizontal`, `vu_meter_bound_horizontal` — for layouts where height is the scarce axis rather than width (the Mixer panel's horizontal channel rows use both):

```rust
let f = fader_horizontal(commands, 0.6);                       // travels left -> right
let m = vu_meter_bound_horizontal(commands, move |rx| level(rx));
```

They are one control mirrored, not two widgets: orientation is a field on the widget's component, and every dimension, the drag axis and the fill's anchor edge are picked from it. Both ship at 120px along the travel axis and 24px (fader) / 14px (meter) across it; stretch the travel axis by overriding `Node` (`flex_grow` plus `width: Auto` / `height: Auto`) — the fills and hit-testing are percentage-based, so any length works.

### Tooltips (`HoverTooltip`)

Tooltips are a **global layer**, not per-widget bubbles: insert `renzora_ember::widgets::HoverTooltip::new("Label")` on any entity that has `Interaction`, and hovering it shows the shared cursor-following bubble after a short delay. Do **not** spawn a bubble node as a child of your widget — bevy_ui clips absolutely-positioned children by every scrolling/clipping ancestor, so a per-widget bubble silently disappears inside panels (`GlobalZIndex` changes paint order, not clipping). The shared bubble is a parentless root node with `Pickable::IGNORE`, so nothing clips it and it never steals hover. The `tooltip(...)` wrapper builder still exists for wrapping non-interactive content, and forwards to the same mechanism. Viewport toolbar buttons, panel toolbar buttons, and the inspector's component rail all use it.

### Dropdowns & popups — don't hand-roll them (`dropdown`, `Popup`)

Two builders cover almost every "click a thing, a panel appears" case, and using them is not just about saving code:

- **A selection** → `dropdown(commands, fonts, &options, selected)`, or `dropdown_compact(.., width)` for a toolbar row (fixed width, tighter padding, 22px tall so it lines up with icon buttons). Both carry a `Bound<usize>`, so `bind_2way(commands, dd, get, set)` wires the box to a resource in both directions. `dropdown_with_icons` takes `(icon, label)` pairs. To hide options that don't apply right now, flip `Node.display` on the rows whose `EmberDropdownOption.dropdown` is your box — `value` stays a stable index, so hiding never renumbers the rest.
- **A panel of mixed content** (switches, sliders, sections) → `popup_panel(commands, &rows)` for the surface, `icon_popup_trigger(commands, fonts, icon, panel)` for a toolbar-sized icon+caret trigger, and `popup_anchor(commands, trigger, panel)` for the wrapper you add to your layout. Add your own marker components to the trigger for styling. `popup_panel_aligned(.., PopupAlign::Left)` when the trigger sits at the *start* of a strip and a right-aligned panel would grow off-screen. For a plain button or icon trigger with one content node, `popover` / `labeled_icon_popover` / `icon_popover` wrap the same machinery in one call.

Ember toggles the panel on trigger clicks, closes it on an outside click, and flips it above the trigger when opening below would run off the window.

The reason to reach for these rather than rolling your own trigger + panel: **ember tags every popup panel and dropdown menu as an `OverlaySurface`**, which is what stops clicks, scrolls and hover from leaking through to whatever is behind the panel. A hand-rolled panel is invisible to that routing, so a click on one of its rows *also* lands in the viewport underneath (selecting an object, starting a box-select) and the viewport's crosshair cursor shows straight through the panel. That was the bug in the viewport toolbar's hand-rolled dropdowns, and — because it carried its own toggle component instead of `Popup` — in `popover` itself. If you must build a floating surface by hand, spawn it with `OverlaySurface` and a `RelativeCursorPosition`.

### Floating menus & submenus (`screen_menu`, `menu_submenu`)

A right-click menu is a `screen_menu(commands, x, y)` at the cursor: ember keeps it on-screen, blocks pointer pass-through, and closes it when you click outside. Fill the entity it returns with `menu_item` / `menu_item_styled` / `menu_header` / `menu_sep` rows — each item carries a `Fn(&mut World)` closure that runs when it's clicked, after which the menu closes itself.

For a nested list, use `menu_submenu(commands, fonts, icon, label)`. It returns `(row, content)`: add `row` to the parent menu like any other item, and fill `content` with items — including further submenus, which nest to any depth. The panel opens on hover, follows the row as the menu scrolls, and flips to the other side when it would run off the window. `menu_submenu_styled` takes an icon color for color-coded rows; pair it with `category_color(name)` to tint a category the same accent the search overlay gives it.

```rust
let menu = screen_menu(&mut commands, cursor.x, cursor.y);
let (row, content) = menu_submenu(&mut commands, &fonts, "lightbulb", "Lighting");
let item = menu_item(&mut commands, &fonts, "lightbulb", "Point Light", move |w| spawn_light(w));
commands.entity(content).add_children(&[item]);
commands.entity(menu).add_children(&[row]);
```

Every row — `menu_item`, `menu_header`, `menu_submenu` — shares one set of metrics (`MENU_ICON`, `MENU_TEXT`, `MENU_PAD_X/Y`, `MENU_GAP` in `popup.rs`): a glyph larger than the label on a thin row, so you pick a row by its icon and a long list still fits on screen. Change them there, not per builder.

Three rules worth knowing when you touch this machinery:

- **Never order a system against `screen_menu_dismiss`.** Any `.before`/`.after` on it makes bevy insert an `ApplyDeferred` in front of it, which flushes *every* pending command — including the menu a panel just spawned for the very press being handled. Dismiss would then see a brand-new menu while the opening click is still `just_pressed` and close it on the frame it appeared.
- **A submenu panel is a parentless root node**, positioned in window pixels and despawned by an `on_remove` hook on its row. It can't be a child of the row: a `screen_menu` keeps its items in a height-capped scroll area, and a scroll area clips its children, so the panel would be sliced off at the menu's edge. Same clipping trap tooltips avoid — and the reason `GlobalZIndex` alone never fixes a vanishing popup.
- **Read a UI node's placement from `UiGlobalTransform`, never `GlobalTransform`.** Bevy 0.19's layout writes the former (an `Affine2` whose `translation` is the node's **centre**, in *physical* px — multiply by `ComputedNode::inverse_scale_factor()` for the logical px that `Val::Px` speaks). A UI node's `GlobalTransform` is left at the origin, so anything positioned from it lands in the window's top-left corner.

### Capped tab strips (`overflow_strip`)

`overflow_strip(commands, budget, name)` is a horizontal strip that refuses to outgrow its width budget: items that don't fit are hidden and folded into a **caret button** (`⌄`) at its end, which opens a menu of exactly those items. It returns `(row, items)` — mount `row`, add your items (or point a `keyed_list` at `items`).

```rust
let (row, items) = overflow_strip(
    commands,
    OverflowBudget::Fill { measure: bar, reserve: 66.0 },
    "doc-tab",
);
keyed_list(commands, items, doc_tab_snapshot);
// …and on each item, so it can be reached once it folds:
commands.entity(tab).insert(
    OverflowEntry::new("file", name, move |w| activate(w, id))
        .on_drag(move |w| start_drag(w, id)),   // optional: draggable out of the menu
);
commands.entity(tab).insert(OverflowKeep);      // active item: never folds
```

Two budgets. `OverflowBudget::Fill { measure, reserve }` takes whatever `measure` was laid out to, less `reserve` for the buttons sharing that container — use it wherever the strip has a container that fills its slot, so nothing folds while there's still room. `OverflowBudget::Fixed(px)` is a constant cap, for a strip with no container of its own to measure (the centered workspace ribbon). Either way the item container hugs its content, so a trailing button (the document tabs' `+`) stays glued to the last item instead of stranding itself at the far edge. Where the layout *around* a strip must not move, fix the width of the strip's container rather than the strip.

The fold is computed from each item's **last width measured while visible**, cached on the item, not from live layout — a hidden node measures zero, so folding an item would shrink the measured content, unfold it, and oscillate every frame. Widths are remembered, so the decision doesn't depend on what's currently folded and settles in one pass.

A **new** item has no remembered width, and the obvious answer — leave it in the flow for a frame so it can be measured — is one frame of an item visibly sitting in a strip it doesn't fit in before folding away. So an item is instead spawned `position: absolute` and `Visibility::Hidden`: taffy still measures it, invisibly and without pushing its neighbours, and `overflow_fit` puts it back the instant it has a width. The strip also caches widths **by `OverflowEntry::label`**, which survives the rebuild a `keyed_list` performs when a row's content changes — the row is a new entity, so its own cached width is gone, and without the label cache every tab activation would re-measure (and so blink) every row the rebuild touched. With it, a rebuilt row is decided in the same frame it was built. `overflow_fit` is ordered after `run_keyed_lists` for exactly that reason.

`OverflowEntry::on_drag` makes the item's row in the caret menu draggable: press and move past a small threshold and the menu closes and the handler runs (the host takes over the drag), press and release without moving and the normal `action` runs. Setting it changes *when* the click fires — an ordinary menu row acts on press, which can't work when the press is also how a drag starts. The document tabs use it so a folded tab can be dragged back out into the strip.

### Wrapping, rearrangeable toolbars (`arrange_row`)

`arrange_row(commands, name)` is a toolbar row that **wraps** rather than hiding what it can't fit, and whose groups can be dragged into a different order.

```rust
let bar = arrange_row(commands, "vp-toolbar");
let holders = arrange_row_items(commands, fonts, bar, &[(tools, "tools"), (snaps, "snaps")]);
// Bind visibility on the returned holders, not on the groups: hiding a group
// directly would leave its grip behind.
```

Each entry is a group plus the stable key its position is saved under. The row publishes its current order on itself as `ArrangeOrder(Vec<String>)`, rewritten after every drop — save that list and write it back to restore an arrangement; the row reorders to match and leaves keys it doesn't recognise where they already are. (The editor mirrors it into `ViewportSettings.toolbar_order`, which rides along to `project.toml`.)

Each group gets a holder with a small **grip** on its left. A holder is one flex item, so a group never splits across lines — one that doesn't fit moves down whole. Hovering the grip highlights the group it belongs to; dragging it carries the group under the cursor with a blue marker opening at the drop point, so the neighbours visibly shift aside. Only the grip starts a drag, so the controls stay clickable at all times — there's no edit mode to enter and leave.

Three earlier versions of this widget solved "more controls than bar" by taking controls *away* — into a dropdown, a floating panel, then a tray under the bar. Each needed a measure-and-fold pass with its own oscillation traps, and each meant the control you wanted might not be on screen. Wrapping has neither problem: the bar gets taller, everything stays visible, and flexbox does all the work. Pass **clusters**, not individual buttons — a group is what moves and what wraps.

Two rules it depends on, both learned by crashing:

- **Never orphan a live node to carry it.** The dragged holder is `position: absolute` but stays parented. Removing its `ChildOf` to float it makes it an untargeted layout root mid-frame and panics taffy inside `ui_layout_system`.
- **Nothing in the chain may clip.** The dragged holder travels outside its container, and bevy_ui clips absolutely positioned descendants like everything else — the trap that eats tooltips and submenu panels.

The drop index is counted against each group's own box in reading order (line, then x), never as a fraction of the row's width: once a row wraps, that fraction says nothing about where the groups actually are.

### Draggable floating cards (`drag_grip`, `DragHandle`)

A floating card that the user can shove out of the way needs a **handle that moves its parent**, not a node that moves itself. `drag_grip(commands, &fonts.phosphor, target)` spawns the conventional six-dot grip already wired to drag `target`; add it to your header and you're done.

```rust
let card = commands.spawn(( /* absolute-positioned card */ )).id();
let grip = drag_grip(&mut commands, &fonts.phosphor, card);
commands.entity(header).add_child(grip);
```

Insert `DragHandle::new(target)` yourself if you'd rather drag by a whole title bar than by a grip, and `.with_margin(px)` to change how much of the card must stay reachable on screen (24px by default).

This is **not** markup's `draggable="true"` ([`Draggable`](#the-markup-interaction-kernel)), which moves the node it's on — right for a game-UI element you drag directly, wrong for a window, where tagging the card would make every press on it (a button, a list row, a text selection) start a drag.

Three things it handles that `Node.left += delta` does not:

- **Anchor handover.** Cards are usually pinned with `right`/`bottom`, leaving `left` as `Val::Auto`. Reading that as zero teleports the card on the first drag, so the handle resolves the target's real on-screen rect from `UiGlobalTransform` instead, then writes `left`/`top` and clears the opposite pair — the two anchor sets fight if you leave both set.
- **Staying reachable.** A card flung past the window edge takes its close button with it, so the position is clamped to keep a margin on screen.
- **Release anywhere.** The drag ends on mouse-button release, not on `Interaction`, so moving faster than the handle follows doesn't silently drop it.

Used by the onboarding tutorial's card, which parks bottom-right — occasionally right on top of the thing a step is pointing at. It puts the handle on the **whole header strip** rather than the grip alone (a 13px glyph is a fussy target for something you reach for precisely when the card is in your way); the Skip and close buttons inside that header carry `FocusPolicy::Block` so their press doesn't also start a drag — see *Blocking the pointer* below.

### Folder picker (`folder_picker`)

`folder_picker(commands, fonts, root, selected, max_depth)` is the shared "where should this land?" control: the project's own directory tree as a bordered, scrolling list of rows, one of them selected. It returns a single box entity that **flex-grows**, so drop it into an overlay body between the fixed content above and the buttons below and it fills the leftover height.

```rust
let picker = folder_picker(commands, fonts, &project_root, &default_dest, 2);
// …later, when your overlay is confirmed:
let dest = pick.path().unwrap_or(&default_dest);   // pick: Res<FolderPick>
```

The pick lives in **one `FolderPick` resource**, not in each caller's state. That's deliberate — a picker only ever appears inside a modal overlay, so at most one is on screen, and a shared resource is what lets the selected-row highlight be a plain reactive `bind_bg` rather than click plumbing rewritten per caller. Seed it by passing `selected`; read it with `FolderPick::path()`.

`root` is always the first row (so "top level" needs no scrolling), `max_depth` bounds the walk below it, and the scan skips dotfolders, `target/` and `node_modules/` and caps out at 300 rows so a huge project can't stall the overlay opening. `folder_dirs(root, max_depth)` exposes the same walk if you want to render rows yourself.

Used by the marketplace's **Install into** confirmation and the Hierarchy's **Attach ▸** overlay.

### Code editor (`code_editor`)

The `code_editor` widget is a monospace, syntax-highlighted, editable text view. It owns no document model: the host crate attaches a `CodeBindingSpec` (via `bind_code`) of closures that shuttle text in and out — `doc_key` (document identity), `load`, `store`, `make_highlighter` (a per-language tokenizer producing colored `CodeToken` runs), and an optional `font_size` (the live zoom). `renzora_code_editor` wires this to its `CodeEditorState` (open files, active tab, zoom).

**Languages.** The tokenizer (`renzora_code_editor::highlight`) covers Lua, Rhai, Rust, WGSL, Python, Shell, SQL, JSON, TOML, **BSN** (the `.bsn` scene format — `//` / `/* */` comments, `entity`/`resource` keywords, PascalCase component type paths), and **HTML** (`.html`/`.htm` markup UI — tag names, attributes, quoted values, `&entities;`, and `<!-- -->` comments that thread across lines), picked by file extension. Cross-line state (block comments, HTML comments) threads between lines as an opaque `u32` so a comment opened off-screen still colors correctly when scrolled into view.

**Colors are themed.** Every token color and editor-chrome color comes from the active theme's `[syntax]` section via ember's `SyntaxPalette` — see *Theming → Code-editor syntax colors*. Editing them in Settings → Theme recolors the open editor live.

**Editor chrome.** Each row carries its own absolute-positioned overlays, spawned back-to-front ahead of the row's gutter and text: the **current-line highlight** (`current_line`, full viewport width), **indent guides** (`indent_guide`, a vertical rule at each interior indent stop — `TAB_WIDTH` = 4 cols), the **selection** rect (`selection`), and **matching-bracket** boxes (`bracket_match`, shown when the caret is next to a bracket and has no selection; the match is found nesting-aware across lines, bounded so a huge file can't stall the render).

**Rendering is incremental.** `code_render` hashes everything a row draws — its token spans and colors, gutter number, fold state, chrome overlays, and the shared metrics/palette epoch — and a row whose hash is unchanged is left completely untouched: no despawn, no respawn, no relayout, and no text re-shaping. This matters because every keystroke dirties the editor, so a full rebuild ran at frame rate while a key was held (~500 entities per frame, which cost enough to drop the editor to ~25 FPS); an edit now rebuilds only the row it changed. Rows are appended to and popped from the tail only, keeping child order equal to visual order. The chrome overlays live on the row for the same reason — as body-level overlays they had to be rebuilt whenever any row changed.

**Sizing is zoom-aware.** All metrics — line height, gutter width, caret height, and the character advance — are derived from the live `font_size` (logical px) the host pushes through the binding (`CodeEditorState.font_size`, driven by Ctrl +/- and the Settings code-font size). There are no hardcoded pixel sizes.

**Advance is measured, not assumed.** Rather than hardcoding a 0.6em advance, a hidden probe (`code_probe`) reads the active mono font's real laid-out width from its `TextLayoutInfo` and feeds the per-font advance ratio back, so Fira Code / Source Code Pro / custom mono fonts get pixel-correct carets. The measurement is scale-invariant and tightly guarded — a bad/early reading falls back to 0.6 with no regression.

**Monospace is intentional.** Bevy 0.19's `PositionedGlyph` exposes a glyph's pixel position but *not* its source character/cluster index, so an arbitrary glyph can't be mapped back to a column — which is what proportional-font click/caret hit-testing would need across our multi-token text. Monospace keeps column ↔ pixel math exact and matches every real code editor; ligature mono fonts still work, since a ligature keeps the combined cell advance.

### Text inputs & forms (`text_input`, `textarea`, `EmberForm`)

`text_input` / `password_input` are single-line fields with full caret editing: click places the caret (measured-advance hit-testing), double-click selects all, **drag selects a range** (highlighted, and consumed by typing/Backspace/Delete/paste like an OS field), arrows/Home/End move, and Ctrl+C/X/V/A plus a right-click Copy/Cut/Paste menu operate on the selection when one exists. `textarea` shares the same component (`EmberTextInput`) but keeps Enter as a literal newline.

**Panels may write `EmberTextInput.value` directly** — e.g. clearing it after a send — and the displayed text follows automatically (back to the placeholder when emptied); a sync system watches for external value changes, so no manual `Text` update is needed.

**Forms.** Insert `EmberForm { submit }` on any container holding inputs and a submit button:

```rust
let input = text_input(commands, &fonts.ui, "Say something...", "");
let send  = button(commands, &fonts.ui, "Send");
commands.entity(row).insert(EmberForm { submit: send });
commands.entity(row).add_children(&[input, send]);
```

- **Enter** in a focused single-line input inside the container simulates a press of `submit` — the panel's existing `Changed<Interaction>` click handler fires unchanged, so there is no separate "submitted" event to wire. (The simulated press is set in `PreUpdate`, so every `Update` handler sees it regardless of system order; hidden forms — a `Display::None` ancestor — never submit.)
- **Tab / Shift+Tab** cycles focus between the form's visible inputs (wrapping, selecting the tabbed-into value). Tab also works without the marker: it falls back to the smallest ancestor subtree containing at least two inputs.

The sign-in modal, chat composer, feed comments, forum reply/new-thread, and teams create/invite forms all use this.

### Reactive values

Builders run once; dynamic values are wired through `renzora_ember::reactive`. A slider stores its value in a `Bound<f32>` so `bind_2way` can read and write it; text is driven with `bind_text`, visibility with `bind_display`, and variable-length lists with `keyed_list`. See *Building Editor Panels → Reactive content* for the full helper table — the same helpers drive widget contents.

### Scroll areas & remembered position

`scroll_view` / `scroll_view_bar` / `scroll_view_pinned` / `scroll_area` wrap content in a smooth-scrolling, auto-hiding-scrollbar viewport. Their position lives on the entity, so a view that gets despawned and rebuilt (a panel that re-spawns, the whole chrome rebuilding on a theme switch) normally snaps back to the top.

To keep the position across rebuilds, use the **keyed** variants and give the view a stable string key:

```rust
let s = scroll_view_keyed(commands, content, "hierarchy");          // flex-fill
let m = scroll_area_keyed(commands, content, 260.0, "status-theme-menu"); // capped
```

The offset is saved in the `ScrollMemory` resource under that key and restored — once the content is laid out — when an identically-keyed view spawns again. Use one **unique** key per logical list; two unrelated lists sharing a key would fight over the same saved offset.

**Wheel over a numeric field.** A `drag_value` (and the markup `drag_value=` kernel) only scrubs its value on **Shift+wheel**. A plain wheel is always handed to the enclosing scroll area, so dragging the panel scrollbar past a field never snags on it and silently changes the number — the panel scroll always wins, and value-scrubbing is an explicit opt-in gesture.

**Three gestures, one target.** The wheel, holding **↑/↓** while hovering, and **middle-click drag** (grab-the-content panning, both axes on `scroll_view_xy` views) all scroll the same view: the frontmost scroll area under the cursor, honoring modal/overlay confinement. Arrow-key scroll stands down while anything owns the arrows as caret keys (focused text input, code editor, editing drag-value). All three multiply by the `ScrollConfig` resource's `speed` — the editor's Settings panel pushes its *Scroll Speed* preference into it (ember can't read `EditorSettings`), the same one-way sync as `DragValueConfig`.

## Theming with `Styled` and `Role`

Instead of baking colors into a widget, attach a `Styled` component naming a `Role`. The `apply_theme` system (in `style::ThemePlugin`) repaints every `Styled` entity from the active `Theme` whenever the theme or the widget's state changes — no rebuild.

```rust
use renzora_ember::style::{Role, Styled, WidgetState};

commands.spawn((
    Node { /* … */ ..default() },
    BackgroundColor(rgb(tab_active())),   // starting color; apply_theme overrides it
    Interaction::default(),
    Styled::new(Role::Button),            // paints from theme.token(Role::Button)
    Name::new("my-button"),
));
```

`Role` values: `Button`, `ButtonAccent`, `IconButton`, `Input`, `Checkbox`, `Segment`, `Toggle`, `Card`, `Badge`, `Alert`, `Toast`, `Tab`, `Panel`, `Menu`. Each maps to a `StyleToken` with per-state fills (`bg`, `bg_hover`, `bg_pressed`, `bg_active`, `bg_disabled`), border colors, geometry (`radius`, padding), and text colors. Your interaction system sets `Styled.state` (`Normal`/`Hover`/`Pressed`/`Active`/`Disabled`); `apply_theme` does the painting:

```rust
fn my_button_interact(
    mut q: Query<(&Interaction, &mut Styled), (With<MyMarker>, Changed<Interaction>)>,
) {
    for (interaction, mut styled) in &mut q {
        styled.state = match interaction {
            Interaction::Pressed => WidgetState::Pressed,
            Interaction::Hovered => WidgetState::Hover,
            Interaction::None    => WidgetState::Normal,
        };
    }
}
```

The `Theme` is a `Reflect` + Serde resource loaded from project `themes/*.toml` (colors are `#RRGGBB` / `#RRGGBBAA` hex), so the editor and the exported game read the same theme. (Game-side UIs may instead use `renzora_game_ui::UiTheme` semantic tokens with the `UiThemed` marker.)

## GPU vector widgets — WGSL `UiMaterial`s

Gauges, charts, and waveforms aren't drawn with rectangles — they're painted by fragment shaders bound to `bevy_ui` `MaterialNode`s. ember ships three `UiMaterial`s, each backed by an embedded `.wgsl` file:

| Material | Shader | Drives |
|---|---|---|
| `ArcMaterial` | `widgets/gauge/gauge.wgsl` | `gauge`, `knob`, and markup `vector="arc"` / `speedometer` |
| `ChartMaterial` | `widgets/chart/chart.wgsl` | `line_chart`, `sparkline`, `line_chart_live`, markup `vector="line"` |
| `WaveMaterial` | `widgets/waveform/waveform.wgsl` | `waveform`, markup `vector="wave"` |

Each material plugin (`GaugePlugin`/`ChartPlugin`/`WaveformPlugin`) is `is_plugin_added`-guarded, because both `WidgetsPlugin` and the markup `vector` runtime register them and re-adding a `UiMaterialPlugin` for the same material would panic. You don't add them yourself — use the builder fns or markup.

> ⚠️ This used to be drawn with `vello` / `bevy_vello` (`UiVelloScene`, a `Camera2d` + `VelloView` on a `RenderLayers` layer). **vello was removed.** Everything now renders as ordinary `bevy_ui` `MaterialNode`s with `bevy_text` children for labels/readouts. There is likewise **no `renzora_gauges` crate** anymore — gauge drawing is ember's `gauge` widget (`ArcMaterial`).

### Vector widgets in markup

In `.html` markup, request a vector widget with the `vector=` attribute. The loader stamps a `VectorSpec` and the attach/sync systems pick the right material and bind `{{ }}` paths every frame:

```html
<!-- a dial bound to a script variable, with a centred readout -->
<node width="120px" height="120px"
      vector="gauge" value="{{ speed }}" min="0" max="240"
      color="#39d98a" readout="{{ speed }}" unit="km/h" />

<!-- a live line chart from a comma string -->
<node width="200px" height="80px" vector="line" data="{{ frame_times }}" />

<!-- a full speedometer composite: arc + ticks + numeric labels + needle -->
<node width="160px" height="160px"
      vector="speedometer" value="{{ rpm }}" min="0" max="8000"
      start="135" sweep="270" count="8" readout="{{ rpm }}" unit="rpm" />
```

Current `VectorKind`s (and their aliases):

| `vector=` | Aliases | Renders with |
|---|---|---|
| `arc` | `gauge`, `ring` | `ArcMaterial` (+ optional centred `readout`) |
| `bars` | `bar` | bevy_ui rectangles, one per datum |
| `line` | `chart` | `ChartMaterial` |
| `wave` | `waveform` | `WaveMaterial` |
| `speedometer` | `dial` | composite: `ArcMaterial` + `bevy_text` ticks/labels + needle + centre readout |

Common attributes: `value`, `data` (comma string, literal or `{{ path }}`), `min`/`max`, `color`, `track`, `fill`, `thickness`, `count`, `start` (deg, default 135), `sweep` (deg, default 270), `inset` (px), `len` (tick px), `readout`, `unit`, `size`/`readsize`.

> The standalone `ticks` / `labels` / `needle` primitives no longer exist as their own `vector=` kinds — they are assembled inside the `speedometer` composite.

## The markup interaction kernel

Separate from the Rust widget library, markup nodes can opt into behavior through kernel attributes (`renzora_ember::markup::widgets`). These are the declarative analogues of the Rust widgets, and their writes route through the scripting layer's `ScriptReflectionQueue` / `ScriptComponent`:

| Attribute | Component | Behavior |
|---|---|---|
| `toggle="Path.bool"` | `Toggle` | Click flips the bound boolean (checkbox / switch). |
| `drag_value="Path.num" drag_min drag_max` | `DragValue` | Drag horizontally to set the bound number (slider / scrollbar). |
| `fill="Path.num" fill_min fill_max` | `ValueFill` | Node width tracks the value's fraction of the range (slider fill / progress). |
| `toggles="name"` | `Disclose` | Click shows/hides the entity with that `Name` (dropdown / accordion / modal). |

Events (`on_press`, `on_enter`, `on_exit`, `on_spawn`, `on_change`) use bevy_hui's `OnUiPress`/`OnUiEnter`/`OnUiExit`/`OnUiSpawn`/`OnUiChange` components, which feed the firing node into a script's `on_ui(name, args, entity)` hook.

> There is **no `MarkupOnPress`, `MarkupId`, or `MarkupClass` component, and no `class=` attribute.** `id=` and `name=` simply set the entity's `Name`. Reference a reusable markup component with `<node template="path/to/widget.html">` — the old file-stem custom-tag registry was removed, and a bare unknown tag now warns and renders nothing.

## Writing your own GPU widget (advanced)

To paint a custom widget on the GPU, define a `UiMaterial` exactly like ember's. Embed a WGSL shader, derive `AsBindGroup`, register a `UiMaterialPlugin`, and attach a `MaterialNode` to your node. The shape mirrors `ArcMaterial`:

```rust
use bevy::asset::{embedded_asset, Asset};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct RingMaterial {
    #[uniform(0)]
    pub color: Vec4,
    #[uniform(0)]
    pub params: Vec4, // x = progress 0..1, y/z/w = your knobs
}

impl UiMaterial for RingMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://my_plugin/ring.wgsl".into()
    }
}

pub struct RingPlugin;
impl Plugin for RingPlugin {
    fn build(&self, app: &mut App) {
        // Guard against double-registration if more than one path adds it.
        if app.is_plugin_added::<UiMaterialPlugin<RingMaterial>>() {
            return;
        }
        embedded_asset!(app, "ring.wgsl");
        app.add_plugins(UiMaterialPlugin::<RingMaterial>::default());
    }
}
```

```wgsl
// ring.wgsl — a UiMaterial fragment shader.
#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Ring { color: vec4<f32>, params: vec4<f32> };
@group(1) @binding(0) var<uniform> u: Ring;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let c = in.size * 0.5;
    let p = in.uv * in.size - c;
    let r = length(p) / (min(in.size.x, in.size.y) * 0.5);
    let on = step(0.85, r) * step(r, 1.0);     // a thin outer ring
    if (on <= 0.0) { discard; }
    return vec4<f32>(u.color.rgb, u.color.a * u.params.x);
}
```

Then spawn the node with a `MaterialNode`, creating the material from `Assets<RingMaterial>`:

```rust
fn spawn_ring(mut commands: Commands, mut mats: ResMut<Assets<RingMaterial>>) {
    let mat = mats.add(RingMaterial {
        color: Vec4::new(0.22, 0.85, 0.54, 1.0),
        params: Vec4::new(0.7, 0.0, 0.0, 0.0),
    });
    commands.spawn((
        Node { width: Val::Px(64.0), height: Val::Px(64.0), ..default() },
        MaterialNode(mat),
        Name::new("ring"),
    ));
}
```

Update the value by mutating the material via its handle in a system (`materials.get_mut(&node.0)`), exactly as `arc_sync`/`chart_sync` do — that's the cheapest way to animate, since it touches only the uniform buffer.

## Where widgets live

Widgets are contributed by plugins. Editor-only widgets and panels ride in editor-scope plugins (`renzora::add!(MyPlugin, Editor)`), linked into the removable `renzora_editor` bundle; game HUD widgets ride in runtime-scope plugins and ship inside the game. Either way you import builders from `renzora_ember::widgets` and theme helpers from `renzora_ember::theme` / `renzora_ember::style`. There is no `renzora::prelude` — use `use renzora::*;` or import individual items.

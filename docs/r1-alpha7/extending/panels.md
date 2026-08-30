# Editor Panels from a Plugin

A [native plugin](native-plugins.md) can add a dock panel built from the editor's own UI framework — the same widgets, the same theme, the same reactive layer every built-in panel uses. There is no separate plugin UI toolkit and no bridge.

```rust
use bevy::prelude::*;
use renzora::core::RenzoraShellExt;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::theme::{rgb, text_primary};

const PANEL_ID: &str = "my_panel";

pub struct MyPanelPlugin;

impl Plugin for MyPanelPlugin {
    fn build(&self, app: &mut App) {
        app.register_shell_panel(PANEL_ID, "My Panel", "sparkle", "Tools");
        app.register_panel_content(PANEL_ID, true, build)
            .systems(Update, my_panel_system);
    }
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    commands
        .spawn((
            Text::new("Hello from a plugin"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id()
}

renzora::plugin!(MyPanelPlugin);
```

Open it from **Add Panel** in the dock; it appears under whatever category you registered.

## Getting `renzora_ember` into your plugin

Ember needs no build configuration. The plugin compiler always passes `--extern renzora_ember`, pointed at the shared `renzora_ember_dylib` image, so the crate is importable whether or not your manifest mentions it.

Add it to `Cargo.toml` anyway, so rust-analyzer can resolve the paths while you author:

```toml
[dependencies]
bevy = "0.19"
renzora = { path = "../../crates/renzora" }
renzora_ember = { path = "../../crates/renzora_ember" }
```

The build strips `bevy` and every `renzora*` entry before it hands the rest to cargo (see [Crates from crates.io](native-plugins.md#crates-from-cratesio)), so those three lines are documentation for your editor and nothing else. From a downloaded engine with no source checkout the paths will not resolve and the plugin still compiles; point them at the SDK, or drop them and lose only autocomplete.

## The two registrations

They are separate on purpose, and both are needed.

**`register_shell_panel(id, title, icon, category)`** is the contract-crate half — the panel's metadata for the dock tab and the Add-Panel picker. It comes from `renzora::core::RenzoraShellExt`. `icon` is a Phosphor icon name in kebab-case (`"align-center-horizontal"`, `"broom"`, `"bug"`); an unknown name resolves to no glyph rather than an error, so check your spelling against the picker. `category` groups the entry in Add Panel and is a free-form string — reuse an existing one (`"Tools"`, `"Scene"`, `"Debug"`) or make your own.

**`register_panel_content(id, scroll, build)`** is ember's half — what to build inside the tab. It comes from `renzora_ember::panel::RegisterPanelContent`. `scroll` wraps the content in a scroll view; pass `false` if your panel scrolls itself or manages its own virtualized list.

The ids must match. A shell panel with no content renders the dock's placeholder; content with no shell entry has no tab to appear in.

## The build closure runs once

`build` has the signature `fn(&mut Commands, &EmberFonts) -> Entity`, and it runs **once** — the first time the tab is activated — returning the root entity the dock parents into the leaf. Everything after that is driven by the reactive layer, not by rebuilding.

This is the single most important thing to internalize about ember. A panel is not an immediate-mode UI that redraws each frame; it is a retained `bevy_ui` tree that you declare once and then *bind*. Rebuilding a panel every frame — despawn the children, spawn them again — is what took the editor to 25 FPS before the reactive layer existed.

`EmberFonts` carries the fonts you need:

| Field | What it is |
|---|---|
| `ui` | the themed UI face, a `FontSource` — pass to `ui_font(&fonts.ui, size)` |
| `mono` | the monospace face, for code and numbers |
| `phosphor` | the icon font, a `Handle<Font>` — used by `icon_text` and `glyph` |
| `default_ui`, `default_mono` | the built-in faces, ignoring any theme override |

Never hardcode a `TextFont`. `ui_font` applies the user's UI-scale setting and snaps the pixel size, which is why a panel built with it stays legible at 150% scale and one built with `TextFont::from_font_size(12.0)` does not.

## Systems are visibility-gated by default

`register_panel_content` returns a `PanelScope`. Systems added through `.systems(schedule, systems)` only run while the panel is the active tab somewhere — including in a torn-off dock window.

```rust
app.register_panel_content(PANEL_ID, true, build)
    .systems(Update, (handle_clicks, refresh_preview))
    .always(Update, watch_for_external_changes);
```

That default is the point. Panel systems that keep running while their tab is hidden are the single largest avoidable cost in the editor's idle frame. Use `.always(...)` only for work that is wrong to pause — a background load, a save, cleanup that must observe a despawn while the tab is hidden.

`PanelScope::app()` hands back the `&mut App` if you need to register something unrelated to the panel while you have the builder open.

## Reactivity, not polling

Bind the parts that change. A binding is a closure that reads the world and produces a value; the reactive driver runs it, compares against last frame's value, and writes only on a change.

```rust
use renzora_ember::reactive::tracked::bind_text;

let label = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0))).id();
bind_text(commands, label, |w| {
    let n = w.get_resource::<EditorSelection>().map(|s| s.get_all().len()).unwrap_or(0);
    format!("{n} selected")
});
```

The `w` handed to a binding is an `Rx`, not a `&World`. It is a tracking wrapper: every resource and component it reads is recorded, and the binding re-runs only when something in that dependency set has changed. That is what makes a hundred bindings cost nothing on an idle frame.

### The binding set

| Function | Writes | Use for |
|---|---|---|
| `bind_text(cmds, e, \|rx\| -> String)` | the entity's `Text` | labels, counters, names |
| `bind_text_color(cmds, e, \|rx\| -> Color)` | `TextColor` | status colouring |
| `bind_bg(cmds, e, \|rx\| -> Color)` | `BackgroundColor` | selection highlight, validity |
| `bind_display(cmds, e, \|rx\| -> bool)` | `Node.display` | show/hide a whole subtree |
| `bind_with(cmds, e, \|rx\| -> V, \|world, e, &V\|)` | anything | the escape hatch — any component, any property |
| `bind_2way(cmds, e, get, set)` | a widget's value, both ways | sliders, checkboxes, dropdowns |

`bind_with` is the general form the others are built on: `value` computes a `PartialEq` value from the `Rx`, and `apply` writes it into the world when it differs. Reach for it when you need to drive something no named binding covers — a `BorderColor`, an image handle, a `ZIndex`.

`bind_2way` is what makes a widget an editor of state rather than a display of it. `get` reads the value out of the world for the widget to show; `set` writes the widget's new value back. The widget's own interaction system calls `set`; the binding calls `get` to keep it honest when the value changes from elsewhere (an undo, a script, another panel).

```rust
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::widgets::checkbox;

let cb = checkbox(commands, false);
bind_2way(
    commands,
    cb,
    |rx| rx.get_resource::<MySettings>().is_some_and(|s| s.enabled),
    |world, on| {
        if let Some(mut s) = world.get_resource_mut::<MySettings>() {
            s.enabled = *on;
        }
    },
);
```

Text inputs have their own two-way helper, `renzora_ember::widgets::bind_text_input(cmds, input, get, set)`, because a text field has an editing state that must not be clobbered mid-keystroke.

### Lists

`keyed_list` is the `<For>` equivalent — the right tool whenever the number of rows can change.

```rust
use renzora_ember::reactive::{keyed_list, KeyedSnapshot};

keyed_list(commands, list_container, |rx| {
    let names: Vec<(Entity, String)> = /* read from rx */ vec![];
    KeyedSnapshot {
        items: names.iter().map(|(e, n)| (e.to_bits(), hash_of(n))).collect(),
        build: Box::new(move |cmds, fonts, i| {
            let (_, name) = &names[i];
            cmds.spawn((Text::new(name.clone()), ui_font(&fonts.ui, 11.0))).id()
        }),
    }
});
```

`items` is one `(stable key, content hash)` pair per row, in display order. The driver diffs that against last frame: a row whose key and hash are unchanged is left completely alone, a changed hash rebuilds just that row, and added/removed keys spawn/despawn just those. A full rebuild never happens.

Pick the key so it survives reordering — an `Entity`'s bits, an asset path's hash, a stable id — and make the content hash cover everything the row draws. A hash that misses a field means the row silently stops updating.

`keyed_list_tokened(cmds, container, token, snapshot)` adds a cheap pre-check: if `token` returns the same `u64` as last frame the snapshot closure is skipped entirely. Use it when producing the snapshot is itself expensive (a directory scan, a big query) — the token can be a count, a version number, a change tick.

> **Despawns inside a reactive list must be `try_despawn`.** The list may already have removed a row this frame; a plain `despawn` on an entity it dropped is a panic.

### What the `Rx` tracks, and what it doesn't

- `rx.get_resource::<R>()` / `rx.resource::<R>()` — tracked.
- `rx.get::<C>(entity)` — tracked, per entity.
- `rx.untracked()` — the raw `&World`, recorded as *nothing*. A binding whose only reads are untracked is permanently dirty and runs every frame. That is occasionally what you want (a clock, an animation) and usually a bug.
- `rx.manually_tracked()` — the raw `&World`, for when you will call `track_resource_id` / `track_component_id` yourself. Use it when you must query in a way the wrapper cannot see through, and then declare what you read.

If a binding is not updating, the first thing to check is whether the thing it reads is actually tracked. If a binding is running every frame, the first thing to check is whether it slipped into `untracked()`.

## Widgets

`renzora_ember::widgets` is the full editor set — everything the built-in panels are made of. Each is a builder function that spawns a subtree and returns its root `Entity`, which you then parent where you want it and, usually, bind.

### Form controls

| Builder | Signature |
|---|---|
| `button` | `(cmds, &fonts.ui, label) -> Entity` |
| `icon_label_button` | `(cmds, fonts, icon, label) -> Entity` |
| `icon_label_button_parts` | `(cmds, fonts, icon, label) -> (root, icon, label)` |
| `icon_button` | `(cmds, fonts, icon) -> Entity` |
| `checkbox` | `(cmds, checked) -> Entity` |
| `slider` | `(cmds, value) -> Entity` (0..1) |
| `slider_ranged` | `(cmds, value, min, max) -> Entity` |
| `dropdown` | `(cmds, fonts, &["a", "b"], selected) -> Entity` |
| `dropdown_with_icons` | `(cmds, fonts, &[(icon, label)], selected) -> Entity` |
| `text_input` | `(cmds, &fonts.ui, placeholder, value) -> Entity` |
| `password_input` | `(cmds, &fonts.ui, placeholder, value) -> Entity` |

Also: `radio`, `segmented`, `stepper`, `toggle_switch`, `multi_select`, `tags_input`, `search`, `textarea`, `form` (the `EmberForm` Tab/Enter wrapper), `font_picker`, `folder_picker`.

### Value editors

`drag_value`, `spin_slider`, `vec3_edit`, `xy_pad`, `knob`, `fader`, `gauge`, `color_picker`, `property_row`, `asset_slot`, `asset_tile`, `curve`, `gradient`.

`property_row` is the labelled `label | editor` row the inspector is built from; use it and your panel lines up with every built-in one for free.

### Containers and chrome

`section`, `card`, `accordion`, `collapsible`, `tabs`, `divider`, `grid`, `table`, `list_group`, `scroll_area`, `navbar`, `breadcrumb`, `pagination`, `sortable`, `virtual_scroll`.

`section` is the workhorse:

```rust
use renzora_ember::widgets::section;
use renzora_ember::theme::accent;

let (root, body) = section(commands, fonts, "sliders", "Options", accent());
// parent `root` into your panel; spawn your rows as children of `body`
```

It returns `(root, body)` — spawn into `body`, parent `root`. The header collapses on click with no work from you. The variants give you more handles: `section_with_header` also returns the header entity (for a trailing toggle or remove button), `section_with_header_open` takes the initial open state, and `section_with_header_icon_open` also returns the leading icon entity so you can make the icon itself a button.

### Feedback and overlays

`alert`, `badge`, `chip`, `progress`, `spinner`, `skeleton`, `toast`, `tooltip`, `modal`, `popover`, `popup`, `context_menu`, `menu`, `submenu`, `icon_menu`.

### Specialized

`code_editor`, `node_graph`, `timeline`, `timeline_view`, `chart`, `markdown`, `rich_text`, `mixer`, `vu_meter`, `waveform`, `audio_player`, `avatar`, `image`, `gallery`, `scene`, `drag_window`.

The `gallery_*` panels in the editor render the entire set live — the fastest way to find the widget you want is to open one and look.

## Theme colours

Never write a literal colour. `renzora_ember::theme` exposes the active palette as functions returning `(u8, u8, u8)`, and `rgb()` converts to a Bevy `Color`:

```rust
use renzora_ember::theme::{rgb, card_bg, text_primary, text_muted, accent, border, divider};

BackgroundColor(rgb(card_bg()))
TextColor(rgb(text_muted()))
```

The full set: `window_bg`, `panel_bg`, `faint_bg`, `header_bg`, `section_bg`, `card_bg`, `popup_bg`, `hover_bg`, `row_even`, `row_odd`, `tab_active`, `tab_hover`, `selection`, `border`, `divider`, `tree_line`, `text_primary`, `text_muted`, `value_text`, `placeholder`, `accent`, `on_accent`, `play_green`, `warn_amber`, `close_red`. `mix(a, b, t)` blends two of them; `rgba([r, g, b, a])` takes an alpha.

These are *functions*, not constants, because the palette is swapped when the user changes theme. Read them inside `build` and inside bindings — never cache one in a `static`.

For per-widget styling beyond colour there is the stylesheet layer: `renzora_ember::theme::style(Role)` returns the `WidgetStyle` a theme declares for a widget role. See [Theming](../editor-dev/theming.md).

## Handling interaction

A click is handled the ordinary Bevy way — a marker component and `Changed<Interaction>`:

```rust
#[derive(Component)]
struct ResetButton;

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands.spawn(Node::default()).id();
    let b = button(commands, &fonts.ui, "Reset");
    commands.entity(b).insert(ResetButton);
    commands.entity(root).add_child(b);
    root
}

fn my_panel_system(q: Query<&Interaction, (With<ResetButton>, Changed<Interaction>)>) {
    for interaction in &q {
        if *interaction == Interaction::Pressed { /* … */ }
    }
}
```

`Changed<Interaction>` matters: `Pressed` persists for as long as the mouse is held, so without it the action fires every frame of the click.

Two things that will catch you once:

**Give every interactive node a `FocusPolicy`.** In Bevy 0.19 the default is `Pass`, so an unmarked node lets a press fall through to whatever is behind it — including the viewport, which will interpret it as a click in the scene.

**Gate empty-space actions on `ScrollbarBusy`.** If your panel does something on a click in its background, that fires while the user is dragging your scrollbar unless you check.

## Beyond the panel: other surfaces a plugin can contribute

A dock panel is the largest surface, not the only one.

### A settings section

`renzora_ember::settings_sections::RegisterSettingsSection` adds a section to the Settings overlay's **Plugins** tab — the right home for a plugin's own preferences, so the user finds them where they look for everything else.

```rust
use renzora_ember::settings_sections::RegisterSettingsSection;

app.register_settings_section("my_plugin", "My Plugin", "sparkle", |cmds, fonts| {
    let root = cmds.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    // … rows, bound the same way as a panel …
    root
});
```

Same build-once + bind model as a panel; `build` runs each time the overlay opens. Registering an id that already exists replaces it.

### A status-bar item

```rust
use renzora::core::{RenzoraShellExt, ShellStatusAlign, ShellStatusItem, ShellStatusSegment};

app.register_shell_status_item(ShellStatusItem {
    id: "my_count",
    align: ShellStatusAlign::Right,
    order: 10,
    render: |world| {
        let n = world.get_resource::<EditorSelection>().map(|s| s.get_all().len()).unwrap_or(0);
        vec![ShellStatusSegment::new("cursor-click", format!("{n}"), text_muted())]
    },
});
```

`render` is a plain `fn(&World) -> Vec<ShellStatusSegment>` called each frame, so live metrics update without re-registering. It is one of the few places in ember that is genuinely immediate-mode, which is why it must stay cheap.

`ShellReadyStatus` is the other half: writing `label = Some(text)` replaces the status bar's left-hand "Ready" for a transient message, and `None` restores it. That is how the auto-save plugin shows its countdown.

#### Showing progress

A segment can carry a progress bar, for background work that outlives the dialog that started it:

```rust
use renzora::core::ShellStatusBar;

ShellStatusSegment::new("download-simple", format!("Installing {name} — {size}"), accent)
    .bar(ShellStatusBar::Busy)
```

`ShellStatusBar::Fraction(f)` draws a determinate fill; `Busy` draws a block that sweeps the track. Reach for `Busy` whenever you do not genuinely know the total — a fraction invented from a curve that creeps toward 90% and waits is a lie the bar tells for the whole of the wait. Put the real number in the *text* instead (bytes so far, files written), which is what the marketplace's background installs do: nothing in the transport reports a file's size, so there is no percentage to be had.

`Busy` animates in its own system, so it costs no rebuilds. A `Fraction` is quantized to whole percent before the status bar hashes the row, so a continuously moving value rebuilds the segment at most a hundred times rather than every frame.

### Viewport toolbar and strips

Three registration points in `renzora_ember::toolbar`, all free functions taking a `Fn(&mut Commands, &EmberFonts) -> Entity`:

| Function | Where it lands |
|---|---|
| `register_viewport_tool_trailing(build)` | the right-hand end of the in-viewport tool strip |
| `register_viewport_tool_group(key, build)` | spliced into the tool strip as a draggable, position-remembered group |
| `register_viewport_top_strip(order, build)` | a full-width bar between the tool strip and the scene |

Choose by width. The tool strip has very little room left, so a group of two or three controls belongs there and anything that will wrap onto its own line belongs in a top strip — that is exactly why the terrain brush settings moved out of the strip. A tool group should `bind_display` its own root off when it is not relevant; an always-visible group takes space in every context. Keep a group's `key` stable across releases, or users' arranged toolbars reset.

### Gizmos

An ordinary `Gizmos` system draws into the viewport with no registration at all — read `EditorSelection`, draw around what is selected, and you have a viewport contribution in a dozen lines. Gate it with `in_three_view` / `in_two_view` if the visual only makes sense through one camera.

### Everything else

Panels are UI; the editor's *behaviour* extension points — inspector fields, entity presets, scene starters, keyboard shortcuts, component icons, viewport tools, console output — are registrations on the contract crate. See **[Editor Features from Code](editor-api.md)**.

## Why your panel is themed correctly

Ember keeps the theme palette, the stylesheet, the UI font scale and the viewport-toolbar lists in process-global statics — one set per *process*, not per crate. A plugin that linked its own private copy of ember would get its own set, and every one of them fails silently: your panel would paint in ember's default colours no matter what theme the user picked, and a `register_viewport_tool_group` call would push into a list nothing reads.

So the plugin build points `--extern renzora_ember` at the shared `renzora_ember_dylib` image rather than at ember's rlib. Nothing about this is visible from a plugin — it is the reason panels work, and the reason a one-panel plugin is 270 KB instead of 32 MB.

The same reasoning is why `renzora` is a shared image, and why the loader refuses to load anything at all when `dynamic_linking` is off: there is no runtime check that would catch a duplicated static, so the only safe answer is not to be in that situation.

## Pitfalls

**Floating UI must be spawned at the root, never as a child.** A popup, menu, tooltip or modal parented under the widget that opened it inherits that widget's clipping and stacking. An `Overflow::clip()` anywhere up the chain will hide a menu inside its own trigger. Spawn it parentless and position it, which is what ember's own `popup`/`menu`/`modal` builders do.

**A floating surface must carry `OverlaySurface`** or clicks pass through it into whatever is behind.

**Don't query `Query<&Window>.single()` unfiltered.** With a torn-off dock window present there is more than one, and the query panics or silently picks the wrong one.

**UI entities have no `GlobalTransform` in Bevy 0.19.** A system that queries one compiles and then never runs, because nothing matches. Use `UiGlobalTransform`.

**Don't order systems against `screen_menu_dismiss`.** It runs where it does for a reason and ordering against it deadlocks the dismissal.

**Give scene content a `Name`.** The hierarchy panel queries `(Entity, &Name)` — no name, no row, no selection, no gizmo — and the scene serializer only writes named entities.

## Where to look for examples

No native plugin ships in the repository, so the reference for panel code is **the editor's own panels** — they are built from exactly the API above, with no privileged access a plugin lacks.

| Look at | For |
|---|---|
| `crates/renzora_ember/src/widgets/gallery.rs` | the shortest complete example of building a large ember tree from one `build` function — and it renders live in the editor, so you can compare code to pixels |
| `crates/renzora_inspector` | `property_row`, sections, and two-way bindings over live component data |
| `crates/renzora_hierarchy` | `keyed_list` over a changing entity set, with selection |
| `crates/renzora_ember/src/widgets/` | each widget's own source, when you need to know exactly what a builder spawns |

The difference between those crates and your plugin is how they are linked, not what they may call: an in-workspace plugin is an `rlib` wired in by the `add!` generator, a native plugin is a `dylib` loaded at startup, and both get the same `&mut World` and the same ember.

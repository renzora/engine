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

## The two registrations

They are separate on purpose, and both are needed.

**`register_shell_panel(id, title, icon, category)`** is the contract-crate half — the panel's metadata for the dock tab and the Add-Panel picker. `icon` is a Phosphor icon name in kebab-case (`"align-center-horizontal"`, `"broom"`, `"bug"`).

**`register_panel_content(id, scroll, build)`** is ember's half — what to build inside the tab. `scroll` wraps the content in a scroll view; pass `false` if your panel scrolls itself.

Your `build` closure runs **once**, the first time the tab is activated. Everything after that is driven by the reactive layer, not by rebuilding.

## Systems are visibility-gated by default

`register_panel_content` returns a `PanelScope`. Systems added through `.systems(...)` only run while the panel is the active tab somewhere — including in a torn-off dock window.

That default is the point. Panel systems that keep running while their tab is hidden are the single largest avoidable cost in the editor's idle frame. Use `.always(...)` only for work that is wrong to pause — a background load, a save, cleanup that must observe a despawn while the tab is hidden.

## Reactivity, not polling

Do not rebuild your panel each frame. Bind the parts that change:

```rust
use renzora_ember::reactive::tracked::bind_text;

let label = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0))).id();
bind_text(commands, label, |w| {
    let n = w.get_resource::<EditorSelection>().map(|s| s.get_all().len()).unwrap_or(0);
    format!("{n} selected")
});
```

A binding re-runs only when something its closure *read* has changed — the dependency set is tracked automatically. `bind_2way`, `bind_display`, `bind_with` and `keyed_list` cover the rest.

## Widgets

`renzora_ember::widgets` is the full editor set: `button`, `icon_button`, `checkbox`, `slider`, `dropdown`, `text_input`, `color_picker`, `vec3_edit`, `drag_value`, `tabs`, `accordion`, `card`, `toast`, `progress`, `curve`, `gradient` and more. Colours come from `renzora_ember::theme` (`rgb(text_muted())`, `rgb(card_bg())`, …), which resolves to the user's active theme.

Handle a click the ordinary Bevy way — a marker component and `Changed<Interaction>`:

```rust
#[derive(Component)]
struct MyButton;

fn my_panel_system(q: Query<&Interaction, (With<MyButton>, Changed<Interaction>)>) {
    for interaction in &q {
        if *interaction == Interaction::Pressed { /* … */ }
    }
}
```

`Changed<Interaction>` matters: `Pressed` persists for as long as the mouse is held, so without it the action fires every frame of the click.

## Why your panel is themed correctly

Ember keeps the theme palette, the stylesheet, the UI font scale and the viewport-toolbar lists in process-global statics — one set per *process*, not per crate. A plugin that linked its own private copy of ember would get its own set, and every one of them fails silently: your panel would paint in ember's default colours no matter what theme the user picked, and a `register_viewport_tool_group` call would push into a list nothing reads.

So the plugin build points `--extern renzora_ember` at the shared `renzora_ember_dylib` image rather than at ember's rlib. Nothing about this is visible from a plugin — it is the reason panels work, and the reason a one-panel plugin is 270 KB instead of 32 MB.

## Other places a plugin can contribute UI

A panel is not the only surface:

- **`register_shell_status_item`** — a status-bar item. Its `render` is `fn(&World) -> Vec<ShellStatusSegment>`, called each frame, so live metrics update without re-registering. `plugins/scene-lint` uses this.
- **`ShellReadyStatus`** — replaces the status bar's left-hand "Ready" label, for a transient message.
- **`renzora_ember::toolbar::register_viewport_tool_group`** — a viewport toolbar group.
- **Gizmos** — an ordinary `Gizmos` system draws into the viewport. `plugins/orrery` reads `EditorSelection` and draws orbit rings for whatever is selected.

## A worked example

`plugins/align-tools` is a complete panel: align, distribute, ground-snap, scatter and reset over the current selection. It is about 350 lines including its comments, and it exercises sections, buttons, a live reactive label, `EditorSelection`, and world-space transform maths against local `Transform` writes.

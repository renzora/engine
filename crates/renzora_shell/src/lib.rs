//! `renzora_shell` — the bevy_ui-native editor shell.
//!
//! The editor's chrome (menu bar, ribbon, document tabs, status bar) plus the
//! wiring that drives the reusable [`renzora_ember`] dock. The dock itself —
//! splits, tabs, drag-docking — lives in `renzora_ember::dock`; the shell just
//! supplies the layout, the dock area, and editor-specific behavior.
//!
//! ## Coexistence during the migration
//! [`renzora::EditorUiBackend`] selects which editor renders — the legacy egui
//! editor (`Egui`, default) or this shell (`BevyUi`) — mutually exclusive (the
//! egui `editor_ui_system` is gated off under `BevyUi`). **F10** toggles.

use bevy::prelude::*;

use renzora::EditorUiBackend;
use renzora_ember::dock::{Dock, DockArea, DockDirty, DockLeaf, DockTab};
use renzora_ember::font::{glyph, icon_item, ui_font, EmberFonts};
use renzora_ember::theme::{
    rgb, ACCENT_BLUE, DIVIDER, HEADER_BG, PLACEHOLDER, PLAY_GREEN, TAB_ACTIVE_BG, TEXT_MUTED,
    TEXT_PRIMARY, WINDOW_BG,
};
use renzora_ember::EmberPlugin;

pub mod dock;

use dock::DockTree;

#[derive(Default)]
pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShellPlugin (bevy_ui editor shell)");
        app.add_plugins(EmberPlugin);
        app.init_resource::<EditorUiBackend>();
        let layouts = dock::workspace_layouts();
        // The dock starts on the first workspace (overrides DockPlugin's empty).
        app.insert_resource(Dock {
            tree: layouts[0].1.clone(),
        });
        app.insert_resource(ShellLayouts { layouts, active: 0 });
        app.init_resource::<renzora::ShellPanelRegistry>();
        app.add_systems(
            Update,
            (
                toggle_backend,
                manage_shell_root,
                apply_panel_meta,
                ribbon_switch,
                content_dispatch,
            ),
        );
    }
}

renzora::add!(ShellPlugin, Editor);

/// The ribbon's workspace layouts and which one is active. Switching saves the
/// current dock tree back into the active slot (so per-layout edits persist)
/// and loads the chosen one into the ember [`Dock`].
#[derive(Resource)]
struct ShellLayouts {
    layouts: Vec<(String, DockTree)>,
    active: usize,
}

/// A ribbon workspace button (Scene, Blueprints, …). Carries its layout index
/// and the entities to restyle when the active layout changes.
#[derive(Component)]
struct RibbonItem {
    index: usize,
    text: Entity,
    underline: Entity,
}

/// Marks the shell's root UI entity so it can be despawned when the backend
/// switches back to egui.
#[derive(Component)]
struct ShellRoot;

// ── Systems ─────────────────────────────────────────────────────────────────

/// F10 flips the active editor UI backend between the legacy egui editor and
/// the bevy_ui shell.
fn toggle_backend(keys: Res<ButtonInput<KeyCode>>, mut backend: ResMut<EditorUiBackend>) {
    if keys.just_pressed(KeyCode::F10) {
        *backend = match *backend {
            EditorUiBackend::Egui => EditorUiBackend::BevyUi,
            EditorUiBackend::BevyUi => EditorUiBackend::Egui,
        };
        info!("[shell] editor UI backend -> {:?}", *backend);
    }
}

/// Spawn the chrome + dock area when the backend is `BevyUi` (and trigger the
/// ember dock to build into it); tear it down when it isn't.
fn manage_shell_root(
    mut commands: Commands,
    backend: Res<EditorUiBackend>,
    fonts: Option<Res<EmberFonts>>,
    mut dirty: ResMut<DockDirty>,
    roots: Query<Entity, With<ShellRoot>>,
) {
    let want = backend.is_bevy_ui();
    let have = !roots.is_empty();
    if want && !have {
        // Wait for fonts so text/icons render from the first frame.
        let Some(fonts) = fonts else {
            return;
        };
        spawn_shell(&mut commands, &fonts.ui);
        // Build the dock into the freshly-spawned `DockArea` (ember rebuilds it
        // from the persisted `Dock.tree`).
        dirty.0 = true;
    } else if !want && have {
        for e in &roots {
            commands.entity(e).despawn();
        }
    }
}

/// Apply real panel titles/icons from [`renzora::ShellPanelRegistry`] onto the
/// dock tabs (overriding ember's humanized defaults). Cheap; only writes on a
/// real change.
fn apply_panel_meta(
    registry: Res<renzora::ShellPanelRegistry>,
    tabs: Query<&DockTab>,
    mut texts: Query<&mut Text>,
) {
    if registry.panels.is_empty() {
        return;
    }
    for tab in &tabs {
        let Some(info) = registry.panels.get(&tab.id) else {
            continue;
        };
        if !info.title.is_empty() {
            if let Ok(mut t) = texts.get_mut(tab.label) {
                if t.0 != info.title {
                    t.0 = info.title.clone();
                }
            }
        }
        if !info.icon.is_empty() {
            if let Ok(mut t) = texts.get_mut(tab.icon) {
                if t.0 != info.icon {
                    t.0 = info.icon.clone();
                }
            }
        }
    }
}

/// Tracks which panel a leaf's content node currently renders, so the dispatch
/// only rebuilds when the active panel changes.
#[derive(Component)]
struct ContentShows(String);

/// Fill each leaf's content with the active panel's UI. The editor's panels
/// (eventually) register their own builders; for now: the `gallery_*` ember
/// component showcases, and a centered title placeholder for everything else.
fn content_dispatch(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    leaves: Query<&DockLeaf>,
    shows: Query<&ContentShows>,
    children_q: Query<&Children>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for leaf in &leaves {
        let current = shows.get(leaf.content).ok().map(|s| s.0.as_str());
        if current == Some(leaf.active.as_str()) {
            continue;
        }
        // Clear whatever the content was showing, then build for the new panel.
        if let Ok(kids) = children_q.get(leaf.content) {
            for child in kids.iter() {
                commands.entity(child).despawn();
            }
        }
        let built = build_panel_content(&mut commands, &fonts, &leaf.active);
        commands.entity(leaf.content).add_child(built);
        commands
            .entity(leaf.content)
            .insert(ContentShows(leaf.active.clone()));
    }
}

/// Build the bevy_ui content for a panel id.
fn build_panel_content(commands: &mut Commands, fonts: &EmberFonts, id: &str) -> Entity {
    use renzora_ember::widgets;
    match id {
        "gallery_typography" => widgets::gallery_typography(commands, fonts),
        "gallery_buttons" => widgets::gallery_buttons(commands, fonts),
        "gallery_inputs" => widgets::gallery_inputs(commands, fonts),
        "gallery_selection" => widgets::gallery_selection(commands, fonts),
        "gallery_feedback" => widgets::gallery_feedback(commands, fonts),
        "gallery_inspector" => widgets::gallery_inspector(commands, fonts),
        "gallery_containers" => widgets::gallery_containers(commands, fonts),
        "gallery_nav" => widgets::gallery_nav(commands, fonts),
        "gallery_data" => widgets::gallery_data(commands, fonts),
        "gallery_forms" => widgets::gallery_forms(commands, fonts),
        "gallery_overlays" => widgets::gallery_overlays(commands, fonts),
        "gallery_menus" => widgets::gallery_menus(commands, fonts),
        "gallery_extras" => widgets::gallery_extras(commands, fonts),
        "gallery_node_graph" => widgets::gallery_node_graph(commands, fonts),
        "gallery_timeline" => widgets::gallery_timeline(commands, fonts),
        "gallery_code" => widgets::gallery_code(commands, fonts),
        "gallery_charts" => widgets::gallery_charts(commands, fonts),
        "gallery_pickers" => widgets::gallery_pickers(commands, fonts),
        "gallery_animation" => widgets::gallery_animation(commands, fonts),
        "gallery_audio" => widgets::gallery_audio(commands, fonts),
        "gallery_colors" => widgets::gallery_colors(commands, fonts),
        _ => {
            // Placeholder: the panel's name, centered.
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Name::new("placeholder"),
                ))
                .id();
            let text = commands
                .spawn((
                    Text::new(renzora_ember::dock::humanize(id)),
                    ui_font(&fonts.ui, 13.0),
                    TextColor(rgb(PLACEHOLDER)),
                ))
                .id();
            commands.entity(container).add_child(text);
            container
        }
    }
}

/// Clicking a ribbon workspace button switches the dock layout: save the current
/// dock back into its slot, load the chosen layout into the ember [`Dock`],
/// flag a rebuild, and restyle the ribbon.
fn ribbon_switch(
    triggers: Query<(&RibbonItem, &Interaction), Changed<Interaction>>,
    items: Query<&RibbonItem>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    mut backgrounds: Query<&mut BackgroundColor>,
    mut colors: Query<&mut TextColor>,
) {
    let mut switch_to = None;
    for (item, interaction) in &triggers {
        if *interaction == Interaction::Pressed {
            switch_to = Some(item.index);
            break;
        }
    }
    let Some(index) = switch_to else {
        return;
    };
    if index == layouts.active || index >= layouts.layouts.len() {
        return;
    }

    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    dock.tree = layouts.layouts[index].1.clone();
    layouts.active = index;
    dirty.0 = true;

    for item in &items {
        let is_active = item.index == index;
        if let Ok(mut c) = colors.get_mut(item.text) {
            c.0 = rgb(if is_active { TEXT_PRIMARY } else { TEXT_MUTED });
        }
        if let Ok(mut b) = backgrounds.get_mut(item.underline) {
            b.0 = if is_active {
                rgb(ACCENT_BLUE)
            } else {
                Color::NONE
            };
        }
    }
}

// ── Chrome ──────────────────────────────────────────────────────────────────

fn spawn_shell(commands: &mut Commands, font: &Handle<Font>) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(WINDOW_BG)),
            ShellRoot,
            renzora::HideInHierarchy,
            Name::new("Renzora Shell"),
        ))
        .id();

    let top_bar = build_top_bar(commands, font);
    let doctabs = build_doc_tabs(commands, font);

    // Dock area — ember reconciles the dock into this (tagged `DockArea`).
    let dock_area = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                overflow: Overflow::clip(),
                ..default()
            },
            DockArea,
            Name::new("dock-area"),
        ))
        .id();

    let statusbar = chrome_row(
        commands,
        font,
        "status-bar",
        22.0,
        rgb(WINDOW_BG),
        16.0,
        10.0,
        &[
            ("Ready", false),
            ("Dark", false),
            ("Vulkan", false),
            ("60 FPS", false),
        ],
    );

    commands
        .entity(root)
        .add_children(&[top_bar, doctabs, dock_area, statusbar]);
}

/// The top bar: File/Edit/View/Help on the left, the layout ribbon centered,
/// action buttons on the right.
fn build_top_bar(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(WINDOW_BG)),
            Name::new("top-bar"),
        ))
        .id();

    let left = zone(commands, "top-left", JustifyContent::FlexStart, 2.0, 1.0);
    let left_kids = text_items(
        commands,
        font,
        &[("File", false), ("Edit", false), ("View", false), ("Help", false)],
        14.0,
    );
    commands.entity(left).add_children(&left_kids);

    let center = zone(commands, "top-center", JustifyContent::Center, 2.0, 0.0);
    let mut center_kids = vec![glyph(commands, "magnifying-glass", TEXT_MUTED, 14.0)];
    for (i, label) in [
        "Scene",
        "Blueprints",
        "Scripting",
        "Animation",
        "Materials",
        "Particles",
        "Video",
        "Audio",
        "Debug",
        "Gallery",
    ]
    .into_iter()
    .enumerate()
    {
        center_kids.push(ribbon_item(commands, font, label, i, i == 0));
    }
    center_kids.push(text_item(commands, font, "+", TEXT_MUTED, 12.0));
    commands.entity(center).add_children(&center_kids);

    let right = zone(commands, "top-right", JustifyContent::FlexEnd, 8.0, 1.0);
    let play = icon_item(commands, "play", PLAY_GREEN, 16.0);
    let code = icon_item(commands, "code", TEXT_MUTED, 16.0);
    let settings = icon_item(commands, "gear", TEXT_MUTED, 16.0);

    let account = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("account"),
        ))
        .id();
    let user = glyph(commands, "user", TEXT_MUTED, 14.0);
    let sign_in = commands
        .spawn((
            Text::new("Sign In"),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id();
    commands.entity(account).add_children(&[user, sign_in]);

    let window = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
            Name::new("window-buttons"),
        ))
        .id();
    let min = icon_item(commands, "minus", TEXT_MUTED, 14.0);
    let max = icon_item(commands, "square", TEXT_MUTED, 13.0);
    let close = icon_item(commands, "x", TEXT_MUTED, 14.0);
    commands.entity(window).add_children(&[min, max, close]);

    commands
        .entity(right)
        .add_children(&[play, code, settings, account, window]);

    commands.entity(bar).add_children(&[left, center, right]);
    bar
}

/// A top-bar ribbon entry (workspace switcher). Full height so the active
/// item's blue underline pins to the bottom edge. Clicking switches workspace
/// `index` (see [`ribbon_switch`]).
fn ribbon_item(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    index: usize,
    active: bool,
) -> Entity {
    let item = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("ribbon:{label}")),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(font, 12.0),
            TextColor(rgb(if active { TEXT_PRIMARY } else { TEXT_MUTED })),
        ))
        .id();
    let text_wrap = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(7.0)),
                ..default()
            },
            Name::new("ribbon-label"),
        ))
        .id();
    commands.entity(text_wrap).add_child(text);
    let underline = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(if active { rgb(ACCENT_BLUE) } else { Color::NONE }),
            Name::new("ribbon-underline"),
        ))
        .id();
    commands.entity(item).insert(RibbonItem {
        index,
        text,
        underline,
    });
    commands.entity(item).add_children(&[text_wrap, underline]);
    item
}

/// The document tab strip: a button-styled active document tab + an add-tab
/// button, with a bottom border separating it from the dock below.
fn build_doc_tabs(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(HEADER_BG)),
            BorderColor::all(rgb(DIVIDER)),
            Name::new("doc-tabs"),
        ))
        .id();
    let tab = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                border: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            BorderColor::all(rgb(ACCENT_BLUE)),
            Name::new("doc:sponza"),
        ))
        .id();
    let ic = glyph(commands, "file", TEXT_PRIMARY, 13.0);
    let lbl = commands
        .spawn((
            Text::new("sponza"),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    let cl = glyph(commands, "x", TEXT_MUTED, 11.0);
    commands.entity(tab).add_children(&[ic, lbl, cl]);

    let plus = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            Name::new("doc-add"),
        ))
        .id();
    let plus_icon = glyph(commands, "plus", TEXT_MUTED, 13.0);
    commands.entity(plus).add_child(plus_icon);
    commands.entity(bar).add_children(&[tab, plus]);
    bar
}

/// A full-height flex row used as a top-bar zone (left / center / right).
fn zone(
    commands: &mut Commands,
    name: &str,
    justify: JustifyContent,
    gap: f32,
    grow: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: justify,
                column_gap: Val::Px(gap),
                flex_grow: grow,
                ..default()
            },
            Name::new(name.to_string()),
        ))
        .id()
}

/// A padded text item (menu entry, ribbon "+"). `active` → primary, else muted.
fn text_items(
    commands: &mut Commands,
    font: &Handle<Font>,
    items: &[(&str, bool)],
    size: f32,
) -> Vec<Entity> {
    items
        .iter()
        .map(|(label, active)| {
            let color = if *active { TEXT_PRIMARY } else { TEXT_MUTED };
            text_item(commands, font, label, color, size)
        })
        .collect()
}

fn text_item(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new(format!("item:{label}")),
        ))
        .with_children(|p| {
            p.spawn((Text::new(label), ui_font(font, size), TextColor(rgb(color))));
        })
        .id()
}

/// A horizontal strip of text items (menu bar, status bar).
fn chrome_row(
    commands: &mut Commands,
    font: &Handle<Font>,
    name: &str,
    height: f32,
    bg: Color,
    gap: f32,
    pad: f32,
    items: &[(&str, bool)],
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(height),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(gap),
                padding: UiRect::horizontal(Val::Px(pad)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(bg),
            Name::new(name.to_string()),
        ))
        .id();

    let kids: Vec<Entity> = items
        .iter()
        .map(|(label, active)| {
            let color = if *active { TEXT_PRIMARY } else { TEXT_MUTED };
            text_item(commands, font, label, color, 12.0)
        })
        .collect();
    commands.entity(row).add_children(&kids);
    row
}

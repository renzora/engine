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

use renzora::{EditorUiBackend, NativePanelIds};
use renzora_ember::dock::{tab_pane, Dock, DockArea, DockDirty, DockLeaf, DockTab, TabPane};
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
        app.init_resource::<renzora::ShellStatusRegistry>();
        app.add_systems(
            Update,
            (
                toggle_backend,
                manage_shell_root,
                apply_panel_meta,
                ribbon_switch,
                content_dispatch,
                top_menu_open,
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

/// Fill each leaf's content with the active panel's UI. Panels that registered a
/// **bevy-native** renderer (`NativePanelIds`) own their own `content` entity and
/// are skipped here. For the rest: the `gallery_*` ember showcases, and a
/// centered title placeholder for everything else. Shares the `PanelContent`
/// marker with native panels so the two never desync over one content entity.
fn content_dispatch(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    native: Option<Res<NativePanelIds>>,
    leaves: Query<&DockLeaf>,
    panes: Query<&TabPane>,
    children_q: Query<&Children>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for leaf in &leaves {
        if leaf.active.is_empty() {
            continue;
        }
        // A panel crate renders this id itself — leave its content alone.
        if native
            .as_ref()
            .is_some_and(|n| n.0.contains(&leaf.active))
        {
            continue;
        }
        // Build the active tab's pane once (lazily). If it already exists, do
        // nothing — `sync_panes` toggles its visibility on tab switch.
        let exists = children_q.get(leaf.content).is_ok_and(|kids| {
            kids.iter()
                .any(|c| panes.get(c).is_ok_and(|p| p.id == leaf.active))
        });
        if exists {
            continue;
        }
        let built = build_panel_content(&mut commands, &fonts, &leaf.active);
        let pane = tab_pane(&mut commands, &leaf.active, built, true);
        commands.entity(leaf.content).add_child(pane);
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
                // Zero minimum so a tall panel's content can't inflate the dock
                // area's min-content height and push it past the window (the
                // flexbox min-content trap — `overflow: clip` alone doesn't
                // override it). Without this, tall content blows up every leaf.
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_basis: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            DockArea,
            Name::new("dock-area"),
        ))
        .id();

    let statusbar = build_status_bar(commands, font);

    commands
        .entity(root)
        .add_children(&[top_bar, doctabs, dock_area, statusbar]);
}

/// The bottom status bar: a "Ready" label + plugin-contributed items from the
/// bevy-native `ShellStatusRegistry`, rendered via a reactive keyed list (so live
/// metrics update without rebuilding the bar).
fn build_status_bar(commands: &mut Commands, _font: &Handle<Font>) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(WINDOW_BG)),
            Name::new("status-bar"),
        ))
        .id();
    renzora_ember::reactive::keyed_list(commands, bar, status_snapshot);
    bar
}

enum StatusRow {
    Label(String, (u8, u8, u8)),
    Seg(renzora::ShellStatusSegment),
    Spacer,
}

/// Flatten the status registry into keyed rows: a Ready label + left items + a
/// flex spacer + right items (each item's `render` is recomputed every frame).
fn status_snapshot(world: &World) -> renzora_ember::reactive::KeyedSnapshot {
    use renzora::ShellStatusAlign;
    use std::hash::{Hash, Hasher};

    let mut rows: Vec<StatusRow> = vec![StatusRow::Label("Ready".to_string(), TEXT_MUTED)];
    if let Some(reg) = world.get_resource::<renzora::ShellStatusRegistry>() {
        let mut left: Vec<&renzora::ShellStatusItem> = reg
            .items
            .iter()
            .filter(|i| i.align == ShellStatusAlign::Left)
            .collect();
        left.sort_by_key(|i| i.order);
        for it in left {
            rows.extend((it.render)(world).into_iter().map(StatusRow::Seg));
        }
        rows.push(StatusRow::Spacer);
        let mut right: Vec<&renzora::ShellStatusItem> = reg
            .items
            .iter()
            .filter(|i| i.align == ShellStatusAlign::Right)
            .collect();
        right.sort_by_key(|i| i.order);
        for it in right {
            rows.extend((it.render)(world).into_iter().map(StatusRow::Seg));
        }
    } else {
        rows.push(StatusRow::Spacer);
    }

    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match r {
                StatusRow::Label(t, c) => {
                    (0u8, t, c).hash(&mut h);
                }
                StatusRow::Seg(s) => {
                    (1u8, &s.icon, &s.text, s.color).hash(&mut h);
                }
                StatusRow::Spacer => 2u8.hash(&mut h),
            }
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| status_row(c, f, &rows[i])),
    }
}

fn status_row(commands: &mut Commands, fonts: &EmberFonts, row: &StatusRow) -> Entity {
    match row {
        StatusRow::Spacer => commands.spawn(Node { flex_grow: 1.0, ..default() }).id(),
        StatusRow::Label(text, color) => commands
            .spawn((
                Text::new(text.clone()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(*color)),
            ))
            .id(),
        StatusRow::Seg(s) => {
            let r = commands
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .id();
            let mut kids = Vec::new();
            let color = (s.color[0], s.color[1], s.color[2]);
            if !s.icon.is_empty() {
                let glyph = renzora_ember::font::icon_glyph(&s.icon)
                    .unwrap_or_else(|| s.icon.chars().next().unwrap_or(' '));
                kids.push(
                    commands
                        .spawn((
                            Text::new(glyph.to_string()),
                            TextFont {
                                font: fonts.phosphor.clone(),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(rgb(color)),
                        ))
                        .id(),
                );
            }
            kids.push(
                commands
                    .spawn((
                        Text::new(s.text.clone()),
                        ui_font(&fonts.ui, 11.0),
                        TextColor(rgb(color)),
                    ))
                    .id(),
            );
            commands.entity(r).add_children(&kids);
            r
        }
    }
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
    let left_kids = vec![
        top_menu_item(commands, font, "File", TopMenuKind::File),
        top_menu_item(commands, font, "Edit", TopMenuKind::Edit),
        top_menu_item(commands, font, "View", TopMenuKind::View),
        top_menu_item(commands, font, "Help", TopMenuKind::Help),
    ];
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


// ── Top-bar menus (File / Edit / View / Help) ────────────────────────────────

#[derive(Clone, Copy)]
enum TopMenuKind {
    File,
    Edit,
    View,
    Help,
}

#[derive(Component)]
struct TopMenu(TopMenuKind);

/// An interactive top-bar menu title (File/Edit/View/Help).
fn top_menu_item(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    kind: TopMenuKind,
) -> Entity {
    let item = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TopMenu(kind),
            Name::new(format!("menu:{label}")),
        ))
        .id();
    renzora_ember::reactive::bind_bg(commands, item, move |w| match w.get::<Interaction>(item) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb((46, 46, 54)),
        _ => Color::NONE,
    });
    commands.entity(item).with_children(|p| {
        p.spawn((
            Text::new(label),
            ui_font(font, 14.0),
            TextColor(rgb(TEXT_MUTED)),
        ));
    });
    item
}

/// Click a top-bar title → open its menu via the shared ember `screen_menu`,
/// anchored to the button's bottom-left (stable, independent of cursor position).
fn top_menu_open(
    q: Query<
        (
            &Interaction,
            &TopMenu,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        Changed<Interaction>,
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for (interaction, menu, rcp, cn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(pos) = anchor_below(&windows, rcp, cn) else {
            continue;
        };
        let root = renzora_ember::widgets::screen_menu(&mut commands, pos.x, pos.y);
        let kids = build_menu_items(&mut commands, &fonts, menu.0);
        commands.entity(root).add_children(&kids);
    }
}

/// The bottom-left of a node in logical window px, derived from the cursor + the
/// node's normalized cursor position (scale-invariant; avoids UI `GlobalTransform`
/// coordinate ambiguity). Used to anchor button dropdowns just under the button.
fn anchor_below(
    windows: &Query<&Window>,
    rcp: &bevy::ui::RelativeCursorPosition,
    cn: &bevy::ui::ComputedNode,
) -> Option<Vec2> {
    let cursor = windows.iter().next().and_then(|w| w.cursor_position())?;
    let size = cn.size() * cn.inverse_scale_factor();
    let norm = rcp.normalized.unwrap_or(Vec2::ZERO);
    let top_left = cursor - (norm + Vec2::splat(0.5)) * size;
    Some(Vec2::new(top_left.x, top_left.y + size.y + 2.0))
}

fn build_menu_items(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: TopMenuKind,
) -> Vec<Entity> {
    use renzora_ember::widgets::{menu_item, menu_sep};
    match kind {
        TopMenuKind::File => vec![
            menu_item(commands, fonts, "folder-plus", "New Project", |w| {
                renzora_editor::handle_new_project(w)
            }),
            menu_item(commands, fonts, "folder-open", "Open Project…", |w| {
                renzora_editor::handle_open_project(w)
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "file-plus", "New Scene", |w| {
                w.insert_resource(renzora::core::NewSceneRequested);
            }),
            menu_item(commands, fonts, "file", "Open Scene…", |w| {
                w.insert_resource(renzora::core::OpenSceneRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "floppy-disk", "Save", |w| {
                w.insert_resource(renzora::core::SaveSceneRequested);
            }),
            menu_item(commands, fonts, "floppy-disk-back", "Save As…", |w| {
                w.insert_resource(renzora::core::SaveAsSceneRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "package", "Export Project…", |w| {
                w.insert_resource(renzora::core::ExportRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "gear", "Settings", |w| {
                if let Some(mut s) = w.get_resource_mut::<renzora_editor::EditorSettings>() {
                    s.show_settings = !s.show_settings;
                }
            }),
        ],
        TopMenuKind::Edit => vec![
            menu_item(commands, fonts, "arrow-u-up-left", "Undo", |w| {
                let f = w.get_resource::<renzora_editor::EditorActionHooks>().and_then(|h| h.undo);
                if let Some(f) = f {
                    f(w);
                }
            }),
            menu_item(commands, fonts, "arrow-u-up-right", "Redo", |w| {
                let f = w.get_resource::<renzora_editor::EditorActionHooks>().and_then(|h| h.redo);
                if let Some(f) = f {
                    f(w);
                }
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "layout", "Reset Layout", reset_layout_action),
        ],
        TopMenuKind::View => vec![
            menu_item(commands, fonts, "magnifying-glass-plus", "Zoom In", |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ZoomIn);
            }),
            menu_item(commands, fonts, "magnifying-glass-minus", "Zoom Out", |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ZoomOut);
            }),
            menu_item(commands, fonts, "magnifying-glass", "Reset Zoom", |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ResetZoom);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "corners-out", "Fit All", |w| {
                w.insert_resource(renzora::core::CameraViewRequest::FrameAll);
            }),
            menu_item(commands, fonts, "eye", "Isolation Mode", |w| {
                let mut iso = w
                    .remove_resource::<renzora::core::IsolationMode>()
                    .unwrap_or_default();
                iso.active = !iso.active;
                w.insert_resource(iso);
            }),
        ],
        TopMenuKind::Help => vec![
            menu_item(commands, fonts, "graduation-cap", "Getting Started Tutorial", |w| {
                w.insert_resource(renzora::core::TutorialRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "book-open", "Documentation", |_| {
                open_url("https://renzora.com/docs")
            }),
            menu_item(commands, fonts, "youtube-logo", "YouTube", |_| {
                open_url("https://youtube.com/@renzoragame")
            }),
            menu_item(commands, fonts, "discord-logo", "Discord", |_| {
                open_url("https://discord.gg/9UHUGUyDJv")
            }),
            menu_item(commands, fonts, "github-logo", "GitHub", |_| {
                open_url("https://github.com/renzora/engine")
            }),
        ],
    }
}

/// Reset the active workspace's dock tree to its registered layout.
fn reset_layout_action(w: &mut World) {
    let tree = w
        .get_resource::<ShellLayouts>()
        .and_then(|l| l.layouts.get(l.active).map(|(_, t)| t.clone()));
    if let Some(tree) = tree {
        if let Some(mut dock) = w.get_resource_mut::<Dock>() {
            dock.tree = tree;
        }
    }
    if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
        d.0 = true;
    }
}

/// Open `url` in the user's default browser (cross-platform).
fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

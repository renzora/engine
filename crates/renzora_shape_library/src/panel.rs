//! The shape library panel: a toolbar (search, a hint, a tile-size slider), a
//! sidebar of categories with counts, and a grid of lit shape thumbnails in
//! collapsible sections. Clicking a tile spawns that shape at the origin
//! (undoable `SpawnShapeCmd`); dragging one hands off to the viewport.
//!
//! It used to be one flat wrap of thirty icon tiles under a search box. With
//! icons shared between unrelated shapes and no grouping, finding "the stairs"
//! meant reading every label; the sidebar and the renders (see `thumbs`) are
//! what make the panel scannable.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::ShapeRegistry;
use renzora_editor_framework::{EditorCommands, ShapeDragState, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_with, keyed_list_tokened};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{ellipsize, scroll_view, slider_ranged, text_input, EmberTextInput, HoverTooltip};
use renzora_ember::UiInteraction;
use renzora_undo::{self, SpawnShapeCmd, UndoContext};

use crate::groups::{color_of, group_of, GROUPS};
use crate::thumbs::ShapeThumbs;

/// Tile thumbnail size range for the slider, and where it starts.
const SIZE_MIN: f32 = 48.0;
const SIZE_MAX: f32 = 128.0;
const SIZE_DEFAULT: f32 = 64.0;

pub struct ShapeLibraryPanel;

impl Plugin for ShapeLibraryPanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShapesState>();
        app.init_resource::<ShapePress>();
        crate::thumbs::register(app);
        // Not scrolled by the dock: the sidebar and toolbar stay put and only
        // the grid scrolls.
        app.register_panel_content("shape_library", false, build).systems(
            Update,
            (shape_search_sync, category_click, section_toggle, shape_press, shape_drag_or_click)
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

#[derive(Resource)]
struct ShapesState {
    search: String,
    /// The sidebar's pick; `None` is All.
    group: Option<&'static str>,
    collapsed: HashSet<&'static str>,
    /// Thumbnail edge in px, from the slider.
    size: f32,
}

impl Default for ShapesState {
    fn default() -> Self {
        Self { search: String::new(), group: None, collapsed: HashSet::new(), size: SIZE_DEFAULT }
    }
}

/// A pressed-but-not-yet-resolved tile: becomes a drag once the cursor moves,
/// or a click (spawn at origin) on release in place.
#[derive(Resource, Default)]
struct ShapePress(Option<PressInfo>);

struct PressInfo {
    id: &'static str,
    name: &'static str,
    color: Color,
    start: Vec2,
}

#[derive(Component)]
struct ShapesSearch;
#[derive(Component)]
struct ShapeTile {
    id: &'static str,
    name: &'static str,
    color: Color,
}
/// A sidebar row; `None` is All.
#[derive(Component)]
struct CategoryBtn(Option<&'static str>);
/// A section header, which folds its section.
#[derive(Component)]
struct SectionHeader(&'static str);

/// (id, name, icon, colour, group) for every shape the search admits.
type ShapeRow = (&'static str, &'static str, &'static str, Color, &'static str);

fn matching(world: &Rx, search: &str) -> Vec<ShapeRow> {
    let Some(reg) = world.get_resource::<ShapeRegistry>() else { return Vec::new() };
    reg.iter()
        .filter(|s| search.is_empty() || s.name.to_lowercase().contains(search))
        .map(|s| (s.id, s.name, s.icon, s.default_color, group_of(s.id)))
        .collect()
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("shape-library"),
        ))
        .id();

    // ── Toolbar ──
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(border())),
        ))
        .id();
    let search = text_input(commands, &fonts.ui, "Search shapes...", "");
    commands.entity(search).insert((
        ShapesSearch,
        Node {
            width: Val::Px(300.0),
            min_width: Val::Px(120.0),
            flex_shrink: 1.0,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let hint = commands
        .spawn((
            Text::new(renzora::lang::t_or(
                "shapes.hint",
                "Click to add at the origin, or drag into the viewport",
            )),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { flex_shrink: 1.0, min_width: Val::Px(0.0), ..default() },
        ))
        .id();
    ellipsize(commands, hint, renzora::lang::t_or("shapes.hint", "Click to add at the origin, or drag into the viewport"), None);
    let small = icon_text(commands, &fonts.phosphor, "square", text_muted(), 10.0);
    let size = slider_ranged(commands, SIZE_DEFAULT, SIZE_MIN, SIZE_MAX);
    commands.entity(size).entry::<Node>().and_modify(|mut n| {
        n.width = Val::Px(90.0);
        n.flex_shrink = 0.0;
    });
    bind_2way(
        commands,
        size,
        |rx| rx.get_resource::<ShapesState>().map(|s| s.size).unwrap_or(SIZE_DEFAULT),
        |w, v: &f32| {
            if let Some(mut s) = w.get_resource_mut::<ShapesState>() {
                // Whole pixels: the grid rebuilds on every change, and a drag
                // otherwise rebuilds it for sub-pixel moves nobody can see.
                let v = v.round();
                if s.size != v {
                    s.size = v;
                }
            }
        },
    );
    let large = icon_text(commands, &fonts.phosphor, "square", text_muted(), 15.0);
    commands.entity(bar).add_children(&[search, spacer, hint, small, size, large]);

    // ── Body: sidebar + grid ──
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();
    let sidebar = commands
        .spawn((
            Node {
                width: Val::Px(170.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(8.0)),
                border: UiRect::right(Val::Px(1.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(rgb(border())),
        ))
        .id();
    keyed_list_tokened(commands, sidebar, sidebar_token, sidebar_snapshot);

    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
            ..default()
        })
        .id();
    keyed_list_tokened(commands, grid, grid_token, grid_snapshot);
    let scroll = scroll_view(commands, grid);
    commands.entity(scroll).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_width = Val::Px(0.0);
    });
    commands.entity(body).add_children(&[sidebar, scroll]);
    commands.entity(root).add_children(&[bar, body]);
    root
}

fn hash_of<T: Hash>(v: T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

fn state_key(world: &Rx) -> (String, Option<&'static str>, Vec<&'static str>, u32) {
    let Some(s) = world.get_resource::<ShapesState>() else {
        return (String::new(), None, Vec::new(), SIZE_DEFAULT as u32);
    };
    let mut collapsed: Vec<&'static str> = s.collapsed.iter().copied().collect();
    collapsed.sort_unstable();
    (s.search.to_lowercase(), s.group, collapsed, s.size as u32)
}

// ── Sidebar ──────────────────────────────────────────────────────────────────

fn sidebar_token(world: &Rx) -> u64 {
    let (search, group, _, _) = state_key(world);
    hash_of((search, group))
}

fn sidebar_snapshot(world: &Rx) -> KeyedSnapshot {
    let (search, picked, _, _) = state_key(world);
    let shapes = matching(world, &search);
    // All, then every group that has something in it, with counts that follow
    // the search so the sidebar says where the matches are.
    let mut rows: Vec<(Option<&'static str>, usize)> = vec![(None, shapes.len())];
    for (g, _) in GROUPS {
        let n = shapes.iter().filter(|s| s.4 == *g).count();
        if n > 0 {
            rows.push((Some(*g), n));
        }
    }
    let items = rows.iter().map(|(g, n)| (hash_of(g), hash_of((n, picked == *g)))).collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (group, count) = rows[i];
            category_row(c, f, group, count, picked == group)
        }),
    }
}

fn category_row(commands: &mut Commands, fonts: &EmberFonts, group: Option<&'static str>, count: usize, selected: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(26.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            CategoryBtn(group),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        if selected {
            mix(card_bg(), accent(), 0.22)
        } else if matches!(w.get::<UiInteraction>(row), Some(UiInteraction::Hovered | UiInteraction::Pressed)) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let label = group.unwrap_or("All");
    // The group's colour labels it, as in Add Component; the selected row
    // switches to the primary text colour so the fill carries the selection.
    let color = match (group, selected) {
        (_, true) | (None, _) => text_primary(),
        (Some(g), false) => color_of(g),
    };
    let name = commands
        .spawn((
            Text::new(renzora::lang::t_or(&format!("shapes.group.{}", label.to_lowercase().replace(' ', "_")), label)),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(color)),
            Node { flex_grow: 1.0, ..default() },
        ))
        .id();
    let n = commands
        .spawn((Text::new(count.to_string()), ui_font(&fonts.mono, 10.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(row).add_children(&[name, n]);
    row
}

fn category_click(q: Query<(&UiInteraction, &CategoryBtn), Changed<UiInteraction>>, mut state: ResMut<ShapesState>) {
    for (i, btn) in &q {
        if *i == UiInteraction::Pressed && state.group != btn.0 {
            state.group = btn.0;
        }
    }
}

// ── Grid ─────────────────────────────────────────────────────────────────────

fn grid_token(world: &Rx) -> u64 {
    let ready = world.get_resource::<ShapeThumbs>().map(|t| t.images.len()).unwrap_or(0);
    hash_of((state_key(world), ready))
}

fn grid_snapshot(world: &Rx) -> KeyedSnapshot {
    let (search, picked, collapsed, size) = state_key(world);
    let shapes = matching(world, &search);
    if shapes.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new(renzora::lang::t_or("shapes.none", "No shapes match.")),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                    Node { margin: UiRect::all(Val::Px(8.0)), ..default() },
                ))
                .id()
            }),
        };
    }
    let thumbs: Vec<(&'static str, Handle<Image>)> = world
        .get_resource::<ShapeThumbs>()
        .map(|t| t.images.iter().map(|(k, v)| (*k, v.clone())).collect())
        .unwrap_or_default();
    // (group, folded, members) in display order.
    let sections: Vec<(&'static str, bool, Vec<ShapeRow>)> = GROUPS
        .iter()
        .filter(|(g, _)| picked.is_none_or(|p| p == *g))
        .map(|(g, _)| (*g, collapsed.contains(g), shapes.iter().filter(|s| s.4 == *g).copied().collect::<Vec<_>>()))
        .filter(|(_, _, m)| !m.is_empty())
        .collect();
    let items = sections
        .iter()
        .map(|(g, folded, m)| (hash_of(g), hash_of((folded, m.iter().map(|s| s.0).collect::<Vec<_>>(), size, thumbs.len()))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (group, folded, members) = &sections[i];
            section(c, f, group, *folded, members, size as f32, &thumbs)
        }),
    }
}

fn section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    group: &'static str,
    folded: bool,
    members: &[ShapeRow],
    size: f32,
    thumbs: &[(&'static str, Handle<Image>)],
) -> Entity {
    let root = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    // Header: small caret, the name in the group's colour, a muted count. The
    // same restrained shape as Add Component's sections.
    let header = commands
        .spawn((
            Node {
                height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                align_self: AlignSelf::FlexStart,
                padding: UiRect::right(Val::Px(6.0)),
                ..default()
            },
            UiInteraction::default(),
            SectionHeader(group),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, if folded { "caret-right" } else { "caret-down" }, text_muted(), 10.0);
    let name = commands
        .spawn((
            Text::new(renzora::lang::t_or(&format!("shapes.group.{}", group.to_lowercase().replace(' ', "_")), group)),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(color_of(group))),
        ))
        .id();
    let count = commands
        .spawn((Text::new(members.len().to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(header).add_children(&[caret, name, count]);
    commands.entity(root).add_child(header);
    if folded {
        return root;
    }
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let tiles: Vec<Entity> = members
        .iter()
        .map(|&(id, name, icon, color, _)| {
            let thumb = thumbs.iter().find(|(k, _)| *k == id).map(|(_, h)| h.clone());
            shape_tile(commands, fonts, id, name, icon, color, size, thumb)
        })
        .collect();
    commands.entity(wrap).add_children(&tiles);
    commands.entity(root).add_child(wrap);
    root
}

#[allow(clippy::too_many_arguments)]
fn shape_tile(
    commands: &mut Commands,
    fonts: &EmberFonts,
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    color: Color,
    size: f32,
    thumb: Option<Handle<Image>>,
) -> Entity {
    let tile = commands
        .spawn((
            Node {
                width: Val::Px(size + 20.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::new(Val::Px(4.0), Val::Px(4.0), Val::Px(6.0), Val::Px(5.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new(name),
            ShapeTile { id, name, color },
            Name::new(format!("shape:{id}")),
        ))
        .id();
    // Hover shows on the border: on a dark theme a background step of a few
    // percent is not enough to tell which of thirty tiles is under the cursor.
    bind_with(
        commands,
        tile,
        move |w| matches!(w.get::<UiInteraction>(tile), Some(UiInteraction::Hovered | UiInteraction::Pressed)),
        |w, e, hot: &bool| {
            let c = if *hot { rgb(accent()) } else { rgb(border()) };
            if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                *b = BorderColor::all(c);
            }
        },
    );
    let picture = match thumb {
        Some(handle) => commands
            .spawn((
                ImageNode::new(handle),
                Node { width: Val::Px(size), height: Val::Px(size), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            ))
            .id(),
        // Only until the registry has been read; the image exists before its
        // render does, so this is a one-frame state at most.
        None => {
            let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), size * 0.5);
            commands.entity(ic).insert(Node {
                width: Val::Px(size),
                height: Val::Px(size),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            });
            ic
        }
    };
    let lbl = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            Node { max_width: Val::Px(size + 12.0), ..default() },
        ))
        .id();
    ellipsize(commands, lbl, name, None);
    commands.entity(tile).add_children(&[picture, lbl]);
    tile
}

fn section_toggle(q: Query<(&UiInteraction, &SectionHeader), Changed<UiInteraction>>, mut state: ResMut<ShapesState>) {
    for (i, h) in &q {
        if *i == UiInteraction::Pressed && !state.collapsed.remove(h.0) {
            state.collapsed.insert(h.0);
        }
    }
}

fn shape_search_sync(input: Query<&EmberTextInput, With<ShapesSearch>>, mut state: ResMut<ShapesState>) {
    for inp in &input {
        if state.search != inp.value {
            state.search = inp.value.clone();
        }
    }
}

/// Record a press on a tile (with the cursor position) as a pending drag/click.
fn shape_press(
    q: Query<(&UiInteraction, &ShapeTile), Changed<UiInteraction>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut press: ResMut<ShapePress>,
) {
    for (interaction, tile) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let start = windows.single().ok().and_then(|w| w.cursor_position()).unwrap_or(Vec2::ZERO);
        press.0 = Some(PressInfo { id: tile.id, name: tile.name, color: tile.color, start });
    }
}

/// Resolve a pending press: drag once the cursor moves past a threshold (hand
/// off to the viewport via `ShapeDragState`), else spawn at the origin on
/// release (the click fallback).
fn shape_drag_or_click(
    mut press: ResMut<ShapePress>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut drag: Option<ResMut<ShapeDragState>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(info) = press.0.as_ref() else { return };

    if mouse.just_released(MouseButton::Left) {
        // No drag happened (else `press` would be cleared) → spawn at origin.
        if let Some(cmds) = cmds {
            let (shape_id, name, color) = (info.id.to_string(), info.name.to_string(), info.color);
            cmds.push(move |world: &mut World| {
                renzora_undo::execute(
                    world,
                    UndoContext::Scene,
                    Box::new(SpawnShapeCmd { entity: Entity::PLACEHOLDER, shape_id, name, position: Vec3::ZERO, color }),
                );
            });
        }
        press.0 = None;
        return;
    }

    let cursor = windows.single().ok().and_then(|w| w.cursor_position());
    if let (Some(cursor), Some(drag)) = (cursor, drag.as_mut()) {
        if (cursor - info.start).length() > 6.0 {
            drag.dragging_shape = Some(info.id);
            drag.native_drag = true;
            press.0 = None;
        }
    }
}

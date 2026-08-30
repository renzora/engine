//! Bevy-native (ember) shape library panel: a search box over a wrapping grid
//! of shape tiles (icon + name). Clicking a tile spawns that shape at the origin
//! (undoable `SpawnShapeCmd`). Reads `ShapeRegistry`.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::ShapeRegistry;
use renzora_editor_framework::{EditorCommands, ShapeDragState, SplashState};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_bg};
use renzora_ember::theme::*;
use renzora_ember::widgets::{text_input, EmberTextInput};
use renzora_undo::{self, SpawnShapeCmd, UndoContext};

/// Tile width. 58 was too narrow for the names the library actually has —
/// "Quarter Pipe", "Half Cylinder", "Spiral Stairs", "Window Wall" all broke
/// onto a second line, and because the tile's height was fixed at `TILE_W + 16`
/// that second line was clipped and every row it appeared in sat ragged against
/// its neighbours.
const TILE_W: f32 = 76.0;
/// Height reserved for the label: two lines at 10px, always, whether or not the
/// name uses both. Reserving the space is what keeps a row of tiles level; the
/// alternative is rows that jump by a line depending on which shapes matched the
/// search.
const LABEL_H: f32 = 26.0;
/// Total tile height: the icon block plus the label block plus padding.
const TILE_H: f32 = 34.0 + LABEL_H + 12.0;

pub struct NativeShapeLibrary;

impl Plugin for NativeShapeLibrary {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShapesState>();
        app.init_resource::<ShapePress>();
        app.register_panel_content("shape_library", true, build)
            .systems(
            Update,
            (shape_search_sync, shape_press, shape_drag_or_click).run_if(in_state(SplashState::Editor)),
        );
    }
}

#[derive(Resource, Default)]
struct ShapesState {
    search: String,
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

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .id();

    let search = text_input(commands, &fonts.ui, "Search shapes...", "");
    commands.entity(search).insert((
        ShapesSearch,
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));

    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    renzora_ember::virtual_scroll::virtual_scroll_versioned(
        commands,
        grid,
        6,
        shapes_token,
        shapes_snapshot,
    );

    commands.entity(root).add_children(&[search, grid]);
    root
}

/// Dirty token: the shape set is static, so only the search box changes which
/// shapes show. Combined with the scroll-window term, the snapshot is skipped
/// on frames where neither changed.
fn shapes_token(world: &Rx) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    world
        .get_resource::<ShapesState>()
        .map(|s| s.search.to_lowercase())
        .unwrap_or_default()
        .hash(&mut h);
    h.finish()
}

fn shapes_snapshot(world: &Rx) -> KeyedSnapshot {
    let search = world.get_resource::<ShapesState>().map(|s| s.search.to_lowercase()).unwrap_or_default();
    let Some(reg) = world.get_resource::<ShapeRegistry>() else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) };
    };
    let shapes: Vec<(&'static str, &'static str, &'static str, Color)> = reg
        .iter()
        .filter(|s| search.is_empty() || s.name.to_lowercase().contains(&search))
        .map(|s| (s.id, s.name, s.icon, s.default_color))
        .collect();
    if shapes.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new("No shapes match."),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                    Node { margin: UiRect::all(Val::Px(8.0)), ..default() },
                ))
                .id()
            }),
        };
    }
    let items: Vec<(u64, u64)> = shapes
        .iter()
        .map(|(id, _, _, _)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            id.hash(&mut h);
            (h.finish(), 0)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, name, icon, color) = shapes[i];
            shape_tile(c, f, id, name, icon, color)
        }),
    }
}

fn shape_tile(
    commands: &mut Commands,
    fonts: &EmberFonts,
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    color: Color,
) -> Entity {
    let tile = commands
        .spawn((
            Node {
                width: Val::Px(TILE_W),
                height: Val::Px(TILE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::vertical(Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            // The full name, for the ones the two-line label still can't hold.
            renzora_ember::widgets::HoverTooltip::new(name),
            ShapeTile { id, name, color },
            Name::new(format!("shape:{id}")),
        ))
        .id();
    bind_bg(commands, tile, move |w| {
        if matches!(
            w.get::<Interaction>(tile),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            rgb(section_bg())
        }
    });
    // The border does the hover work the background alone was too subtle for —
    // `section_bg` to `hover_bg` is a couple of percent of lightness on a dark
    // theme, and on a grid of thirty tiles that is not enough to tell which one
    // is under the cursor.
    renzora_ember::reactive::tracked::bind_with(
        commands,
        tile,
        move |w| {
            matches!(
                w.get::<Interaction>(tile),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            )
        },
        |w, e, hot: &bool| {
            let c = if *hot { rgb(accent()) } else { rgb(border()) };
            if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                *b = BorderColor::all(c);
            }
        },
    );

    // Icon in a fixed block, so the label starts at the same height on every
    // tile whatever the glyph's own metrics are.
    let ic = renzora_ember::font::icon_text(commands, &fonts.phosphor, icon, text_primary(), 24.0);
    commands.entity(ic).insert(Node {
        height: Val::Px(30.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    });

    let lbl = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 10.0),
            // `text_primary`, not `text_muted`: the name is what you read to
            // pick a shape, and several of the icons are shared between shapes
            // (Sphere and Hemisphere are the same globe), so the label is
            // frequently the only thing telling two tiles apart.
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::justify(bevy::text::Justify::Center),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(LABEL_H),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(tile).add_children(&[ic, lbl]);
    tile
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
    q: Query<(&Interaction, &ShapeTile), Changed<Interaction>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut press: ResMut<ShapePress>,
) {
    for (interaction, tile) in &q {
        if *interaction != Interaction::Pressed {
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

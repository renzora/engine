//! Bevy-native (ember) port of the egui `ShapeLibraryPanel`: a search box over a
//! wrapping grid of shape tiles (icon + name). Clicking a tile spawns that shape
//! at the origin (undoable `SpawnShapeCmd`). Reads `ShapeRegistry`.

use bevy::prelude::*;

use renzora::core::ShapeRegistry;
use renzora_editor::{EditorCommands, SplashState};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_bg, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{text_input, EmberTextInput};
use renzora_undo::{self, SpawnShapeCmd, UndoContext};

const TILE_W: f32 = 58.0;

pub struct NativeShapeLibrary;

impl Plugin for NativeShapeLibrary {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShapesState>();
        app.register_panel_content("shape_library", true, build);
        app.add_systems(
            Update,
            (shape_search_sync, shape_click).run_if(in_state(SplashState::Editor)),
        );
    }
}

#[derive(Resource, Default)]
struct ShapesState {
    search: String,
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
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(6.0)),
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
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    keyed_list(commands, grid, shapes_snapshot);

    commands.entity(root).add_children(&[search, grid]);
    root
}

fn shapes_snapshot(world: &World) -> KeyedSnapshot {
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
                height: Val::Px(TILE_W + 16.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(Color::NONE),
            Interaction::default(),
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
    let ic = commands
        .spawn((
            Text::new(icon.to_string()),
            TextFont { font: fonts.phosphor.clone(), font_size: 26.0, ..default() },
            TextColor(rgb(text_primary())),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::new_with_justify(bevy::text::Justify::Center),
            Node { max_width: Val::Px(TILE_W - 4.0), overflow: Overflow::clip(), ..default() },
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

fn shape_click(q: Query<(&Interaction, &ShapeTile), Changed<Interaction>>, cmds: Option<Res<EditorCommands>>) {
    let Some(cmds) = cmds else { return };
    for (interaction, tile) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (shape_id, name, color) = (tile.id.to_string(), tile.name.to_string(), tile.color);
        cmds.push(move |world: &mut World| {
            renzora_undo::execute(
                world,
                UndoContext::Scene,
                Box::new(SpawnShapeCmd {
                    entity: Entity::PLACEHOLDER,
                    shape_id,
                    name,
                    position: Vec3::ZERO,
                    color,
                }),
            );
        });
    }
}

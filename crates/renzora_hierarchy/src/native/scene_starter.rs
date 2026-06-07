//! Empty-scene starter picker for the native hierarchy. When the scene has no
//! entities, the tree is replaced by a set of cards — one per registered
//! [`SceneStarter`] (3D/2D camera, Environment, UI Canvas, …) — each of which
//! spawns that starter's entities on click. Mirrors the egui panel's picker.

use bevy::prelude::*;

use renzora_editor_framework::{EditorCommands, SceneStarterRegistry, SplashState};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::{keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;

use crate::cache::HierarchyTreeCache;

/// A starter card → spawns the starter with this id on click.
#[derive(Component)]
pub(crate) struct HierStarterCard(&'static str);

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, starter_click.run_if(in_state(SplashState::Editor)));
}

/// True when the scene has no entities (the picker should show).
pub(crate) fn scene_is_empty(world: &World) -> bool {
    world.get_resource::<HierarchyTreeCache>().is_none_or(|c| c.nodes.is_empty())
}

/// Build the picker container (header + a reactive list of starter cards). Shown
/// via `bind_display` only while the scene is empty.
pub(crate) fn build_picker(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("hier-starter-picker"),
        ))
        .id();

    let title = commands
        .spawn((
            Text::new("This scene is empty"),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_primary())),
            Node { margin: UiRect::bottom(Val::Px(2.0)), ..default() },
        ))
        .id();
    let sub = commands
        .spawn((
            Text::new("Pick a starter, or just add entities manually."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() },
        ))
        .id();

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, starter_snapshot);

    commands.entity(root).add_children(&[title, sub, list]);
    root
}

fn starter_snapshot(world: &World) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    // (id, title, description, icon-glyph).
    let cards: Vec<(&'static str, &'static str, &'static str, &'static str)> = world
        .get_resource::<SceneStarterRegistry>()
        .map(|r| r.iter().map(|s| (s.id, s.title, s.description, s.icon)).collect())
        .unwrap_or_default();
    let items: Vec<(u64, u64)> = cards
        .iter()
        .map(|(id, ..)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            id.hash(&mut h);
            (h.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, title, desc, icon) = cards[i];
            build_card(c, f, id, title, desc, icon)
        }),
    }
}

fn build_card(commands: &mut Commands, fonts: &EmberFonts, id: &'static str, title: &str, desc: &str, icon: &str) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(52.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            HierStarterCard(id),
            Name::new("hier-starter-card"),
        ))
        .id();
    // The starter icon is a phosphor glyph char (rendered with the phosphor font).
    let glyph = commands
        .spawn((
            Text::new(icon.to_string()),
            TextFont { font: fonts.phosphor.clone(), font_size: 22.0, ..default() },
            TextColor(rgb(text_primary())),
        ))
        .id();
    let text_col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() })
        .id();
    let t = commands.spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::new_with_no_wrap())).id();
    let d = commands.spawn((Text::new(desc.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())), bevy::text::TextLayout::new_with_no_wrap())).id();
    commands.entity(text_col).add_children(&[t, d]);
    commands.entity(card).add_children(&[glyph, text_col]);
    card
}

fn starter_click(
    q: Query<(&Interaction, &HierStarterCard), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, card) in &q {
        if *interaction == Interaction::Pressed {
            let id = card.0;
            cmds.push(move |world: &mut World| {
                let spawn = world
                    .get_resource::<SceneStarterRegistry>()
                    .and_then(|r| r.get(id))
                    .map(|s| s.spawn_fn);
                if let Some(spawn) = spawn {
                    spawn(world);
                }
            });
        }
    }
}

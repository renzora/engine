//! Starter picker for the native hierarchy. When the tree is empty it is
//! replaced by a header, a two-column grid of tiles (one per registered
//! [`SceneStarter`]: 3D/2D camera, Environment, UI Canvas, ...) and a row of
//! **common entities** (Cube, Sphere, Plane, Light, Camera, Empty) that each
//! spawn one thing.
//!
//! The common entities are there because a starter is a whole arrangement, and
//! the most frequent thing to want in an empty scene is one cube to look at.
//! Before, that meant opening Add Entity and searching for it, from a panel
//! that was offering everything except the obvious.
//!
//! The tree can be empty for two different reasons, and the picker says which:
//! the scene really has nothing in it, or a [`HierarchyFilter`] has narrowed it
//! to a component set that nothing currently matches. In the second case only
//! starters that *produce* something the filter admits are offered (a scene
//! full of geometry, filtered to UI canvases, was offering to build a terrain),
//! and the common entities are hidden, since none of them would show up in a
//! filtered tree either.

use bevy::prelude::*;

use renzora::core::ShapeRegistry;
use renzora_editor_framework::{
    EditorCommands, HierarchyFilter, SceneStarterRegistry, SplashState,
};
use renzora_ember::font::{icon_glyph, ui_font, EmberFonts};
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_display, bind_text, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::{scroll_view, HoverTint};
use renzora_ember::UiInteraction;
use renzora_undo::{execute, SpawnEntityCmd, SpawnEntityKind, SpawnShapeCmd, UndoContext};

use crate::cache::HierarchyTreeCache;

/// A starter tile → spawns the starter with this id on click.
#[derive(Component)]
pub(crate) struct HierStarterCard(&'static str);

/// What a common-entity tile spawns: a mesh shape from the [`ShapeRegistry`]
/// or a preset from the spawn registry, the same two sources Add Entity uses.
#[derive(Clone, Copy)]
enum QuickAdd {
    Shape(&'static str),
    Preset(&'static str),
}

/// A common-entity tile.
#[derive(Component)]
pub(crate) struct HierQuickAdd(QuickAdd);

/// (label, phosphor icon, what it spawns). Ids are looked up at click time, so
/// a build without one of them simply does nothing for that tile.
const COMMON: [(&str, &str, QuickAdd); 6] = [
    ("Cube", "cube", QuickAdd::Shape("cube")),
    ("Sphere", "sphere", QuickAdd::Shape("sphere")),
    ("Plane", "square", QuickAdd::Shape("plane")),
    ("Light", "lightbulb", QuickAdd::Preset("point_light")),
    ("Camera", "video-camera", QuickAdd::Preset("camera_3d")),
    ("Empty", "circle-dashed", QuickAdd::Preset("empty_entity")),
];

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (starter_click, quick_add_click).run_if(in_state(SplashState::Editor)),
    );
}

/// True when the scene has no entities (the picker should show).
pub(crate) fn scene_is_empty(world: &Rx) -> bool {
    world.get_resource::<HierarchyTreeCache>().is_none_or(|c| c.nodes.is_empty())
}

/// Is the tree narrowed to a component set rather than showing the whole scene?
///
/// Decides what the picker *says*, not what it offers: an empty tree means
/// something different when a filter is producing it.
fn is_scoped(world: &Rx) -> bool {
    matches!(
        world.get_resource::<HierarchyFilter>(),
        Some(HierarchyFilter::OnlyWithComponents(_))
    )
}

fn glyph(commands: &mut Commands, fonts: &EmberFonts, icon: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    // `icon` is a kebab-case Phosphor *name*, so resolve it to its single PUA
    // glyph. Passing the raw name lets ligature substitution mangle multi-word
    // names ("image-square" renders as two glyphs) and an unmatched name renders
    // nothing at all.
    let ch = icon_glyph(icon).unwrap_or('\u{E4C6}'); // fallback: "question"
    commands
        .spawn((
            Text::new(ch.to_string()),
            TextFont {
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(size),
                ..default()
            },
            TextColor(rgb(color)),
        ))
        .id()
}

/// Card fill with live hover feedback; theme tokens, never a white wash.
fn tile_tint() -> HoverTint {
    HoverTint::solid(rgb(card_bg()), rgb(hover_bg()), mix(hover_bg(), accent(), 0.18))
}

fn section_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_uppercase()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id()
}

/// Build the picker (header, starter grid, common entities). Shown via
/// `bind_display` only while the scene is empty.
pub(crate) fn build_picker(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(24.0), Val::Px(12.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("hier-starter-content"),
        ))
        .id();

    // Header: a tinted icon badge over a centred title and subtitle.
    let badge = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Px(44.0),
                align_self: AlignSelf::Center,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(mix(card_bg(), accent(), 0.16)),
            BorderColor::all(mix(border(), accent(), 0.35)),
        ))
        .id();
    let badge_icon = glyph(commands, fonts, "stack", 22.0, accent());
    commands.entity(badge).add_child(badge_icon);

    // The heading tells the truth about *why* the tree is empty. Filtered to UI
    // canvases in a scene full of geometry, "This scene is empty" was simply
    // false, and the false part is the one the user has to act on.
    let title = commands
        .spawn((
            Text::new(renzora::lang::t("hierarchy.starter.title")),
            ui_font(&fonts.ui, 15.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::justify(bevy::text::Justify::Center),
            Node { align_self: AlignSelf::Center, ..default() },
        ))
        .id();
    bind_text(commands, title, |w| {
        if is_scoped(w) {
            renzora::lang::t_or("hierarchy.starter.title_scoped", "No UI canvases here")
        } else {
            renzora::lang::t("hierarchy.starter.title")
        }
    });
    let sub = commands
        .spawn((
            Text::new(renzora::lang::t("hierarchy.starter.subtitle")),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::justify(bevy::text::Justify::Center),
            Node { align_self: AlignSelf::Center, margin: UiRect::bottom(Val::Px(14.0)), ..default() },
        ))
        .id();
    bind_text(commands, sub, |w| {
        if is_scoped(w) {
            renzora::lang::t_or(
                "hierarchy.starter.subtitle_scoped",
                "Add a canvas, then give it a UI template.",
            )
        } else {
            renzora::lang::t("hierarchy.starter.subtitle")
        }
    });

    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
            row_gap: Val::Px(8.0),
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    keyed_list(commands, grid, starter_snapshot);

    // Common entities: hidden while the tree is filtered, because a cube added
    // to a tree showing only UI canvases would vanish the moment it spawned.
    let common = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(18.0)),
            ..default()
        })
        .id();
    bind_display(commands, common, |w| !is_scoped(w));
    let common_label = section_label(
        commands,
        fonts,
        &renzora::lang::t_or("hierarchy.starter.common", "Common entities"),
    );
    let common_grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
            row_gap: Val::Px(6.0),
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let tiles: Vec<Entity> = COMMON
        .iter()
        .map(|(label, icon, what)| build_common_tile(commands, fonts, label, icon, *what))
        .collect();
    commands.entity(common_grid).add_children(&tiles);
    commands.entity(common).add_children(&[common_label, common_grid]);

    commands.entity(content).add_children(&[badge, title, sub, grid, common]);

    // Scrollable: a short Hierarchy pane would otherwise clip the common row
    // with no way to reach it.
    let scroll = scroll_view(commands, content);
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("hier-starter-picker"),
        ))
        .id();
    commands.entity(root).add_child(scroll);
    root
}

fn starter_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    // Only starters that build something the current tree would show.
    //
    // The picker appears whenever the *tree* is empty, and the tree can be
    // empty because it is filtered rather than because the scene is. In the UI
    // workspace that meant a scene full of geometry offering to create a
    // terrain: an answer to a question nobody asked, on a screen that was also
    // claiming the scene was empty.
    let scope = world.get_resource::<HierarchyFilter>().cloned();
    let keep = |produces: &'static [&'static str]| match &scope {
        Some(HierarchyFilter::OnlyWithComponents(names)) => {
            produces.iter().any(|p| names.contains(p))
        }
        // Unfiltered, or a filter that only hides things: everything is
        // reachable, so everything is offered.
        _ => true,
    };
    // (id, title, description, icon-glyph).
    let cards: Vec<(&'static str, &'static str, &'static str, &'static str)> = world
        .get_resource::<SceneStarterRegistry>()
        .map(|r| {
            r.iter()
                .filter(|s| keep(s.produces))
                .map(|s| (s.id, s.title, s.description, s.icon))
                .collect()
        })
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

/// A starter tile: icon badge, title, and a description that may wrap to two
/// lines. The old full-width rows cut descriptions off mid-word ("sprites, UI,
/// retro pixe") because they had one line to fit in.
fn build_card(commands: &mut Commands, fonts: &EmberFonts, id: &'static str, title: &str, desc: &str, icon: &str) -> Entity {
    let card = commands
        .spawn((
            Node {
                min_width: Val::Px(0.0),
                min_height: Val::Px(92.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            tile_tint(),
            UiInteraction::default(),
            HierStarterCard(id),
            Name::new("hier-starter-card"),
        ))
        .id();
    let badge = commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(mix(card_bg(), accent(), 0.2)),
            Pickable::IGNORE,
        ))
        .id();
    let g = glyph(commands, fonts, icon, 16.0, accent());
    commands.entity(badge).add_child(g);
    let t = commands
        .spawn((
            Text::new(title.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    let d = commands
        .spawn((
            Text::new(desc.to_string()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
            Node { max_width: Val::Percent(100.0), margin: UiRect::top(Val::Px(-3.0)), ..default() },
        ))
        .id();
    commands.entity(card).add_children(&[badge, t, d]);
    card
}

fn build_common_tile(commands: &mut Commands, fonts: &EmberFonts, label: &str, icon: &str, what: QuickAdd) -> Entity {
    let tile = commands
        .spawn((
            Node {
                min_width: Val::Px(0.0),
                height: Val::Px(54.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            tile_tint(),
            UiInteraction::default(),
            HierQuickAdd(what),
            Name::new("hier-quick-add"),
        ))
        .id();
    let g = glyph(commands, fonts, icon, 16.0, text_primary());
    let key = format!("hierarchy.starter.quick.{}", label.to_lowercase());
    let t = commands
        .spawn((
            Text::new(renzora::lang::t_or(&key, label)),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    commands.entity(tile).add_children(&[g, t]);
    tile
}

fn starter_click(
    q: Query<(&UiInteraction, &HierStarterCard), Changed<UiInteraction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, card) in &q {
        if *interaction == UiInteraction::Pressed {
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

/// A common-entity tile → spawn it undoably at the scene root, through the same
/// commands Add Entity uses, so Ctrl+Z takes it back the same way.
fn quick_add_click(
    q: Query<(&UiInteraction, &HierQuickAdd), Changed<UiInteraction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, tile) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let what = tile.0;
        cmds.push(move |world: &mut World| match what {
            QuickAdd::Shape(id) => {
                let Some((name, color)) = world
                    .get_resource::<ShapeRegistry>()
                    .and_then(|r| r.get(id))
                    .map(|s| (s.name.to_string(), s.default_color))
                else {
                    return;
                };
                execute(
                    world,
                    UndoContext::Scene,
                    Box::new(SpawnShapeCmd {
                        entity: Entity::PLACEHOLDER,
                        shape_id: id.to_string(),
                        name,
                        position: Vec3::ZERO,
                        color,
                    }),
                );
            }
            QuickAdd::Preset(id) => {
                execute(
                    world,
                    UndoContext::Scene,
                    Box::new(SpawnEntityCmd {
                        entity: Entity::PLACEHOLDER,
                        kind: SpawnEntityKind::Preset { id: id.to_string() },
                    }),
                );
            }
        });
    }
}

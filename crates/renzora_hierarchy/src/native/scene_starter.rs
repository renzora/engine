//! Starter picker for the native hierarchy. When the tree is empty it is
//! replaced by a set of cards — one per registered [`SceneStarter`] (3D/2D
//! camera, Environment, UI Canvas, …) — each of which spawns that starter's
//! entities on click.
//!
//! The tree can be empty for two different reasons, and the picker says which:
//! the scene really has nothing in it, or a [`HierarchyFilter`] has narrowed it
//! to a component set that nothing currently matches. In the second case only
//! starters that *produce* something the filter admits are offered — a scene
//! full of geometry, filtered to UI canvases, was offering to build a terrain.

use bevy::prelude::*;

use renzora_editor_framework::{
    EditorCommands, HierarchyFilter, SceneStarterRegistry, SplashState,
};
use renzora_ember::font::{icon_glyph, ui_font, EmberFonts};
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_text, keyed_list};
use renzora_ember::theme::*;

use crate::cache::HierarchyTreeCache;

/// A starter card → spawns the starter with this id on click.
#[derive(Component)]
pub(crate) struct HierStarterCard(&'static str);

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, starter_click.run_if(in_state(SplashState::Editor)));
}

/// True when the scene has no entities (the picker should show).
pub(crate) fn scene_is_empty(world: &Rx) -> bool {
    world.get_resource::<HierarchyTreeCache>().is_none_or(|c| c.nodes.is_empty())
}

/// Is the tree narrowed to a component set rather than showing the whole scene?
///
/// Decides what the picker *says*, not what it offers — an empty tree means
/// something different when a filter is producing it.
fn is_scoped(world: &Rx) -> bool {
    matches!(
        world.get_resource::<HierarchyFilter>(),
        Some(HierarchyFilter::OnlyWithComponents(_))
    )
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

    // The heading tells the truth about *why* the tree is empty. Filtered to UI
    // canvases in a scene full of geometry, "This scene is empty" was simply
    // false, and the false part is the one the user has to act on.
    let title = commands
        .spawn((
            Text::new(renzora::lang::t("hierarchy.starter.title")),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_primary())),
            Node { margin: UiRect::bottom(Val::Px(2.0)), ..default() },
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
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() },
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

fn starter_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    // Only starters that build something the current tree would show.
    //
    // The picker appears whenever the *tree* is empty, and the tree can be
    // empty because it is filtered rather than because the scene is. In the UI
    // workspace that meant a scene full of geometry offering to create a
    // terrain — an answer to a question nobody asked, on a screen that was also
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
    // `icon` is a kebab-case Phosphor *name* (per the SceneStarter contract), so
    // resolve it to its single PUA glyph before rendering. Passing the raw name
    // to the phosphor font lets ligature substitution mangle multi-word names —
    // e.g. "image-square" renders as two glyphs (image + square) and a name with
    // no ligature match renders nothing. `icon_glyph` gives the one correct glyph.
    let glyph_ch = icon_glyph(icon).unwrap_or('\u{E4C6}'); // fallback: "question"
    let glyph = commands
        .spawn((
            Text::new(glyph_ch.to_string()),
            TextFont {
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(22.0),
                ..default()
            },
            TextColor(rgb(text_primary())),
        ))
        .id();
    let text_col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() })
        .id();
    let t = commands.spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap())).id();
    let d = commands.spawn((Text::new(desc.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())), bevy::text::TextLayout::no_wrap())).id();
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

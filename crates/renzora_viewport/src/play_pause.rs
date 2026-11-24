//! The label over the viewport while it is paused for a running game.
//!
//! **Why it exists.** While a game started with Play runs, the viewport stops
//! rendering and keeps its last image (see `redraw::game_running`). A picture
//! that stops moving with nothing to say why reads as a hang, so this says
//! where the game is instead.
//!
//! **It doesn't take the pointer**, like the stats readout and the load card:
//! the hierarchy, inspector and the live link all keep working while the game
//! runs, and the viewport's tools are still there underneath.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_display;
use renzora_ember::theme::{panel_bg, rgb, text_primary};
use renzora_project_api::PlayModeState;

/// Build the label for a viewport's content node. Slot 0 only, like the other
/// cards: one game, so one line.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let cover = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(12.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
            Name::new("vp-play-pause"),
        ))
        .id();
    bind_display(commands, cover, |w| {
        w.get_resource::<PlayModeState>().is_some_and(|p| p.game_launched)
    });

    let card = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg()).with_alpha(0.9)),
            bevy::picking::Pickable::IGNORE,
            Name::new("vp-play-pause-card"),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(renzora_lang_api::t("viewport.paused_for_game")),
                ui_font(&fonts.ui, 13.0),
                TextColor(rgb(text_primary())),
                bevy::picking::Pickable::IGNORE,
            ));
        })
        .id();
    commands.entity(cover).add_child(card);
    cover
}

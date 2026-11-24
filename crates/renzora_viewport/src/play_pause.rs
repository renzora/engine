//! What the viewport shows while it is paused for a running game: a blank
//! panel with one line, centred, saying where the game is.
//!
//! **Why it exists.** While a game started with Play runs, the viewport stops
//! rendering (see `redraw::game_running`). Its last image would sit there
//! looking live while the game moved on in its own window, and a picture that
//! stops moving with nothing to say why reads as a hang.
//!
//! **It takes the pointer.** The scene is hidden behind it, so a click there
//! must not land on a gizmo or select an object nobody can see. The
//! hierarchy, inspector and the live link all keep working while the game runs.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_display;
use renzora_ember::theme::{panel_bg, rgb, text_muted};
use renzora_project_api::PlayModeState;

/// Build the cover for a viewport's content node. Slot 0 only, like the other
/// cards: one game, so one message.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let cover = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            Name::new("vp-play-pause"),
        ))
        .with_children(|cover| {
            cover.spawn((
                Text::new(renzora_lang_api::t("viewport.paused_for_game")),
                ui_font(&fonts.ui, 15.0),
                TextColor(rgb(text_muted())),
                bevy::picking::Pickable::IGNORE,
            ));
        })
        .id();
    bind_display(commands, cover, |w| {
        w.get_resource::<PlayModeState>().is_some_and(|p| p.game_launched)
    });
    cover
}

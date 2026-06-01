//! Mixer strip — composes a fader, a VU meter, mute/solo toggles and a channel
//! label into one audio-mixer channel.

use bevy::prelude::*;

use crate::font::{ui_font, EmberFonts};
use crate::theme::{rgb, ACCENT_BLUE, TEXT_PRIMARY};

use super::fader::fader;
use super::vu_meter::vu_meter;

#[derive(Component)]
pub(crate) struct MixerButton {
    on: Color,
    off: Color,
    active: bool,
}

fn toggle_btn(commands: &mut Commands, fonts: &EmberFonts, label: &str, on: Color) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((42, 42, 52))),
            Interaction::default(),
            MixerButton {
                on,
                off: rgb((42, 42, 52)),
                active: false,
            },
            Name::new("mixer-toggle"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    commands.entity(btn).add_child(text);
    btn
}

/// A mixer channel strip: label, fader + live VU meter, and mute/solo toggles.
pub fn mixer_strip(commands: &mut Commands, fonts: &EmberFonts, name: &str, gain: f32) -> Entity {
    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb((26, 26, 32))),
            BorderColor::all(rgb((48, 48, 58))),
            Name::new("mixer-strip"),
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    let meters = commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .id();
    let f = fader(commands, gain);
    let vu = vu_meter(commands);
    commands.entity(meters).add_children(&[f, vu]);
    let buttons = commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(5.0),
            ..default()
        },))
        .id();
    let mute = toggle_btn(commands, fonts, "M", rgb((225, 90, 80)));
    let solo = toggle_btn(commands, fonts, "S", rgb(ACCENT_BLUE));
    commands.entity(buttons).add_children(&[mute, solo]);
    commands.entity(root).add_children(&[label, meters, buttons]);
    root
}

pub(crate) fn mixer_toggle(
    mut buttons: Query<(&Interaction, &mut MixerButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, mut btn, mut bg) in &mut buttons {
        if *interaction == Interaction::Pressed {
            btn.active = !btn.active;
            bg.0 = if btn.active { btn.on } else { btn.off };
        }
    }
}

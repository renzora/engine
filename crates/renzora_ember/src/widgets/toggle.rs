//! Toggle — an on/off switch with a sliding knob.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::style::{Role, Styled, WidgetState};
use crate::theme::*;

#[derive(Component)]
pub(crate) struct EmberToggle {
    on: bool,
}

/// An on/off switch — click to flip.
pub fn toggle(commands: &mut Commands, on: bool) -> Entity {
    let track = commands
        .spawn((
            Node {
                width: Val::Px(38.0),
                height: Val::Px(20.0),
                padding: UiRect::all(Val::Px(2.0)),
                align_items: AlignItems::Center,
                justify_content: if on {
                    JustifyContent::FlexEnd
                } else {
                    JustifyContent::FlexStart
                },
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(if on { rgb(accent()) } else { rgb(card_bg()) }),
            Interaction::default(),
            EmberToggle { on },
            Styled::with_state(
                Role::Toggle,
                if on {
                    WidgetState::Active
                } else {
                    WidgetState::Normal
                },
            ),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("toggle"),
        ))
        .id();
    let knob = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(on_accent())),
            BorderColor::all(rgb(border())),
            Name::new("toggle-knob"),
        ))
        .id();
    commands.entity(track).add_child(knob);
    track
}

pub(crate) fn toggle_interact(
    mut q: Query<(&Interaction, &mut EmberToggle, &mut Styled, &mut Node), Changed<Interaction>>,
) {
    for (interaction, mut tog, mut styled, mut node) in &mut q {
        if *interaction == Interaction::Pressed {
            tog.on = !tog.on;
            styled.state = if tog.on {
                WidgetState::Active
            } else {
                WidgetState::Normal
            };
            node.justify_content = if tog.on {
                JustifyContent::FlexEnd
            } else {
                JustifyContent::FlexStart
            };
        }
    }
}

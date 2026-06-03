//! Checkbox — a toggleable box with an inner check mark.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::reactive::Bound;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::*;

/// Mark ref; the checked state lives in `Bound<bool>` (so `bind_2way` can drive it).
#[derive(Component)]
pub(crate) struct EmberCheckbox {
    mark: Entity,
}

/// A checkbox — click to toggle.
pub fn checkbox(commands: &mut Commands, checked: bool) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(if checked {
                rgb(accent())
            } else {
                Color::NONE
            }),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            Styled::with_state(
                Role::Checkbox,
                if checked {
                    WidgetState::Active
                } else {
                    WidgetState::Normal
                },
            ),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("checkbox"),
        ))
        .id();
    // Inner check mark (a small white square), shown only when checked.
    let mark = commands
        .spawn((
            Node {
                width: Val::Px(8.0),
                height: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                display: if checked {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
            BackgroundColor(rgb((245, 245, 250))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("check"),
        ))
        .id();
    commands.entity(box_e).insert((EmberCheckbox { mark }, Bound::<bool>(checked)));
    commands.entity(box_e).add_child(mark);
    box_e
}

/// Click → flip the model (`Bound<bool>`); visuals follow via [`checkbox_apply`].
pub(crate) fn checkbox_interact(
    mut boxes: Query<(&Interaction, &mut Bound<bool>), (With<EmberCheckbox>, Changed<Interaction>)>,
) {
    for (interaction, mut b) in &mut boxes {
        if *interaction == Interaction::Pressed {
            b.0 = !b.0;
        }
    }
}

/// Model (`Bound<bool>`) → box fill + mark + styled state (click or state push).
pub(crate) fn checkbox_apply(
    mut boxes: Query<(&EmberCheckbox, &Bound<bool>, &mut BackgroundColor, &mut Styled), Changed<Bound<bool>>>,
    mut nodes: Query<&mut Node>,
) {
    for (cb, b, mut bg, mut styled) in &mut boxes {
        bg.0 = if b.0 { rgb(accent()) } else { Color::NONE };
        styled.state = if b.0 {
            WidgetState::Active
        } else {
            WidgetState::Normal
        };
        if let Ok(mut n) = nodes.get_mut(cb.mark) {
            n.display = if b.0 { Display::Flex } else { Display::None };
        }
    }
}

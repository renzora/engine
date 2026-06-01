//! Button — a clickable box with themed hover/press states.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::{rgb, TAB_ACTIVE_BG, TEXT_PRIMARY};

/// Marks a button so [`button_interact`] drives its `Styled.state`. Shared with
/// other button-like widgets (e.g. the number stepper's `±` keys).
#[derive(Component)]
pub(crate) struct EmberButton;

/// A clickable button with hover/press color states.
pub fn button(commands: &mut Commands, font: &Handle<Font>, label: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            Interaction::default(),
            EmberButton,
            Styled::new(Role::Button),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("button"),
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    commands.entity(b).add_child(t);
    b
}

pub(crate) fn button_interact(
    mut q: Query<(&Interaction, &mut Styled), (With<EmberButton>, Changed<Interaction>)>,
) {
    for (interaction, mut styled) in &mut q {
        styled.state = match interaction {
            Interaction::Pressed => WidgetState::Pressed,
            Interaction::Hovered => WidgetState::Hover,
            Interaction::None => WidgetState::Normal,
        };
    }
}

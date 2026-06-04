//! Button — a clickable box with themed hover/press states.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::style::{Role, Styled, WidgetState};
use crate::theme::*;

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
            BackgroundColor(rgb(tab_active())),
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
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(b).add_child(t);
    b
}

/// A clickable button with a leading Phosphor icon and a label, themed with the
/// same hover/press states as [`button`].
pub fn icon_label_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
) -> Entity {
    let b = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            EmberButton,
            Styled::new(Role::Button),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("icon-button"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(b).add_children(&[ic, t]);
    b
}

/// An icon-only square button (Styled `IconButton`), themed with the same
/// hover/press states as [`button`]. For chrome actions (close, gear, …).
pub fn icon_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            EmberButton,
            Styled::new(Role::IconButton),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("icon-button"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 13.0);
    commands.entity(b).add_child(ic);
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

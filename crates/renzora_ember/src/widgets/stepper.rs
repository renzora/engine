//! Number stepper — a value with `−` / `+` keys.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled};
use crate::theme::*;

use super::button::EmberButton;
use super::common::format_num;

#[derive(Component)]
pub(crate) struct EmberStepper {
    value: f32,
    step: f32,
    display: Entity,
}

#[derive(Component)]
pub(crate) struct EmberStepButton {
    stepper: Entity,
    dir: f32,
}

/// A number stepper: `[−] value [+]`. Returns the container.
pub fn number_stepper(commands: &mut Commands, font: &Handle<Font>, value: f32, step: f32) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("stepper"),
        ))
        .id();
    let display = commands
        .spawn((
            Text::new(format_num(value)),
            ui_font(font, 12.0),
            TextColor(rgb(text_primary())),
            Node {
                min_width: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .id();
    commands.entity(row).insert(EmberStepper {
        value,
        step,
        display,
    });
    let minus = step_button(commands, font, row, "−", -1.0);
    let plus = step_button(commands, font, row, "+", 1.0);
    commands.entity(row).add_children(&[minus, display, plus]);
    row
}

fn step_button(
    commands: &mut Commands,
    font: &Handle<Font>,
    stepper: Entity,
    label: &str,
    dir: f32,
) -> Entity {
    commands
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
            EmberStepButton { stepper, dir },
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("step-button"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(label),
                ui_font(font, 14.0),
                TextColor(rgb(text_primary())),
            ));
        })
        .id()
}

pub(crate) fn stepper_interact(
    pressed: Query<(&Interaction, &EmberStepButton), Changed<Interaction>>,
    mut steppers: Query<&mut EmberStepper>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, btn) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok(mut s) = steppers.get_mut(btn.stepper) {
            s.value += btn.dir * s.step;
            let (display, value) = (s.display, s.value);
            if let Ok(mut t) = texts.get_mut(display) {
                *t = Text::new(format_num(value));
            }
        }
    }
}

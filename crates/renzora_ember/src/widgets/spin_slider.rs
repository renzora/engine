//! Spin-slider — a Godot-style inline value field: a label + value with a fill
//! bar showing the value in `[min, max]`; drag horizontally to scrub.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled};
use crate::theme::*;

use super::common::format_num;

#[derive(Component)]
pub(crate) struct EmberSpin {
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    fill: Entity,
    text: Entity,
    last_x: Option<f32>,
}

fn frac(value: f32, min: f32, max: f32) -> f32 {
    ((value - min) / (max - min).max(1e-4)).clamp(0.0, 1.0)
}

/// A spin-slider showing `value` in `[min, max]`. Drag to scrub by `step`/px.
pub fn spin_slider(commands: &mut Commands, font: &Handle<Font>, label: &str, value: f32, min: f32, max: f32) -> Entity {
    let row = commands
        .spawn((
            Node {
                min_width: Val::Px(150.0),
                height: Val::Px(22.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Styled::new(Role::Input),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::EwResize),
            Name::new("spin-slider"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                height: Val::Percent(100.0),
                width: Val::Percent(frac(value, min, max) * 100.0),
                ..default()
            },
            BackgroundColor(rgb(accent()).with_alpha(0.30)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("spin-fill"),
        ))
        .id();
    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(8.0)),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            Name::new("spin-content"),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(font, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(format_num(value)),
            ui_font(font, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(content).add_children(&[lbl, text]);
    let step = (max - min) / 200.0;
    commands.entity(row).insert(EmberSpin {
        value,
        min,
        max,
        step: if step > 0.0 { step } else { 0.01 },
        fill,
        text,
        last_x: None,
    });
    commands.entity(row).add_children(&[fill, content]);
    row
}

pub(crate) fn spin_drag(
    windows: Query<&Window>,
    mut spins: Query<(&Interaction, &mut EmberSpin)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    let cursor_x = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|p| p.x);
    for (interaction, mut s) in &mut spins {
        if *interaction == Interaction::Pressed {
            if let (Some(cx), Some(last)) = (cursor_x, s.last_x) {
                let d = cx - last;
                if d != 0.0 {
                    s.value = (s.value + d * s.step).clamp(s.min, s.max);
                    let f = frac(s.value, s.min, s.max);
                    let (fill, text, v) = (s.fill, s.text, s.value);
                    if let Ok(mut n) = nodes.get_mut(fill) {
                        n.width = Val::Percent(f * 100.0);
                    }
                    if let Ok(mut t) = texts.get_mut(text) {
                        *t = Text::new(format_num(v));
                    }
                }
            }
            s.last_x = cursor_x;
        } else if s.last_x.is_some() {
            s.last_x = None;
        }
    }
}

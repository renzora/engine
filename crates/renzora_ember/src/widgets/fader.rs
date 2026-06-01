//! Fader — a vertical slider.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::theme::{rgb, ACCENT_BLUE};

#[derive(Component)]
pub(crate) struct EmberFader {
    value: f32,
    fill: Entity,
    thumb: Entity,
}

/// A vertical fader (drag to change `value` 0..1).
pub fn fader(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let col = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(120.0),
                position_type: PositionType::Relative,
                ..default()
            },
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::NsResize),
            Name::new("fader"),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(9.0),
                width: Val::Px(6.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((55, 55, 66))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-track"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(9.0),
                bottom: Val::Px(0.0),
                width: Val::Px(6.0),
                height: Val::Percent(v * 100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-fill"),
        ))
        .id();
    let thumb = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(3.0),
                bottom: Val::Percent(v * 100.0),
                margin: UiRect::bottom(Val::Px(-5.0)),
                width: Val::Px(18.0),
                height: Val::Px(10.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((240, 240, 245))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-thumb"),
        ))
        .id();
    commands.entity(col).add_children(&[track, fill, thumb]);
    commands.entity(col).insert(EmberFader { value: v, fill, thumb });
    col
}

pub(crate) fn fader_drag(
    mut faders: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &mut EmberFader)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, rcp, mut f) in &mut faders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let v = (0.5 - n.y).clamp(0.0, 1.0);
        if (v - f.value).abs() < 0.001 {
            continue;
        }
        f.value = v;
        if let Ok(mut node) = nodes.get_mut(f.fill) {
            node.height = Val::Percent(v * 100.0);
        }
        if let Ok(mut node) = nodes.get_mut(f.thumb) {
            node.bottom = Val::Percent(v * 100.0);
        }
    }
}

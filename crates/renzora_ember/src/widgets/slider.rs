//! Slider — a draggable 0..1 value track.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::theme::{rgb, ACCENT_BLUE};

#[derive(Component)]
pub(crate) struct EmberSlider {
    pub(crate) value: f32,
    fill: Entity,
    thumb: Entity,
}

/// A draggable slider with `value` in 0..1. Click/drag anywhere on it to set
/// the value.
pub fn slider(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    // 18px-tall hit area so it's easy to grab; the visual track is 6px.
    let row = commands
        .spawn((
            Node {
                width: Val::Px(160.0),
                height: Val::Px(18.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("slider"),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((55, 55, 66))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("slider-track"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(v * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            Name::new("slider-fill"),
        ))
        .id();
    let thumb = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(v * 100.0),
                margin: UiRect::left(Val::Px(-7.0)),
                width: Val::Px(14.0),
                height: Val::Px(14.0),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb((240, 240, 245))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("slider-thumb"),
        ))
        .id();
    commands.entity(track).add_child(fill);
    commands.entity(row).add_children(&[track, thumb]);
    commands.entity(row).insert(EmberSlider { value: v, fill, thumb });
    row
}

pub(crate) fn slider_drag(
    mut sliders: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &mut EmberSlider)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, rcp, mut s) in &mut sliders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        // `normalized` is centered (-0.5..0.5); shift to 0..1.
        let v = (n.x + 0.5).clamp(0.0, 1.0);
        if (v - s.value).abs() < 0.001 {
            continue;
        }
        s.value = v;
        if let Ok(mut fnode) = nodes.get_mut(s.fill) {
            fnode.width = Val::Percent(v * 100.0);
        }
        if let Ok(mut tnode) = nodes.get_mut(s.thumb) {
            tnode.left = Val::Percent(v * 100.0);
        }
    }
}

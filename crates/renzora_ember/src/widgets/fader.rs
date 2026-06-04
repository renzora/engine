//! Fader — a vertical slider.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::reactive::Bound;
use crate::theme::*;

/// References to the fader's fill/thumb so the model→visuals system can move
/// them. The value itself lives in `Bound<f32>` (so [`bind_2way`] can drive it).
#[derive(Component)]
pub(crate) struct EmberFader {
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
            BackgroundColor(rgb(card_bg())),
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
            BackgroundColor(rgb(accent())),
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
            BackgroundColor(rgb(on_accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-thumb"),
        ))
        .id();
    commands.entity(col).add_children(&[track, fill, thumb]);
    commands
        .entity(col)
        .insert((EmberFader { fill, thumb }, Bound::<f32>(v)));
    col
}

/// User drag → write the model (`Bound<f32>`). Visuals follow via [`fader_apply`].
pub(crate) fn fader_drag(
    mut faders: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &mut Bound<f32>), With<EmberFader>>,
) {
    for (interaction, rcp, mut b) in &mut faders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let v = (0.5 - n.y).clamp(0.0, 1.0);
        if (v - b.0).abs() >= 0.001 {
            b.0 = v;
        }
    }
}

/// Model (`Bound<f32>`) → fill/thumb position. Runs whenever the model changes,
/// whether the user dragged or [`bind_2way`] pushed a new value from state.
pub(crate) fn fader_apply(
    faders: Query<(&EmberFader, &Bound<f32>), Changed<Bound<f32>>>,
    mut nodes: Query<&mut Node>,
) {
    for (f, b) in &faders {
        let v = b.0.clamp(0.0, 1.0);
        if let Ok(mut node) = nodes.get_mut(f.fill) {
            node.height = Val::Percent(v * 100.0);
        }
        if let Ok(mut node) = nodes.get_mut(f.thumb) {
            node.bottom = Val::Percent(v * 100.0);
        }
    }
}

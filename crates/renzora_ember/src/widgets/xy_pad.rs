//! XY pad — a 2D draggable handle.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::theme::{rgb, ACCENT_BLUE};

#[derive(Component)]
pub(crate) struct EmberXyPad {
    handle: Entity,
}

/// A 2D XY pad (drag the handle; `x`/`y` are 0..1, y up).
pub fn xy_pad(commands: &mut Commands, x: f32, y: f32) -> Entity {
    let px = x.clamp(0.0, 1.0);
    let py = y.clamp(0.0, 1.0);
    let pad = commands
        .spawn((
            Node {
                width: Val::Px(120.0),
                height: Val::Px(120.0),
                position_type: PositionType::Relative,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb((30, 30, 38))),
            BorderColor::all(rgb((60, 60, 72))),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Move),
            Name::new("xy-pad"),
        ))
        .id();
    let handle = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(px * 100.0),
                top: Val::Percent((1.0 - py) * 100.0),
                margin: UiRect::new(Val::Px(-6.0), Val::Px(0.0), Val::Px(-6.0), Val::Px(0.0)),
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("xy-handle"),
        ))
        .id();
    commands.entity(pad).add_child(handle);
    commands.entity(pad).insert(EmberXyPad { handle });
    pad
}

pub(crate) fn xy_pad_drag(
    pads: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &EmberXyPad)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, rcp, pad) in &pads {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let nx = (n.x + 0.5).clamp(0.0, 1.0);
        let ny = (n.y + 0.5).clamp(0.0, 1.0);
        if let Ok(mut node) = nodes.get_mut(pad.handle) {
            node.left = Val::Percent(nx * 100.0);
            node.top = Val::Percent(ny * 100.0);
        }
    }
}

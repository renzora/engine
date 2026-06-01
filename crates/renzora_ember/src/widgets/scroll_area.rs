//! Scroll area — a clipping viewport that scrolls its content with the wheel.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct EmberScroll;

/// Wraps `content` in a fixed-height viewport that scrolls vertically on wheel.
pub fn scroll_area(commands: &mut Commands, content: Entity, max_height: f32) -> Entity {
    let view = commands
        .spawn((
            Node {
                max_height: Val::Px(max_height),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            bevy::ui::RelativeCursorPosition::default(),
            ScrollPosition::default(),
            EmberScroll,
            Name::new("scroll-area"),
        ))
        .id();
    commands.entity(view).add_child(content);
    view
}

pub(crate) fn scroll_drive(
    mut wheel: MessageReader<MouseWheel>,
    mut areas: Query<(&bevy::ui::RelativeCursorPosition, &mut ScrollPosition), With<EmberScroll>>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy == 0.0 {
        return;
    }
    for (rcp, mut sp) in &mut areas {
        if rcp.cursor_over {
            sp.y = (sp.y - dy * 24.0).max(0.0);
        }
    }
}

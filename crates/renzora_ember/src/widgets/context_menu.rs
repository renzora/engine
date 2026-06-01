//! Context menu — right-click a target to open a menu (with submenus) at the
//! cursor. Left-click anywhere closes it.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use crate::font::EmberFonts;

#[derive(Component)]
pub(crate) struct EmberContextTarget {
    menu: Entity,
}

/// Wraps `target` so right-clicking it opens a context menu. `items` are
/// `(label, submenu_labels)` — a non-empty submenu opens on hover.
pub fn context_menu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    target: Entity,
    items: &[(&str, &[&str])],
) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            RelativeCursorPosition::default(),
            Name::new("context-menu"),
        ))
        .id();
    let menu = super::menu::build_menu(commands, fonts, items, false);
    commands.entity(wrap).insert(EmberContextTarget { menu });
    commands.entity(wrap).add_children(&[target, menu]);
    wrap
}

pub(crate) fn context_menu_open(
    mouse: Res<ButtonInput<MouseButton>>,
    targets: Query<(&RelativeCursorPosition, &ComputedNode, &EmberContextTarget)>,
    mut nodes: Query<&mut Node>,
) {
    let right = mouse.just_pressed(MouseButton::Right);
    let left = mouse.just_pressed(MouseButton::Left);
    if !right && !left {
        return;
    }
    for (rcp, computed, target) in &targets {
        if right && rcp.cursor_over {
            if let Some(nrm) = rcp.normalized {
                let size = computed.size() * computed.inverse_scale_factor();
                let lx = (nrm.x + 0.5) * size.x;
                let ly = (nrm.y + 0.5) * size.y;
                if let Ok(mut n) = nodes.get_mut(target.menu) {
                    n.left = Val::Px(lx);
                    n.top = Val::Px(ly);
                    n.display = Display::Flex;
                }
            }
        } else if left {
            // Any left-click closes the menu (item clicks already hide it too).
            if let Ok(mut n) = nodes.get_mut(target.menu) {
                n.display = Display::None;
            }
        }
    }
}

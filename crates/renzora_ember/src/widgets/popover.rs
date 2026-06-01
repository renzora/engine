//! Popover — a button that toggles a panel below it.

use bevy::prelude::*;

use crate::font::EmberFonts;
use crate::theme::rgb;

use super::button::button;

#[derive(Component)]
pub(crate) struct EmberPopover {
    panel: Entity,
    open: bool,
}

/// A button that toggles a popover panel below it (holding `content`).
pub fn popover(commands: &mut Commands, fonts: &EmberFonts, label: &str, content: Entity) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("popover"),
        ))
        .id();
    let trigger = button(commands, &fonts.ui, label);
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                margin: UiRect::top(Val::Px(4.0)),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb((30, 30, 38))),
            BorderColor::all(rgb((60, 60, 74))),
            GlobalZIndex(600),
            Name::new("popover-panel"),
        ))
        .id();
    commands.entity(panel).add_child(content);
    commands
        .entity(trigger)
        .insert(EmberPopover { panel, open: false });
    commands.entity(wrap).add_children(&[trigger, panel]);
    wrap
}

pub(crate) fn popover_toggle(
    mut pops: Query<(&Interaction, &mut EmberPopover), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut pop) in &mut pops {
        if *interaction != Interaction::Pressed {
            continue;
        }
        pop.open = !pop.open;
        if let Ok(mut n) = nodes.get_mut(pop.panel) {
            n.display = if pop.open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

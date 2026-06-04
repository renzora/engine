//! Tooltip — a hover bubble above a wrapped target.

use bevy::prelude::*;

use crate::font::ui_font;
use crate::theme::*;

#[derive(Component)]
pub(crate) struct EmberTooltip {
    tip: Entity,
}

/// Wraps `target` so hovering it reveals a tooltip bubble above.
pub fn tooltip(commands: &mut Commands, font: &Handle<Font>, label: &str, target: Entity) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            Name::new("tooltip"),
        ))
        .id();
    let tip = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            GlobalZIndex(600),
            Name::new("tooltip-tip"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(label),
                ui_font(font, 11.0),
                TextColor(rgb(text_primary())),
                TextLayout::new_with_no_wrap(),
            ));
        })
        .id();
    commands.entity(wrap).insert(EmberTooltip { tip });
    commands.entity(wrap).add_children(&[target, tip]);
    wrap
}

pub(crate) fn tooltip_hover(
    tips: Query<(&Interaction, &EmberTooltip), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, tt) in &tips {
        if let Ok(mut n) = nodes.get_mut(tt.tip) {
            n.display = if *interaction == Interaction::None {
                Display::None
            } else {
                Display::Flex
            };
        }
    }
}

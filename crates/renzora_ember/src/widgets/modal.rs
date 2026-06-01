//! Modal — a stage with a button that reveals a dimming overlay + dialog.

use bevy::prelude::*;

use crate::font::EmberFonts;
use crate::style::{Role, Styled};
use crate::theme::{rgb, PANEL_BG, TEXT_MUTED, TEXT_PRIMARY};

use super::button::button;
use super::common::text_node;

#[derive(Component)]
pub(crate) struct EmberModalOpen {
    overlay: Entity,
}

#[derive(Component)]
pub(crate) struct EmberModalClose {
    overlay: Entity,
}

/// A self-contained modal demo: a stage with an "open" button that reveals a
/// dimming overlay + a centered dialog (with a close button).
pub fn modal(commands: &mut Commands, fonts: &EmberFonts, title: &str, body: &str) -> Entity {
    let stage = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(150.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb((26, 26, 32))),
            BorderColor::all(rgb((48, 48, 58))),
            Name::new("modal-stage"),
        ))
        .id();
    let overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(500),
            Name::new("modal-overlay"),
        ))
        .id();
    let dialog = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                min_width: Val::Px(200.0),
                ..default()
            },
            BackgroundColor(rgb(PANEL_BG)),
            BorderColor::all(rgb((60, 60, 74))),
            Styled::new(Role::Card),
            Name::new("modal-dialog"),
        ))
        .id();
    let t = text_node(commands, &fonts.ui, title, 14.0, TEXT_PRIMARY);
    let b = text_node(commands, &fonts.ui, body, 12.0, TEXT_MUTED);
    let close = button(commands, &fonts.ui, "Close");
    commands.entity(close).insert(EmberModalClose { overlay });
    commands.entity(dialog).add_children(&[t, b, close]);
    commands.entity(overlay).add_child(dialog);
    let open = button(commands, &fonts.ui, "Open modal");
    commands.entity(open).insert(EmberModalOpen { overlay });
    commands.entity(stage).add_children(&[open, overlay]);
    stage
}

pub(crate) fn modal_toggle(
    opens: Query<(&Interaction, &EmberModalOpen), Changed<Interaction>>,
    closes: Query<(&Interaction, &EmberModalClose), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, o) in &opens {
        if *interaction == Interaction::Pressed {
            if let Ok(mut n) = nodes.get_mut(o.overlay) {
                n.display = Display::Flex;
            }
        }
    }
    for (interaction, c) in &closes {
        if *interaction == Interaction::Pressed {
            if let Ok(mut n) = nodes.get_mut(c.overlay) {
                n.display = Display::None;
            }
        }
    }
}

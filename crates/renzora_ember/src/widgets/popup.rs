//! Generic popup behavior: a trigger that toggles a panel, with click-outside
//! dismiss — so every dropdown/menu/color-popup gets consistent open/close for
//! free instead of re-implementing it per panel.
//!
//! Attach [`Popup`] to a trigger entity (the clickable element). Ember drives the
//! `panel`'s `Node.display`: clicking the trigger toggles it, and clicking
//! anywhere outside both the trigger and the panel closes it. The panel must
//! carry a `RelativeCursorPosition` so outside-click can tell when the cursor is
//! over it.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use bevy::window::PrimaryWindow;

/// Marks a trigger that opens/closes `panel`. Build the panel with
/// `display: Display::None` + a `RelativeCursorPosition`.
#[derive(Component)]
pub struct Popup {
    pub panel: Entity,
    pub open: bool,
}

impl Popup {
    pub fn new(panel: Entity) -> Self {
        Self { panel, open: false }
    }
}

/// Close a popup by its trigger entity (e.g. after picking an option).
pub fn close_popup(commands: &mut Commands, trigger: Entity) {
    commands.queue(move |world: &mut World| {
        let panel = world.get::<Popup>(trigger).map(|p| p.panel);
        if let Some(mut p) = world.get_mut::<Popup>(trigger) {
            p.open = false;
        }
        if let Some(panel) = panel {
            if let Some(mut n) = world.get_mut::<Node>(panel) {
                n.display = Display::None;
            }
        }
    });
}

fn set_panel_display(nodes: &mut Query<&mut Node>, panel: Entity, open: bool) {
    if let Ok(mut n) = nodes.get_mut(panel) {
        let want = if open { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
}

/// Click the trigger → toggle its panel.
pub(crate) fn popup_toggle(
    mut triggers: Query<(&Interaction, &mut Popup), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut p) in &mut triggers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        p.open = !p.open;
        let (panel, open) = (p.panel, p.open);
        set_panel_display(&mut nodes, panel, open);
    }
}

/// Flip an open popup above its trigger when opening below would overflow the
/// bottom of the window (and there's room above) — so popups near the bottom of
/// a scroll area aren't cut off.
pub(crate) fn popup_position(
    triggers: Query<(&Popup, &ComputedNode, &GlobalTransform)>,
    panels: Query<&ComputedNode>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut nodes: Query<&mut Node>,
) {
    let Some(win_h) = windows.iter().next().map(|w| w.physical_height() as f32) else {
        return;
    };
    for (p, trigger_cn, gt) in &triggers {
        if !p.open {
            continue;
        }
        let Ok(panel_cn) = panels.get(p.panel) else {
            continue;
        };
        let panel_h = panel_cn.size().y;
        let half = trigger_cn.size().y * 0.5;
        let center = gt.translation().y;
        // Flip up only when opening below overflows AND there's room above.
        let flip = (center + half) + panel_h > win_h && (center - half) - panel_h > 0.0;
        if let Ok(mut n) = nodes.get_mut(p.panel) {
            let (top, bottom) = if flip {
                (Val::Auto, Val::Percent(100.0))
            } else {
                (Val::Percent(100.0), Val::Auto)
            };
            if n.top != top || n.bottom != bottom {
                n.top = top;
                n.bottom = bottom;
                n.margin.top = if flip { Val::Px(0.0) } else { Val::Px(2.0) };
                n.margin.bottom = if flip { Val::Px(2.0) } else { Val::Px(0.0) };
            }
        }
    }
}

/// Press anywhere outside an open popup's trigger + panel → close it.
pub(crate) fn popup_dismiss(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Query<&RelativeCursorPosition>,
    mut triggers: Query<(&Interaction, &mut Popup)>,
    mut nodes: Query<&mut Node>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    for (interaction, mut p) in &mut triggers {
        if !p.open {
            continue;
        }
        let over_panel = cursor.get(p.panel).map(|r| r.cursor_over).unwrap_or(false);
        // Trigger is `None` only when the cursor isn't over it.
        if *interaction == Interaction::None && !over_panel {
            p.open = false;
            let panel = p.panel;
            set_panel_display(&mut nodes, panel, false);
        }
    }
}

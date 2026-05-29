//! `cursor="..."` — set the OS cursor icon on hover (CSS-style), plus an
//! automatic `grabbing` cursor while a drag is in progress.
//!
//! `<button cursor="pointer">`, `<node cursor="grab" drag_item="true">`, etc.
//! While `dnd` is dragging, the cursor becomes `grabbing` regardless of hover.
//!
//! Note: this drives the **OS** cursor. If a custom markup cursor
//! (`name="cursor_follow"`) is active it hides the OS cursor, so this has no
//! visible effect there — style the custom cursor instead.

use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

use crate::dnd::DragState;

/// Stamped from `cursor="..."`; the OS cursor shown while this node is hovered.
#[derive(Component)]
pub struct HoverCursor(pub SystemCursorIcon);

/// Map a CSS-ish cursor name to a `SystemCursorIcon`.
pub fn parse_cursor(name: &str) -> Option<SystemCursorIcon> {
    use SystemCursorIcon as C;
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "default" => C::Default,
        "pointer" | "hand" => C::Pointer,
        "grab" | "openhand" => C::Grab,
        "grabbing" | "closedhand" => C::Grabbing,
        "text" => C::Text,
        "move" => C::Move,
        "wait" => C::Wait,
        "progress" => C::Progress,
        "help" => C::Help,
        "crosshair" => C::Crosshair,
        "not-allowed" | "notallowed" => C::NotAllowed,
        "no-drop" | "nodrop" => C::NoDrop,
        "ew-resize" | "col-resize" => C::EwResize,
        "ns-resize" | "row-resize" => C::NsResize,
        "all-scroll" => C::AllScroll,
        "zoom-in" => C::ZoomIn,
        "zoom-out" => C::ZoomOut,
        _ => return None,
    })
}

fn apply_cursor_icon(
    drag: Res<DragState>,
    hovered: Query<(&Interaction, &HoverCursor)>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut commands: Commands,
    mut last: Local<Option<SystemCursorIcon>>,
) {
    let target = if drag.is_dragging() {
        SystemCursorIcon::Grabbing
    } else {
        hovered
            .iter()
            .find(|(i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed))
            .map(|(_, hc)| hc.0)
            .unwrap_or(SystemCursorIcon::Default)
    };

    if *last != Some(target) {
        *last = Some(target);
        if let Ok(win) = windows.single() {
            commands.entity(win).insert(CursorIcon::System(target));
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, apply_cursor_icon);
}

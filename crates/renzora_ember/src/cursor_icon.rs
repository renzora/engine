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

// The auto "grabbing" cursor reads the markup drag state; without the `game_ui`
// markup module (lean export) there is no drag, so the param is gated out.
#[cfg(feature = "game_ui")]
use crate::markup::dnd::DragState;

/// Stamped from `cursor="..."`; the OS cursor shown while this node is hovered.
#[derive(Component)]
pub struct HoverCursor(pub SystemCursorIcon);

/// Opt-out marker for [`auto_pointer_cursor`] — for `Interaction` entities
/// that track hover/press but aren't "clickable" (drag surfaces, etc.).
#[derive(Component)]
pub struct NoAutoCursor;

/// Every interactive element gets a pointer cursor by default: any entity
/// with `Interaction` that hasn't declared its own [`HoverCursor`] (text
/// inputs use `Text`, drag handles use `Grab`, ...) is stamped `Pointer`.
/// Runs continuously so late-spawned widgets are covered; `Without` keeps it
/// a no-op after the first frame per entity.
pub(crate) fn auto_pointer_cursor(
    mut commands: Commands,
    q: Query<Entity, (With<Interaction>, Without<HoverCursor>, Without<NoAutoCursor>)>,
) {
    for e in &q {
        commands.entity(e).insert(HoverCursor(SystemCursorIcon::Pointer));
    }
}

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
    #[cfg(feature = "game_ui")] drag: Res<DragState>,
    hovered: Query<(&Interaction, &HoverCursor)>,
    viewport_request: Option<Res<renzora::core::viewport_types::ViewportCursorRequest>>,
    windows: Query<(Entity, &Window)>,
    primary: Query<Entity, With<PrimaryWindow>>,
    mut commands: Commands,
    mut last: Local<Option<(Entity, SystemCursorIcon)>>,
) {
    #[cfg(feature = "game_ui")]
    let dragging = drag.is_dragging();
    #[cfg(not(feature = "game_ui"))]
    let dragging = false;

    let target = if dragging {
        SystemCursorIcon::Grabbing
    } else {
        let widget = hovered
            .iter()
            .find(|(i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed))
            .map(|(_, hc)| hc.0);
        let request = viewport_request.and_then(|r| r.0);
        // A concrete widget cursor (a button's `pointer`, a text field's `text`)
        // always wins. But the viewport paints a blanket `crosshair` over its
        // whole body, and that must NOT mask the 2D interaction layer's request
        // (Move over a selected sprite, resize/rotate over a handle) — otherwise
        // the picker cursor never shows. So when the only hovered cursor is that
        // `crosshair` fallback, the viewport request takes precedence; the
        // request is only ever set inside the 2D viewport, never over a widget.
        match widget {
            Some(c) if c != SystemCursorIcon::Crosshair => c,
            other => request.or(other).unwrap_or(SystemCursorIcon::Default),
        }
    };

    // The icon goes on the window the cursor is actually in — hover state only
    // fires there, so a floating dock window's widgets set its own cursor.
    // Fall back to the primary window so a `Default` reset still lands
    // somewhere when the cursor is between windows.
    let win = windows
        .iter()
        .find(|(_, w)| w.cursor_position().is_some())
        .map(|(e, _)| e)
        .or_else(|| primary.single().ok());
    let Some(win) = win else { return };

    if *last != Some((win, target)) {
        // Reset the previous window's cursor when the pointer moves to another
        // window mid-gesture (e.g. left a floating window showing a resize
        // cursor) so it doesn't stick.
        if let Some((old_win, old_icon)) = *last {
            if old_win != win && old_icon != SystemCursorIcon::Default {
                // `get_entity`: the old window may be a floating dock window
                // that was just closed.
                if let Ok(mut ec) = commands.get_entity(old_win) {
                    ec.insert(CursorIcon::System(SystemCursorIcon::Default));
                }
            }
        }
        *last = Some((win, target));
        commands.entity(win).insert(CursorIcon::System(target));
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, apply_cursor_icon);
}

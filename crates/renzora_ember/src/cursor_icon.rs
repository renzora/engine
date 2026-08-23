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

/// The cursor of the topmost laid-out hovered node, from
/// `(hovered, size, stack index, cursor)` candidates. See the call site for why
/// both the stack index and the size test are load-bearing.
fn topmost_hovered(
    candidates: impl Iterator<Item = (bool, Vec2, u32, SystemCursorIcon)>,
) -> Option<SystemCursorIcon> {
    candidates
        .filter(|(hovered, size, _, _)| *hovered && size.x > 0.0 && size.y > 0.0)
        .max_by_key(|(_, _, stack, _)| *stack)
        .map(|(_, _, _, cursor)| cursor)
}

fn apply_cursor_icon(
    #[cfg(feature = "game_ui")] drag: Res<DragState>,
    hovered: Query<(
        &Interaction,
        &HoverCursor,
        &bevy::ui::ComputedNode,
        &bevy::ui::ComputedStackIndex,
    )>,
    viewport_request: Option<Res<renzora::core::viewport_types::ViewportCursorRequest>>,
    windows: Query<(Entity, &Window)>,
    cursor_opts: Query<&bevy::window::CursorOptions>,
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
        // The **topmost** hovered node wins, by `ComputedStackIndex` — the same
        // order bevy_ui paints in and hands interactions out in.
        //
        // This used to take the first match in query order, which is no order at
        // all: several nodes are legitimately hovered at once wherever one
        // overlaps another that doesn't block the pointer, and whichever
        // happened to come first in the archetype owned the cursor. The editor
        // hit that repeatedly — a resize cursor showing on the bottom panel's
        // buttons and dropdown, an ew-resize showing on its ns-resize grip —
        // because the dock header's filler underneath them is a resize surface,
        // and it kept winning a race it should never have been in.
        //
        // The zero-size test stays, and is separate: `Interaction` is only
        // updated for entities the focus pass can see, so a node hidden with
        // `Display::None` keeps whatever value it held when it was hidden (hide
        // one under the cursor — closing a panel by clicking its own toggle does
        // exactly that — and it reads `Hovered` forever). A stale entry like
        // that can carry a high stack index, so topmost alone wouldn't catch it.
        // Zero computed size does, including via a hidden *ancestor*, which
        // checking this node's own `display` would miss.
        let widget = topmost_hovered(hovered.iter().map(|(i, hc, cn, stack)| {
            (
                matches!(i, Interaction::Hovered | Interaction::Pressed),
                cn.size(),
                stack.0,
                hc.0,
            )
        }));
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

    // Don't paint a cursor icon on a window whose OS cursor is hidden. Something
    // deliberately hid it — an editor-camera right/middle look-drag, a terrain
    // brush, a modal transform, a script `lock_cursor()` — and re-asserting a
    // `CursorIcon` here re-shows it, which is why the viewport's crosshair used
    // to stay visible (pinned by the camera's cursor-lock) during a right-drag
    // orbit instead of disappearing. Leave the hidden cursor hidden.
    if cursor_opts.get(win).is_ok_and(|o| !o.visible) {
        return;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Vec2 = Vec2::new(100.0, 20.0);

    /// Overlapping hovered nodes are normal — anything that doesn't block the
    /// pointer leaves the node beneath it hovered too — so the cursor has to
    /// come from the one on top, not from whichever the query yields first.
    ///
    /// Regression guard: the bottom panel's controls and its ns-resize grip sit
    /// over the dock header's filler, which is an ew-resize surface. Taking the
    /// first match let the filler supply the cursor for all of them.
    #[test]
    fn the_topmost_hovered_node_supplies_the_cursor() {
        let filler = (true, SIZE, 10, SystemCursorIcon::EwResize);
        let grip = (true, SIZE, 42, SystemCursorIcon::NsResize);
        // Either order in the query; the answer is the same.
        assert!(matches!(
            topmost_hovered([filler, grip].into_iter()),
            Some(SystemCursorIcon::NsResize)
        ));
        assert!(matches!(
            topmost_hovered([grip, filler].into_iter()),
            Some(SystemCursorIcon::NsResize)
        ));
    }

    /// A node hidden under the cursor keeps `Interaction::Hovered` forever, and
    /// can carry a *higher* stack index than anything real — so topmost alone
    /// isn't enough. Its computed size collapses to zero, which is the test.
    #[test]
    fn a_hidden_node_never_supplies_the_cursor() {
        let hidden = (true, Vec2::ZERO, 900, SystemCursorIcon::NsResize);
        let button = (true, SIZE, 5, SystemCursorIcon::Pointer);
        assert!(matches!(
            topmost_hovered([hidden, button].into_iter()),
            Some(SystemCursorIcon::Pointer)
        ));
    }

    #[test]
    fn nothing_hovered_yields_no_cursor() {
        let idle = (false, SIZE, 7, SystemCursorIcon::Pointer);
        assert!(topmost_hovered([idle].into_iter()).is_none());
    }
}

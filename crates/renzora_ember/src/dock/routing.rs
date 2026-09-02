//! Which tree does a dock area belong to?
//!
//! Three kinds of area coexist — the primary [`Dock`], the pinned [`FixedDock`],
//! and any number of floating windows — and every drag, drop, tab switch and
//! close has to mutate the right one. These helpers are the single place that
//! decision is made, plus the screen-space cursor tracking the cross-window
//! gestures depend on.

use bevy::prelude::*;

use crate::dock::components::GlobalCursor;
use crate::dock::tree::DockTree;
use crate::dock::{Dock, DockDirty, DockWindows, FixedDock};

/// The live tree owning dock area `area`: a floating window's tree if `area`
/// belongs to one, the [`FixedDock`]'s if it is that area, else the primary
/// [`Dock`]'s.
///
/// The primary is the fallback rather than a match, so a new area kind that
/// forgets to register here silently mutates the primary tree instead of its
/// own. That is why [`FixedDock::area`] is checked explicitly before falling
/// through: every drag, drop, tab switch and close routes through this one
/// function, so getting it wrong once corrupts the workspace layout from eight
/// different call sites.
pub(crate) fn area_tree_mut<'a>(
    area: Entity,
    dock: &'a mut Dock,
    fixed: &'a mut FixedDock,
    wins: &'a mut DockWindows,
) -> &'a mut DockTree {
    if fixed.area == Some(area) {
        return &mut fixed.tree;
    }
    match wins.0.iter_mut().find(|s| s.area == area) {
        Some(st) => &mut st.tree,
        None => &mut dock.tree,
    }
}

/// Flag `area`'s tree for rebuild — [`DockDirty`] for the primary dock,
/// [`FixedDock::dirty`] for the fixed area, the per-window flag for a floating
/// one.
pub(crate) fn flag_area_dirty(
    area: Entity,
    dirty: &mut DockDirty,
    fixed: &mut FixedDock,
    wins: &mut DockWindows,
) {
    if fixed.area == Some(area) {
        fixed.dirty = true;
        return;
    }
    match wins.0.iter_mut().find(|s| s.area == area) {
        Some(st) => st.dirty = true,
        None => dirty.0 = true,
    }
}

/// The OS window hosting dock area `area` (the floating window's, else the
/// primary window).
pub(crate) fn area_window(
    area: Entity,
    wins: &DockWindows,
    primary: Option<Entity>,
) -> Option<Entity> {
    wins.0
        .iter()
        .find(|s| s.area == area)
        .map(|s| s.window)
        .or(primary)
}

/// Does `global` (physical screen px) land inside `win`'s client area? Only
/// answerable once winit has reported the window's position (`At`).
pub(crate) fn window_contains(win: &Window, global: Vec2) -> bool {
    let bevy::window::WindowPosition::At(origin) = win.position else {
        return false;
    };
    let local = global - origin.as_vec2();
    local.x >= 0.0
        && local.y >= 0.0
        && local.x < win.physical_width() as f32
        && local.y < win.physical_height() as f32
}

/// `global` (physical screen px) converted into `win`-local physical px, if the
/// window's position is known.
pub(crate) fn window_local(win: &Window, global: Vec2) -> Option<Vec2> {
    let bevy::window::WindowPosition::At(origin) = win.position else {
        return None;
    };
    Some(global - origin.as_vec2())
}

/// Track the cursor in physical screen space from raw move messages — see
/// [`GlobalCursor`] for why this can't just read `Window::cursor_position()`.
pub(crate) fn track_global_cursor(
    mut cursor: ResMut<GlobalCursor>,
    mut moves: MessageReader<bevy::window::CursorMoved>,
    windows: Query<&Window>,
) {
    for ev in moves.read() {
        let Ok(win) = windows.get(ev.window) else {
            continue;
        };
        // `position` is `Automatic` until winit reports the first `Moved` for
        // this window. Falling back to a zero origin keeps single-window
        // coordinates self-consistent (everything is relative to that window)
        // so divider/tab drags still work before the position resolves.
        let origin = match win.position {
            bevy::window::WindowPosition::At(origin) => origin.as_vec2(),
            _ => Vec2::ZERO,
        };
        cursor.pos = Some(origin + ev.position * win.scale_factor());
    }
}

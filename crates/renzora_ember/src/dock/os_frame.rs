//! Floating dock windows under the native OS frame, on Windows.
//!
//! The main editor window wears the OS frame (`renzora::EDITOR_OS_FRAME`), and
//! that is what gives it Windows Snap: dragging it to a screen edge, the snap
//! layout grid on the maximize button, Win+arrow. Floating windows used to be
//! undecorated, with a title bar of our own that moved the window by setting its
//! position every frame. Windows only snaps a window it is moving itself, so a
//! float could never join the grid. Under [`OS_FRAME`] a float gets the real
//! frame instead, and Windows does all of that for it.
//!
//! # Re-docking survives the change
//!
//! The custom title bar also re-docked: drop the float on one of the main
//! window's tab bars and the panel went back there. With the OS moving the
//! window, nothing tells us a drag started or ended, so [`float_os_move_redock`]
//! works it out from what it can see: a float that moved while the cursor sat
//! in its caption is being dragged by its title bar, and the moment the left
//! button is up again the drag is over. If the cursor is then over a main-window
//! tab bar, the panel docks there.
//!
//! Only tab bars count, not the dock's edge and corner bands the custom drag
//! also accepted. With the editor maximized those bands run along the screen
//! edges, which is exactly where you drop a window to snap it, and a drop
//! there has to mean snap.
//!
//! The button and cursor are read from Win32 rather than from Bevy. A press
//! on a native caption is not a client-area click, so Bevy never sees it, and
//! while Windows runs its move loop Bevy may not update at all: the whole drag
//! can arrive afterwards as moves reported with the button already up. Both
//! cases end the same way here.

use bevy::prelude::*;

use crate::dock::components::{DockLeaf, TabBarOf};
use crate::dock::drag::{insert_action, set_zone_rect, DropAction};
use crate::dock::routing::{area_tree_mut, flag_area_dirty};
use crate::dock::tree::DropZone;
use crate::dock::windows::{DockWindowCloseRequests, FloatDrag, FloatingDockArea};
use crate::dock::{Dock, DockArea, DockDirty, DockWindows, FixedDock};

/// Whether floating dock windows wear the native frame. Windows only: Snap is
/// a Windows feature, and elsewhere the custom title bar's re-dock drag is
/// kept as it was.
pub(crate) const OS_FRAME: bool = cfg!(target_os = "windows") && renzora::EDITOR_OS_FRAME;

/// Frames after a tear-off during which a float's moves are not taken as a
/// title-bar drag. The tear-off moves the window itself, and its last move can
/// be reported a frame after the button came up, with the cursor still over the
/// tab bar the panel was just torn from. Nobody can release and grab the
/// caption again this fast.
const TEAROFF_GRACE_FRAMES: u32 = 10;

/// A title-bar drag of a float, as far as it can be seen.
pub(crate) struct OsMove {
    window: Entity,
    action: Option<DropAction>,
    shown_overlay: Option<Entity>,
}

/// Follow OS title-bar drags of floating windows and re-dock on release over
/// a main-window tab bar. See the module doc.
#[allow(clippy::too_many_arguments)]
pub(crate) fn float_os_move_redock(
    mut moved: MessageReader<bevy::window::WindowMoved>,
    mut state: Local<Option<OsMove>>,
    mut grace: Local<Vec<(Entity, u32)>>,
    tear_off: Res<FloatDrag>,
    handles: Query<&bevy::window::RawHandleWrapper>,
    primary: Query<&bevy::window::RawHandleWrapper, With<bevy::window::PrimaryWindow>>,
    areas: Query<(Entity, Option<&FloatingDockArea>), With<DockArea>>,
    tabbars: Query<(&TabBarOf, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>,
    leaves: Query<&DockLeaf>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<Dock>,
    mut fixed: ResMut<FixedDock>,
    mut dirty: ResMut<DockDirty>,
    mut wins: ResMut<DockWindows>,
    mut close_queue: ResMut<DockWindowCloseRequests>,
) {
    if !OS_FRAME {
        moved.clear();
        return;
    }

    // Moves of a window being torn off, or just released from one, are the
    // tear-off's own. Count the grace down, and restart it while it lasts.
    grace.retain_mut(|(_, n)| {
        *n = n.saturating_sub(1);
        *n > 0
    });
    if let Some(w) = tear_off.window() {
        match grace.iter_mut().find(|(e, _)| *e == w) {
            Some((_, n)) => *n = TEAROFF_GRACE_FRAMES,
            None => grace.push((w, TEAROFF_GRACE_FRAMES)),
        }
    }

    let Some(cursor) = win32::cursor() else {
        moved.clear();
        return;
    };
    for m in moved.read() {
        let float = wins.0.iter().any(|s| s.window == m.window);
        if !float || grace.iter().any(|(e, _)| *e == m.window) {
            continue;
        }
        if state.as_ref().is_some_and(|s| s.window == m.window) {
            continue;
        }
        if handles.get(m.window).is_ok_and(|h| win32::in_caption(h, cursor)) {
            *state = Some(OsMove { window: m.window, action: None, shown_overlay: None });
        }
    }
    let Some(st) = state.as_mut() else { return };

    // The main window's tab bar under the cursor, if any.
    let mut target: Option<(DropAction, Entity)> = None;
    if let Some(local) = primary.single().ok().and_then(|h| win32::client_point(h, cursor)) {
        let primary_area = areas.iter().find(|(_, f)| f.is_none()).map(|(a, _)| a);
        let bar = tabbars.iter().find_map(|(bar, cn, gt)| cn.contains_point(*gt, local).then_some(bar.0));
        if let Some(ld) = bar.and_then(|b| leaves.get(b).ok()) {
            if primary_area == Some(ld.area) {
                if let Some(rep) = ld.tabs.first().cloned() {
                    target = Some((DropAction::Tab { area: ld.area, rep, before: None }, ld.overlay));
                }
            }
        }
    }

    // The same drop preview the tab drag shows, while Bevy is running to draw it.
    let overlay = target.as_ref().map(|(_, o)| *o);
    if st.shown_overlay != overlay {
        if let Some(old) = st.shown_overlay {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        if let Some(new) = overlay {
            if let Ok(mut n) = nodes.get_mut(new) {
                n.display = Display::Flex;
                set_zone_rect(&mut n, DropZone::Center);
            }
        }
        st.shown_overlay = overlay;
    }
    st.action = target.map(|(a, _)| a);

    if win32::left_button_down() {
        return;
    }

    // Released: dock the float's panel where it was dropped, if anywhere.
    let Some(done) = state.take() else { return };
    if let Some(e) = done.shown_overlay {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::None;
        }
    }
    let Some(action) = done.action else { return };
    let panel = wins
        .0
        .iter()
        .find(|s| s.window == done.window)
        .and_then(|s| s.tree.first_panel().map(str::to_string));
    let Some(panel) = panel else { return };
    if let Some(s) = wins.0.iter_mut().find(|s| s.window == done.window) {
        s.tree.remove_panel(&panel);
    }
    insert_action(area_tree_mut(action.area(), &mut dock, &mut fixed, &mut wins), &panel, &action);
    flag_area_dirty(action.area(), &mut dirty, &mut fixed, &mut wins);
    close_queue.0.push(done.window);
}

#[cfg(target_os = "windows")]
mod win32 {
    use bevy::math::{IVec2, Vec2};

    #[repr(C)]
    #[derive(Default)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    const VK_LBUTTON: i32 = 0x01;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
        fn GetAsyncKeyState(key: i32) -> i16;
        fn ScreenToClient(hwnd: isize, point: *mut Point) -> i32;
        fn ClientToScreen(hwnd: isize, point: *mut Point) -> i32;
        fn GetClientRect(hwnd: isize, rect: *mut Rect) -> i32;
        fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    }

    fn hwnd(handle: &bevy::window::RawHandleWrapper) -> Option<isize> {
        match handle.get_window_handle() {
            raw_window_handle::RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
            _ => None,
        }
    }

    /// The cursor in physical screen px.
    pub(super) fn cursor() -> Option<IVec2> {
        let mut p = Point::default();
        // SAFETY: `p` is a valid, writable POINT.
        (unsafe { GetCursorPos(&mut p) } != 0).then(|| IVec2::new(p.x, p.y))
    }

    /// Whether the left mouse button is physically down right now.
    pub(super) fn left_button_down() -> bool {
        // SAFETY: a plain query with no pointers. The high bit is "down".
        (unsafe { GetAsyncKeyState(VK_LBUTTON) } as u16 & 0x8000) != 0
    }

    /// Whether `cursor` is in the window's caption: inside its frame, above
    /// its client area, and between the client area's sides.
    ///
    /// Measured from rectangles rather than by asking the window
    /// (`WM_NCHITTEST`), because asking sends it a message, and this runs on a
    /// Bevy worker thread while the window's own thread is busy running the
    /// frame: the message would wait for a reply that never comes.
    pub(super) fn in_caption(handle: &bevy::window::RawHandleWrapper, cursor: IVec2) -> bool {
        let Some(hwnd) = hwnd(handle) else { return false };
        let (mut outer, mut client, mut origin) = (Rect::default(), Rect::default(), Point::default());
        // SAFETY: valid out-pointers; a stale handle only makes the calls fail.
        let ok = unsafe {
            GetWindowRect(hwnd, &mut outer) != 0
                && GetClientRect(hwnd, &mut client) != 0
                && ClientToScreen(hwnd, &mut origin) != 0
        };
        ok && cursor.y >= outer.top
            && cursor.y < origin.y
            && cursor.x >= origin.x
            && cursor.x < origin.x + client.right
    }

    /// `cursor` in the window's client-area px, if it is inside the client area.
    pub(super) fn client_point(handle: &bevy::window::RawHandleWrapper, cursor: IVec2) -> Option<Vec2> {
        let hwnd = hwnd(handle)?;
        let (mut p, mut client) = (Point { x: cursor.x, y: cursor.y }, Rect::default());
        // SAFETY: valid out-pointers; a stale handle only makes the calls fail.
        let ok = unsafe { ScreenToClient(hwnd, &mut p) != 0 && GetClientRect(hwnd, &mut client) != 0 };
        (ok && p.x >= 0 && p.y >= 0 && p.x < client.right && p.y < client.bottom)
            .then(|| Vec2::new(p.x as f32, p.y as f32))
    }
}

/// Never reached: [`OS_FRAME`] is false off Windows, so the system returns
/// before asking.
#[cfg(not(target_os = "windows"))]
mod win32 {
    use bevy::math::{IVec2, Vec2};

    pub(super) fn cursor() -> Option<IVec2> {
        None
    }
    pub(super) fn left_button_down() -> bool {
        false
    }
    pub(super) fn in_caption(_handle: &bevy::window::RawHandleWrapper, _cursor: IVec2) -> bool {
        false
    }
    pub(super) fn client_point(_handle: &bevy::window::RawHandleWrapper, _cursor: IVec2) -> Option<Vec2> {
        None
    }
}

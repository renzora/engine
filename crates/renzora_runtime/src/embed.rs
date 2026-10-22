//! Running inside the editor's Game panel.
//!
//! When the editor starts the game with `--embed-parent <hwnd>`, the game's
//! window becomes a child window of the editor's, and the editor then places
//! it over its Game panel every frame. Keyboard and mouse go straight to the
//! game's own window, so there is no input forwarding and no copying of frames
//! between the processes: the game still renders with its own GPU device, in
//! its own process, and only its window lives inside the editor's.
//!
//! **Why the game reparents itself rather than the editor doing it.** The
//! window is created hidden and only made a child here, so it never appears on
//! screen as a separate window first. The editor shows it once it has a place
//! for it.
//!
//! **Why nothing else may touch the window's shape afterwards.** winit keeps
//! its own idea of the window's style and rewrites the whole style whenever a
//! decoration, resize or fullscreen setting changes, which would turn the
//! child back into a top-level window. So an embedded game skips the project's
//! window settings entirely (see `apply_window_config`).
//!
//! Windows only for now. Elsewhere the flag is ignored and the editor does not
//! pass it.

use bevy::prelude::*;

/// The editor window handle passed in `--embed-parent`, or `None` when the
/// game runs in a window of its own.
pub fn embed_parent() -> Option<isize> {
    static PARENT: std::sync::OnceLock<Option<isize>> = std::sync::OnceLock::new();
    *PARENT.get_or_init(|| {
        if !cfg!(target_os = "windows") {
            return None;
        }
        let mut args = std::env::args();
        while let Some(arg) = args.next() {
            if arg == "--embed-parent" {
                return args.next().and_then(|v| v.parse().ok());
            }
        }
        None
    })
}

pub(crate) fn register(app: &mut App) {
    #[cfg(target_os = "windows")]
    if embed_parent().is_some() {
        app.add_systems(Update, adopt_embed_parent);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

/// Keep the game's window a child of the editor's.
///
/// Checked every frame rather than done once, because it does not stay done:
/// winit holds its own record of the window's style and writes the whole style
/// back from it whenever it applies a window change, which drops `WS_CHILD` and
/// turns the window top-level again. The editor finds the game's window by its
/// parent, so a game that lost it was never found, never shown, and Play
/// looked like it did nothing. Two reads a frame cost nothing next to that.
///
/// **Main thread only** (`NonSendMarker`). Changing a window's style or parent
/// from a thread that does not own it sends the owner synchronous messages and
/// waits for the answer. Run on a worker, this waited on the main thread, which
/// was itself inside the frame waiting on the worker: the game deadlocked on
/// its first frame with its window still hidden, and the Game panel never got
/// anything to show.
///
/// **An editor window that is gone ends the game.** Closing the editor takes
/// its windows with it, this one's included, and the game used to carry on
/// without one: `SetParent` failed every frame against the dead handle, so it
/// ran on hidden, holding the executable open (which blocks the next build)
/// with no window anyone could close. A game started for the Game panel has no
/// other place to be, so it quits.
#[cfg(target_os = "windows")]
fn adopt_embed_parent(
    _main_thread: bevy::ecs::system::NonSendMarker,
    mut adoptions: Local<u32>,
    mut warned: Local<bool>,
    windows: Query<&bevy::window::RawHandleWrapper, With<bevy::window::PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    use winit::raw_window_handle::RawWindowHandle;

    let Some(parent) = embed_parent() else {
        return;
    };
    // SAFETY: `IsWindow` only reads; any value is a valid argument.
    if unsafe { win32::IsWindow(parent) } == 0 {
        info!("[embed] the editor window is gone, so the game is closing");
        exit.write(AppExit::Success);
        return;
    }
    let Ok(handle) = windows.single() else {
        return;
    };
    let RawWindowHandle::Win32(win) = handle.get_window_handle() else {
        return;
    };
    let hwnd = win.hwnd.get();
    // SAFETY: `hwnd` is this process's own live window, and `parent` is a
    // window handle; Win32 validates it and fails the call if it is stale.
    unsafe {
        // A window style is a 32-bit DWORD, whatever the pointer-sized API.
        let style = win32::GetWindowLongPtrW(hwnd, win32::GWL_STYLE) as u32;
        if style & win32::WS_CHILD != 0 && win32::GetParent(hwnd) == parent {
            return;
        }
        let style = (style & !win32::TOP_LEVEL_STYLES) | win32::WS_CHILD | win32::WS_CLIPSIBLINGS;
        win32::SetWindowLongPtrW(hwnd, win32::GWL_STYLE, style as i32 as isize);
        if win32::SetParent(hwnd, parent) == 0 {
            // Once: this is retried every frame, and a line per frame is
            // enough log traffic to stall the game.
            if !std::mem::replace(&mut *warned, true) {
                warn!("[embed] could not place the game inside the editor; it stays hidden");
            }
            return;
        }
    }
    *adoptions += 1;
    if *adoptions == 2 {
        info!("[embed] the game's window lost its place in the editor and was put back");
    }
}

#[cfg(target_os = "windows")]
mod win32 {
    pub const GWL_STYLE: i32 = -16;
    pub const WS_CHILD: u32 = 0x4000_0000;
    pub const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
    /// `WS_POPUP | WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX |
    /// WS_MAXIMIZEBOX`: everything that makes a window a top-level one.
    pub const TOP_LEVEL_STYLES: u32 =
        0x8000_0000 | 0x00C0_0000 | 0x0004_0000 | 0x0008_0000 | 0x0002_0000 | 0x0001_0000;

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
        pub fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
        pub fn SetParent(child: isize, parent: isize) -> isize;
        pub fn GetParent(hwnd: isize) -> isize;
        pub fn IsWindow(hwnd: isize) -> i32;
    }
}

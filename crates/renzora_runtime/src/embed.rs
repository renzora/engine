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

/// Make the game's window a child of the editor's, once the native window
/// exists. One attempt per frame until it does, then never again.
#[cfg(target_os = "windows")]
fn adopt_embed_parent(
    mut done: Local<bool>,
    windows: Query<&bevy::window::RawHandleWrapper, With<bevy::window::PrimaryWindow>>,
) {
    use winit::raw_window_handle::RawWindowHandle;

    if *done {
        return;
    }
    let (Some(parent), Ok(handle)) = (embed_parent(), windows.single()) else {
        return;
    };
    let RawWindowHandle::Win32(win) = handle.get_window_handle() else {
        *done = true;
        return;
    };
    *done = true;
    let hwnd = win.hwnd.get();
    // SAFETY: `hwnd` is this process's own live window, and `parent` is a
    // window handle; Win32 validates it and fails the call if it is stale.
    unsafe {
        // A window style is a 32-bit DWORD, whatever the pointer-sized API.
        let style = win32::GetWindowLongPtrW(hwnd, win32::GWL_STYLE) as u32;
        let style = (style & !win32::TOP_LEVEL_STYLES) | win32::WS_CHILD | win32::WS_CLIPSIBLINGS;
        win32::SetWindowLongPtrW(hwnd, win32::GWL_STYLE, style as i32 as isize);
        if win32::SetParent(hwnd, parent) == 0 {
            warn!("[embed] could not place the game inside the editor; it stays hidden");
        }
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
    }
}

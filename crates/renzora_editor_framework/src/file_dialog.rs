//! Native file dialogs that belong to the editor window.
//!
//! An `rfd::FileDialog` with no parent is a free-floating window. While the
//! editor was minimised, Save Scene As came up on its own, and the "replace the
//! existing file?" prompt it raised appeared off screen and hidden. A dialog
//! waiting on its own hidden prompt refuses every click, so the only ways out
//! were guessing a key or killing the editor.
//!
//! [`editor_file_dialog`] restores a minimised editor first and makes it the
//! dialog's owner, so the dialog and anything it raises stay on top of the
//! editor and centred on it. Windows only: elsewhere rfd parents through the
//! platform's own sheet mechanics, and the bare dialog is unchanged.

use bevy::prelude::*;
#[cfg(target_os = "windows")]
use bevy::window::{PrimaryWindow, RawHandleWrapper};

/// A file dialog owned by the editor window, which is restored first if it was
/// minimised. Call from an exclusive system, which is how every editor dialog
/// is opened: the dialog blocks, and the window handle is read on the thread
/// that owns the window.
pub fn editor_file_dialog(world: &mut World) -> rfd::FileDialog {
    let dialog = rfd::FileDialog::new();
    #[cfg(target_os = "windows")]
    {
        let mut windows = world.query_filtered::<&RawHandleWrapper, With<PrimaryWindow>>();
        if let Ok(handle) = windows.single(world) {
            restore_if_minimised(handle);
            // SAFETY: exclusive systems run on the main thread, which owns the
            // window, and the wrapper is only borrowed for the call.
            let parent = unsafe { handle.get_handle() };
            return dialog.set_parent(&parent);
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = world;
    dialog
}

#[cfg(target_os = "windows")]
fn restore_if_minimised(handle: &RawHandleWrapper) {
    use raw_window_handle::RawWindowHandle;

    const SW_RESTORE: i32 = 9;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn IsIconic(hwnd: isize) -> i32;
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    }

    let RawWindowHandle::Win32(win) = handle.get_window_handle() else {
        return;
    };
    let hwnd = win.hwnd.get();
    // SAFETY: both only take a window handle, and Win32 checks it.
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
    }
}

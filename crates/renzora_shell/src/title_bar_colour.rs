//! Paint the Windows title bar in the theme's window colour.
//!
//! The editor runs under the OS window frame (`renzora::EDITOR_OS_FRAME`), so
//! Windows draws the title bar itself, in its own grey. Directly beneath it
//! sits our top bar, and an unrelated grey meeting the theme's own colours
//! read as two bands that didn't belong together.
//!
//! Windows 11 lets an app choose its caption colour (`DWMWA_CAPTION_COLOR`).
//! This gives it the theme's window colour: the darkest chrome tone, one step
//! under the top bar (which is lifted off it, see `BarStyle::top_bar`). The
//! two were tried as one identical colour first, and with nothing between
//! them the title bar and the bar of controls under it ran together; a step
//! apart they read as the frame and the toolbar it holds. It also asks for the
//! dark caption text and buttons when the colour is dark, so a light theme does
//! not get white text on a white bar or the reverse. Windows 10 ignores the
//! attribute; the call simply fails and nothing changes.
//!
//! Re-applied whenever the theme changes, since both colours follow it.
//!
//! Every window gets it, not only the main one: floating panel windows wear
//! the OS frame too (so they can join Windows Snap), and a default grey caption
//! on each of them would be the same mismatch over again.

use bevy::prelude::*;

/// Apply the caption colour to each window as it appears, and to all of them
/// again whenever the palette's window colour changes. Each window remembers
/// the colour it was last given, so the frames in between cost one palette
/// read per window.
pub(crate) fn sync_title_bar_colour(
    windows: Query<(Entity, &bevy::window::RawHandleWrapper)>,
    mut applied: Local<Vec<(Entity, [u8; 3])>>,
) {
    if !renzora::EDITOR_OS_FRAME {
        return;
    }
    let (r, g, b) = renzora_ember::theme::window_bg();
    let rgb = [r, g, b];
    // Forget closed windows, so a new one reusing the entity is coloured.
    applied.retain(|(e, _)| windows.contains(*e));
    // A window may not have its native handle on its first frames; it is
    // picked up once it does.
    for (e, handle) in &windows {
        let done = applied.iter().any(|(w, c)| *w == e && *c == rgb);
        if done || !win32::set_caption(handle, rgb) {
            continue;
        }
        match applied.iter_mut().find(|(w, _)| *w == e) {
            Some(entry) => entry.1 = rgb,
            None => applied.push((e, rgb)),
        }
    }
}

#[cfg(target_os = "windows")]
mod win32 {
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_CAPTION_COLOR: u32 = 35;

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(hwnd: isize, attribute: u32, value: *const core::ffi::c_void, size: u32) -> i32;
    }

    /// Set the caption colour (and dark caption text on a dark bar). Returns
    /// whether there was a window to set it on, so a missing handle is retried
    /// next frame rather than recorded as done.
    pub(super) fn set_caption(handle: &bevy::window::RawHandleWrapper, rgb: [u8; 3]) -> bool {
        let raw_window_handle::RawWindowHandle::Win32(win) = handle.get_window_handle() else {
            return false;
        };
        let hwnd = win.hwnd.get();
        // COLORREF is 0x00BBGGRR.
        let colour: u32 = rgb[0] as u32 | (rgb[1] as u32) << 8 | (rgb[2] as u32) << 16;
        // Rec. 709 luma, on the bytes: under half is a dark bar.
        let luma = 0.2126 * rgb[0] as f32 + 0.7152 * rgb[1] as f32 + 0.0722 * rgb[2] as f32;
        let dark: i32 = (luma < 128.0) as i32;
        // SAFETY: `hwnd` is a live editor window's handle, and each value
        // pointer is to a local of exactly the size passed with it. A stale
        // handle or an attribute this Windows lacks only makes the call fail.
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const i32 as *const _,
                std::mem::size_of::<i32>() as u32,
            );
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_CAPTION_COLOR,
                &colour as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
        true
    }
}

/// Only Windows lets an app colour the system title bar this way.
#[cfg(not(target_os = "windows"))]
mod win32 {
    pub(super) fn set_caption(_handle: &bevy::window::RawHandleWrapper, _rgb: [u8; 3]) -> bool {
        true
    }
}

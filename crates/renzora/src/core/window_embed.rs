//! Putting one process's window inside another process's window.
//!
//! The play panel shows a running game that is not this process. The game keeps
//! its own window and its own swapchain and renders at full speed into it; the
//! window manager is told that window is a *child* of the editor's, so it is
//! composited inside the editor's client area, clipped to it, and moves with it.
//! Nothing is copied, nothing is encoded, and input needs no forwarding because
//! the child is a real window the OS routes events to.
//!
//! # What this costs, and it is not small
//!
//! A child window is a separate surface the compositor paints **over** the
//! parent's client area, after the parent has finished drawing. It is not in
//! bevy_ui's z-order and no `GlobalZIndex` can beat it: the editor cannot draw
//! anything on top of the game, ever. A dropdown that overlaps it, a tooltip, a
//! modal, the dock's drag preview: all of them are painted over.
//!
//! The answer is [`set_visible`]: hide the child whenever the editor needs that
//! rectangle, and show it again afterwards. That is a policy the caller owns,
//! because only the caller knows what is on screen.
//!
//! # Platforms
//!
//! Windows and X11 confine a child window to its parent's client area, which is
//! exactly the behaviour wanted. Wayland has no equivalent (subsurfaces exist
//! only within one client) and macOS child windows float rather than clip, so
//! both fall back to the game in its own window. Every function here is a no-op
//! off Windows for now, and reports so through its return value rather than
//! silently doing nothing.
//!
//! # Why the calls are declared rather than depended on
//!
//! Four stable `user32` exports against a whole `windows-sys` version to
//! unify. The same trade as `renzora_native_build`'s `nice`, for the same
//! reason.

/// A native window handle, as a plain integer.
///
/// An `isize` rather than a typed handle because it has to survive a trip
/// through a command line: the editor passes its own handle to the game process
/// as `--embed <handle>`, and the game passes it straight back to the window
/// manager. Nothing in between needs to know what it points at.
pub type NativeWindow = isize;

// Turning a `RawHandleWrapper` into a `NativeWindow` is deliberately not here.
// It needs the `raw_window_handle` crate to name the handle enum, and this is
// the contract crate: Bevy and serde, nothing else. The two callers that need
// it each do the match themselves, which is eight lines and keeps each honest
// about the platforms it handles.

/// Make `child` a child window of `parent`, confined to its client area.
///
/// Returns whether it took effect. `false` means this platform does not do
/// this, and the caller should leave the game in its own window rather than
/// pretend.
///
/// The child is expected to have been created **hidden**. Reparenting an
/// already-visible top-level window works, but the window flashes on screen at
/// its own size first, which reads as a bug.
pub fn embed_into(child: NativeWindow, parent: NativeWindow) -> bool {
    #[cfg(target_os = "windows")]
    {
        // `WS_CHILD` is what confines the window to the parent's client area;
        // the two it replaces are what made it a free-floating top-level window.
        // Setting the parent without changing the style leaves a window that is
        // parented but still drawn with a frame and still positioned in screen
        // coordinates.
        const GWL_STYLE: i32 = -16;
        const WS_CHILD: i64 = 0x4000_0000;
        const WS_POPUP: i64 = 0x8000_0000;
        const WS_OVERLAPPEDWINDOW: i64 = 0x00CF_0000;
        const WS_VISIBLE: i64 = 0x1000_0000;

        unsafe {
            if SetParent(child, parent).is_null() {
                return false;
            }
            let style = GetWindowLongPtrW(child, GWL_STYLE);
            let style = (style & !(WS_POPUP | WS_OVERLAPPEDWINDOW)) | WS_CHILD;
            // `WS_VISIBLE` is cleared deliberately: the caller shows the window
            // once it has been placed, so it never appears at the wrong size in
            // the wrong corner for a frame.
            SetWindowLongPtrW(child, GWL_STYLE, style & !WS_VISIBLE);
        }
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (child, parent);
        false
    }
}

/// Move and size an embedded window, in physical pixels relative to the
/// parent's client area.
///
/// Called every frame the panel's rectangle could have moved, which is most of
/// them: a dock drag, a window resize, a panel scroll. The call is cheap and
/// the alternative is tracking what changed, which is the same work with a
/// chance of being wrong.
pub fn place(child: NativeWindow, x: i32, y: i32, width: u32, height: u32) {
    #[cfg(target_os = "windows")]
    {
        // Z-order untouched, focus untouched: this is a geometry call and
        // nothing else. Taking focus here would steal it from the editor on
        // every frame the panel moved.
        const SWP_NOZORDER: u32 = 0x0004;
        const SWP_NOACTIVATE: u32 = 0x0010;
        // A zero-sized window is not an error worth reporting, but it is worth
        // not asking for: a panel mid-collapse lays out to nothing.
        let width = width.max(1) as i32;
        let height = height.max(1) as i32;
        unsafe {
            SetWindowPos(
                child,
                std::ptr::null_mut(),
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (child, x, y, width, height);
    }
}

/// Show or hide an embedded window.
///
/// The whole mitigation for a child window's unbeatable z-order: hide it while
/// the editor needs to draw over that rectangle, and show it again afterwards.
/// The game keeps running either way, which is why this is a hide and not a
/// pause.
pub fn set_visible(child: NativeWindow, visible: bool) {
    #[cfg(target_os = "windows")]
    {
        const SW_HIDE: i32 = 0;
        // `SHOWNA` rather than `SHOW`: show without activating. The editor keeps
        // keyboard focus when the game reappears, so a dropdown closing does not
        // also hand the keyboard to the game.
        const SW_SHOWNA: i32 = 8;
        unsafe {
            ShowWindow(child, if visible { SW_SHOWNA } else { SW_HIDE });
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (child, visible);
    }
}

#[cfg(target_os = "windows")]
extern "system" {
    fn SetParent(hwnd: NativeWindow, parent: NativeWindow) -> *mut core::ffi::c_void;
    fn GetWindowLongPtrW(hwnd: NativeWindow, index: i32) -> i64;
    fn SetWindowLongPtrW(hwnd: NativeWindow, index: i32, value: i64) -> i64;
    fn SetWindowPos(
        hwnd: NativeWindow,
        insert_after: *mut core::ffi::c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
    fn ShowWindow(hwnd: NativeWindow, cmd: i32) -> i32;
}

/// The line an embedded game prints once it has become a child window, so the
/// editor learns the handle it has to place.
///
/// The editor knows the handle it *gave* (its own, as the parent) and not the
/// one it needs (the game's). Rather than open a channel for one integer, the
/// game prints it: the editor already pipes and reads the child's stdout to
/// forward its log to the Console, so the pipe exists and is already being
/// drained. The reader recognises this prefix, takes the value, and does not
/// show the line.
///
/// Asking the window manager instead ("which window is a child of mine?") looks
/// simpler and is not: a window can acquire children nobody asked for, from IME
/// and accessibility, and picking the wrong one would move something that is not
/// the game.
pub const EMBED_HANDSHAKE: &str = "[renzora-embed] hwnd=";

/// Format the handshake line. Printed by the game, parsed by the editor.
pub fn handshake_line(child: NativeWindow) -> String {
    format!("{EMBED_HANDSHAKE}{child}")
}

/// The handle in a handshake line, if that is what this line is.
pub fn handshake_handle(line: &str) -> Option<NativeWindow> {
    line.trim().strip_prefix(EMBED_HANDSHAKE)?.trim().parse().ok()
}

/// The `--embed <handle>` argument, if this process was launched to be embedded.
///
/// Read from the command line rather than passed through a resource because it
/// has to be known before the window is created: an embedded game creates its
/// window hidden, and a window that has already appeared cannot be un-appeared.
pub fn embed_target_from_args() -> Option<NativeWindow> {
    std::env::args()
        .skip_while(|a| a != "--embed")
        .nth(1)
        .and_then(|v| v.parse::<NativeWindow>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The handle survives the round trip through a command line, which is the
    /// only thing the argument has to do. A handle that parsed as something else
    /// would reparent into whatever window happened to have that id.
    #[test]
    fn a_handle_survives_being_written_as_an_argument() {
        let handle: NativeWindow = 0x0012_3456;
        let rendered = handle.to_string();
        assert_eq!(rendered.parse::<NativeWindow>().ok(), Some(handle));
    }

    /// No `--embed` is the common case: every game launched in its own window,
    /// and every editor process.
    #[test]
    fn no_embed_argument_reads_as_not_embedded() {
        // The test binary's own arguments, which do not include `--embed`.
        assert!(embed_target_from_args().is_none());
    }

    /// The handshake has to survive the round trip through a pipe, which adds a
    /// newline and may add carriage returns on Windows.
    #[test]
    fn the_handshake_round_trips_through_a_pipe() {
        let child: NativeWindow = 0x00BE_EF01;
        let line = handshake_line(child);
        assert_eq!(handshake_handle(&line), Some(child));
        assert_eq!(handshake_handle(&format!("{line}\r\n")), Some(child));
    }

    /// An ordinary log line must not be mistaken for a handshake and swallowed:
    /// the reader hides whatever this matches, so a false positive is a log line
    /// the user never sees.
    #[test]
    fn an_ordinary_log_line_is_not_a_handshake() {
        assert!(handshake_handle("INFO the game started").is_none());
        assert!(handshake_handle("[renzora-embed] attached").is_none());
        assert!(handshake_handle("").is_none());
    }
}

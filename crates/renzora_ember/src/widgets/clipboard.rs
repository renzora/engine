//! The system clipboard, in one place.
//!
//! Three widgets need it — the code editor, the text input, and the console's
//! Copy button — and each had grown its own private pair of these two functions.
//! They were identical, which is the usual reason a fourth copy gets written
//! instead of a call.
//!
//! # Platform
//!
//! Native only. `arboard` has no wasm backend, and the browser clipboard is
//! async and gated behind a user gesture, so a synchronous read cannot be made
//! to work there. On wasm both calls compile and do nothing: copy silently
//! fails and paste yields `None`, which is what the editor already did.
//!
//! The same no-op path serves a build without the `editor_tools` feature — a
//! lean export, where the widgets that want a clipboard (code editor, console
//! Copy) are not on screen and `arboard` was pure weight.

/// Put `s` on the clipboard. Silently does nothing if the clipboard is
/// unavailable — there is no useful recovery, and a copy that fails is not worth
/// interrupting anyone over.
#[cfg(all(feature = "editor_tools", not(target_arch = "wasm32")))]
pub fn set_text(s: &str) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text(s.to_string());
    }
}

/// The clipboard's current text, if there is any and it can be read.
#[cfg(all(feature = "editor_tools", not(target_arch = "wasm32")))]
pub fn get_text() -> Option<String> {
    arboard::Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok())
}

#[cfg(not(all(feature = "editor_tools", not(target_arch = "wasm32"))))]
pub fn set_text(_s: &str) {}

#[cfg(not(all(feature = "editor_tools", not(target_arch = "wasm32"))))]
pub fn get_text() -> Option<String> {
    None
}

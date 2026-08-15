//! The browser's filesystem, behind an API the editor can call.
//!
//! A web build has no `std::fs` worth the name, but it is not sandboxed away
//! from the user's disk either: the **File System Access API** hands back a
//! handle to any directory the user picks, with real read/write. That is what
//! lets one project folder — `C:\Users\me\Documents\demo22` — be opened by the
//! desktop editor and the web editor alike, with no copy and no import step.
//!
//! ## What the browser gives you, and what it does not
//!
//! - **A handle, never a path.** `.name` is `demo22`; the absolute path is not
//!   exposed, by design. Web-side code therefore addresses files as paths
//!   *relative to the handle*.
//! - **Only after a gesture.** `showDirectoryPicker()` must be called from a
//!   click. The editor cannot reopen a project from a config value on startup;
//!   it needs one deliberate pick. Handles are serializable into IndexedDB, so
//!   after that a "recent project" costs a single permission prompt rather than
//!   a fresh trip through the file dialog.
//! - **Asynchronously.** Every read and write is a `Promise`. The synchronous
//!   file API (`createSyncAccessHandle`) exists but is confined to Workers *and*
//!   to the origin-private filesystem, so it cannot touch a picked folder. This
//!   is the single fact that shapes everything built on top of this module.
//! - **In Chromium.** Firefox and Safari do not implement the directory picker.
//!
//! ## Scope right now
//!
//! First step only: open the picker, and enumerate what is in the chosen
//! directory. Enough to prove the handle and the permission flow behave as
//! documented before a VFS is built on them. The handle is kept afterwards
//! ([`with_handle`]) so the next step can read from it.
//!
//! The eventual split, for whoever picks this up: the editor's own `std::fs`
//! callers (project.toml, scenes, scripts — all small) get an in-memory VFS
//! filled at open and flushed on save, while large assets go through a custom
//! Bevy `AssetSource`, whose reader is already async and so needs no shim.

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::{
    create_dir_all, has_project, invalidate_dir, list_dir, pick_directory, read_bytes, read_text,
    read_text_cached, reopen_project, spawn_create_dir, spawn_write_text, take_picked_project,
    to_relative, with_handle, write_text, DirEntry, PickedProject,
};

/// Desktop stubs.
///
/// Present so call sites read the same on both targets — the desktop editor
/// uses `rfd` and never calls these, but a shared code path that *might* should
/// not need a `#[cfg]` to compile.
#[cfg(not(target_arch = "wasm32"))]
mod native_stub {
    /// No-op: desktop opens a native dialog through `rfd` instead.
    pub fn pick_directory(_allow_new: bool) {}

    /// Always `None`: there is no browser handle off the web.
    pub fn with_handle<R>(_f: impl FnOnce(&()) -> R) -> Option<R> {
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native_stub::{pick_directory, with_handle};

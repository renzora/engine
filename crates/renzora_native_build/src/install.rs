//! Where the engine is installed — which is not always where the executable is.
//!
//! Everything a plugin build reads or writes hangs off one directory: `sdk/`,
//! `sdk.tar.zst`, and `plugins/<name>/`. Finding it looks like
//! `current_exe().parent()`, and on Windows and macOS that is exactly right.
//!
//! # The AppImage case, which it gets wrong
//!
//! A Linux release ships a single `.AppImage` with `sdk.tar.zst` **beside** it in
//! the zip, because the SDK cannot live inside: an AppImage is a read-only
//! squashfs, so a tree that has to be unpacked at first launch could not be
//! written there even if it fitted.
//!
//! At run time the AppImage mounts itself and executes the binary from inside
//! that mount, so `current_exe()` returns something like
//! `/tmp/.mount_Renzoraxxxxx/usr/bin/renzora-editor`. Its parent is a temporary,
//! read-only directory that has no `sdk.tar.zst` next to it and could not accept
//! one. The archive would never be found, setup would never run, and the failure
//! would read as "this build shipped without an SDK" — pointing nowhere near the
//! actual cause.
//!
//! The AppImage runtime sets `APPIMAGE` to the absolute path of the `.AppImage`
//! file itself, which is what makes this recoverable: its parent directory is the
//! one the user unzipped, and the one the archive is in.
//!
//! # macOS
//!
//! No special case needed. The executable sits in
//! `Renzora Engine.app/Contents/MacOS/`, and so do the shared dylibs and
//! `plugins/` — the bundle is an ordinary writable directory, so the SDK belongs
//! there with them and `current_exe().parent()` finds it.

use std::path::{Path, PathBuf};

/// The directory holding `sdk/`, `sdk.tar.zst` and `plugins/`.
///
/// Prefer this over `current_exe().parent()` anywhere the answer is used to find
/// engine data rather than the binary itself.
pub fn root() -> Option<PathBuf> {
    // `APPIMAGE` is set by the AppImage runtime to the archive's own path. Only
    // trust it when it actually points at a file: it is an ordinary environment
    // variable and a stale one inherited from a parent process would otherwise
    // send the whole SDK lookup somewhere arbitrary.
    if let Some(dir) = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        return Some(dir);
    }
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
}

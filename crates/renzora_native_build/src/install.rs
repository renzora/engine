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
//! # macOS, where reading and writing part company
//!
//! `root()` needs no special case: the executable sits in
//! `Renzora Engine.app/Contents/MacOS/` alongside the shared dylibs,
//! `plugins/` and `sdk.tar.zst`, and `current_exe().parent()` finds all of it.
//! There is no `$APPIMAGE` equivalent to recover the directory *outside* the
//! bundle, so everything the editor reads has to be inside it — anything the
//! packaging leaves beside the `.app` is somewhere this function never looks.
//! `sdk.tar.zst` shipped there from the first native macOS build until it was
//! found: `sdk_state` returned `Absent`, first-launch setup never ran, and the
//! editor could build neither a Rust script nor a native plugin, with no error
//! naming the cause because "no SDK in this build" is a legitimate state.
//!
//! **Writing is the opposite.** A `.app` is signed, and its signature seals
//! `Contents/`. Unpacking 1.9 GB of SDK into `Contents/MacOS/sdk/` — which is
//! what every other platform does — invalidates that seal, and so does deleting
//! the archive afterwards. The bundle keeps running, because the kernel enforces
//! the main executable's own signature and Gatekeeper has already assessed the
//! download by then, but `codesign --verify` fails on any copy that has been
//! launched once, and a re-quarantined copy (re-downloaded, moved between
//! machines, shared) is then refused outright.
//!
//! So on macOS the two directions are split: the archive is read from inside the
//! bundle and the tree is written to [`sdk_dir`] under Application Support. The
//! bundle is never modified, which is what a `.app` is supposed to be, and the
//! archive is never deleted — it costs 457 MB inside the app and buys the
//! ability to rebuild the tree if a user clears their Application Support.
//!
//! Three places keep the read side working, and all must stay in step with
//! `root()`: `xtask`'s `bundle::macos::wrap` (which moves `resources/` in), the
//! `Pack the plugin SDK` step in `.github/workflows/build-engine.yml` (which
//! writes the archive into `Contents/MacOS/` before the bundle is signed), and
//! `scripts/package-release.sh`, which fails the release if it finds an SDK
//! beside a `.app` instead of inside it.

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

/// Where the unpacked SDK tree lives for an install rooted at `root`.
///
/// `<root>/sdk` everywhere except inside a macOS `.app`, where the tree cannot
/// be written without breaking the bundle's signature (see the module docs) and
/// moves to `~/Library/Application Support/renzora/sdk` instead.
///
/// Takes `root` rather than calling [`root`] so that a caller which already has
/// one does not resolve it twice, and so this is testable without a real
/// install. The archive it is unpacked *from* stays at `<root>/sdk.tar.zst` on
/// every platform — only the destination moves.
pub fn sdk_dir(root: &Path) -> PathBuf {
    if cfg!(target_os = "macos") && in_app_bundle(root) {
        if let Some(dir) = data_dir() {
            return dir.join("sdk");
        }
    }
    root.join("sdk")
}

/// Is `root` the `Contents/MacOS` directory of a macOS application bundle?
///
/// Pure path shape, deliberately: a flat `dist/macos-arm64/` tree from
/// `cargo renzora dist` (no `--bundle`) is not a bundle and keeps the ordinary
/// `<root>/sdk` layout, which is what makes a contributor's build behave like
/// every other platform's. Same for an exported game, which ships flat.
///
/// Not `cfg`-gated so it can be tested from any host; [`sdk_dir`] is what
/// applies the platform rule.
fn in_app_bundle(root: &Path) -> bool {
    if root.file_name().is_none_or(|n| n != "MacOS") {
        return false;
    }
    let Some(contents) = root.parent() else { return false };
    if contents.file_name().is_none_or(|n| n != "Contents") {
        return false;
    }
    contents
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".app"))
}

/// `~/Library/Application Support/renzora`, the macOS home for engine data that
/// is regenerable but too large to live in the bundle.
///
/// Apple's location for exactly this: not a cache (losing it costs a two-second
/// unpack, but the user did not ask for that), not a preference, and findable by
/// someone hunting for the 1.9 GB an editor put on their disk.
///
/// Spelled out by hand rather than taken from `dirs::data_local_dir()`, which
/// returns this same path on macOS: this crate carries no dependencies at all
/// and must not start (see its `Cargo.toml` header — `xtask` depends on it
/// precisely because it adds nothing to xtask's build). The lowercase `renzora`
/// is not a typo, it matches what `renzora_marketplace` already writes there
/// through `dirs`, so the engine keeps one directory rather than two.
///
/// Small state — settings, layout, crash reports — stays in `~/.renzora/` on
/// every platform. The split is by size, not by kind: a dotfile directory is
/// the wrong place to drop gigabytes on macOS.
pub fn data_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(home.join("Library").join("Application Support").join("renzora"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_a_bundle() {
        assert!(in_app_bundle(Path::new("/A/Renzora Engine.app/Contents/MacOS")));
        assert!(in_app_bundle(Path::new("/A/Anything.app/Contents/MacOS")));
    }

    #[test]
    fn rejects_everything_else() {
        // A flat staged tree, which is what a contributor builds.
        assert!(!in_app_bundle(Path::new("/A/dist/macos-arm64")));
        // The bundle itself, and the level above the executables.
        assert!(!in_app_bundle(Path::new("/A/Renzora Engine.app")));
        assert!(!in_app_bundle(Path::new("/A/Renzora Engine.app/Contents")));
        // Right leaf names, no `.app` — a source tree that happens to match.
        assert!(!in_app_bundle(Path::new("/A/Renzora/Contents/MacOS")));
        assert!(!in_app_bundle(Path::new("/")));
    }

    /// The rule that keeps dev builds, exported games and this crate's own
    /// tests on the ordinary layout, on every host including macOS.
    #[test]
    fn flat_trees_keep_their_own_sdk() {
        let flat = Path::new("/A/dist/macos-arm64");
        assert_eq!(sdk_dir(flat), flat.join("sdk"));
    }
}

//! The game's archive, as plugins see it.
//!
//! An exported game carries its project in an `.rpak`, beside the executable
//! or appended to it. The engine reads assets from it through the VFS, which
//! lives in the runtime crate a plugin does not link. A plugin that shipped
//! files of its own in the archive (see `plugin_export`) reads them back here:
//! bytes, not text, and a listing by prefix, since a plugin's files are often
//! a tree (a compiled build, a runtime) it does not know file by file.
//!
//! A static the runtime installs when it opens the archive, rather than a
//! resource, so a plugin's worker threads can read without a world. It lives
//! in this crate, inside the shared `renzora_dylib` image, so the runtime and
//! every plugin see one archive. In the editor, or a game run from a project
//! folder, nothing is installed and every read is `None`.

use std::sync::{Arc, RwLock};

/// What the runtime hands over: the archive's reads.
pub trait GameArchive: Send + Sync {
    /// The file at `path` (forward slashes, relative to the project root).
    fn read(&self, path: &str) -> Option<Vec<u8>>;
    /// Every file whose path starts with `prefix`.
    fn paths_under(&self, prefix: &str) -> Vec<String>;
}

static ARCHIVE: RwLock<Option<Arc<dyn GameArchive>>> = RwLock::new(None);

/// Called by the runtime when it opens the game's archive.
pub fn install_game_archive(archive: Arc<dyn GameArchive>) {
    if let Ok(mut slot) = ARCHIVE.write() {
        *slot = Some(archive);
    }
}

/// Whether this process is a game running from an archive.
pub fn has_game_archive() -> bool {
    ARCHIVE.read().is_ok_and(|a| a.is_some())
}

/// A file from the game's archive; `None` outside a game or for a path it
/// does not hold.
pub fn game_archive_read(path: &str) -> Option<Vec<u8>> {
    let archive = ARCHIVE.read().ok()?.clone()?;
    archive.read(path)
}

/// Every path in the game's archive under `prefix`, sorted; empty outside a
/// game.
pub fn game_archive_paths(prefix: &str) -> Vec<String> {
    let Some(archive) = ARCHIVE.read().ok().and_then(|a| a.clone()) else {
        return Vec::new();
    };
    let mut paths = archive.paths_under(prefix);
    paths.sort();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;
    impl GameArchive for Fake {
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            (path == "plugins/x/a.bin").then(|| vec![1, 2, 3])
        }
        fn paths_under(&self, prefix: &str) -> Vec<String> {
            ["plugins/x/b.bin", "plugins/x/a.bin", "scenes/main.bsn"]
                .into_iter()
                .filter(|p| p.starts_with(prefix))
                .map(str::to_string)
                .collect()
        }
    }

    #[test]
    fn a_plugin_reads_and_lists_what_the_runtime_installed() {
        install_game_archive(Arc::new(Fake));
        assert!(has_game_archive());
        assert_eq!(game_archive_read("plugins/x/a.bin"), Some(vec![1, 2, 3]));
        assert_eq!(game_archive_read("missing"), None);
        assert_eq!(game_archive_paths("plugins/x/"), vec!["plugins/x/a.bin", "plugins/x/b.bin"]);
    }
}

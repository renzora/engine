//! Pluggable asset-byte loader.
//!
//! Kira loads sounds from the real filesystem (`from_file`), but the engine
//! serves project assets through a virtual filesystem that may be backed by an
//! `.rpak` archive (exported games) rather than loose files on disk. To avoid a
//! circular dependency (`renzora_audio` can't depend on `renzora_engine`), the
//! host installs a loader closure here; the audio systems call `load_asset_bytes`
//! to pull a clip's bytes from wherever they actually live.

use std::sync::{Mutex, OnceLock};

type LoaderFn = dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync;

static LOADER: OnceLock<Mutex<Option<Box<LoaderFn>>>> = OnceLock::new();

fn cell() -> &'static Mutex<Option<Box<LoaderFn>>> {
    LOADER.get_or_init(|| Mutex::new(None))
}

/// Install the project/VFS-aware byte loader. Called by the host engine once the
/// virtual filesystem and project root are known. Takes a project-relative key
/// (e.g. `"audio/music.ogg"`) and returns the file's bytes, or `None`.
pub fn set_asset_byte_loader(f: Box<LoaderFn>) {
    *cell().lock().unwrap() = Some(f);
}

/// Load raw bytes for a project-relative asset key via the installed loader.
/// Returns `None` if no loader is installed or the asset can't be found — the
/// caller should then fall back to a direct filesystem read.
pub fn load_asset_bytes(relative: &str) -> Option<Vec<u8>> {
    cell().lock().unwrap().as_ref().and_then(|f| f(relative))
}

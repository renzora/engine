//! The project/VFS-aware byte loader, for assets Bevy's `AssetServer` never sees.

/// Pluggable loader that reads raw bytes for a project-relative asset key
/// (e.g. `"particles/fire.particle"`). The host engine installs it once with a
/// virtual-filesystem-aware closure (rpak archive in exported games, loose
/// files on disk in the editor). Crates that load assets *outside* Bevy's
/// AssetServer — audio (Kira) and particle effects — go through this so they
/// work in exported `.rpak` builds, not just on-disk projects.
type AssetByteLoader = dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync;

static ASSET_BYTE_LOADER: std::sync::OnceLock<std::sync::Mutex<Option<Box<AssetByteLoader>>>> =
    std::sync::OnceLock::new();

fn asset_byte_loader_cell() -> &'static std::sync::Mutex<Option<Box<AssetByteLoader>>> {
    ASSET_BYTE_LOADER.get_or_init(|| std::sync::Mutex::new(None))
}

/// Install the project/VFS-aware byte loader. Called by the host engine once
/// the virtual filesystem and project root are known.
pub fn set_asset_byte_loader(f: Box<AssetByteLoader>) {
    if let Ok(mut guard) = asset_byte_loader_cell().lock() {
        *guard = Some(f);
    }
}

/// Load raw bytes for a project-relative asset key via the installed loader.
/// Returns `None` if no loader is installed or the asset can't be found.
pub fn load_asset_bytes(relative: &str) -> Option<Vec<u8>> {
    asset_byte_loader_cell()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|f| f(relative)))
        .flatten()
}

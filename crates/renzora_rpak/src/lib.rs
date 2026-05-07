//! Renzora `.rpak` asset archive format.
//!
//! An `.rpak` is an indexed archive of project assets with **per-entry**
//! compression. The whole archive is not zstd'd as a single blob — each
//! entry decompresses independently on access, so reading one file does
//! not require reading the whole archive into memory.
//!
//! See [`format`] for the on-disk layout. Quick summary:
//!
//! - 32-byte header at the start (magic, version, pointer to index)
//! - data section: concatenated per-entry payloads
//! - index section at the end: enumerates every entry's location, size, and
//!   compression scheme
//! - optional 16-byte footer (only when appended to a binary) with the rpak's
//!   total size + magic, so the loader can locate the rpak start
//!
//! The reader keeps the whole rpak's compressed bytes in memory and
//! decompresses individual entries on demand. A future revision will swap
//! the byte storage for a `PakBackend` trait (mmap, file handle, Android
//! `AAssetManager`, in-memory) without changing the format.

pub mod backend;
pub mod format;
mod read;

pub use backend::{BytesBackend, PakBackend};
#[cfg(not(target_arch = "wasm32"))]
pub use backend::{FileBackend, MmapBackend};

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        mod pack;
        pub use pack::{RpakPacker, pack_project, pack_project_with_progress, pack_project_filtered, SERVER_EXTENSIONS};
    }
}

pub use format::{Compression, PakEntry, FORMAT_VERSION};
pub use read::RpakArchive;

/// Magic bytes that appear at the start of every rpak header **and** at the
/// end of an appended-to-binary footer. Kept public for binary-detection
/// callers outside the crate.
pub const RPAK_MAGIC: &[u8; 4] = format::RPAK_MAGIC;

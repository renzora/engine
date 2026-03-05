//! Renzora `.rpak` asset archive format.
//!
//! An `.rpak` file is a zstd-compressed archive containing project assets.
//!
//! ## Layout
//!
//! ```text
//! [entry_count: u32]
//! for each entry:
//!   [path_len: u32] [path: utf8] [offset: u64] [size: u64]
//! [raw file data concatenated]
//! ```
//!
//! The entire blob above is zstd-compressed and written to disk.
//!
//! When appended to a binary (self-contained mode), a footer is added:
//!
//! ```text
//! [runtime binary bytes]
//! [zstd-compressed rpak data]
//! [rpak_data_size: u64 LE]
//! [magic: b"RPAK"]
//! ```

mod pack;
mod read;

pub use pack::{RpakPacker, pack_directory};
pub use read::RpakArchive;

/// Magic bytes at the end of a self-contained binary.
pub const RPAK_MAGIC: &[u8; 4] = b"RPAK";

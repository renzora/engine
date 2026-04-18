//! USDC file header parsing.

use crate::{UsdError, UsdResult};

/// The 8-byte magic identifying a USDC file.
const MAGIC: &[u8; 8] = b"PXR-USDC";

/// Parsed USDC header.
#[derive(Debug)]
pub struct Header {
    /// File format version (major.minor.patch encoded).
    pub version: [u8; 3],
    /// Offset to the table of contents.
    pub toc_offset: u64,
}

impl Header {
    /// Minimum header size: 8 (magic) + 3 (version) + 5 (padding) + 8 (toc offset) = 24.
    /// Actual header is 88 bytes in most versions but we only read what we need.
    const MIN_SIZE: usize = 24;

    pub fn read(data: &[u8]) -> UsdResult<Self> {
        if data.len() < Self::MIN_SIZE {
            return Err(UsdError::Parse("File too small for USDC header".into()));
        }

        if &data[0..8] != MAGIC {
            return Err(UsdError::Parse("Not a USDC file (bad magic)".into()));
        }

        let version = [data[8], data[9], data[10]];

        // TOC offset is at byte 16 in all known versions
        let toc_offset = u64::from_le_bytes(data[16..24].try_into().unwrap());

        if toc_offset as usize >= data.len() {
            return Err(UsdError::Parse("TOC offset beyond end of file".into()));
        }

        log::debug!(
            "USDC version {}.{}.{}, TOC at offset {}",
            version[0],
            version[1],
            version[2],
            toc_offset
        );

        Ok(Header {
            version,
            toc_offset,
        })
    }

    /// Check if this version supports LZ4 compression (>= 0.4.0).
    pub fn has_lz4(&self) -> bool {
        self.version[0] > 0 || self.version[1] >= 4
    }
}

//! On-disk format for `.rpak` v2.
//!
//! ## Layout
//!
//! ```text
//! [ Header — 32 bytes, fixed ]
//!   magic              "RPAK"        4 bytes
//!   version            u32           4 bytes  (= 2)
//!   flags              u32           4 bytes  (bit 0 = index zstd-compressed)
//!   index_offset       u64           8 bytes  (relative to start-of-rpak)
//!   index_compressed   u32           4 bytes  (size of index as stored)
//!   index_uncompressed u32           4 bytes  (size after decompression)
//!
//! [ Data section — variable, starts at offset 32 ]
//!   Concatenated entry payloads, each independently `Stored` or `Zstd`.
//!
//! [ Index section — at index_offset, optionally zstd-compressed ]
//!   count u32
//!   per entry:
//!     path_len            u32
//!     path                utf8
//!     offset              u64    (relative to start-of-rpak)
//!     compressed_size     u64
//!     uncompressed_size   u64
//!     compression         u8     (0=Stored, 1=Zstd)
//!     entry_flags         u8     (reserved)
//!     padding             u16    (=0)
//!     crc32               u32    (0 = not computed)
//!
//! [ Footer — present only when appended to a binary, 16 bytes ]
//!   rpak_total_size      u64    (size of [Header..end-of-Index])
//!   magic                "RPAK" 4 bytes
//!   reserved             u32
//! ```
//!
//! ## Detection
//!
//! A file is read in this order:
//! 1. Last 16 bytes: if bytes\[8..12\] == "RPAK", the file is appended-to-binary.
//!    Compute `rpak_start = file_size - 16 - rpak_total_size` and read the header there.
//! 2. First 8 bytes: if \[0..4\] == "RPAK" and \[4..8\] as u32 == 2, it's a standalone
//!    rpak with `rpak_start = 0`.
//! 3. Otherwise: not an rpak.

use std::io;

/// Format version. Bump on incompatible layout changes.
pub const FORMAT_VERSION: u32 = 2;

/// Header magic, present at start-of-rpak in every file.
pub const RPAK_MAGIC: &[u8; 4] = b"RPAK";

/// Fixed header length, in bytes.
pub const HEADER_LEN: u64 = 32;

/// Footer length when appended to a binary, in bytes.
pub const FOOTER_LEN: u64 = 16;

/// Per-entry compression scheme.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compression {
    /// Raw bytes — `compressed_size == uncompressed_size`. Read returns a slice.
    Stored = 0,
    /// Zstd-compressed. Read decompresses into a fresh buffer.
    Zstd = 1,
}

impl Compression {
    pub fn from_u8(v: u8) -> io::Result<Self> {
        match v {
            0 => Ok(Self::Stored),
            1 => Ok(Self::Zstd),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown compression scheme: {}", other),
            )),
        }
    }
}

/// Header flag: index is zstd-compressed.
pub const HEADER_FLAG_INDEX_COMPRESSED: u32 = 0x01;

/// One entry as it appears in the index.
///
/// Offsets are relative to the start of the rpak (i.e. offset 0 = the magic
/// `RPAK` at the start of the header). For appended-to-binary archives the
/// reader rebases by `rpak_start` before issuing actual reads.
#[derive(Clone, Debug)]
pub struct PakEntry {
    pub path: String,
    pub offset: u64,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub compression: Compression,
    pub flags: u8,
    pub crc32: u32,
}

/// Decoded header.
#[derive(Clone, Debug)]
pub struct Header {
    pub version: u32,
    pub flags: u32,
    pub index_offset: u64,
    pub index_compressed: u32,
    pub index_uncompressed: u32,
}

impl Header {
    pub fn index_is_compressed(&self) -> bool {
        self.flags & HEADER_FLAG_INDEX_COMPRESSED != 0
    }

    pub fn write_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(RPAK_MAGIC);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&self.flags.to_le_bytes());
        out.extend_from_slice(&self.index_offset.to_le_bytes());
        out.extend_from_slice(&self.index_compressed.to_le_bytes());
        out.extend_from_slice(&self.index_uncompressed.to_le_bytes());
    }

    pub fn parse(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < HEADER_LEN as usize {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "header truncated",
            ));
        }
        if &bytes[0..4] != RPAK_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "missing RPAK magic at header start",
            ));
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != FORMAT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsupported rpak version: {} (expected {})",
                    version, FORMAT_VERSION
                ),
            ));
        }
        let flags = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let index_offset = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
        let index_compressed = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let index_uncompressed = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        // bytes[28..32] reserved
        Ok(Self {
            version,
            flags,
            index_offset,
            index_compressed,
            index_uncompressed,
        })
    }
}

/// Encode the index as a sequence of entries (uncompressed bytes).
///
/// The caller decides whether to zstd-compress the result before writing
/// (and sets `HEADER_FLAG_INDEX_COMPRESSED` accordingly).
pub fn encode_index(entries: &[PakEntry]) -> Vec<u8> {
    let mut out = Vec::with_capacity(entries.len() * 64);
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for e in entries {
        let path_bytes = e.path.as_bytes();
        out.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(path_bytes);
        out.extend_from_slice(&e.offset.to_le_bytes());
        out.extend_from_slice(&e.compressed_size.to_le_bytes());
        out.extend_from_slice(&e.uncompressed_size.to_le_bytes());
        out.push(e.compression as u8);
        out.push(e.flags);
        out.extend_from_slice(&[0u8; 2]); // padding
        out.extend_from_slice(&e.crc32.to_le_bytes());
    }
    out
}

/// Decode the index from its uncompressed byte representation.
pub fn decode_index(raw: &[u8]) -> io::Result<Vec<PakEntry>> {
    let mut pos = 0usize;
    if raw.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated index header",
        ));
    }
    let count = u32::from_le_bytes(raw[0..4].try_into().unwrap()) as usize;
    pos += 4;

    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        if pos + 4 > raw.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated index entry path_len",
            ));
        }
        let path_len = u32::from_le_bytes(raw[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;

        if pos + path_len > raw.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated index entry path",
            ));
        }
        let path = std::str::from_utf8(&raw[pos..pos + path_len])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
            .to_string();
        pos += path_len;

        // offset(8) + compressed(8) + uncompressed(8) + compression(1) + flags(1) + pad(2) + crc(4) = 32
        if pos + 32 > raw.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated index entry payload",
            ));
        }
        let offset = u64::from_le_bytes(raw[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let compressed_size = u64::from_le_bytes(raw[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let uncompressed_size = u64::from_le_bytes(raw[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let compression = Compression::from_u8(raw[pos])?;
        pos += 1;
        let flags = raw[pos];
        pos += 1;
        // padding[2]
        pos += 2;
        let crc32 = u32::from_le_bytes(raw[pos..pos + 4].try_into().unwrap());
        pos += 4;

        entries.push(PakEntry {
            path,
            offset,
            compressed_size,
            uncompressed_size,
            compression,
            flags,
            crc32,
        });
    }

    Ok(entries)
}

/// Build the 16-byte appended-to-binary footer for a rpak whose total
/// (header through index) is `rpak_total_size` bytes.
pub fn encode_footer(rpak_total_size: u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[0..8].copy_from_slice(&rpak_total_size.to_le_bytes());
    out[8..12].copy_from_slice(RPAK_MAGIC);
    // bytes 12..16 reserved (=0)
    out
}

/// Try to detect an appended-to-binary rpak by looking at the last 16 bytes.
/// Returns `Some((rpak_start, rpak_total_size))` if found.
pub fn detect_appended_footer(file_size: u64, last_16: &[u8; 16]) -> Option<(u64, u64)> {
    if &last_16[8..12] != RPAK_MAGIC {
        return None;
    }
    let total = u64::from_le_bytes(last_16[0..8].try_into().unwrap());
    if total < HEADER_LEN || total + FOOTER_LEN > file_size {
        return None;
    }
    let start = file_size - FOOTER_LEN - total;
    Some((start, total))
}

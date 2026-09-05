#![allow(dead_code)] // USD Crate format reader — partial implementation, helpers staged.

//! USDC token table parser.
//!
//! The TOKENS section contains all unique strings used in the file
//! (prim names, property names, type names, etc.). In USDC >= 0.4.0
//! this section is LZ4-compressed.

use super::super::{UsdError, UsdResult};
use super::sections::{TableOfContents, SECTION_TOKENS};

/// Read and decompress the token table.
pub fn read_tokens(data: &[u8], toc: &TableOfContents) -> UsdResult<Vec<String>> {
    let section = match toc.find(SECTION_TOKENS) {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    // `offset + size`, both out of the file. See `sections::section_data` for
    // what writing that as a bare `+` used to do.
    let sec_start = section.offset as usize;
    let sec_data = super::read::slice(data, sec_start, section.size as usize)
        .ok_or_else(|| UsdError::Parse("TOKENS section extends beyond file".into()))?;

    if sec_data.len() < 8 {
        return Ok(Vec::new());
    }

    // First 8 bytes: token count
    let token_count = super::read::le_u64(sec_data, 0).unwrap_or(0) as usize;

    if token_count == 0 {
        return Ok(Vec::new());
    }

    let remaining = &sec_data[8..];

    // Try to determine if this is compressed or uncompressed.
    // In compressed format (>= 0.4.0): next 8 bytes are uncompressed size,
    // then 8 bytes compressed size, then compressed data.
    // In uncompressed format: raw null-terminated strings.

    // Heuristic: if the next 8+8 bytes give plausible sizes, treat as compressed
    let token_bytes = if remaining.len() >= 16 {
        let uncompressed_size = super::read::le_u64(remaining, 0).unwrap_or(0) as usize;
        let compressed_size = super::read::le_u64(remaining, 8).unwrap_or(0) as usize;

        if compressed_size > 0
            && compressed_size < remaining.len()
            && uncompressed_size > 0
            && uncompressed_size < 100_000_000
            && 16 + compressed_size <= remaining.len()
        {
            // Compressed
            let compressed = &remaining[16..16 + compressed_size];
            match super::compression::decompress_lz4(compressed, uncompressed_size) {
                Ok(decompressed) => decompressed,
                Err(_) => {
                    // Fall back to treating as uncompressed
                    remaining.to_vec()
                }
            }
        } else {
            remaining.to_vec()
        }
    } else {
        remaining.to_vec()
    };

    // Parse null-terminated strings
    let mut tokens = Vec::with_capacity(token_count);
    let mut start = 0;
    for _ in 0..token_count {
        let end = token_bytes[start..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| start + p)
            .unwrap_or(token_bytes.len());

        let s = std::str::from_utf8(&token_bytes[start..end])
            .unwrap_or("")
            .to_string();
        tokens.push(s);
        start = end + 1;

        if start > token_bytes.len() {
            break;
        }
    }

    log::debug!("Read {} tokens", tokens.len());
    Ok(tokens)
}

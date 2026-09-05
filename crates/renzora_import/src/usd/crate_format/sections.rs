#![allow(dead_code)] // USD Crate format reader — partial implementation, helpers staged.

//! USDC table of contents and section parsing.

use super::super::{UsdError, UsdResult};
use super::header::Header;

/// Known section names in a USDC file.
pub const SECTION_TOKENS: &str = "TOKENS";
pub const SECTION_STRINGS: &str = "STRINGS";
pub const SECTION_FIELDS: &str = "FIELDS";
pub const SECTION_FIELDSETS: &str = "FIELDSETS";
pub const SECTION_PATHS: &str = "PATHS";
pub const SECTION_SPECS: &str = "SPECS";

/// A section entry in the TOC.
#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub offset: u64,
    pub size: u64,
}

/// Parsed table of contents.
#[derive(Debug)]
pub struct TableOfContents {
    pub sections: Vec<Section>,
}

impl TableOfContents {
    pub fn read(data: &[u8], header: &Header) -> UsdResult<Self> {
        let offset = header.toc_offset as usize;

        // `offset` is a `u64` straight out of the file. The guard here used to
        // be `if offset + 8 > data.len()`, which a `toc_offset` of `u64::MAX`
        // walks through: the addition wraps (this profile has overflow-checks
        // off), 7 is not greater than any real length, and the slice index one
        // line down panicked. `le_u64` does the add with `checked_add`.
        let section_count = super::read::le_u64(data, offset)
            .ok_or_else(|| UsdError::Parse("TOC header truncated".into()))?;

        if section_count > 64 {
            return Err(UsdError::Parse(format!(
                "Unreasonable section count: {}",
                section_count
            )));
        }

        let mut sections = Vec::new();
        // Same wrap, one step along: `offset` survived its own check, but
        // `offset + 8` is still file-derived arithmetic.
        let mut pos = offset
            .checked_add(8)
            .ok_or_else(|| UsdError::Parse("TOC header truncated".into()))?;

        for _ in 0..section_count {
            // Each section entry: 16 bytes name (null-padded) + 8 bytes offset + 8 bytes size
            let entry = super::read::slice(data, pos, 32)
                .ok_or_else(|| UsdError::Parse("TOC entry truncated".into()))?;

            let name_bytes = &entry[..16];
            let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(16);
            let name = std::str::from_utf8(&name_bytes[..name_end])
                .unwrap_or("")
                .to_string();

            // Infallible now: `entry` is exactly 32 bytes, so both reads are in
            // bounds by construction rather than by a separate check.
            let sec_offset = super::read::le_u64(entry, 16).unwrap_or(0);
            let sec_size = super::read::le_u64(entry, 24).unwrap_or(0);

            log::debug!(
                "Section '{}': offset={}, size={}",
                name,
                sec_offset,
                sec_size
            );

            sections.push(Section {
                name,
                offset: sec_offset,
                size: sec_size,
            });

            pos += 32;
        }

        Ok(TableOfContents { sections })
    }

    /// Find a section by name.
    pub fn find(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// Get section data slice.
    pub fn section_data<'a>(&self, data: &'a [u8], name: &str) -> UsdResult<&'a [u8]> {
        let section = self
            .find(name)
            .ok_or_else(|| UsdError::Parse(format!("Missing section: {}", name)))?;
        let start = section.offset as usize;
        // Both halves come out of the TOC, so `start + size` is two hostile
        // numbers added together. It used to be written that way, and wrapping
        // it produced an `end` BELOW `start`: smaller than `data.len()`, so the
        // check below passed, and then `&data[start..end]` panicked with "slice
        // index starts at N but ends at M". `read::slice` adds with
        // `checked_add` and indexes with `get`, so both failures are the same
        // `None`.
        super::read::slice(data, start, section.size as usize).ok_or_else(|| {
            UsdError::Parse(format!(
                "Section '{}' extends beyond file (offset={}, size={}, file_len={})",
                name,
                start,
                section.size,
                data.len()
            ))
        })
    }
}

/// Read the STRINGS section -- an array of u32 indices into the token table.
pub fn read_string_indices(data: &[u8], toc: &TableOfContents) -> UsdResult<Vec<u32>> {
    let section = match toc.find(SECTION_STRINGS) {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let start = section.offset as usize;
    let count = section.size as usize / 4;
    let mut indices = Vec::with_capacity(count);

    for i in 0..count {
        // `start` is file-derived and `count` is derived from a file-derived
        // size, so both halves of this can wrap. A `None` from either means the
        // table runs past the end, which is the same "stop here" this loop
        // already did for the truncated case.
        let Some(offset) = i.checked_mul(4).and_then(|d| start.checked_add(d)) else {
            break;
        };
        let Some(index) = super::read::le_u32(data, offset) else {
            break;
        };
        indices.push(index);
    }

    Ok(indices)
}

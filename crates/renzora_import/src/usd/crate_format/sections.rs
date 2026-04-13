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

        if offset + 8 > data.len() {
            return Err(UsdError::Parse("TOC header truncated".into()));
        }

        let section_count = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());

        if section_count > 64 {
            return Err(UsdError::Parse(format!(
                "Unreasonable section count: {}",
                section_count
            )));
        }

        let mut sections = Vec::new();
        let mut pos = offset + 8;

        for _ in 0..section_count {
            // Each section entry: 16 bytes name (null-padded) + 8 bytes offset + 8 bytes size
            if pos + 32 > data.len() {
                return Err(UsdError::Parse("TOC entry truncated".into()));
            }

            let name_bytes = &data[pos..pos + 16];
            let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(16);
            let name = std::str::from_utf8(&name_bytes[..name_end])
                .unwrap_or("")
                .to_string();

            let sec_offset =
                u64::from_le_bytes(data[pos + 16..pos + 24].try_into().unwrap());
            let sec_size =
                u64::from_le_bytes(data[pos + 24..pos + 32].try_into().unwrap());

            log::warn!(
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
        let section = self.find(name).ok_or_else(|| {
            UsdError::Parse(format!("Missing section: {}", name))
        })?;
        let start = section.offset as usize;
        let end = start + section.size as usize;
        if end > data.len() {
            return Err(UsdError::Parse(format!(
                "Section '{}' extends beyond file (offset={}, size={}, file_len={})",
                name, start, section.size, data.len()
            )));
        }
        Ok(&data[start..end])
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
        let offset = start + i * 4;
        if offset + 4 > data.len() {
            break;
        }
        indices.push(u32::from_le_bytes(
            data[offset..offset + 4].try_into().unwrap(),
        ));
    }

    Ok(indices)
}

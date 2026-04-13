//! USDC spec table parser.
//!
//! SPECS section:
//!   u64: numSpecs
//!   [u64 compSize][data]: integer-coded u32 path indices
//!   [u64 compSize][data]: integer-coded u32 fieldset indices
//!   [u64 compSize][data]: integer-coded u32 spec types

use super::super::UsdResult;
use super::compression;
use super::sections::{TableOfContents, SECTION_SPECS};

pub const SPEC_TYPE_PRIM: u32 = 1;
pub const SPEC_TYPE_ATTRIBUTE: u32 = 2;
pub const SPEC_TYPE_RELATIONSHIP: u32 = 4;

#[derive(Debug, Clone)]
pub struct Spec {
    pub path_index: u32,
    pub fieldset_index: u32,
    pub spec_type: u32,
}

pub fn read_specs(data: &[u8], toc: &TableOfContents) -> UsdResult<Vec<Spec>> {
    let section = match toc.find(SECTION_SPECS) {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let s = section.offset as usize;
    let e = s + section.size as usize;
    if e > data.len() { return Err(super::super::UsdError::Parse("SPECS truncated".into())); }
    let sd = &data[s..e];
    if sd.len() < 8 { return Ok(Vec::new()); }

    let num_specs = u64::from_le_bytes(sd[0..8].try_into().unwrap()) as usize;
    let mut pos = 8usize;
    if num_specs == 0 { return Ok(Vec::new()); }

    let path_indices = compression::read_compressed_ints_with_count(sd, &mut pos, num_specs)?;
    let fieldset_indices = compression::read_compressed_ints_with_count(sd, &mut pos, num_specs)?;
    let spec_types = compression::read_compressed_ints_with_count(sd, &mut pos, num_specs)?;

    let specs: Vec<Spec> = (0..num_specs)
        .map(|i| Spec {
            path_index: path_indices.get(i).copied().unwrap_or(0),
            fieldset_index: fieldset_indices.get(i).copied().unwrap_or(0),
            spec_type: spec_types.get(i).copied().unwrap_or(0),
        })
        .collect();

    log::debug!("Read {} specs", specs.len());
    Ok(specs)
}

#![allow(dead_code)] // USD Crate format reader — partial implementation, helpers staged.

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

// `SdfSpecType`, in the order USD declares it — these are ordinals, not flags,
// which is what the previous values (1/2/4) assumed. With prims mis-numbered as
// 1 the walker classified every prim as something else and reported "no meshes"
// on a file whose 2008 specs had all decoded correctly.
pub const SPEC_TYPE_UNKNOWN: u32 = 0;
pub const SPEC_TYPE_ATTRIBUTE: u32 = 1;
pub const SPEC_TYPE_CONNECTION: u32 = 2;
pub const SPEC_TYPE_EXPRESSION: u32 = 3;
pub const SPEC_TYPE_MAPPER: u32 = 4;
pub const SPEC_TYPE_MAPPER_ARG: u32 = 5;
pub const SPEC_TYPE_PRIM: u32 = 6;
pub const SPEC_TYPE_PSEUDO_ROOT: u32 = 7;
pub const SPEC_TYPE_RELATIONSHIP: u32 = 8;
pub const SPEC_TYPE_RELATIONSHIP_TARGET: u32 = 9;
pub const SPEC_TYPE_VARIANT: u32 = 10;
pub const SPEC_TYPE_VARIANT_SET: u32 = 11;

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

    // `offset + size`, both out of the file. See `sections::section_data` for
    // what writing that as a bare `+` used to do.
    let s = section.offset as usize;
    let sd = super::read::slice(data, s, section.size as usize)
        .ok_or_else(|| super::super::UsdError::Parse("SPECS truncated".into()))?;
    if sd.len() < 8 {
        return Ok(Vec::new());
    }

    let num_specs = super::read::le_u64(sd, 0).unwrap_or(0) as usize;
    let mut pos = 8usize;
    if num_specs == 0 {
        return Ok(Vec::new());
    }

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

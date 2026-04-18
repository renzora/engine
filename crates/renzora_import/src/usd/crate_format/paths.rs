#![allow(dead_code)] // USD Crate format reader — partial implementation, helpers staged.

//! USDC path table parser.
//!
//! PATHS section (v0.4.0+):
//!   u64: numPaths
//!   [u64 compSize][data]: integer-coded u32 pathIndexes
//!   [u64 compSize][data]: integer-coded i32 elementTokenIndexes
//!   [u64 compSize][data]: integer-coded i32 jumps
//!
//! Jump encoding:
//!   -2 = leaf (no child, no sibling)
//!   -1 = has child only (child is next entry)
//!    0 = has sibling only (sibling is next entry)
//!   >0 = has both child and sibling; child is next entry, sibling at thisIndex + jump

use super::super::{UsdError, UsdResult};
use super::compression;
use super::sections::{TableOfContents, SECTION_PATHS};

#[derive(Debug, Clone)]
pub struct PathEntry {
    pub name: String,
    pub parent_index: i32,
}

pub fn read_paths(
    data: &[u8],
    toc: &TableOfContents,
    tokens: &[String],
) -> UsdResult<Vec<PathEntry>> {
    let section = match toc.find(SECTION_PATHS) {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let s = section.offset as usize;
    let e = s + section.size as usize;
    if e > data.len() { return Err(UsdError::Parse("PATHS truncated".into())); }
    let sd = &data[s..e];
    if sd.len() < 8 { return Ok(Vec::new()); }

    let num_paths = u64::from_le_bytes(sd[0..8].try_into().unwrap()) as usize;
    let mut pos = 8usize;
    if num_paths == 0 { return Ok(Vec::new()); }

    let path_indexes = compression::read_compressed_ints_with_count(sd, &mut pos, num_paths)?;
    let elem_token_indexes = compression::read_compressed_signed_ints(sd, &mut pos, num_paths)?;
    let jumps = compression::read_compressed_signed_ints(sd, &mut pos, num_paths)?;

    // Reconstruct paths using the jump-encoded tree traversal
    let mut paths = vec![PathEntry { name: String::new(), parent_index: -1 }; num_paths];
    let mut parent_stack: Vec<i32> = vec![-1]; // stack of parent path indices

    let n = path_indexes.len().min(elem_token_indexes.len()).min(jumps.len());

    for i in 0..n {
        let path_idx = path_indexes[i] as usize;
        let token_idx = elem_token_indexes[i];
        let jump = jumps[i];

        if path_idx >= num_paths {
            continue;
        }

        // Negative token index means property path; use abs value
        let actual_token = token_idx.unsigned_abs() as usize;
        let name = if actual_token < tokens.len() {
            tokens[actual_token].clone()
        } else {
            format!("__path_{}", path_idx)
        };

        let parent_index = *parent_stack.last().unwrap_or(&-1);
        paths[path_idx] = PathEntry { name, parent_index };

        let has_child = jump == -1 || jump > 0;
        let has_sibling = jump >= 0;

        if has_child {
            parent_stack.push(path_idx as i32);
        }

        if !has_child && !has_sibling {
            // Leaf: pop parent stack
            parent_stack.pop();
        }
        // If has_sibling but no child: parent stays the same (next entry is sibling)
        // If has both: child is next, sibling handled by jump offset (we don't track that here
        // since we process entries sequentially and the sibling will be encountered later)
    }

    log::debug!("Read {} paths", paths.len());
    Ok(paths)
}

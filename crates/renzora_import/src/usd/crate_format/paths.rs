#![allow(dead_code)] // USD Crate format reader — partial implementation, helpers staged.

//! USDC path table parser.
//!
//! PATHS section (v0.4.0+):
//!   u64: numPaths
//!   u64: numPaths *again* — Pixar writes the count twice, once in
//!        `_ReadPaths` and once in `_ReadCompressedPaths`, which reads the
//!        section independently. Consuming only one leaves every following
//!        offset eight bytes short, and the first compressed block then
//!        decodes as garbage.
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
    /// True when this entry is a *property* of its parent prim (`/prim.attr`)
    /// rather than a child prim (`/prim/child`). USD signals it with a negative
    /// element-token index. Mesh geometry lives in properties, so telling the
    /// two apart is what lets a `Mesh` prim find its `points`.
    pub is_property: bool,
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

    // `offset + size`, both out of the file. See `sections::section_data` for
    // what writing that as a bare `+` used to do.
    let s = section.offset as usize;
    let sd = super::read::slice(data, s, section.size as usize)
        .ok_or_else(|| UsdError::Parse("PATHS truncated".into()))?;
    if sd.len() < 8 {
        return Ok(Vec::new());
    }

    let num_paths = super::read::le_u64(sd, 0).unwrap_or(0) as usize;
    let mut pos = 8usize;
    if num_paths == 0 {
        return Ok(Vec::new());
    }

    // The repeated count. Guarded rather than assumed: older writers that emit
    // it once would otherwise have their first compressed block skipped.
    if let Some(repeat) = super::read::le_u64(sd, 8) {
        if repeat as usize == num_paths {
            pos = 16;
        }
    }

    let path_indexes = compression::read_compressed_ints_with_count(sd, &mut pos, num_paths)?;
    let elem_token_indexes = compression::read_compressed_signed_ints(sd, &mut pos, num_paths)?;
    let jumps = compression::read_compressed_signed_ints(sd, &mut pos, num_paths)?;

    // Reconstruct the tree by walking the jump encoding.
    //
    // The jumps describe a depth-first walk, and following them is the only way
    // to know where a subtree *ends*. A previous version kept a parent stack and
    // pushed on every child, but nothing ever told it when to pop — so every
    // prim became a child of the one before it, and a flat scene of 93 meshes
    // came out as a single 90-deep chain.
    //
    // Mirrors `Pxr_UsdCrate::_BuildDecompressedPathsImpl`: walk forward while
    // the current entry has a sibling or a child; when it has both, set the
    // sibling aside to resume later and descend into the child, which is always
    // the very next entry.
    let mut paths = vec![
        PathEntry {
            name: String::new(),
            parent_index: -1,
            is_property: false,
        };
        num_paths
    ];

    let n = path_indexes
        .len()
        .min(elem_token_indexes.len())
        .min(jumps.len());

    // (start index, parent path index). Explicit rather than recursive: a deep
    // scene would otherwise nest as far as the tree does.
    let mut work: Vec<(usize, i32)> = vec![(0, -1)];
    // Guards a malformed file whose jumps form a cycle.
    let mut visited = vec![false; n];

    while let Some((start, start_parent)) = work.pop() {
        let mut cur = start;
        let mut parent = start_parent;
        loop {
            if cur >= n || visited[cur] {
                break;
            }
            visited[cur] = true;
            let this = cur;
            cur += 1;

            let path_idx = path_indexes[this] as usize;
            if path_idx >= num_paths {
                break;
            }

            // The first entry is the absolute root: it has no element token of
            // its own, and its children hang directly off it.
            let is_root = this == 0 && start_parent == -1;
            let token_idx = elem_token_indexes[this];
            let is_property = token_idx < 0;
            let name = if is_root {
                String::new()
            } else {
                let t = token_idx.unsigned_abs() as usize;
                tokens
                    .get(t)
                    .cloned()
                    .unwrap_or_else(|| format!("__path_{}", path_idx))
            };
            paths[path_idx] = PathEntry {
                name,
                parent_index: parent,
                is_property,
            };

            let jump = jumps[this];
            let has_child = jump == -1 || jump > 0;
            let has_sibling = jump >= 0;

            if has_child && has_sibling {
                // The sibling is reached by the jump; come back to it once this
                // subtree is done.
                let sibling = this.saturating_add(jump as usize);
                if sibling < n {
                    work.push((sibling, parent));
                }
            }
            if has_child {
                // The child is the next entry, and this becomes its parent.
                parent = path_idx as i32;
            } else if !has_sibling {
                // Leaf with no sibling: this branch of the walk is finished.
                break;
            }
            // Sibling only: same parent, carry on to the next entry.
        }
    }

    log::debug!("Read {} paths", paths.len());
    Ok(paths)
}

//! Writing a build into a terrain without losing the hand sculpting.
//!
//! # Two layers, one buffer
//!
//! A terrain chunk has one authoritative height buffer, `base_heights`, and
//! every brush, the erosion tools, the heightmap import, the MCP tools and
//! undo read and write it. Splitting it into two real buffers (a generator
//! buffer and a sculpt delta) would have meant rewriting every one of those.
//!
//! So the split is kept *beside* the buffer instead of inside it:
//!
//! - [`TerrainGeneratorLayer`] on each chunk holds what the last build wrote,
//!   in metres. That is the **generator layer**.
//! - `base_heights` stays what it always was, the ground as it is now. The
//!   **sculpt layer** is the difference: `base - generator`. A brush that
//!   raises a hill by 3 m changes `base` and leaves the generator layer
//!   alone, so the hill is 3 m of sculpt.
//!
//! A rebuild computes the new generator layer and writes
//! `base = new generator + (old base - old generator)`: the sculpting moves
//! onto the new ground. Nothing outside this module had to change.
//!
//! A chunk with no generator layer yet (never built, or rebuilt by a
//! resolution change, which respawns chunks) has no sculpt layer to keep: its
//! first build replaces its heights. That is one undo step, like any build.
//!
//! The composed `heights` (base plus the height edit layers and paint carve,
//! see `renzora_terrain::height_layers`) are untouched by any of this: they
//! are recomposed from `base` as always.

use bevy::prelude::*;
use renzora_terrain::data::{TerrainChunkData, TerrainData};
use serde::{Deserialize, Serialize};

use crate::field::{Field, Grid};

/// What the last build wrote into this chunk, metres, row-major over the
/// chunk's own `resolution * resolution` vertices. Empty until a build.
#[derive(Component, Clone, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct TerrainGeneratorLayer {
    #[serde(default)]
    #[reflect(default)]
    pub heights: Vec<f32>,
}

/// The terrain's whole vertex grid (neighbouring chunks share their edge
/// vertices) as an evaluation grid over its full extent.
pub fn terrain_grid(terrain: &TerrainData) -> Grid {
    let (w, h) = renzora_terrain::erosion::grid_size(terrain);
    Grid::new(w, h, Vec2::new(terrain.total_width(), terrain.total_depth()))
}

/// The chunk-local vertex `(lx, lz)` of chunk `(cx, cz)` on the whole grid.
fn global(terrain: &TerrainData, cx: u32, cz: u32, lx: u32, lz: u32) -> (usize, usize) {
    let cells = terrain.chunk_resolution.saturating_sub(1) as usize;
    (cx as usize * cells + lx as usize, cz as usize * cells + lz as usize)
}

/// The generator layer as one whole-terrain field in metres: what the Current
/// Terrain node reads. A chunk that has never been built contributes its
/// present heights, so a first build that starts from Current Terrain starts
/// from the ground as it is.
pub fn gather_generator_layer(terrain: &TerrainData, chunks: &[(&TerrainChunkData, Option<&TerrainGeneratorLayer>)]) -> Field {
    let grid = terrain_grid(terrain);
    let mut out = Field::of(&grid, terrain.min_height);
    let res = terrain.chunk_resolution;
    let range = terrain.height_range();
    for (chunk, layer) in chunks {
        let built = layer.filter(|l| l.heights.len() == chunk.base_heights.len());
        for lz in 0..res {
            for lx in 0..res {
                let i = (lz * res + lx) as usize;
                let (gx, gz) = global(terrain, chunk.chunk_x, chunk.chunk_z, lx, lz);
                if gx >= grid.w || gz >= grid.h {
                    continue;
                }
                let m = match built {
                    Some(l) => l.heights[i],
                    None => terrain.min_height + chunk.base_heights.get(i).copied().unwrap_or(0.0) * range,
                };
                out.data[gz * grid.w + gx] = m;
            }
        }
    }
    out
}

/// What [`apply_generator`] did.
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyReport {
    pub chunks: usize,
    /// Chunks that had a generator layer, so kept their sculpting.
    pub kept_sculpt: usize,
    /// The new `(min_height, max_height)`, when the result did not fit the
    /// old range and the range grew to hold it.
    pub range_grown: Option<(f32, f32)>,
}

/// Write `height` (metres, on [`terrain_grid`] or resampled to it) as the new
/// generator layer of every chunk, keeping each chunk's sculpt layer. See the
/// module docs.
///
/// The terrain's height range grows (never shrinks) when the result does not
/// fit it, because heights are stored normalised to it and anything past the
/// top would clip flat. Every chunk is rewritten from metres, so growing the
/// range moves nothing.
pub fn apply_generator(
    terrain: &mut TerrainData,
    chunks: &mut [(&mut TerrainChunkData, &mut TerrainGeneratorLayer)],
    height: &Field,
) -> ApplyReport {
    let grid = terrain_grid(terrain);
    let height = if height.w == grid.w && height.h == grid.h { height.clone() } else { height.resampled(grid.w, grid.h) };
    let res = terrain.chunk_resolution;
    let (min, range) = (terrain.min_height, terrain.height_range());
    let mut kept = 0;
    // Pass one, in metres: the new generator layer and the final ground.
    let mut planned: Vec<(Vec<f32>, Vec<f32>)> = Vec::with_capacity(chunks.len());
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for (chunk, layer) in chunks.iter() {
        let n = chunk.base_heights.len();
        let had = layer.heights.len() == n && n > 0;
        if had {
            kept += 1;
        }
        let mut gen = vec![0.0f32; n];
        let mut ground = vec![0.0f32; n];
        for lz in 0..res {
            for lx in 0..res {
                let i = (lz * res + lx) as usize;
                if i >= n {
                    continue;
                }
                let (gx, gz) = global(terrain, chunk.chunk_x, chunk.chunk_z, lx, lz);
                let new = height.at(gx.min(grid.w - 1), gz.min(grid.h - 1));
                let sculpt = if had { min + chunk.base_heights[i] * range - layer.heights[i] } else { 0.0 };
                gen[i] = new;
                ground[i] = new + sculpt;
                lo = lo.min(ground[i]);
                hi = hi.max(ground[i]);
            }
        }
        planned.push((gen, ground));
    }
    // Room for the result, with a metre spare so the top is not flush with
    // the clamp.
    let mut range_grown = None;
    if lo.is_finite() && (lo < terrain.min_height || hi > terrain.max_height) {
        let new_min = terrain.min_height.min((lo - 1.0).floor());
        let new_max = terrain.max_height.max((hi + 1.0).ceil());
        terrain.min_height = new_min;
        terrain.max_height = new_max;
        range_grown = Some((new_min, new_max));
    }
    let (min, range) = (terrain.min_height, terrain.height_range().max(1e-6));
    for ((chunk, layer), (gen, ground)) in chunks.iter_mut().zip(planned) {
        for (b, g) in chunk.base_heights.iter_mut().zip(&ground) {
            *b = ((g - min) / range).clamp(0.0, 1.0);
        }
        layer.heights = gen;
        chunk.dirty = true;
    }
    ApplyReport { chunks: chunks.len(), kept_sculpt: kept, range_grown }
}

/// Throw away the sculpt layer: set every built chunk's ground back to its
/// generator layer. Chunks never built are left alone. Returns how many
/// chunks changed.
pub fn reset_sculpt(terrain: &TerrainData, chunks: &mut [(&mut TerrainChunkData, &TerrainGeneratorLayer)]) -> usize {
    let (min, range) = (terrain.min_height, terrain.height_range().max(1e-6));
    let mut changed = 0;
    for (chunk, layer) in chunks.iter_mut() {
        if layer.heights.len() != chunk.base_heights.len() {
            continue;
        }
        let mut moved = false;
        for (b, g) in chunk.base_heights.iter_mut().zip(&layer.heights) {
            let n = ((g - min) / range).clamp(0.0, 1.0);
            if (*b - n).abs() > 1e-7 {
                *b = n;
                moved = true;
            }
        }
        if moved {
            chunk.dirty = true;
            changed += 1;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> TerrainData {
        TerrainData { chunks_x: 2, chunks_z: 2, chunk_size: 16.0, chunk_resolution: 9, min_height: 0.0, max_height: 100.0, ..default() }
    }

    fn chunks(t: &TerrainData) -> Vec<(TerrainChunkData, TerrainGeneratorLayer)> {
        let mut out = Vec::new();
        for cz in 0..t.chunks_z {
            for cx in 0..t.chunks_x {
                out.push((TerrainChunkData::new(cx, cz, t.chunk_resolution, 0.2), TerrainGeneratorLayer::default()));
            }
        }
        out
    }

    fn apply(t: &mut TerrainData, cs: &mut [(TerrainChunkData, TerrainGeneratorLayer)], f: &Field) -> ApplyReport {
        let mut refs: Vec<(&mut TerrainChunkData, &mut TerrainGeneratorLayer)> = cs.iter_mut().map(|(c, l)| (c, l)).collect();
        apply_generator(t, &mut refs, f)
    }

    fn metres(t: &TerrainData, c: &TerrainChunkData, x: u32, z: u32) -> f32 {
        t.min_height + c.get_base_height(x, z, t.chunk_resolution) * t.height_range()
    }

    fn slope(t: &TerrainData, k: f32) -> Field {
        let grid = terrain_grid(t);
        Field::from_fn(&grid, |x, z| 10.0 + k * (x as f32 * 0.7 + z as f32 * 1.3))
    }

    #[test]
    fn seams_agree_and_match_the_field() {
        let mut t = terrain();
        let mut cs = chunks(&t);
        let f = slope(&t, 1.0);
        apply(&mut t, &mut cs, &f);
        let res = t.chunk_resolution;
        let last = res - 1;
        // Chunk (0,0)'s right edge is chunk (1,0)'s left edge.
        for z in 0..res {
            let a = metres(&t, &cs[0].0, last, z);
            let b = metres(&t, &cs[1].0, 0, z);
            assert!((a - b).abs() < 1e-3, "seam at z {z}: {a} vs {b}");
            let (gx, gz) = global(&t, 0, 0, last, z);
            assert!((a - f.at(gx, gz)).abs() < 1e-2);
        }
        // And (0,0)'s top edge is (0,1)'s bottom edge.
        for x in 0..res {
            let a = metres(&t, &cs[0].0, x, last);
            let b = metres(&t, &cs[2].0, x, 0);
            assert!((a - b).abs() < 1e-3);
        }
    }

    #[test]
    fn sculpting_survives_a_rebuild() {
        let mut t = terrain();
        let mut cs = chunks(&t);
        let (gentle, steep) = (slope(&t, 1.0), slope(&t, 2.0));
        let first = apply(&mut t, &mut cs, &gentle);
        assert_eq!(first.kept_sculpt, 0, "nothing to keep before the first build");
        // Sculpt: raise one vertex of chunk 3 by 5 m.
        let res = t.chunk_resolution;
        let i = (4 * res + 4) as usize;
        cs[3].0.base_heights[i] += 5.0 / t.height_range();
        let second = apply(&mut t, &mut cs, &steep);
        assert_eq!(second.kept_sculpt, 4);
        for (k, (c, l)) in cs.iter().enumerate() {
            for (j, (b, g)) in c.base_heights.iter().zip(&l.heights).enumerate() {
                let sculpt = t.min_height + b * t.height_range() - g;
                let want = if k == 3 && j == i { 5.0 } else { 0.0 };
                assert!((sculpt - want).abs() < 1e-2, "chunk ({},{}) vertex {j}: sculpt {sculpt}", c.chunk_x, c.chunk_z);
            }
        }
    }

    #[test]
    fn the_range_grows_to_fit_and_heights_stay_put() {
        let mut t = terrain();
        let mut cs = chunks(&t);
        let grid = terrain_grid(&t);
        let tall = Field::from_fn(&grid, |x, _| x as f32 * 20.0 - 30.0);
        let report = apply(&mut t, &mut cs, &tall);
        let (lo, hi) = report.range_grown.expect("past both ends");
        assert!(lo <= -31.0 && hi >= tall.min_max().1 + 1.0);
        let m = metres(&t, &cs[1].0, t.chunk_resolution - 1, 0);
        assert!((m - tall.at(grid.w - 1, 0)).abs() < 0.05, "{m}");
    }

    #[test]
    fn resetting_the_sculpt_returns_to_the_generator() {
        let mut t = terrain();
        let mut cs = chunks(&t);
        let f = slope(&t, 1.0);
        apply(&mut t, &mut cs, &f);
        cs[0].0.base_heights[3] += 0.1;
        let mut refs: Vec<(&mut TerrainChunkData, &TerrainGeneratorLayer)> = cs.iter_mut().map(|(c, l)| (c, &*l)).collect();
        assert_eq!(reset_sculpt(&t, &mut refs), 1);
        let g = cs[0].1.heights[3];
        assert!((metres(&t, &cs[0].0, 3, 0) - g).abs() < 1e-3);
    }

    #[test]
    fn gathering_reads_the_layer_or_the_ground() {
        let mut t = terrain();
        let mut cs = chunks(&t);
        // Never built: the ground, 0.2 of 100 m.
        let refs: Vec<(&TerrainChunkData, Option<&TerrainGeneratorLayer>)> = cs.iter().map(|(c, l)| (c, Some(l))).collect();
        let before = gather_generator_layer(&t, &refs);
        assert!(before.data.iter().all(|v| (v - 20.0).abs() < 1e-3));
        let f = slope(&t, 1.0);
        apply(&mut t, &mut cs, &f);
        let refs: Vec<(&TerrainChunkData, Option<&TerrainGeneratorLayer>)> = cs.iter().map(|(c, l)| (c, Some(l))).collect();
        let after = gather_generator_layer(&t, &refs);
        for (a, b) in after.data.iter().zip(&f.data) {
            assert!((a - b).abs() < 1e-3);
        }
    }
}

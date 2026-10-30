//! Heightfields and the grid they sit on, plus the field operations more than
//! one node needs: blur, slope, flow accumulation, basin filling.
//!
//! A [`Field`] is a row-major `f32` grid with no units of its own; a node's pin
//! kind says whether it holds metres or a 0..1 mask. A [`Grid`] says where the
//! samples are: `w` by `h` vertices spread evenly over `extent` metres, the
//! first at the terrain's minimum corner and the last at its maximum, which is
//! exactly how the terrain's whole-grid vertices lie (see
//! `renzora_terrain::erosion::grid_size`).

use bevy::math::Vec2;
use bevy::tasks::{ComputeTaskPool, TaskPool};

/// Where a field's samples are, in terrain-local metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    /// Width and depth covered, metres.
    pub extent: Vec2,
}

impl Grid {
    pub fn new(w: usize, h: usize, extent: Vec2) -> Self {
        Self { w: w.max(2), h: h.max(2), extent }
    }

    /// A grid about `samples` vertices along the longer side over `extent`,
    /// the other side in proportion: the shape previews are evaluated at.
    pub fn fitting(extent: Vec2, samples: usize) -> Self {
        let samples = samples.max(2);
        let long = extent.x.max(extent.y).max(1e-3);
        let w = ((extent.x / long) * (samples - 1) as f32).round() as usize + 1;
        let h = ((extent.y / long) * (samples - 1) as f32).round() as usize + 1;
        Self::new(w, h, extent)
    }

    pub fn len(&self) -> usize {
        self.w * self.h
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Metres between neighbouring samples, along x and z.
    pub fn spacing(&self) -> Vec2 {
        Vec2::new(self.extent.x / (self.w - 1) as f32, self.extent.y / (self.h - 1) as f32)
    }

    /// The mean of the two spacings, for kernels that are round.
    pub fn cell(&self) -> f32 {
        let s = self.spacing();
        (s.x + s.y) * 0.5
    }

    /// Terrain-local metres of sample `(x, z)`.
    pub fn pos(&self, x: usize, z: usize) -> Vec2 {
        let s = self.spacing();
        Vec2::new(x as f32 * s.x, z as f32 * s.y)
    }

    /// The centre of the extent, the origin offsets are measured from.
    pub fn centre(&self) -> Vec2 {
        self.extent * 0.5
    }

    /// A radius in metres as cells, never below `min`.
    pub fn cells(&self, metres: f32, min: f32) -> f32 {
        (metres / self.cell().max(1e-6)).max(min)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

impl Field {
    pub fn filled(w: usize, h: usize, value: f32) -> Self {
        Self { w, h, data: vec![value; w * h] }
    }

    pub fn of(grid: &Grid, value: f32) -> Self {
        Self::filled(grid.w, grid.h, value)
    }

    /// Evaluate `f` at every sample, rows spread over the compute pool.
    pub fn from_fn(grid: &Grid, f: impl Fn(usize, usize) -> f32 + Sync) -> Self {
        Self { w: grid.w, h: grid.h, data: par_fill(grid.w, grid.h, f) }
    }

    pub fn at(&self, x: usize, z: usize) -> f32 {
        self.data[z * self.w + x]
    }

    /// Read at a fractional cell position, clamped to the edge.
    pub fn bilinear(&self, fx: f32, fz: f32) -> f32 {
        let fx = fx.clamp(0.0, (self.w - 1) as f32);
        let fz = fz.clamp(0.0, (self.h - 1) as f32);
        let x0 = (fx.floor() as usize).min(self.w.saturating_sub(2));
        let z0 = (fz.floor() as usize).min(self.h.saturating_sub(2));
        let (x1, z1) = ((x0 + 1).min(self.w - 1), (z0 + 1).min(self.h - 1));
        let (u, v) = (fx - x0 as f32, fz - z0 as f32);
        let a = self.at(x0, z0) * (1.0 - u) + self.at(x1, z0) * u;
        let b = self.at(x0, z1) * (1.0 - u) + self.at(x1, z1) * u;
        a * (1.0 - v) + b * v
    }

    /// Read at terrain-local metres on `grid`.
    pub fn sample_metres(&self, grid: &Grid, p: Vec2) -> f32 {
        // Through the field's own cell count rather than `grid`'s spacing, so
        // a field at another resolution over the same extent reads right.
        let fx = p.x / grid.extent.x.max(1e-6) * (self.w - 1) as f32;
        let fz = p.y / grid.extent.y.max(1e-6) * (self.h - 1) as f32;
        self.bilinear(fx, fz)
    }

    /// The same field at another resolution, bilinear.
    pub fn resampled(&self, w: usize, h: usize) -> Field {
        if w == self.w && h == self.h {
            return self.clone();
        }
        let sx = (self.w - 1) as f32 / (w - 1).max(1) as f32;
        let sz = (self.h - 1) as f32 / (h - 1).max(1) as f32;
        let grid = Grid::new(w, h, Vec2::ONE);
        Field::from_fn(&grid, |x, z| self.bilinear(x as f32 * sx, z as f32 * sz))
    }

    pub fn min_max(&self) -> (f32, f32) {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for &v in &self.data {
            if v.is_finite() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if lo > hi {
            (0.0, 0.0)
        } else {
            (lo, hi)
        }
    }

    pub fn map(&self, f: impl Fn(f32) -> f32 + Sync) -> Field {
        let grid = Grid::new(self.w, self.h, Vec2::ONE);
        Field::from_fn(&grid, |x, z| f(self.at(x, z)))
    }

    /// Replace any NaN or infinity by 0. A node that divides by a range of
    /// zero must not poison everything downstream of it.
    pub fn sanitize(&mut self) {
        for v in &mut self.data {
            if !v.is_finite() {
                *v = 0.0;
            }
        }
    }
}

/// Fill a `w` by `h` grid with `f(x, z)`, in row bands on the compute pool.
/// Small grids run inline: spawning tasks for a 64 cell preview costs more than
/// it saves. Deterministic whatever the thread count, because every sample is
/// computed independently.
pub fn par_fill(w: usize, h: usize, f: impl Fn(usize, usize) -> f32 + Sync) -> Vec<f32> {
    let mut data = vec![0.0f32; w * h];
    if w * h < 16_384 {
        for (i, v) in data.iter_mut().enumerate() {
            *v = f(i % w, i / w);
        }
        return data;
    }
    let pool = ComputeTaskPool::get_or_init(TaskPool::new);
    let bands = (pool.thread_num().max(1) * 4).min(h);
    let rows = h.div_ceil(bands).max(1);
    let f = &f;
    pool.scope(|s| {
        for (band, chunk) in data.chunks_mut(rows * w).enumerate() {
            s.spawn(async move {
                let first = band * rows * w;
                for (i, v) in chunk.iter_mut().enumerate() {
                    let idx = first + i;
                    *v = f(idx % w, idx / w);
                }
            });
        }
    });
    data
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x >= e1 { 1.0 } else { 0.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 1 inside `[lo, hi]`, fading to 0 over `falloff` either side.
pub fn band(x: f32, lo: f32, hi: f32, falloff: f32) -> f32 {
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    if falloff <= 0.0 {
        return if x >= lo && x <= hi { 1.0 } else { 0.0 };
    }
    smoothstep(lo - falloff, lo, x) * (1.0 - smoothstep(hi, hi + falloff, x))
}

/// Box blur of radius `r` cells, `passes` times (three passes is close to a
/// Gaussian). Separable running sums, so the cost does not grow with `r`.
pub fn blur(field: &Field, r: usize, passes: usize) -> Field {
    if r == 0 || passes == 0 {
        return field.clone();
    }
    let (w, h) = (field.w, field.h);
    let mut a = field.data.clone();
    let mut b = vec![0.0f32; w * h];
    let mut line = Vec::new();
    let mut out = Vec::new();
    for _ in 0..passes {
        for z in 0..h {
            line.clear();
            line.extend_from_slice(&a[z * w..(z + 1) * w]);
            blur_line(&line, r, &mut out);
            b[z * w..(z + 1) * w].copy_from_slice(&out);
        }
        for x in 0..w {
            line.clear();
            line.extend((0..h).map(|z| b[z * w + x]));
            blur_line(&line, r, &mut out);
            for z in 0..h {
                a[z * w + x] = out[z];
            }
        }
    }
    Field { w, h, data: a }
}

/// One line of a box blur, edges clamped.
fn blur_line(line: &[f32], r: usize, out: &mut Vec<f32>) {
    let n = line.len();
    out.clear();
    out.resize(n, 0.0);
    if n == 0 {
        return;
    }
    let at = |i: isize| line[i.clamp(0, n as isize - 1) as usize];
    let ri = r as isize;
    let mut sum: f32 = (-ri..=ri).map(at).sum();
    let norm = 1.0 / (2 * r + 1) as f32;
    for (i, o) in out.iter_mut().enumerate() {
        *o = sum * norm;
        let i = i as isize;
        sum += at(i + ri + 1) - at(i - ri);
    }
}

/// Blur by a radius in metres on `grid`.
pub fn blur_metres(field: &Field, grid: &Grid, radius: f32) -> Field {
    let r = (radius / grid.cell().max(1e-6) / 1.7).round() as usize;
    blur(field, r, 3)
}

/// Height gradient in metres per metre at every sample (central differences,
/// one-sided at the edge).
pub fn gradient(field: &Field, grid: &Grid) -> Vec<Vec2> {
    let s = grid.spacing();
    let (w, h) = (field.w, field.h);
    let mut out = vec![Vec2::ZERO; w * h];
    for z in 0..h {
        for x in 0..w {
            let (xl, xr) = (x.saturating_sub(1), (x + 1).min(w - 1));
            let (zd, zu) = (z.saturating_sub(1), (z + 1).min(h - 1));
            let dx = (field.at(xr, z) - field.at(xl, z)) / ((xr - xl).max(1) as f32 * s.x);
            let dz = (field.at(x, zu) - field.at(x, zd)) / ((zu - zd).max(1) as f32 * s.y);
            out[z * w + x] = Vec2::new(dx, dz);
        }
    }
    out
}

/// Steepness in degrees at every sample.
pub fn slope_degrees(field: &Field, grid: &Grid) -> Field {
    let g = gradient(field, grid);
    Field { w: field.w, h: field.h, data: g.iter().map(|d| d.length().atan().to_degrees()).collect() }
}

const NEIGHBOURS: [(isize, isize); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// Uphill drainage area through every sample, square metres, by multiple
/// flow direction: each cell passes what it gathered to every lower neighbour
/// in proportion to the drop. Single-direction flow (all of it to the
/// steepest neighbour) draws rivers as straight grid-aligned lines.
pub fn flow_accumulation(field: &Field, grid: &Grid) -> Field {
    let (w, h) = (field.w, field.h);
    let s = grid.spacing();
    let cell_area = s.x * s.y;
    let mut order: Vec<usize> = (0..w * h).collect();
    // Highest first, so every cell has received all its inflow before it
    // passes it on. `total_cmp` orders NaN deterministically.
    order.sort_by(|a, b| field.data[*b].total_cmp(&field.data[*a]));
    let mut acc = vec![cell_area; w * h];
    let diag = (s.x * s.x + s.y * s.y).sqrt();
    let mut drops = [0.0f32; 8];
    for &i in &order {
        let (x, z) = ((i % w) as isize, (i / w) as isize);
        let here = field.data[i];
        let mut total = 0.0f32;
        for (n, (dx, dz)) in NEIGHBOURS.iter().enumerate() {
            let (nx, nz) = (x + dx, z + dz);
            drops[n] = 0.0;
            if nx < 0 || nz < 0 || nx >= w as isize || nz >= h as isize {
                continue;
            }
            let j = nz as usize * w + nx as usize;
            let dist = if n < 4 { if *dx != 0 { s.x } else { s.y } } else { diag };
            let drop = (here - field.data[j]) / dist;
            if drop > 0.0 {
                let d = drop.powf(1.1);
                drops[n] = d;
                total += d;
            }
        }
        if total <= 0.0 {
            continue;
        }
        let give = acc[i];
        for (n, (dx, dz)) in NEIGHBOURS.iter().enumerate() {
            if drops[n] > 0.0 {
                let j = (z + dz) as usize * w + (x + dx) as usize;
                acc[j] += give * drops[n] / total;
            }
        }
    }
    Field { w, h, data: acc }
}

/// Drainage area as a 0..1 mask, `ln` scaled between two areas.
pub fn flow_mask(area: &Field, min_area: f32, max_area: f32) -> Field {
    let lo = min_area.max(1e-3).ln();
    let hi = max_area.max(min_area * 1.001).ln();
    area.map(|a| ((a.max(1e-6).ln() - lo) / (hi - lo)).clamp(0.0, 1.0))
}

/// Total-order wrapper so heights can sit in a binary heap.
#[derive(Clone, Copy, PartialEq)]
struct Ord32(f32);
impl Eq for Ord32 {}
impl PartialOrd for Ord32 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ord32 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// The water level every sample would hold if rain filled every basin to the
/// height it spills at (priority flood from the edges, which drain off the
/// map). Never below the ground.
pub fn fill_basins(field: &Field) -> Field {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let (w, h) = (field.w, field.h);
    let mut level = field.data.clone();
    let mut done = vec![false; w * h];
    let mut heap = BinaryHeap::new();
    for z in 0..h {
        for x in 0..w {
            if x == 0 || z == 0 || x == w - 1 || z == h - 1 {
                let i = z * w + x;
                done[i] = true;
                heap.push(Reverse((Ord32(level[i]), i)));
            }
        }
    }
    while let Some(Reverse((Ord32(l), i))) = heap.pop() {
        let (x, z) = ((i % w) as isize, (i / w) as isize);
        for (dx, dz) in NEIGHBOURS {
            let (nx, nz) = (x + dx, z + dz);
            if nx < 0 || nz < 0 || nx >= w as isize || nz >= h as isize {
                continue;
            }
            let j = nz as usize * w + nx as usize;
            if done[j] {
                continue;
            }
            done[j] = true;
            level[j] = level[j].max(l);
            heap.push(Reverse((Ord32(level[j]), j)));
        }
    }
    Field { w, h, data: level }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_spacing_and_positions_span_the_extent() {
        let g = Grid::new(5, 3, Vec2::new(40.0, 20.0));
        assert_eq!(g.spacing(), Vec2::new(10.0, 10.0));
        assert_eq!(g.pos(4, 2), Vec2::new(40.0, 20.0));
        let f = Grid::fitting(Vec2::new(200.0, 100.0), 65);
        assert_eq!((f.w, f.h), (65, 33));
    }

    #[test]
    fn parallel_fill_matches_a_plain_loop() {
        let (w, h) = (300, 211);
        let par = par_fill(w, h, |x, z| (x * 7 + z * 13) as f32);
        for z in 0..h {
            for x in 0..w {
                assert_eq!(par[z * w + x], (x * 7 + z * 13) as f32);
            }
        }
    }

    #[test]
    fn blur_keeps_a_constant_and_spreads_a_spike() {
        let flat = Field::filled(16, 16, 3.0);
        assert!(blur(&flat, 3, 3).data.iter().all(|v| (v - 3.0).abs() < 1e-4));
        let mut spike = Field::filled(17, 17, 0.0);
        spike.data[8 * 17 + 8] = 100.0;
        let b = blur(&spike, 2, 3);
        assert!(b.at(8, 8) < 100.0 && b.at(9, 8) > 0.0);
        let sum: f32 = b.data.iter().sum();
        assert!((sum - 100.0).abs() < 1.0, "{sum}");
    }

    #[test]
    fn flow_gathers_at_the_bottom_of_a_slope() {
        let grid = Grid::new(20, 20, Vec2::splat(19.0));
        let f = Field::from_fn(&grid, |x, z| (x + z) as f32);
        let area = flow_accumulation(&f, &grid);
        assert!(area.at(0, 0) > area.at(19, 19) * 50.0);
    }

    #[test]
    fn a_pit_fills_to_its_rim() {
        let mut f = Field::filled(9, 9, 10.0);
        for z in 3..6 {
            for x in 3..6 {
                f.data[z * 9 + x] = 2.0;
            }
        }
        let filled = fill_basins(&f);
        assert_eq!(filled.at(4, 4), 10.0);
        assert_eq!(filled.at(0, 0), 10.0);
    }
}

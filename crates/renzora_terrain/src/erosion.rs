//! Erosion: what turns smooth noise into ground that looks weathered.
//!
//! Two processes, both run on a [`HeightWindow`], a rectangle of the terrain's
//! heights gathered across chunk seams so a gully can run from one chunk into
//! the next:
//!
//! - **Hydraulic** ([`hydraulic`]): simulated raindrops roll downhill, pick up
//!   soil where they speed up and drop it where they slow down. This carves the
//!   branching gullies and fills the valley floors with sediment, which is the
//!   look generated terrain lacks most.
//! - **Thermal** ([`thermal`]): any slope steeper than the angle loose material
//!   rests at sheds the excess onto its lower neighbour, softening noisy spikes
//!   into scree slopes.
//!
//! Heights are worked in cell units (metres divided by the grid spacing), so the
//! same parameters behave the same on a fine or a coarse terrain.
//!
//! On a fine grid the droplets run on a coarser copy (about 3 m a cell, see
//! [`DROPLET_CELL_METRES`]) and only the change they make is interpolated back
//! up, then blurred over a couple of metres before it lands. Run directly on a
//! 1 m grid, with droplets sparse per cell and a brush a few cells wide, every
//! droplet cut its own groove and slopes came out combed with parallel
//! striations.
//!
//! The brushes of the same names used to nudge each vertex toward its four
//! neighbours inside one chunk, scaled by the frame time: a faint smoothing,
//! nothing like erosion, and it stopped at every chunk edge.

use bevy::prelude::*;

use crate::data::{TerrainChunkData, TerrainData};

/// A rectangle of the terrain's base heights, in metres, on the
/// whole-terrain vertex grid (neighbouring chunks share their edge vertices,
/// so the grid is `chunks * (resolution - 1) + 1` vertices across).
pub struct HeightWindow {
    /// First grid cell of the window.
    pub x0: usize,
    pub z0: usize,
    pub w: usize,
    pub h: usize,
    /// Metres between grid vertices.
    pub spacing: f32,
    pub data: Vec<f32>,
}

/// The whole-terrain grid size, in vertices per side.
pub fn grid_size(terrain: &TerrainData) -> (usize, usize) {
    let cells = terrain.chunk_resolution.saturating_sub(1) as usize;
    (terrain.chunks_x as usize * cells + 1, terrain.chunks_z as usize * cells + 1)
}

/// The chunks that hold grid vertex `(gx, gz)`, as `(chunk x, chunk z, local
/// x, local z)`. A vertex on a seam belongs to every chunk that meets there.
fn owners(terrain: &TerrainData, gx: usize, gz: usize) -> impl Iterator<Item = (u32, u32, u32, u32)> {
    let cells = terrain.chunk_resolution.saturating_sub(1) as usize;
    let along = move |g: usize, count: u32| {
        let c = (g / cells).min(count as usize - 1);
        let mut out = [(c, g - c * cells); 2];
        let mut n = 1;
        // On a seam the previous chunk owns it too, as its last vertex.
        if g.is_multiple_of(cells) && c > 0 && g / cells == c {
            out[1] = (c - 1, cells);
            n = 2;
        }
        (out, n)
    };
    let (xs, nx) = along(gx, terrain.chunks_x);
    let (zs, nz) = along(gz, terrain.chunks_z);
    (0..nx).flat_map(move |i| {
        (0..nz).map(move |j| (xs[i].0 as u32, zs[j].0 as u32, xs[i].1 as u32, zs[j].1 as u32))
    })
}

impl HeightWindow {
    /// Gather grid vertices `x0..x0 + w`, `z0..z0 + h` (clipped to the
    /// terrain) from `chunks`, whose order does not matter.
    pub fn gather(
        terrain: &TerrainData,
        chunks: &[&TerrainChunkData],
        x0: usize,
        z0: usize,
        w: usize,
        h: usize,
    ) -> Self {
        let (gw, gh) = grid_size(terrain);
        let x0 = x0.min(gw.saturating_sub(1));
        let z0 = z0.min(gh.saturating_sub(1));
        let w = w.min(gw - x0).max(1);
        let h = h.min(gh - z0).max(1);
        let index = chunk_index(terrain, chunks.iter().map(|c| (c.chunk_x, c.chunk_z)));
        let res = terrain.chunk_resolution;
        let mut data = vec![0.0; w * h];
        for z in 0..h {
            for x in 0..w {
                let (gx, gz) = (x0 + x, z0 + z);
                if let Some((cx, cz, lx, lz)) = owners(terrain, gx, gz).next() {
                    if let Some(&i) = index.get(&(cx, cz)) {
                        let n = chunks[i].get_base_height(lx, lz, res);
                        data[z * w + x] = terrain.min_height + n * terrain.height_range();
                    }
                }
            }
        }
        Self { x0, z0, w, h, spacing: terrain.vertex_spacing(), data }
    }

    /// The whole terrain.
    pub fn gather_all(terrain: &TerrainData, chunks: &[&TerrainChunkData]) -> Self {
        let (gw, gh) = grid_size(terrain);
        Self::gather(terrain, chunks, 0, 0, gw, gh)
    }

    /// The window around a terrain-local point (0..total width), `radius`
    /// metres out plus `margin` cells, so a process near the edge has room.
    pub fn around(
        terrain: &TerrainData,
        chunks: &[&TerrainChunkData],
        center: Vec2,
        radius: f32,
        margin: usize,
    ) -> Self {
        let s = terrain.vertex_spacing();
        let r = (radius / s).ceil() as usize + margin;
        let cx = (center.x / s).round().max(0.0) as usize;
        let cz = (center.y / s).round().max(0.0) as usize;
        Self::gather(terrain, chunks, cx.saturating_sub(r), cz.saturating_sub(r), 2 * r + 1, 2 * r + 1)
    }

    /// Write the window back into `chunks`' base heights, only where a value
    /// moved, marking each chunk it changed dirty. Returns how many vertices
    /// changed.
    pub fn scatter(&self, terrain: &TerrainData, original: &HeightWindow, chunks: &mut [&mut TerrainChunkData]) -> usize {
        let index = chunk_index(terrain, chunks.iter().map(|c| (c.chunk_x, c.chunk_z)));
        let res = terrain.chunk_resolution;
        let range = terrain.height_range().max(1e-6);
        let mut changed = 0;
        for z in 0..self.h {
            for x in 0..self.w {
                let i = z * self.w + x;
                if (self.data[i] - original.data[i]).abs() < 1e-5 {
                    continue;
                }
                changed += 1;
                let n = ((self.data[i] - terrain.min_height) / range).clamp(0.0, 1.0);
                for (cx, cz, lx, lz) in owners(terrain, self.x0 + x, self.z0 + z) {
                    if let Some(&c) = index.get(&(cx, cz)) {
                        chunks[c].set_height(lx, lz, res, n);
                    }
                }
            }
        }
        changed
    }

    fn at(&self, x: usize, z: usize) -> f32 {
        self.data[z * self.w + x]
    }

    /// Height and slope at a point in window cells, bilinear.
    fn sample(&self, p: Vec2) -> (f32, Vec2) {
        let x = (p.x.floor() as usize).min(self.w - 2);
        let z = (p.y.floor() as usize).min(self.h - 2);
        let (u, v) = (p.x - x as f32, p.y - z as f32);
        let (a, b, c, d) = (self.at(x, z), self.at(x + 1, z), self.at(x, z + 1), self.at(x + 1, z + 1));
        let grad = Vec2::new((b - a) * (1.0 - v) + (d - c) * v, (c - a) * (1.0 - u) + (d - b) * u);
        let height = a * (1.0 - u) * (1.0 - v) + b * u * (1.0 - v) + c * (1.0 - u) * v + d * u * v;
        (height, grad)
    }

    fn add_bilinear(&mut self, p: Vec2, amount: f32) {
        let x = (p.x.floor() as usize).min(self.w - 2);
        let z = (p.y.floor() as usize).min(self.h - 2);
        let (u, v) = (p.x - x as f32, p.y - z as f32);
        let w = self.w;
        self.data[z * w + x] += amount * (1.0 - u) * (1.0 - v);
        self.data[z * w + x + 1] += amount * u * (1.0 - v);
        self.data[(z + 1) * w + x] += amount * (1.0 - u) * v;
        self.data[(z + 1) * w + x + 1] += amount * u * v;
    }

    /// Spread `amount` over a disc of `weights` (from [`disc_weights`])
    /// centred on the cell nearest `p`. Returns how much was placed: less than
    /// `amount` where the disc hangs off the window. A negative amount digs.
    fn add_disc(&mut self, p: Vec2, weights: &[(i32, i32, f32)], amount: f32) -> f32 {
        let (cx, cz) = (p.x.round() as i32, p.y.round() as i32);
        let mut placed = 0.0;
        for &(bx, bz, wt) in weights {
            let (x, z) = (cx + bx, cz + bz);
            if x >= 0 && z >= 0 && (x as usize) < self.w && (z as usize) < self.h {
                let d = amount * wt;
                self.data[z as usize * self.w + x as usize] += d;
                placed += d;
            }
        }
        placed
    }

    /// A copy at `1 / factor` the resolution, each coarse vertex the average of
    /// the fine vertices nearest it.
    fn downsample(&self, factor: usize) -> HeightWindow {
        let w = (self.w - 1).div_ceil(factor) + 1;
        let h = (self.h - 1).div_ceil(factor) + 1;
        let half = (factor / 2) as i64;
        let mut data = vec![0.0; w * h];
        for cz in 0..h {
            for cx in 0..w {
                let (fx, fz) = ((cx * factor) as i64, (cz * factor) as i64);
                let (mut sum, mut n) = (0.0f32, 0.0f32);
                for z in (fz - half).max(0)..=(fz + half).min(self.h as i64 - 1) {
                    for x in (fx - half).max(0)..=(fx + half).min(self.w as i64 - 1) {
                        sum += self.data[z as usize * self.w + x as usize];
                        n += 1.0;
                    }
                }
                data[cz * w + cx] = sum / n.max(1.0);
            }
        }
        HeightWindow { x0: 0, z0: 0, w, h, spacing: self.spacing * factor as f32, data }
    }

    /// Add a coarse field (laid out as [`HeightWindow::downsample`] makes it)
    /// back onto this window, bilinearly interpolated.
    fn add_upsampled(&mut self, coarse: &[f32], cw: usize, ch: usize, factor: usize) {
        let f = factor as f32;
        for z in 0..self.h {
            let gz = (z as f32 / f).min((ch - 1) as f32);
            let z0 = (gz.floor() as usize).min(ch.saturating_sub(2));
            let v = gz - z0 as f32;
            for x in 0..self.w {
                let gx = (x as f32 / f).min((cw - 1) as f32);
                let x0 = (gx.floor() as usize).min(cw.saturating_sub(2));
                let u = gx - x0 as f32;
                let at = |x: usize, z: usize| coarse[z.min(ch - 1) * cw + x.min(cw - 1)];
                let d = at(x0, z0) * (1.0 - u) * (1.0 - v)
                    + at(x0 + 1, z0) * u * (1.0 - v)
                    + at(x0, z0 + 1) * (1.0 - u) * v
                    + at(x0 + 1, z0 + 1) * u * v;
                self.data[z * self.w + x] += d;
            }
        }
    }
}

/// Weights of a cone-shaped disc of radius `r` cells, summing to 1.
fn disc_weights(r: f32) -> Vec<(i32, i32, f32)> {
    let ri = r.ceil() as i32;
    let mut brush = Vec::new();
    for dz in -ri..=ri {
        for dx in -ri..=ri {
            let d = ((dx * dx + dz * dz) as f32).sqrt();
            if d < r {
                brush.push((dx, dz, 1.0 - d / r));
            }
        }
    }
    let total: f32 = brush.iter().map(|b| b.2).sum();
    brush.iter_mut().for_each(|b| b.2 /= total.max(1e-6));
    brush
}

/// Grid spacing, in metres, the droplets run at. A droplet moves one cell a
/// step and its brush is a few cells wide, so on a fine grid it carves a
/// groove a few metres across and, with droplets sparse per cell (500k over
/// four million vertices is one per eight), each groove stays on its own:
/// combed slopes. Running on a ~3 m grid packs about one droplet per cell,
/// so their paths overlap and merge into dendritic gullies, and the fine grid
/// receives the change smoothly interpolated.
pub const DROPLET_CELL_METRES: f32 = 3.0;

/// How many fine cells make one droplet cell, for a window `cells` across.
fn coarse_factor(spacing: f32, cells: usize) -> usize {
    let mut f = (DROPLET_CELL_METRES / spacing.max(1e-4)).round().clamp(1.0, 8.0) as usize;
    // Keep enough of a grid for droplets to run on.
    while f > 1 && cells / f < 32 {
        f -= 1;
    }
    f
}

fn chunk_index(
    terrain: &TerrainData,
    coords: impl Iterator<Item = (u32, u32)>,
) -> bevy::platform::collections::HashMap<(u32, u32), usize> {
    let _ = terrain;
    coords.enumerate().map(|(i, c)| (c, i)).collect()
}

/// Hydraulic erosion settings. The defaults carve gullies a few metres across
/// and leave soft sediment floors, on any grid spacing.
#[derive(Clone, Copy, Debug)]
pub struct Hydraulic {
    pub droplets: u32,
    /// Radius of the patch a droplet erodes from, in **metres** (converted to
    /// cells by the window's spacing, never under 1.5 cells). It used to be in
    /// cells, three of them, which on a 1 m grid made every droplet carve its
    /// own 3 m groove: a comb of parallel striations down every slope.
    pub erosion_radius: f32,
    /// 0..1: how much a droplet keeps its direction rather than follow the slope.
    pub inertia: f32,
    pub capacity: f32,
    pub erode_speed: f32,
    pub deposit_speed: f32,
    pub evaporate: f32,
    pub gravity: f32,
    pub lifetime: u32,
    pub seed: u32,
}

impl Default for Hydraulic {
    fn default() -> Self {
        Self {
            droplets: 20_000,
            erosion_radius: 6.0,
            // Enough momentum that neighbouring droplets on a smooth slope
            // wander into each other's paths and merge into gullies, rather
            // than each following the exact fall line in parallel.
            inertia: 0.12,
            capacity: 4.0,
            erode_speed: 0.3,
            deposit_speed: 0.3,
            evaporate: 0.015,
            gravity: 4.0,
            lifetime: 64,
            seed: 1,
        }
    }
}

/// A small, fast, seedable generator: erosion needs a lot of random numbers
/// and none of them have to be good.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Roll `settings.droplets` raindrops over the window. Droplets start where
/// `start` (in window cells, a centre and a radius) says, or anywhere.
///
/// Heights are scaled into cell units for the run, so the parameters mean the
/// same on any grid spacing.
pub fn hydraulic(window: &mut HeightWindow, settings: &Hydraulic, start: Option<(Vec2, f32)>) {
    if window.w < 4 || window.h < 4 {
        return;
    }
    let to_cells = 1.0 / window.spacing.max(1e-4);
    window.data.iter_mut().for_each(|h| *h *= to_cells);

    // Erosion brush: a disc of `erosion_radius` metres. Deposits spread over a
    // disc half that size, so dropped sediment settles as a soft floor rather
    // than a bead on one cell.
    let r = (settings.erosion_radius * to_cells).clamp(1.5, 12.0);
    let brush = disc_weights(r);
    let deposit_brush = disc_weights((r * 0.5).max(1.0));

    let mut rng = Rng(settings.seed.wrapping_mul(2_654_435_761).max(1));
    let max = Vec2::new((window.w - 2) as f32, (window.h - 2) as f32);
    for _ in 0..settings.droplets {
        let mut pos = match start {
            Some((c, rad)) => {
                let a = rng.next() * std::f32::consts::TAU;
                let d = rad * rng.next().sqrt();
                (c + Vec2::new(a.cos(), a.sin()) * d).clamp(Vec2::ZERO, max)
            }
            None => Vec2::new(rng.next() * max.x, rng.next() * max.y),
        };
        let mut dir = Vec2::ZERO;
        let (mut speed, mut water, mut sediment) = (1.0f32, 1.0f32, 0.0f32);
        for _ in 0..settings.lifetime {
            let (height, grad) = window.sample(pos);
            dir = dir * settings.inertia - grad * (1.0 - settings.inertia);
            let len = dir.length();
            if len < 1e-6 {
                break;
            }
            dir /= len;
            let old = pos;
            pos += dir;
            if pos.x < 0.0 || pos.y < 0.0 || pos.x > max.x || pos.y > max.y {
                break;
            }
            let dh = window.sample(pos).0 - height;
            let capacity = (-dh * speed * water * settings.capacity).max(0.01);
            if dh > 0.0 {
                // Uphill: fill the hollow behind, exactly where it is.
                let drop = dh.min(sediment);
                sediment -= drop;
                window.add_bilinear(old, drop);
            } else if sediment > capacity {
                // Overloaded: drop the excess, spread.
                let drop = (sediment - capacity) * settings.deposit_speed;
                sediment -= window.add_disc(old, &deposit_brush, drop);
            } else {
                // `add_disc` answers what it placed, negative here: the soil
                // the droplet picked up.
                let take = ((capacity - sediment) * settings.erode_speed).min(-dh);
                sediment -= window.add_disc(old, &brush, -take);
            }
            speed = (speed * speed + dh * settings.gravity).max(0.0).sqrt();
            water *= 1.0 - settings.evaporate;
        }
    }

    window.data.iter_mut().for_each(|h| *h /= to_cells);
}

/// Shed every slope steeper than `repose_degrees` onto its lowest neighbour,
/// `iterations` times, each move scaled by `weight` (0..1) for the vertex it
/// starts from. A brush passes its falloff as the weight; the whole-terrain
/// pass passes 1.
///
/// The material is shared among every lower neighbour of the eight that is
/// past the angle, in proportion to how far past it is. Sending it all to the
/// single lowest of four neighbours carved the scree along the grid axes.
pub fn thermal(window: &mut HeightWindow, repose_degrees: f32, iterations: u32, weight: impl Fn(usize, usize) -> f32) {
    let talus = repose_degrees.to_radians().tan() * window.spacing;
    let talus_diag = talus * std::f32::consts::SQRT_2;
    let (w, h) = (window.w, window.h);
    const NEIGHBOURS: [(isize, isize); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
    for _ in 0..iterations {
        for z in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let i = z * w + x;
                let here = window.data[i];
                let mut excess = [0.0f32; 8];
                let (mut total, mut largest) = (0.0f32, 0.0f32);
                for (n, (dx, dz)) in NEIGHBOURS.iter().enumerate() {
                    let j = (z as isize + dz) as usize * w + (x as isize + dx) as usize;
                    let lim = if n < 4 { talus } else { talus_diag };
                    let e = here - window.data[j] - lim;
                    if e > 0.0 {
                        excess[n] = e;
                        total += e;
                        largest = largest.max(e);
                    }
                }
                if total <= 0.0 {
                    continue;
                }
                let moved = largest * 0.5 * weight(x, z).clamp(0.0, 1.0);
                window.data[i] -= moved;
                for (n, (dx, dz)) in NEIGHBOURS.iter().enumerate() {
                    if excess[n] > 0.0 {
                        let j = (z as isize + dz) as usize * w + (x as isize + dx) as usize;
                        window.data[j] += moved * excess[n] / total;
                    }
                }
            }
        }
    }
}

/// Spacing, in metres, of the sediment smoothing: the change erosion made is
/// blurred at about this scale before it lands, so no feature of it is
/// narrower than a couple of metres. Erosion's own detail is gullies metres
/// across; anything finer is the grid showing through.
const SEDIMENT_SMOOTH_METRES: f32 = 2.0;

/// Erode `chunks` (all of one terrain): hydraulic erosion with `droplets`
/// raindrops, then `thermal_iterations` of thermal weathering. `area` limits it
/// to a disc in terrain-local metres (0..total width), raindrops landing inside
/// it and weathering fading out toward its edge; `None` is the whole terrain.
///
/// The one entry point the Erosion and Hydraulic brushes, the Generate panel's
/// Erode button and the MCP all use. Returns how many vertices moved.
pub fn erode(
    terrain: &TerrainData,
    chunks: &mut [&mut TerrainChunkData],
    area: Option<(Vec2, f32)>,
    droplets: u32,
    thermal_iterations: u32,
    repose_degrees: f32,
    seed: u32,
) -> usize {
    let (original, mut window) = {
        let refs: Vec<&TerrainChunkData> = chunks.iter().map(|c| &**c).collect();
        let gather = || match area {
            // A margin of a few cells: droplets roll out of the disc before
            // they stop, and must not fall off the window's edge to do it.
            Some((c, r)) => HeightWindow::around(terrain, &refs, c, r, 8),
            None => HeightWindow::gather_all(terrain, &refs),
        };
        (gather(), gather())
    };
    erode_window(&mut window, area, Hydraulic { droplets, seed, ..default() }, thermal_iterations, repose_degrees);
    window.scatter(terrain, &original, chunks)
}

/// The body of [`erode`] on a window that is not gathered from chunks: the
/// terrain graph's Erosion node runs this on its own heightfield, with its own
/// [`Hydraulic`] settings, and keeps the result rather than scattering it.
///
/// `area` is in the window's own metres (cell index times spacing, offset by
/// the window origin), exactly as [`erode`] reads it.
pub fn erode_window(
    window: &mut HeightWindow,
    area: Option<(Vec2, f32)>,
    settings: Hydraulic,
    thermal_iterations: u32,
    repose_degrees: f32,
) {
    let spacing = window.spacing;
    let start = area.map(|(c, r)| {
        let at = Vec2::new(c.x / spacing - window.x0 as f32, c.y / spacing - window.z0 as f32);
        (at, r / spacing)
    });
    if settings.droplets > 0 {
        let before = window.data.clone();
        let factor = coarse_factor(spacing, window.w.min(window.h));
        if factor > 1 {
            // Droplets on a coarser grid (see `DROPLET_CELL_METRES`), the
            // change they made carried back up to full resolution. The fine
            // ground's own detail is kept: only the difference is resampled.
            let mut coarse = window.downsample(factor);
            let coarse_before = coarse.data.clone();
            let start = start.map(|(c, r)| (c / factor as f32, r / factor as f32));
            hydraulic(&mut coarse, &settings, start);
            let delta: Vec<f32> = coarse.data.iter().zip(&coarse_before).map(|(a, b)| a - b).collect();
            window.add_upsampled(&delta, coarse.w, coarse.h, factor);
        } else {
            hydraulic(window, &settings, start);
        }
        // Sediment smoothing: blur the change, not the ground.
        let cells = ((SEDIMENT_SMOOTH_METRES / spacing).round() as usize).clamp(1, 4);
        let delta: Vec<f32> = window.data.iter().zip(&before).map(|(a, b)| a - b).collect();
        let smoothed = crate::sculpt::box_blur(&delta, window.w, window.h, cells, 2);
        for (h, (b, d)) in window.data.iter_mut().zip(before.iter().zip(&smoothed)) {
            *h = b + d;
        }
    }
    if thermal_iterations > 0 {
        let (x0, z0) = (window.x0, window.z0);
        thermal(window, repose_degrees, thermal_iterations, |x, z| match area {
            Some((c, r)) => {
                let p = Vec2::new((x0 + x) as f32, (z0 + z) as f32) * spacing;
                (1.0 - p.distance(c) / r.max(1e-3)).clamp(0.0, 1.0)
            }
            None => 1.0,
        });
    }
}

/// Grid spacing, in fine cells, the droplets of [`erode_window`] run at for a
/// window of this spacing and size: how many droplet cells a window has is
/// what a resolution independent rain amount is counted in.
pub fn droplet_factor(spacing: f32, cells: usize) -> usize {
    coarse_factor(spacing, cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> TerrainData {
        TerrainData { chunks_x: 2, chunks_z: 2, chunk_size: 16.0, chunk_resolution: 17, min_height: 0.0, max_height: 100.0, ..default() }
    }

    fn chunks(t: &TerrainData, f: impl Fn(f32, f32) -> f32) -> Vec<TerrainChunkData> {
        let mut out = Vec::new();
        for cz in 0..t.chunks_z {
            for cx in 0..t.chunks_x {
                let mut c = TerrainChunkData::new(cx, cz, t.chunk_resolution, 0.0);
                for z in 0..t.chunk_resolution {
                    for x in 0..t.chunk_resolution {
                        let wx = cx as f32 * t.chunk_size + x as f32 * t.vertex_spacing();
                        let wz = cz as f32 * t.chunk_size + z as f32 * t.vertex_spacing();
                        c.base_heights[(z * t.chunk_resolution + x) as usize] = f(wx, wz) / 100.0;
                    }
                }
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn a_window_reads_across_seams_and_writes_both_sides_of_them() {
        let t = terrain();
        let mut cs = chunks(&t, |x, _| x);
        let refs: Vec<&TerrainChunkData> = cs.iter().collect();
        let original = HeightWindow::gather_all(&t, &refs);
        assert_eq!((original.w, original.h), (33, 33));
        // Heights rise 1 m per metre along x, across the seam at x = 16.
        assert!((original.at(16, 3) - 16.0).abs() < 1e-3);
        assert!((original.at(20, 3) - 20.0).abs() < 1e-3);

        let mut edited = HeightWindow { data: original.data.clone(), ..HeightWindow::gather_all(&t, &refs) };
        edited.data[3 * 33 + 16] = 50.0;
        let mut muts: Vec<&mut TerrainChunkData> = cs.iter_mut().collect();
        assert_eq!(edited.scatter(&t, &original, &mut muts), 1);
        // The seam vertex lives in chunk (0,0) as x = 16 and chunk (1,0) as x = 0.
        let res = t.chunk_resolution;
        assert!((cs[0].get_base_height(16, 3, res) - 0.5).abs() < 1e-5);
        assert!((cs[1].get_base_height(0, 3, res) - 0.5).abs() < 1e-5);
        assert!(cs[0].dirty && cs[1].dirty);
    }

    #[test]
    fn rain_carves_a_slope_and_deposits_below_it() {
        let t = terrain();
        let cs = chunks(&t, |x, _| 40.0 - x * 0.8);
        let refs: Vec<&TerrainChunkData> = cs.iter().collect();
        let before = HeightWindow::gather_all(&t, &refs);
        let mut after = HeightWindow::gather_all(&t, &refs);
        hydraulic(&mut after, &Hydraulic { droplets: 4000, ..default() }, None);
        let changed = before.data.iter().zip(&after.data).filter(|(a, b)| (*a - *b).abs() > 0.01).count();
        assert!(changed > before.data.len() / 10, "only {changed} vertices moved");
        // Material is moved, not destroyed or invented wholesale.
        let (sb, sa): (f32, f32) = (before.data.iter().sum(), after.data.iter().sum());
        assert!((sb - sa).abs() < sb.abs() * 0.05, "{sb} -> {sa}");
    }

    #[test]
    fn a_cliff_settles_to_its_angle_of_repose() {
        let t = terrain();
        // 10 m: settled at 35 degrees it spreads over ~14 of the 33 cells.
        let cs = chunks(&t, |x, _| if x < 16.0 { 0.0 } else { 10.0 });
        let refs: Vec<&TerrainChunkData> = cs.iter().collect();
        let mut w = HeightWindow::gather_all(&t, &refs);
        thermal(&mut w, 35.0, 200, |_, _| 1.0);
        let steepest = (1..w.w)
            .map(|x| (w.at(x, 10) - w.at(x - 1, 10)).abs() / w.spacing)
            .fold(0.0f32, f32::max);
        assert!(steepest < 35f32.to_radians().tan() * 1.6, "still {steepest}");
    }

    /// A 256 m cone at 1 m spacing, 95 m high with 31 degree flanks, and
    /// droplets at the density of 500k over a 2 km terrain (one per eight
    /// vertices).
    fn cone() -> (TerrainData, Vec<TerrainChunkData>) {
        let t = TerrainData { chunks_x: 4, chunks_z: 4, chunk_size: 64.0, chunk_resolution: 65, min_height: 0.0, max_height: 100.0, ..default() };
        let cs = chunks(&t, |x, z| (95.0 - 0.6 * Vec2::new(x - 128.0, z - 128.0).length()).max(5.0));
        (t, cs)
    }

    const CONE_DROPLETS: u32 = 257 * 257 / 8;

    /// Striation across the slope: sample the change erosion made around
    /// circles on the cone's flank (perpendicular to the fall line) every
    /// half metre, and split it into what varies faster than 8 m along the
    /// circle (grooves, combs) and the total. Returns (fine rms, total rms).
    fn striation(before: &HeightWindow, after: &HeightWindow) -> (f32, f32) {
        let (mut fine, mut total, mut n) = (0.0f32, 0.0f32, 0.0f32);
        for radius in [35.0f32, 50.0, 65.0, 80.0] {
            let count = (std::f32::consts::TAU * radius / 0.5) as usize;
            let delta: Vec<f32> = (0..count)
                .map(|i| {
                    let a = i as f32 / count as f32 * std::f32::consts::TAU;
                    let p = Vec2::splat(128.0) + Vec2::new(a.cos(), a.sin()) * radius;
                    after.sample(p).0 - before.sample(p).0
                })
                .collect();
            let half = 8; // +-4 m of arc
            for i in 0..count {
                let mean = (0..=2 * half).map(|k| delta[(i + count + k - half) % count]).sum::<f32>() / (2 * half + 1) as f32;
                fine += (delta[i] - mean).powi(2);
                total += delta[i].powi(2);
                n += 1.0;
            }
        }
        ((fine / n).sqrt(), (total / n).sqrt())
    }

    #[test]
    fn eroding_a_cone_leaves_gullies_not_a_comb() {
        let (t, mut cs) = cone();
        let before = HeightWindow::gather_all(&t, &cs.iter().collect::<Vec<_>>());

        // The old shape of the process, for comparison: droplets on the fine
        // grid with a 3 m brush, nothing smoothed.
        let mut naive = HeightWindow::gather_all(&t, &cs.iter().collect::<Vec<_>>());
        let old = Hydraulic { droplets: CONE_DROPLETS, erosion_radius: 3.0, inertia: 0.05, evaporate: 0.02, lifetime: 40, seed: 3, ..default() };
        hydraulic(&mut naive, &old, None);
        thermal(&mut naive, 36.0, 10, |_, _| 1.0);
        let (naive_fine, naive_total) = striation(&before, &naive);

        let mut muts: Vec<&mut TerrainChunkData> = cs.iter_mut().collect();
        erode(&t, &mut muts, None, CONE_DROPLETS, 10, 36.0, 3);
        let after = HeightWindow::gather_all(&t, &cs.iter().collect::<Vec<_>>());
        let (fine, total) = striation(&before, &after);

        println!(
            "cross-slope change: default erosion {fine:.3} m fine of {total:.3} m total ({:.0}%); \
             fine-grid 3 m brush {naive_fine:.3} m of {naive_total:.3} m ({:.0}%)",
            100.0 * fine / total,
            100.0 * naive_fine / naive_total
        );
        assert!(total > 0.05, "erosion did almost nothing: {total} m");
        // As shares of the change each made: the default run moves an order
        // of magnitude more ground, so absolute numbers would flatter the old
        // one.
        assert!(fine < total * 0.25, "striated: {fine} of {total}");
        assert!(fine / total < 0.4 * naive_fine / naive_total, "{fine}/{total} vs {naive_fine}/{naive_total}");
    }

    /// Valleys fill: the sediment lands at the foot of the cone, not nowhere.
    #[test]
    fn eroding_a_cone_deposits_at_its_foot() {
        let (t, mut cs) = cone();
        let before = HeightWindow::gather_all(&t, &cs.iter().collect::<Vec<_>>());
        let mut muts: Vec<&mut TerrainChunkData> = cs.iter_mut().collect();
        erode(&t, &mut muts, None, CONE_DROPLETS, 0, 36.0, 5);
        let after = HeightWindow::gather_all(&t, &cs.iter().collect::<Vec<_>>());
        let (mut foot, mut flank) = (0.0f32, 0.0f32);
        for z in 0..after.h {
            for x in 0..after.w {
                let d = Vec2::new(x as f32 - 128.0, z as f32 - 128.0).length();
                let change = after.at(x, z) - before.at(x, z);
                if (140.0..=152.0).contains(&d) {
                    foot += change;
                } else if (30.0..=120.0).contains(&d) {
                    flank += change;
                }
            }
        }
        assert!(flank < 0.0, "the flanks did not erode: {flank}");
        assert!(foot > 0.0, "nothing deposited at the foot: {foot}");
    }
}

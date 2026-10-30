//! Painted layer weights: storage, the brush, and coverage.
//!
//! Every chunk of a terrain with a landscape material carries a
//! [`TerrainChunkWeights`]: a square grid of [`WEIGHT_CHANNELS`] bytes per
//! texel over the chunk, cell-centred (texel `i` covers `i..i+1` of
//! `chunk_size / resolution` metres), so neighbouring chunks' grids butt up
//! against each other with no shared row and the whole terrain is one even
//! grid. That is what lets the GPU copy be a single atlas the shader filters
//! straight across chunk seams.
//!
//! The channels of one texel always sum to 255: painting a layer raises it
//! and lowers the others in proportion, so every point is fully covered by
//! some mix of layers and nothing has to be renormalised at draw time.
//!
//! A chunk nobody has painted holds no texels at all and reads as channel 0
//! everywhere, which is the material's first layer. Saved, its weights are a
//! short string (run-length encoded per channel, then base64), so an
//! unpainted chunk costs a few bytes of scene and a painted one a fraction of
//! what its raw bytes would as a list of numbers.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::{compute_brush_falloff, BrushFalloffType, BrushShape, TerrainChunkData, TerrainData};
use crate::paint::PaintBrushType;

/// Weight channels a terrain stores: two RGBA8 slices on the GPU.
pub const WEIGHT_CHANNELS: usize = 8;

/// One texel's weights, 0..255 each, summing to 255.
pub type Texel = [u8; WEIGHT_CHANNELS];

/// A texel that is all channel `c`.
pub fn solid(c: usize) -> Texel {
    let mut t = [0u8; WEIGHT_CHANNELS];
    t[c.min(WEIGHT_CHANNELS - 1)] = 255;
    t
}

/// Weight texels along one side of a chunk.
///
/// One per height cell (`chunk_resolution - 1`), so a texel is the size of
/// the ground's own triangles: half a metre on a default terrain. Capped so
/// the whole terrain's atlas stays inside the 8192 texels a GPU texture is
/// guaranteed to allow.
pub fn weight_resolution(terrain: &TerrainData) -> u32 {
    let cells = terrain.chunk_resolution.saturating_sub(1).max(4);
    let most = terrain.chunks_x.max(terrain.chunks_z).max(1);
    cells.min(128).min((8192 / most).max(1))
}

/// A chunk's painted weights. See the module docs.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct TerrainChunkWeights {
    /// What the scene saves: `"<resolution>:<base64 of the per-channel
    /// runs>"`, or empty for a chunk that was never painted. Kept current by
    /// [`TerrainChunkWeights::encode`] after every change, so a save never has
    /// to ask.
    #[serde(default)]
    #[reflect(default)]
    pub encoded: String,
    /// Texels along one side; 0 until decoded or first painted.
    #[serde(skip)]
    #[reflect(ignore)]
    pub resolution: u32,
    /// Row-major texels (`z * resolution + x`); empty means all channel 0.
    #[serde(skip)]
    #[reflect(ignore)]
    pub texels: Vec<Texel>,
    /// Texel rectangle `[min_x, min_z, max_x, max_z]` (inclusive) changed
    /// since the GPU copy was last updated.
    #[serde(skip)]
    #[reflect(ignore)]
    pub gpu_dirty: Option<[u32; 4]>,
    /// The texels changed since `encoded` and `sums` were last brought up to
    /// date.
    #[serde(skip)]
    #[reflect(ignore)]
    pub stale: bool,
    /// Per-channel totals over the chunk, for coverage. Valid while `stale` is
    /// false.
    #[serde(skip)]
    #[reflect(ignore)]
    pub sums: [u64; WEIGHT_CHANNELS],
}

impl TerrainChunkWeights {
    /// Decode `encoded` if the texels have not been read yet, at
    /// `resolution` (resampled if the terrain's resolution has changed since
    /// it was saved). Returns whether texels exist after the call.
    pub fn ensure_decoded(&mut self, resolution: u32) -> bool {
        if self.resolution == resolution && !self.texels.is_empty() {
            return true;
        }
        if self.texels.is_empty() && !self.encoded.is_empty() {
            if let Some((res, texels)) = decode(&self.encoded) {
                self.resolution = res;
                self.texels = texels;
            }
        }
        if !self.texels.is_empty() && self.resolution != resolution {
            self.texels = resample(&self.texels, self.resolution, resolution);
            self.resolution = resolution;
            self.stale = true;
            self.gpu_dirty = Some([0, 0, resolution - 1, resolution - 1]);
        }
        if self.texels.is_empty() {
            self.resolution = resolution;
        }
        !self.texels.is_empty()
    }

    /// Give the chunk texels to paint into: all `base` if it had none.
    pub fn ensure_texels(&mut self, resolution: u32, base: usize) {
        if !self.ensure_decoded(resolution) {
            self.texels = vec![solid(base); (resolution * resolution) as usize];
            self.resolution = resolution;
        }
    }

    /// The texel at `(x, z)`, channel 0 when unpainted.
    pub fn texel(&self, x: u32, z: u32) -> Texel {
        self.texels
            .get((z * self.resolution + x) as usize)
            .copied()
            .unwrap_or_else(|| solid(0))
    }

    /// Grow the GPU dirty rectangle to take in `rect`.
    pub fn mark_dirty(&mut self, rect: [u32; 4]) {
        self.gpu_dirty = Some(match self.gpu_dirty {
            Some([a, b, c, d]) => [a.min(rect[0]), b.min(rect[1]), c.max(rect[2]), d.max(rect[3])],
            None => rect,
        });
        self.stale = true;
    }

    /// Bring `encoded` and `sums` up to date with the texels.
    pub fn encode(&mut self) {
        self.encoded = if self.texels.is_empty() { String::new() } else { encode(self.resolution, &self.texels) };
        self.sums = channel_sums(self.resolution, &self.texels);
        self.stale = false;
    }

    /// Replace the texels wholesale (undo, migration) and mark everything
    /// dirty.
    pub fn set_texels(&mut self, resolution: u32, texels: Vec<Texel>) {
        self.resolution = resolution;
        self.texels = texels;
        self.gpu_dirty = Some([0, 0, resolution.saturating_sub(1), resolution.saturating_sub(1)]);
        self.encode();
    }
}

/// Per-channel totals; an unpainted chunk is all channel 0.
pub fn channel_sums(resolution: u32, texels: &[Texel]) -> [u64; WEIGHT_CHANNELS] {
    let mut sums = [0u64; WEIGHT_CHANNELS];
    if texels.is_empty() {
        sums[0] = 255 * (resolution as u64).pow(2);
        return sums;
    }
    for t in texels {
        for c in 0..WEIGHT_CHANNELS {
            sums[c] += t[c] as u64;
        }
    }
    sums
}

/// Nearest-neighbour resample of a texel grid.
pub fn resample(texels: &[Texel], from: u32, to: u32) -> Vec<Texel> {
    let mut out = Vec::with_capacity((to * to) as usize);
    for z in 0..to {
        for x in 0..to {
            let sx = ((x as f32 + 0.5) * from as f32 / to as f32) as u32;
            let sz = ((z as f32 + 0.5) * from as f32 / to as f32) as u32;
            let i = (sz.min(from - 1) * from + sx.min(from - 1)) as usize;
            out.push(texels.get(i).copied().unwrap_or_else(|| solid(0)));
        }
    }
    out
}

// ── Normalisation ────────────────────────────────────────────────────────────

/// Bytes to fractions.
pub fn unpack(t: Texel) -> [f32; WEIGHT_CHANNELS] {
    t.map(|b| b as f32 / 255.0)
}

/// Fractions to bytes that sum to exactly 255. Rounding error lands on the
/// largest channel, where it is least visible. All zero becomes `base`.
pub fn pack(w: [f32; WEIGHT_CHANNELS], base: usize) -> Texel {
    let total: f32 = w.iter().map(|v| v.max(0.0)).sum();
    if total <= 1e-6 {
        return solid(base);
    }
    let mut out = [0u8; WEIGHT_CHANNELS];
    let mut sum = 0i32;
    let mut largest = 0;
    for c in 0..WEIGHT_CHANNELS {
        let v = (w[c].max(0.0) / total * 255.0).round().clamp(0.0, 255.0) as u8;
        out[c] = v;
        sum += v as i32;
        if w[c] > w[largest] {
            largest = c;
        }
    }
    out[largest] = (out[largest] as i32 + 255 - sum).clamp(0, 255) as u8;
    out
}

/// Move channel `c` toward `target` by `amount` (0..1) and take what it gains
/// from the others, or give what it loses to them, in proportion to what they
/// hold. With nothing else present, lowering hands the weight to `base`
/// (painting the base layer down with nothing beside it changes nothing).
pub fn push_toward(w: &mut [f32; WEIGHT_CHANNELS], c: usize, target: f32, amount: f32, base: usize) {
    let amount = amount.clamp(0.0, 1.0);
    let target = target.clamp(0.0, 1.0);
    let next = w[c] + (target - w[c]) * amount;
    let others: f32 = (0..WEIGHT_CHANNELS).filter(|&i| i != c).map(|i| w[i]).sum();
    let rest = (1.0 - next).max(0.0);
    if others > 1e-6 {
        let scale = rest / others;
        for (i, v) in w.iter_mut().enumerate() {
            if i != c {
                *v *= scale;
            }
        }
        w[c] = next;
    } else if base != c {
        w[c] = next;
        w[base] = rest;
    }
    // else: channel `c` is all there is and it is the base; it stays whole.
}

// ── Brush ────────────────────────────────────────────────────────────────────

/// One dab of the weight brush.
#[derive(Clone, Debug)]
pub struct WeightDab {
    pub mode: PaintBrushType,
    /// Centre in map metres: from the terrain's corner (its minimum X/Z), so
    /// `0..total_width` across.
    pub center: Vec2,
    /// Radius in metres.
    pub radius: f32,
    /// 0..1: how far this dab moves a texel at the brush centre toward its
    /// result. Already scaled by frame time by the caller.
    pub amount: f32,
    /// 0..1: the weight Paint and Fill aim the layer at.
    pub target: f32,
    pub falloff: f32,
    pub falloff_type: BrushFalloffType,
    pub shape: BrushShape,
    /// The weight channel being painted.
    pub channel: usize,
    /// The channel that takes weight nothing else can (the material's first
    /// layer's).
    pub base: usize,
    /// Only where the ground's slope is inside this range, in degrees.
    pub slope_mask: Option<Vec2>,
    /// Only where the ground's world height is inside this range, in metres.
    pub height_mask: Option<Vec2>,
    /// The terrain root's world height, for the height mask.
    pub root_y: f32,
}

/// 1 inside `lo..hi`, 0 outside, with a short fade either side.
fn in_range(x: f32, range: Vec2, fade: f32) -> f32 {
    let lo = ((x - range.x) / fade + 0.5).clamp(0.0, 1.0);
    let hi = ((range.y - x) / fade + 0.5).clamp(0.0, 1.0);
    lo * hi
}

/// Slope in degrees and world height of the ground at chunk-local metres `p`.
fn ground_at(terrain: &TerrainData, chunk: &TerrainChunkData, p: Vec2, root_y: f32) -> (f32, f32) {
    let res = terrain.chunk_resolution;
    let spacing = terrain.vertex_spacing();
    let heights = if chunk.heights.is_empty() { &chunk.base_heights } else { &chunk.heights };
    let sample = |x: f32, z: f32| -> f32 {
        let gx = (x / spacing).clamp(0.0, (res - 1) as f32);
        let gz = (z / spacing).clamp(0.0, (res - 1) as f32);
        let (x0, z0) = (gx.floor() as u32, gz.floor() as u32);
        let (x1, z1) = ((x0 + 1).min(res - 1), (z0 + 1).min(res - 1));
        let (tx, tz) = (gx - x0 as f32, gz - z0 as f32);
        let h = |x: u32, z: u32| heights.get((z * res + x) as usize).copied().unwrap_or(0.0);
        let top = h(x0, z0) + (h(x1, z0) - h(x0, z0)) * tx;
        let bottom = h(x0, z1) + (h(x1, z1) - h(x0, z1)) * tx;
        (top + (bottom - top) * tz) * terrain.height_range() + terrain.min_height
    };
    let y = sample(p.x, p.y);
    let dx = (sample(p.x + spacing, p.y) - sample(p.x - spacing, p.y)) / (2.0 * spacing);
    let dz = (sample(p.x, p.y + spacing) - sample(p.x, p.y - spacing)) / (2.0 * spacing);
    (dx.hypot(dz).atan().to_degrees(), y + root_y)
}

/// Distance from the brush centre as a fraction of the radius, by shape.
fn shape_distance(shape: BrushShape, d: Vec2, radius: f32) -> f32 {
    let d = d / radius.max(1e-4);
    match shape {
        BrushShape::Circle => d.length(),
        BrushShape::Square => d.x.abs().max(d.y.abs()),
        BrushShape::Diamond => d.x.abs() + d.y.abs(),
    }
}

/// The chunks of one terrain the brush can reach, with their heights for the
/// masks.
pub struct WeightTarget<'a> {
    pub chunk: &'a TerrainChunkData,
    pub weights: &'a mut TerrainChunkWeights,
}

/// Apply one dab across every chunk it reaches, seams included. Returns
/// whether any texel changed.
///
/// Works in whole-terrain texel coordinates: Smooth reads its neighbours from
/// whichever chunk holds them, so it blurs across a chunk edge exactly as it
/// does inside one.
pub fn apply_weight_dab(terrain: &TerrainData, chunks: &mut [WeightTarget], dab: &WeightDab) -> bool {
    let res = weight_resolution(terrain);
    let cell = terrain.chunk_size / res as f32;
    let (wide, deep) = (terrain.chunks_x * res, terrain.chunks_z * res);
    let r = dab.radius.max(cell * 0.5);
    let lo_x = ((dab.center.x - r) / cell - 0.5).floor().max(0.0) as i64;
    let hi_x = ((dab.center.x + r) / cell - 0.5).ceil().min(wide as f32 - 1.0) as i64;
    let lo_z = ((dab.center.y - r) / cell - 0.5).floor().max(0.0) as i64;
    let hi_z = ((dab.center.y + r) / cell - 0.5).ceil().min(deep as f32 - 1.0) as i64;
    if lo_x > hi_x || lo_z > hi_z {
        return false;
    }

    let index: std::collections::HashMap<(u32, u32), usize> = chunks
        .iter()
        .enumerate()
        .map(|(i, t)| ((t.chunk.chunk_x, t.chunk.chunk_z), i))
        .collect();
    let read = |chunks: &[WeightTarget], gx: i64, gz: i64| -> Option<[f32; WEIGHT_CHANNELS]> {
        if gx < 0 || gz < 0 || gx >= wide as i64 || gz >= deep as i64 {
            return None;
        }
        let (cx, cz) = (gx as u32 / res, gz as u32 / res);
        let i = *index.get(&(cx, cz))?;
        Some(unpack(chunks[i].weights.texel(gx as u32 % res, gz as u32 % res)))
    };

    // Smooth reads a snapshot, so the blur does not run along the scan.
    let snapshot: Option<Vec<Option<[f32; WEIGHT_CHANNELS]>>> = (dab.mode == PaintBrushType::Smooth).then(|| {
        let mut v = Vec::new();
        for gz in (lo_z - 1)..=(hi_z + 1) {
            for gx in (lo_x - 1)..=(hi_x + 1) {
                v.push(read(chunks, gx, gz));
            }
        }
        v
    });
    let snap_w = (hi_x - lo_x + 3) as usize;
    let snap_at = |gx: i64, gz: i64| -> Option<[f32; WEIGHT_CHANNELS]> {
        let s = snapshot.as_ref()?;
        s[((gz - lo_z + 1) as usize) * snap_w + (gx - lo_x + 1) as usize]
    };

    let mut changed = false;
    for gz in lo_z..=hi_z {
        for gx in lo_x..=hi_x {
            let centre = Vec2::new((gx as f32 + 0.5) * cell, (gz as f32 + 0.5) * cell);
            let t = shape_distance(dab.shape, centre - dab.center, r);
            if t >= 1.0 {
                continue;
            }
            let mut f = compute_brush_falloff(t, dab.falloff.clamp(0.0, 1.0), dab.falloff_type);
            if f <= 0.0 {
                continue;
            }
            let (cx, cz) = (gx as u32 / res, gz as u32 / res);
            let Some(&i) = index.get(&(cx, cz)) else { continue };
            let (lx, lz) = (gx as u32 % res, gz as u32 % res);
            if dab.slope_mask.is_some() || dab.height_mask.is_some() {
                let local = Vec2::new((lx as f32 + 0.5) * cell, (lz as f32 + 0.5) * cell);
                let (slope, height) = ground_at(terrain, chunks[i].chunk, local, dab.root_y);
                if let Some(range) = dab.slope_mask {
                    f *= in_range(slope, range, 1.5);
                }
                if let Some(range) = dab.height_mask {
                    f *= in_range(height, range, 1.5);
                }
                if f <= 0.0 {
                    continue;
                }
            }

            let target = &mut chunks[i];
            // Unpainted ground is channel 0 (see the module docs), whatever
            // the base is.
            target.weights.ensure_texels(res, 0);
            let idx = (lz * res + lx) as usize;
            let before = target.weights.texels[idx];
            let mut w = unpack(before);
            match dab.mode {
                PaintBrushType::Paint => push_toward(&mut w, dab.channel, dab.target, dab.amount * f, dab.base),
                PaintBrushType::Erase => push_toward(&mut w, dab.channel, 0.0, dab.amount * f, dab.base),
                // Fill lays the layer straight down at the target weight, with
                // only the falloff to soften its edge.
                PaintBrushType::Fill => push_toward(&mut w, dab.channel, dab.target, f, dab.base),
                PaintBrushType::Smooth => {
                    let mut avg = [0.0f32; WEIGHT_CHANNELS];
                    let mut n = 0.0f32;
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            if let Some(v) = snap_at(gx + dx, gz + dz) {
                                for c in 0..WEIGHT_CHANNELS {
                                    avg[c] += v[c];
                                }
                                n += 1.0;
                            }
                        }
                    }
                    let k = (dab.amount * f).clamp(0.0, 1.0);
                    for c in 0..WEIGHT_CHANNELS {
                        w[c] += (avg[c] / n.max(1.0) - w[c]) * k;
                    }
                }
            }
            let after = pack(w, dab.base);
            if after != before {
                target.weights.texels[idx] = after;
                target.weights.mark_dirty([lx, lz, lx, lz]);
                changed = true;
            }
        }
    }
    changed
}

// ── Encoding ─────────────────────────────────────────────────────────────────

fn push_varint(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn read_varint(bytes: &[u8], at: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    for shift in (0..35).step_by(7) {
        let b = *bytes.get(*at)?;
        *at += 1;
        v |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// `"<resolution>:<base64>"` of each channel's runs in turn, each run a value
/// byte then its length as a varint. A channel is mostly one value over large
/// areas (zero where its layer was never painted), so per-channel runs are far
/// shorter than per-texel ones.
pub fn encode(resolution: u32, texels: &[Texel]) -> String {
    let mut raw = Vec::new();
    for c in 0..WEIGHT_CHANNELS {
        let mut i = 0;
        while i < texels.len() {
            let v = texels[i][c];
            let mut j = i + 1;
            while j < texels.len() && texels[j][c] == v {
                j += 1;
            }
            raw.push(v);
            push_varint(&mut raw, (j - i) as u32);
            i = j;
        }
    }
    format!("{resolution}:{}", base64_encode(&raw))
}

/// The inverse of [`encode`]; `None` for anything malformed.
pub fn decode(s: &str) -> Option<(u32, Vec<Texel>)> {
    let (res, body) = s.split_once(':')?;
    let res: u32 = res.parse().ok()?;
    let count = (res as usize).checked_mul(res as usize)?;
    if count == 0 || count > 8192 * 8192 {
        return None;
    }
    let raw = base64_decode(body)?;
    let mut texels = vec![[0u8; WEIGHT_CHANNELS]; count];
    let mut at = 0;
    for c in 0..WEIGHT_CHANNELS {
        let mut i = 0;
        while i < count {
            let v = *raw.get(at)?;
            at += 1;
            let n = read_varint(&raw, &mut at)? as usize;
            if n == 0 || i + n > count {
                return None;
            }
            for t in &mut texels[i..i + n] {
                t[c] = v;
            }
            i += n;
        }
    }
    Some((res, texels))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let bytes = s.trim().as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for quad in bytes.chunks(4) {
        let pad = quad.iter().rev().take_while(|&&c| c == b'=').count();
        let mut n = 0u32;
        for &c in &quad[..4 - pad] {
            n = n << 6 | value(c)?;
        }
        n <<= 6 * pad as u32;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> TerrainData {
        TerrainData { chunks_x: 2, chunks_z: 1, chunk_size: 16.0, chunk_resolution: 17, ..default() }
    }

    fn dab(mode: PaintBrushType, center: Vec2, radius: f32) -> WeightDab {
        WeightDab {
            mode,
            center,
            radius,
            amount: 1.0,
            target: 1.0,
            falloff: 0.0,
            falloff_type: BrushFalloffType::Flat,
            shape: BrushShape::Circle,
            channel: 1,
            base: 0,
            slope_mask: None,
            height_mask: None,
            root_y: 0.0,
        }
    }

    #[test]
    fn packed_weights_always_sum_to_255() {
        for w in [[0.3, 0.3, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0], [1.0 / 3.0; 8], [0.0; 8], [0.9, 0.05, 0.05, 0.0, 0.0, 0.0, 0.0, 0.0]] {
            let t = pack(w, 2);
            assert_eq!(t.iter().map(|&b| b as u32).sum::<u32>(), 255, "{w:?} -> {t:?}");
        }
        assert_eq!(pack([0.0; 8], 2), solid(2));
    }

    #[test]
    fn painting_raises_one_layer_and_lowers_the_others_in_proportion() {
        let mut w = [0.5, 0.3, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0];
        push_toward(&mut w, 1, 1.0, 0.5, 0);
        assert!((w[1] - 0.65).abs() < 1e-5, "{w:?}");
        // The others keep their 5:2 ratio and the total stays one.
        assert!((w[0] / w[2] - 2.5).abs() < 1e-4, "{w:?}");
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        // Lowering the only layer hands its weight to the base.
        let mut solo = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        push_toward(&mut solo, 1, 0.0, 0.25, 0);
        assert!((solo[0] - 0.25).abs() < 1e-5 && (solo[1] - 0.75).abs() < 1e-5, "{solo:?}");
        // The base alone cannot be painted below whole.
        let mut base = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        push_toward(&mut base, 0, 0.0, 1.0, 0);
        assert_eq!(base[0], 1.0);
    }

    #[test]
    fn a_dab_paints_across_the_seam_and_nowhere_else() {
        let t = terrain();
        let res = weight_resolution(&t);
        assert_eq!(res, 16);
        let chunks_data: Vec<TerrainChunkData> =
            (0..2).map(|x| TerrainChunkData::new(x, 0, t.chunk_resolution, 0.2)).collect();
        let mut weights = vec![TerrainChunkWeights::default(), TerrainChunkWeights::default()];
        {
            let mut targets: Vec<WeightTarget> = chunks_data
                .iter()
                .zip(weights.iter_mut())
                .map(|(chunk, weights)| WeightTarget { chunk, weights })
                .collect();
            // On the seam between the two chunks (x = 16 m).
            assert!(apply_weight_dab(&t, &mut targets, &dab(PaintBrushType::Fill, Vec2::new(16.0, 8.0), 2.0)));
        }
        // Both sides of the seam were painted, and far away was not.
        let left = weights[0].texel(15, 8);
        let right = weights[1].texel(0, 8);
        assert_eq!(left, solid(1));
        assert_eq!(right, solid(1));
        assert_eq!(weights[0].texel(0, 0), solid(0));
        assert!(weights[0].gpu_dirty.is_some() && weights[1].gpu_dirty.is_some());
    }

    #[test]
    fn erase_and_smooth_keep_texels_normalised() {
        let t = terrain();
        let chunks_data: Vec<TerrainChunkData> =
            (0..2).map(|x| TerrainChunkData::new(x, 0, t.chunk_resolution, 0.2)).collect();
        let mut weights = vec![TerrainChunkWeights::default(), TerrainChunkWeights::default()];
        let mut targets: Vec<WeightTarget> = chunks_data
            .iter()
            .zip(weights.iter_mut())
            .map(|(chunk, weights)| WeightTarget { chunk, weights })
            .collect();
        apply_weight_dab(&t, &mut targets, &dab(PaintBrushType::Fill, Vec2::new(8.0, 8.0), 4.0));
        apply_weight_dab(&t, &mut targets, &dab(PaintBrushType::Smooth, Vec2::new(8.0, 8.0), 6.0));
        let mut half = dab(PaintBrushType::Erase, Vec2::new(8.0, 8.0), 6.0);
        half.amount = 0.5;
        apply_weight_dab(&t, &mut targets, &half);
        for w in &weights {
            for texel in &w.texels {
                assert_eq!(texel.iter().map(|&b| b as u32).sum::<u32>(), 255, "{texel:?}");
            }
        }
    }

    #[test]
    fn the_slope_mask_keeps_paint_off_flat_ground() {
        let t = terrain();
        let chunks_data: Vec<TerrainChunkData> =
            (0..2).map(|x| TerrainChunkData::new(x, 0, t.chunk_resolution, 0.2)).collect();
        let mut weights = vec![TerrainChunkWeights::default(), TerrainChunkWeights::default()];
        let mut targets: Vec<WeightTarget> = chunks_data
            .iter()
            .zip(weights.iter_mut())
            .map(|(chunk, weights)| WeightTarget { chunk, weights })
            .collect();
        let mut steep_only = dab(PaintBrushType::Fill, Vec2::new(8.0, 8.0), 4.0);
        steep_only.slope_mask = Some(Vec2::new(30.0, 90.0));
        assert!(!apply_weight_dab(&t, &mut targets, &steep_only));
    }

    #[test]
    fn weights_round_trip_through_their_saved_form() {
        let res = 16;
        let mut texels = vec![solid(0); (res * res) as usize];
        for (i, t) in texels.iter_mut().enumerate().take(40) {
            *t = pack([0.5, (i as f32) / 40.0, 0.1, 0.0, 0.0, 0.0, 0.0, 0.2], 0);
        }
        let s = encode(res, &texels);
        assert_eq!(decode(&s), Some((res, texels.clone())));
        // An unpainted chunk is a handful of bytes.
        assert!(encode(res, &vec![solid(0); 256]).len() < 40);
        assert_eq!(decode("garbage"), None);
        assert_eq!(decode("4:%%%%"), None);
    }

    #[test]
    fn an_unpainted_chunk_covers_its_base_layer() {
        let sums = channel_sums(4, &[]);
        assert_eq!(sums[0], 255 * 16);
        assert_eq!(sums[1..].iter().sum::<u64>(), 0);
    }

    /// The old overlay painter spent 484 ms a frame on a large terrain. A dab
    /// here touches only the texels under it: a 40 m brush over a 16 x 16
    /// chunk terrain of 64 m chunks must stay far inside one frame.
    #[test]
    fn a_large_dab_on_a_large_terrain_is_fast() {
        let t = TerrainData { chunks_x: 16, chunks_z: 16, chunk_size: 64.0, chunk_resolution: 129, ..default() };
        let chunks_data: Vec<TerrainChunkData> = (0..16 * 16)
            .map(|i| TerrainChunkData::new(i % 16, i / 16, t.chunk_resolution, 0.2))
            .collect();
        let mut weights: Vec<TerrainChunkWeights> = (0..chunks_data.len()).map(|_| TerrainChunkWeights::default()).collect();
        let mut targets: Vec<WeightTarget> = chunks_data
            .iter()
            .zip(weights.iter_mut())
            .map(|(chunk, weights)| WeightTarget { chunk, weights })
            .collect();
        let mut d = dab(PaintBrushType::Paint, Vec2::new(512.0, 512.0), 40.0);
        d.amount = 0.2;
        d.slope_mask = Some(Vec2::new(0.0, 45.0));
        let start = std::time::Instant::now();
        for _ in 0..10 {
            apply_weight_dab(&t, &mut targets, &d);
        }
        let per_dab = start.elapsed() / 10;
        assert!(per_dab.as_millis() < 30, "{per_dab:?} per dab");
    }
}

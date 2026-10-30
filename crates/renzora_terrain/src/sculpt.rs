#![allow(unused_variables)]

//! Terrain sculpt brush algorithms — 16 brush types with noise utilities.
//!
//! Pure data operations on heightmaps. No systems here — those live in the editor crate.

use bevy::math::Vec2;

use crate::data::*;
use crate::erosion::HeightWindow;

// ── Noise Utilities ──────────────────────────────────────────────────────────

/// Murmur-style integer hash.
#[inline]
fn hash_u32(x: u32) -> u32 {
    let mut h = x.wrapping_mul(2747636419);
    h ^= h >> 16;
    h = h.wrapping_mul(2246822519);
    h ^= h >> 13;
    h = h.wrapping_mul(3266489917);
    h ^= h >> 16;
    h
}

/// 2D hash returning a value in [0, 1].
#[inline]
fn hash2d(x: i32, y: i32, seed: u32) -> f32 {
    let hx = hash_u32(x as u32 ^ seed);
    let hy = hash_u32(y as u32 ^ seed.wrapping_add(0xDEAD_BEEF));
    let h = hash_u32(hx.wrapping_add(hy));
    (h as f32) / (u32::MAX as f32)
}

/// Value noise with smoothstep interpolation.
#[inline]
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    // Not `fract()`: it keeps the sign, so below zero it disagreed with
    // `floor` and the noise jumped at every lattice line.
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);

    let a = hash2d(ix, iy, seed);
    let b = hash2d(ix + 1, iy, seed);
    let c = hash2d(ix, iy + 1, seed);
    let d = hash2d(ix + 1, iy + 1, seed);

    let h0 = a + (b - a) * ux;
    let h1 = c + (d - c) * ux;
    h0 + (h1 - h0) * uy
}

/// Fractal Brownian Motion: layered value noise. Returns roughly [0, 1].
pub fn fbm(x: f32, y: f32, octaves: u32, lacunarity: f32, persistence: f32, seed: u32) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max_amp = 0.0f32;

    for i in 0..octaves {
        let oct_seed = seed.wrapping_add(i.wrapping_mul(12_345));
        value += value_noise(x * frequency, y * frequency, oct_seed) * amplitude;
        max_amp += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    if max_amp > 0.0 {
        value / max_amp
    } else {
        0.0
    }
}

/// Ridge noise: sharp mountain ridges via `1 - |signed_noise|`.
pub fn ridge_noise(
    x: f32,
    y: f32,
    octaves: u32,
    lacunarity: f32,
    persistence: f32,
    seed: u32,
) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max_amp = 0.0f32;

    for i in 0..octaves {
        let oct_seed = seed.wrapping_add(i.wrapping_mul(12_345));
        let n = value_noise(x * frequency, y * frequency, oct_seed) * 2.0 - 1.0;
        value += (1.0 - n.abs()) * amplitude;
        max_amp += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    if max_amp > 0.0 {
        value / max_amp
    } else {
        0.0
    }
}

/// Billow noise: rounded puffy terrain via `|signed_noise|`.
pub fn billow_noise(
    x: f32,
    y: f32,
    octaves: u32,
    lacunarity: f32,
    persistence: f32,
    seed: u32,
) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max_amp = 0.0f32;

    for i in 0..octaves {
        let oct_seed = seed.wrapping_add(i.wrapping_mul(12_345));
        let n = value_noise(x * frequency, y * frequency, oct_seed) * 2.0 - 1.0;
        value += n.abs() * amplitude;
        max_amp += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    if max_amp > 0.0 {
        value / max_amp
    } else {
        0.0
    }
}

/// Domain-warped FBM: distort input coords by another noise field.
pub fn warped_fbm(
    x: f32,
    y: f32,
    octaves: u32,
    lacunarity: f32,
    persistence: f32,
    seed: u32,
    warp_strength: f32,
) -> f32 {
    let warp_seed = seed.wrapping_add(999);
    let wx = fbm(x, y, octaves.min(3), lacunarity, persistence, warp_seed) * 2.0 - 1.0;
    let wy = fbm(
        x + 5.2,
        y + 1.3,
        octaves.min(3),
        lacunarity,
        persistence,
        warp_seed.wrapping_add(1),
    ) * 2.0
        - 1.0;
    fbm(
        x + wx * warp_strength,
        y + wy * warp_strength,
        octaves,
        lacunarity,
        persistence,
        seed,
    )
}

/// Hybrid: blend ridge noise with FBM for natural mountain terrain.
pub fn hybrid_noise(
    x: f32,
    y: f32,
    octaves: u32,
    lacunarity: f32,
    persistence: f32,
    seed: u32,
) -> f32 {
    let r = ridge_noise(x, y, octaves, lacunarity, persistence, seed);
    let f = fbm(x, y, octaves, lacunarity, persistence, seed);
    r * 0.6 + f * 0.4
}

/// Value noise centred on 0 (-1..1), with its gradient.
///
/// Quintic interpolation rather than the smoothstep of [`value_noise`]: the
/// derivative damping in [`eroded_noise`] reads the gradient, and smoothstep's
/// gradient has a crease at every lattice line that shows up as a grid of
/// ridges in the damped detail.
#[inline]
fn value_noise_d(x: f32, y: f32, seed: u32) -> (f32, f32, f32) {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let ux = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0);
    let uy = fy * fy * fy * (fy * (fy * 6.0 - 15.0) + 10.0);
    let dux = 30.0 * fx * fx * (fx * (fx - 2.0) + 1.0);
    let duy = 30.0 * fy * fy * (fy * (fy - 2.0) + 1.0);

    let a = hash2d(ix, iy, seed);
    let b = hash2d(ix + 1, iy, seed);
    let c = hash2d(ix, iy + 1, seed);
    let d = hash2d(ix + 1, iy + 1, seed);
    let (k1, k2, k4) = (b - a, c - a, a - b - c + d);

    let v = a + k1 * ux + k2 * uy + k4 * ux * uy;
    (
        v * 2.0 - 1.0,
        2.0 * dux * (k1 + k4 * uy),
        2.0 * duy * (k2 + k4 * ux),
    )
}

/// Rotate a point by the octave rotation (about 37 degrees). Without it every
/// octave's lattice lines up with the first and the result reads as a grid:
/// ridges and valleys running along x and z.
#[inline]
fn rotate_octave(x: f32, y: f32) -> (f32, f32) {
    (0.8 * x - 0.6 * y, 0.6 * x + 0.8 * y)
}

/// The default mountain noise: ground that looks weathered before any erosion
/// has run. Returns roughly 0..1.
///
/// Three things, each fixing something the plain modes get wrong:
///
/// - **Domain warp.** The coordinates are pushed around by a low-frequency
///   field first (`warp`, in noise units), so ridgelines meander and branch
///   instead of running as straight lattice-aligned facets.
/// - **Ridged multifractal base, with a soft crest.** Each ridge octave is
///   weighted by the one before it, so small ridges grow on the flanks of big
///   ones as spurs rather than sprinkling the valley floors. `1 - |n|` has a
///   kink at every zero crossing, which is what made [`ridge_noise`] peaks
///   into sharp pyramids; `1 - sqrt(n^2 + e)` rounds the crest over a few
///   metres while keeping it a clear ridgeline.
/// - **Derivative damped detail.** Each detail octave is divided by
///   `1 + |accumulated gradient|^2`, so fine relief survives on gentle ground
///   and fades on steep slopes, which is what water does to a hillside: the
///   look of erosion, without the cost. The detail is also scaled down at low
///   elevation, so valleys stay soft.
pub fn eroded_noise(
    x: f32,
    y: f32,
    octaves: u32,
    lacunarity: f32,
    persistence: f32,
    seed: u32,
    warp: f32,
) -> f32 {
    let lacunarity = lacunarity.clamp(1.5, 3.0);
    let persistence = persistence.clamp(0.1, 0.9);
    // Start off-axis, so even the first octave is not lattice aligned.
    let (x, y) = rotate_octave(x, y);

    let w = warp.clamp(0.0, 4.0);
    let (px, py) = if w > 0.0 {
        let wx = fbm(x * 0.5 + 3.1, y * 0.5 + 7.7, 3, 2.0, 0.5, seed ^ 0x51_5151) * 2.0 - 1.0;
        let wy = fbm(x * 0.5 - 4.3, y * 0.5 + 1.9, 3, 2.0, 0.5, seed ^ 0xA3_A3A3) * 2.0 - 1.0;
        (x + wx * w, y + wy * w)
    } else {
        (x, y)
    };

    // Ridged multifractal. `d` gathers the gradient of everything so far, in
    // input units, for the damping below.
    let ridge_octaves = octaves.clamp(1, 4);
    let (mut qx, mut qy) = (px, py);
    let (mut freq, mut amp, mut weight) = (1.0f32, 1.0f32, 1.0f32);
    let (mut ridged, mut norm) = (0.0f32, 0.0f32);
    let (mut dx, mut dy) = (0.0f32, 0.0f32);
    for i in 0..ridge_octaves {
        let (n, nx, ny) = value_noise_d(qx, qy, seed.wrapping_add(i.wrapping_mul(7_919)));
        // The rounding grows with frequency, so finer ridges are rounded over
        // a similar width in input units instead of creasing again.
        let soft = (n * n + 0.015 * freq).sqrt();
        let signal = (1.0 - soft).max(0.0);
        let s2 = signal * signal;
        ridged += s2 * weight * amp;
        norm += amp;
        // d(s2)/dn = 2 * signal * -(n / soft), then chain through the octave's
        // frequency to get it per input unit.
        let g = -2.0 * signal * (n / soft) * weight * amp * freq;
        dx += nx * g;
        dy += ny * g;
        // Smoothstepped: a plain clamp creases the next octave wherever the
        // weight saturates.
        let s = (signal * 2.0).clamp(0.0, 1.0);
        weight = s * s * (3.0 - 2.0 * s);
        let (rx, ry) = rotate_octave(qx, qy);
        qx = rx * lacunarity;
        qy = ry * lacunarity;
        freq *= lacunarity;
        amp *= persistence;
    }
    let base = if norm > 0.0 { ridged / norm } else { 0.0 };

    // Derivative damped detail, two octaves above the ridges' first.
    let (mut qx, mut qy) = rotate_octave(px * 2.0 + 17.3, py * 2.0 - 9.1);
    let (mut freq, mut amp) = (2.0f32, 1.0f32);
    let (mut detail, mut dnorm) = (0.0f32, 0.0f32);
    // The ridge gradient is per noise unit; scaled down so the damping keys on
    // genuinely steep ground, not on any ridge flank at all.
    let (mut gx, mut gy) = (dx * 0.35, dy * 0.35);
    for i in 0..octaves.clamp(1, 8) {
        let (n, nx, ny) = value_noise_d(qx, qy, seed.wrapping_add(0x9E37).wrapping_add(i.wrapping_mul(1_013)));
        gx += nx * amp * freq * 0.25;
        gy += ny * amp * freq * 0.25;
        detail += amp * n / (1.0 + gx * gx + gy * gy);
        dnorm += amp;
        let (rx, ry) = rotate_octave(qx, qy);
        qx = rx * lacunarity;
        qy = ry * lacunarity;
        freq *= lacunarity;
        amp *= persistence;
    }
    let detail = if dnorm > 0.0 { detail / dnorm } else { 0.0 };

    // High ground carries the most detail, valley floors the least.
    let h = base * 0.78 + detail * 0.22 * (0.3 + 0.7 * base) + 0.06;
    h.clamp(0.0, 1.0)
}

/// Evaluate noise with the selected mode.
pub fn eval_noise(
    x: f32,
    y: f32,
    mode: crate::data::NoiseMode,
    octaves: u32,
    lacunarity: f32,
    persistence: f32,
    seed: u32,
    warp_strength: f32,
) -> f32 {
    use crate::data::NoiseMode;
    match mode {
        NoiseMode::Fbm => fbm(x, y, octaves, lacunarity, persistence, seed),
        NoiseMode::Ridge => ridge_noise(x, y, octaves, lacunarity, persistence, seed),
        NoiseMode::Billow => billow_noise(x, y, octaves, lacunarity, persistence, seed),
        NoiseMode::Warped => {
            warped_fbm(x, y, octaves, lacunarity, persistence, seed, warp_strength)
        }
        NoiseMode::Hybrid => hybrid_noise(x, y, octaves, lacunarity, persistence, seed),
        NoiseMode::Eroded => {
            eroded_noise(x, y, octaves, lacunarity, persistence, seed, warp_strength)
        }
    }
}

// ── Brush Application ────────────────────────────────────────────────────────

/// Apply the stamp brush to a chunk's heightmap (single application, not continuous).
///
/// Samples the stamp image within the brush radius, rotated by `stamp_rotation`,
/// and blends it into the heightmap according to `stamp_blend_mode`.
pub fn apply_stamp(
    chunk: &mut TerrainChunkData,
    terrain: &TerrainData,
    settings: &TerrainSettings,
    stamp: &crate::data::StampBrushData,
    local_x: f32,
    local_z: f32,
) {
    if !stamp.is_loaded() {
        return;
    }

    let spacing = terrain.vertex_spacing();
    let resolution = terrain.chunk_resolution;
    let height_range = terrain.height_range();
    let brush_radius = settings.brush_radius;
    let strength = settings.brush_strength;

    let chunk_origin_x = chunk.chunk_x as f32 * terrain.chunk_size;
    let chunk_origin_z = chunk.chunk_z as f32 * terrain.chunk_size;
    let chunk_end_x = chunk_origin_x + terrain.chunk_size;
    let chunk_end_z = chunk_origin_z + terrain.chunk_size;

    if local_x + brush_radius < chunk_origin_x
        || local_x - brush_radius > chunk_end_x
        || local_z + brush_radius < chunk_origin_z
        || local_z - brush_radius > chunk_end_z
    {
        return;
    }

    let cos_r = settings.stamp_rotation.cos();
    let sin_r = settings.stamp_rotation.sin();
    let height_scale = settings.stamp_height_scale;

    for vz in 0..resolution {
        for vx in 0..resolution {
            let wx = chunk_origin_x + vx as f32 * spacing;
            let wz = chunk_origin_z + vz as f32 * spacing;

            let dx = wx - local_x;
            let dz = wz - local_z;
            let dist = (dx * dx + dz * dz).sqrt();

            if dist > brush_radius {
                continue;
            }

            // Rotate offset by stamp rotation, then map to UV [0,1]
            let rx = dx * cos_r + dz * sin_r;
            let rz = -dx * sin_r + dz * cos_r;
            let u = (rx / brush_radius + 1.0) * 0.5;
            let v = (rz / brush_radius + 1.0) * 0.5;

            if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                continue;
            }

            // stamp pixel (0-1) * height_scale * strength = normalized height delta
            let stamp_sample = stamp.sample(u, v);
            let stamp_value = stamp_sample * height_scale * strength;
            let current = chunk.get_base_height(vx, vz, resolution);

            // Edge falloff so the stamp doesn't create hard edges
            let t = dist / brush_radius;
            let edge_blend = compute_brush_falloff(t, settings.falloff, settings.falloff_type);

            let new_h = match settings.stamp_blend_mode {
                StampBlendMode::Add => current + stamp_value * edge_blend,
                StampBlendMode::Subtract => current - stamp_value * edge_blend,
                StampBlendMode::Replace => {
                    let target = stamp_value;
                    current + (target - current) * edge_blend
                }
                StampBlendMode::Max => {
                    let raised = current + stamp_value;
                    current + (raised - current) * edge_blend
                }
                StampBlendMode::Min => {
                    let lowered = current - stamp_value;
                    current + (lowered - current).min(0.0) * edge_blend
                }
            };

            chunk.set_height(vx, vz, resolution, new_h.clamp(0.0, 1.0));
        }
    }
}

/// Apply a single brush stroke to a chunk's heightmap.
///
/// `local_x`, `local_z` are the brush center in terrain-local coordinates
/// (i.e. 0..total_width, 0..total_depth). `dt` is the frame delta time.
/// `shift` indicates whether the Shift key is held for inverse operations.
pub fn apply_brush(
    chunk: &mut TerrainChunkData,
    terrain: &TerrainData,
    settings: &TerrainSettings,
    sculpt_state: &TerrainSculptState,
    local_x: f32,
    local_z: f32,
    dt: f32,
    shift: bool,
) {
    if is_area_brush(settings.brush_type) {
        // One chunk on its own: the area pass clips to it, so this still
        // works, but it cannot see across the seam. Callers holding the whole
        // terrain use [`apply_brush_dab`].
        apply_area_brush(&mut [chunk], terrain, settings, sculpt_state, Vec2::new(local_x, local_z), dt, shift);
        return;
    }
    let spacing = terrain.vertex_spacing();
    let resolution = terrain.chunk_resolution;
    let height_range = terrain.height_range();
    let brush_radius = settings.brush_radius;
    let strength = settings.brush_strength * dt * 2.0;

    let chunk_origin_x = chunk.chunk_x as f32 * terrain.chunk_size;
    let chunk_origin_z = chunk.chunk_z as f32 * terrain.chunk_size;
    let chunk_end_x = chunk_origin_x + terrain.chunk_size;
    let chunk_end_z = chunk_origin_z + terrain.chunk_size;

    // Quick bounds check — skip if brush doesn't overlap chunk
    if local_x + brush_radius < chunk_origin_x
        || local_x - brush_radius > chunk_end_x
        || local_z + brush_radius < chunk_origin_z
        || local_z - brush_radius > chunk_end_z
    {
        return;
    }

    for vz in 0..resolution {
        for vx in 0..resolution {
            let wx = chunk_origin_x + vx as f32 * spacing;
            let wz = chunk_origin_z + vz as f32 * spacing;

            let dx = wx - local_x;
            let dz = wz - local_z;

            let dist = match settings.brush_shape {
                BrushShape::Circle => (dx * dx + dz * dz).sqrt(),
                BrushShape::Square => dx.abs().max(dz.abs()),
                BrushShape::Diamond => dx.abs() + dz.abs(),
            };

            if dist > brush_radius {
                continue;
            }

            let t = dist / brush_radius;
            let falloff = compute_brush_falloff(t, settings.falloff, settings.falloff_type);
            let effect = strength * falloff;

            apply_brush_at_vertex(
                chunk,
                vx,
                vz,
                resolution,
                settings,
                sculpt_state,
                effect,
                height_range,
                wx,
                wz,
                dist,
                brush_radius,
                shift,
            );
        }
    }
}

/// Every brush reads the sculpt layer (`base_heights`), which is what it
/// writes. The composed `heights` buffer is only rebuilt once a frame, so a
/// brush reading it saw its own earlier dabs in the same frame undone (a
/// scripted stroke or repeated passes compounded nothing), and with height
/// layers present a smooth or flatten baked their deltas into the sculpt layer.
#[allow(clippy::too_many_arguments)]
fn apply_brush_at_vertex(
    chunk: &mut TerrainChunkData,
    vx: u32,
    vz: u32,
    resolution: u32,
    settings: &TerrainSettings,
    sculpt_state: &TerrainSculptState,
    effect: f32,
    height_range: f32,
    world_x: f32,
    world_z: f32,
    dist: f32,
    brush_radius: f32,
    shift: bool,
) {
    match settings.brush_type {
        TerrainBrushType::Raise => {
            chunk.modify_height(vx, vz, resolution, effect / height_range);
        }
        TerrainBrushType::Lower => {
            chunk.modify_height(vx, vz, resolution, -effect / height_range);
        }
        TerrainBrushType::Sculpt => {
            let delta = effect / height_range;
            chunk.modify_height(vx, vz, resolution, if shift { -delta } else { delta });
        }
        TerrainBrushType::SetHeight
        | TerrainBrushType::Smooth
        | TerrainBrushType::Flatten
        | TerrainBrushType::Ramp
        | TerrainBrushType::Terrace
        | TerrainBrushType::Relax
        | TerrainBrushType::Cliff => {
            // Area brushes: they read a neighbourhood far wider than one
            // vertex's neighbours, so they run over a whole window in
            // `apply_area_brush`, never through here.
        }
        TerrainBrushType::Erase => {
            let current = chunk.get_base_height(vx, vz, resolution);
            let new_h = current + (0.2 - current) * (effect * 2.0).min(1.0);
            chunk.set_height(vx, vz, resolution, new_h);
        }
        TerrainBrushType::Noise => {
            if shift {
                // Shift: simple box smooth
                let current = chunk.get_base_height(vx, vz, resolution);
                let mut sum = 0.0;
                let mut count = 0.0;
                for nz in vz.saturating_sub(1)..=(vz + 1).min(resolution - 1) {
                    for nx in vx.saturating_sub(1)..=(vx + 1).min(resolution - 1) {
                        sum += chunk.get_base_height(nx, nz, resolution);
                        count += 1.0;
                    }
                }
                let avg = sum / count;
                chunk.set_height(vx, vz, resolution, current + (avg - current) * effect);
            } else {
                let scale = settings.noise_scale.max(0.1);
                let n = eval_noise(
                    world_x / scale,
                    world_z / scale,
                    settings.noise_mode,
                    settings.noise_octaves.clamp(1, 8),
                    settings.noise_lacunarity,
                    settings.noise_persistence,
                    settings.noise_seed,
                    settings.warp_strength,
                );
                let centered = n - 0.5;
                chunk.modify_height(vx, vz, resolution, effect * centered / height_range);
            }
        }
        TerrainBrushType::Erosion => {
            // Thermal erosion: material slides down slopes steeper than talus angle
            let current = chunk.get_base_height(vx, vz, resolution);
            let talus = 0.004;
            let neighbors = [
                (vx.wrapping_sub(1), vz),
                (vx + 1, vz),
                (vx, vz.wrapping_sub(1)),
                (vx, vz + 1),
            ];
            let mut total_excess = 0.0f32;
            let mut steep_count = 0u32;
            for (nx, nz) in neighbors {
                if nx < resolution && nz < resolution {
                    let diff = current - chunk.get_base_height(nx, nz, resolution);
                    if diff > talus {
                        total_excess += diff - talus;
                        steep_count += 1;
                    }
                }
            }
            if steep_count > 0 {
                let erode = (total_excess / steep_count as f32) * effect * 0.6;
                chunk.modify_height(vx, vz, resolution, -erode);
            }
        }
        TerrainBrushType::Hydro => {
            // Hydraulic erosion: water flows downhill carrying sediment
            let current = chunk.get_base_height(vx, vz, resolution);
            let neighbors = [
                (vx.wrapping_sub(1), vz),
                (vx + 1, vz),
                (vx, vz.wrapping_sub(1)),
                (vx, vz + 1),
            ];
            let mut max_drop = 0.0f32;
            let mut drop_count = 0u32;
            for (nx, nz) in neighbors {
                if nx < resolution && nz < resolution {
                    let drop = current - chunk.get_base_height(nx, nz, resolution);
                    if drop > 0.001 {
                        max_drop += drop;
                        drop_count += 1;
                    }
                }
            }
            if drop_count > 0 {
                let sediment = (max_drop / drop_count as f32) * effect * 0.45;
                chunk.modify_height(vx, vz, resolution, -sediment);
            }
        }
        TerrainBrushType::Retop => {
            // Wide 5x5 kernel aggressive smooth
            let current = chunk.get_base_height(vx, vz, resolution);
            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            for nz in vz.saturating_sub(2)..=(vz + 2).min(resolution - 1) {
                for nx in vx.saturating_sub(2)..=(vx + 2).min(resolution - 1) {
                    sum += chunk.get_base_height(nx, nz, resolution);
                    count += 1.0;
                }
            }
            let avg = sum / count;
            let new_h = current + (avg - current) * (effect * 3.0).min(1.0);
            chunk.set_height(vx, vz, resolution, new_h);
        }
        TerrainBrushType::Pinch => {
            // Amplify deviation from local average (Shift = smooth towards average)
            let current = chunk.get_base_height(vx, vz, resolution);
            let left = if vx > 0 {
                chunk.get_base_height(vx - 1, vz, resolution)
            } else {
                current
            };
            let right = if vx < resolution - 1 {
                chunk.get_base_height(vx + 1, vz, resolution)
            } else {
                current
            };
            let up = if vz < resolution - 1 {
                chunk.get_base_height(vx, vz + 1, resolution)
            } else {
                current
            };
            let down = if vz > 0 {
                chunk.get_base_height(vx, vz - 1, resolution)
            } else {
                current
            };
            let avg = (left + right + up + down) * 0.25;
            let deviation = current - avg;
            let target = if shift {
                current - deviation * effect
            } else {
                current + deviation * effect * 0.5
            };
            chunk.set_height(vx, vz, resolution, target.clamp(0.0, 1.0));
        }
        TerrainBrushType::Stamp => {
            // Stamp brush uses apply_stamp() directly, not the per-vertex brush loop.
        }
    }
}

// ── Area brushes ─────────────────────────────────────────────────────────────

/// Steepest slope, in degrees, that Flatten, Set Height and Ramp leave
/// between the ground they moved and the ground around it. Without a limit
/// the falloff band was the only transition, and digging a 30 m lake with a
/// few metres of falloff left 30 m vertical walls.
pub const SHOULDER_DEGREES: f32 = 40.0;

/// The brushes that read a neighbourhood wider than a vertex's immediate
/// neighbours, and so run over a [`HeightWindow`] gathered across chunk seams
/// instead of vertex by vertex inside one chunk.
pub fn is_area_brush(brush: TerrainBrushType) -> bool {
    matches!(
        brush,
        TerrainBrushType::Smooth
            | TerrainBrushType::Relax
            | TerrainBrushType::Flatten
            | TerrainBrushType::SetHeight
            | TerrainBrushType::Ramp
            | TerrainBrushType::Terrace
            | TerrainBrushType::Cliff
    )
}

/// One dab of any brush except Stamp, Erosion and Hydraulic, over all of a
/// terrain's `chunks`. The entry point the mouse brush and the scripted stroke
/// both use: area brushes see across chunk seams, the rest run per chunk.
#[allow(clippy::too_many_arguments)]
pub fn apply_brush_dab(
    terrain: &TerrainData,
    chunks: &mut [&mut TerrainChunkData],
    settings: &TerrainSettings,
    sculpt_state: &TerrainSculptState,
    local_x: f32,
    local_z: f32,
    dt: f32,
    shift: bool,
) {
    if is_area_brush(settings.brush_type) {
        apply_area_brush(chunks, terrain, settings, sculpt_state, Vec2::new(local_x, local_z), dt, shift);
    } else {
        for chunk in chunks.iter_mut() {
            apply_brush(chunk, terrain, settings, sculpt_state, local_x, local_z, dt, shift);
        }
    }
}

/// The blur radius, in cells, Smooth uses for a brush: a tenth of the brush
/// radius. Three box passes of it are close to a gaussian with that sigma,
/// reaching about three times as far, so a 55 m brush rounds off features
/// tens of metres across instead of averaging each vertex with the next one.
fn smooth_cells(settings: &TerrainSettings, spacing: f32) -> usize {
    ((settings.brush_radius * 0.1 / spacing.max(1e-4)).round() as usize).clamp(1, 48)
}

/// Gather the brush's window, apply one dab of an area brush, write it back.
fn apply_area_brush(
    chunks: &mut [&mut TerrainChunkData],
    terrain: &TerrainData,
    settings: &TerrainSettings,
    sculpt_state: &TerrainSculptState,
    center: Vec2,
    dt: f32,
    shift: bool,
) {
    if chunks.is_empty() || terrain.chunk_resolution < 2 {
        return;
    }
    let spacing = terrain.vertex_spacing();
    let radius = settings.brush_radius.max(spacing);
    let blur = smooth_cells(settings, spacing);
    // Room for the blur to read real ground at the brush rim, plus a ring of
    // untouched cells the slope limit can anchor on.
    let reach = (radius / spacing).ceil() as i64 + 3 * blur as i64 + 2;

    // Clip to the chunks given, so a single-chunk call does not read the
    // zeros of chunks it was not handed.
    let cells = (terrain.chunk_resolution - 1) as i64;
    let bx0 = chunks.iter().map(|c| c.chunk_x as i64 * cells).min().unwrap_or(0);
    let bz0 = chunks.iter().map(|c| c.chunk_z as i64 * cells).min().unwrap_or(0);
    let bx1 = chunks.iter().map(|c| (c.chunk_x as i64 + 1) * cells).max().unwrap_or(0);
    let bz1 = chunks.iter().map(|c| (c.chunk_z as i64 + 1) * cells).max().unwrap_or(0);
    let cx = (center.x / spacing).round() as i64;
    let cz = (center.y / spacing).round() as i64;
    let (x0, z0) = ((cx - reach).max(bx0), (cz - reach).max(bz0));
    let (x1, z1) = ((cx + reach).min(bx1), (cz + reach).min(bz1));
    if x1 <= x0 || z1 <= z0 {
        return;
    }

    let original = {
        let refs: Vec<&TerrainChunkData> = chunks.iter().map(|c| &**c).collect();
        HeightWindow::gather(terrain, &refs, x0 as usize, z0 as usize, (x1 - x0 + 1) as usize, (z1 - z0 + 1) as usize)
    };
    let (w, h) = (original.w, original.h);

    // Per-cell effect: strength times the falloff, 0 outside the brush.
    let strength = settings.brush_strength * dt * 2.0;
    let mut effect = vec![0.0f32; w * h];
    let mut any = false;
    for z in 0..h {
        for x in 0..w {
            let p = Vec2::new((original.x0 + x) as f32, (original.z0 + z) as f32) * spacing - center;
            let dist = match settings.brush_shape {
                BrushShape::Circle => p.length(),
                BrushShape::Square => p.x.abs().max(p.y.abs()),
                BrushShape::Diamond => p.x.abs() + p.y.abs(),
            };
            if dist < radius {
                let e = strength * compute_brush_falloff(dist / radius, settings.falloff, settings.falloff_type);
                if e > 0.0 {
                    effect[z * w + x] = e;
                    any = true;
                }
            }
        }
    }
    if !any {
        return;
    }

    let to_metres = |n: f32| terrain.min_height + n * terrain.height_range();
    let before = &original.data;
    let mut out = before.clone();
    let world = |x: usize, z: usize| Vec2::new((original.x0 + x) as f32, (original.z0 + z) as f32) * spacing;

    // The steepest slope the result may have where it moved, or None for no
    // limit (the smoothing brushes only ever make slopes gentler).
    let limit_degrees = match settings.brush_type {
        TerrainBrushType::Smooth | TerrainBrushType::Relax => {
            let blurred = |src: &[f32], cells: usize| box_blur(src, w, h, cells, 3);
            let target = if settings.brush_type == TerrainBrushType::Smooth {
                blurred(before, blur)
            } else {
                // Taubin relaxation: a smoothing step, then a slightly larger
                // step back out. It removes bumps without shrinking the hills
                // and valleys around them, which is what makes it Relax and
                // not a second Smooth.
                let cells = ((blur as f32 * 0.6).round() as usize).max(1);
                let mut cur = before.clone();
                for _ in 0..2 {
                    let b0 = blurred(&cur, cells);
                    let h1: Vec<f32> = cur.iter().zip(&b0).map(|(v, b)| v + 0.6 * (b - v)).collect();
                    let b1 = blurred(&h1, cells);
                    cur = h1.iter().zip(&b1).map(|(v, b)| v - 0.63 * (b - v)).collect();
                }
                cur
            };
            let rate = if settings.brush_type == TerrainBrushType::Smooth { 2.0 } else { 2.5 };
            for i in 0..w * h {
                out[i] = before[i] + (target[i] - before[i]) * (effect[i] * rate).min(1.0);
            }
            None
        }
        TerrainBrushType::Flatten => {
            let Some(target) = sculpt_state.flatten_start_height else {
                return;
            };
            let target = to_metres(target);
            for i in 0..w * h {
                let current = before[i];
                let applies = match settings.flatten_mode {
                    FlattenMode::Both => true,
                    FlattenMode::Raise => current < target,
                    FlattenMode::Lower => current > target,
                };
                if applies {
                    out[i] = current + (target - current) * (effect[i] * 2.0).min(1.0);
                }
            }
            Some(SHOULDER_DEGREES)
        }
        TerrainBrushType::SetHeight => {
            let target = to_metres(settings.target_height);
            for i in 0..w * h {
                out[i] = before[i] + (target - before[i]) * (effect[i] * 3.0).min(1.0);
            }
            Some(SHOULDER_DEGREES)
        }
        TerrainBrushType::Ramp => {
            let target = to_metres(sculpt_state.flatten_start_height.unwrap_or(0.5));
            for z in 0..h {
                for x in 0..w {
                    let i = z * w + x;
                    if effect[i] <= 0.0 {
                        continue;
                    }
                    let t = (world(x, z).distance(center) / radius).clamp(0.0, 1.0);
                    let t = if shift { t } else { 1.0 - t };
                    // Smoothstep rather than linear, so the ramp eases into
                    // the ground at both ends instead of creasing.
                    let t = t * t * (3.0 - 2.0 * t);
                    let ramp_h = before[i] + (target - before[i]) * t;
                    out[i] = before[i] + (ramp_h - before[i]) * (effect[i] * 2.0).min(1.0);
                }
            }
            Some(SHOULDER_DEGREES)
        }
        TerrainBrushType::Terrace => {
            let step = settings.terrace_height.max(0.25);
            let sharp = settings.terrace_sharpness.clamp(0.0, 1.0);
            // Riser share of each step: most of it at 0, a fifth at 1.
            let riser = 0.7 - 0.5 * sharp;
            let rate = 0.5 + sharp * 1.5;
            for z in 0..h {
                for x in 0..w {
                    let i = z * w + x;
                    if effect[i] <= 0.0 {
                        continue;
                    }
                    let p = world(x, z);
                    // Noise on the step edges: shifts where each riser starts,
                    // so a terrace follows the hill in a wavering line, not a
                    // contour. Never negative and never past the tread, so a
                    // level tread maps to itself and a held stroke keeps it.
                    let wobble = fbm(p.x / (step * 4.0), p.y / (step * 4.0), 3, 2.0, 0.5, 0x7E44) * 0.8 * (1.0 - riser) * step;
                    let target = strata(before[i] + wobble, step, riser);
                    out[i] = before[i] + (target - before[i]) * (effect[i] * rate).min(1.0);
                }
            }
            Some(45.0 + 25.0 * sharp)
        }
        TerrainBrushType::Cliff => {
            if shift {
                let target = box_blur(before, w, h, blur, 3);
                for i in 0..w * h {
                    out[i] = before[i] + (target[i] - before[i]) * (effect[i] * 2.0).min(1.0);
                }
                None
            } else {
                // Rock strata: horizontal bands, a few metres thick, that
                // undulate and dip with low-frequency noise. Each band gets a
                // hardness: hard bands stand as steep faces with a ledge on
                // top, soft ones weather back to a slope. The result is
                // ledges and outcrops following the rock, not grooves
                // following the fall line, which is what amplifying the slope
                // per vertex produced.
                let band = (radius * 0.12).clamp(1.0, 12.0);
                for z in 0..h {
                    for x in 0..w {
                        let i = z * w + x;
                        if effect[i] <= 0.0 {
                            continue;
                        }
                        let p = world(x, z);
                        let dip = (fbm(p.x / (band * 8.0), p.y / (band * 8.0), 3, 2.0, 0.5, 0xC11F) - 0.5) * 1.2 * band;
                        let y = before[i] + dip;
                        let layer = (y / band).floor() as i32;
                        let hardness = hash2d(layer, 0, 0x57A7);
                        let riser = 0.95 - 0.7 * hardness;
                        let target = strata(y, band, riser) - dip;
                        out[i] = before[i] + (target - before[i]) * (effect[i] * 2.0).min(1.0);
                    }
                }
                Some(78.0)
            }
        }
        _ => return,
    };

    if let Some(degrees) = limit_degrees {
        let free: Vec<bool> = effect.iter().map(|e| *e > 0.0).collect();
        limit_slope(&mut out, before, &free, w, h, spacing, degrees);
    }

    let edited = HeightWindow { x0: original.x0, z0: original.z0, w, h, spacing, data: out };
    edited.scatter(terrain, &original, chunks);
}

/// Quantize `y` into steps of `step` metres, each a level tread followed by a
/// riser taking the last `riser` (0..1) of the step, eased with smoothstep.
/// Continuous in `y`, so there is never a jump from one tread to the next: the
/// riser is a slope, and how steep it ends up is the slope limit's business.
fn strata(y: f32, step: f32, riser: f32) -> f32 {
    let s = y / step;
    let level = s.floor();
    let f = s - level;
    let r = riser.clamp(0.05, 1.0);
    let t = ((f - (1.0 - r)) / r).clamp(0.0, 1.0);
    (level + t * t * (3.0 - 2.0 * t)) * step
}

/// `passes` box blurs of radius `r` cells, separable, edges clamped. Three
/// passes are within a few percent of a gaussian, at a cost that does not grow
/// with the radius.
pub(crate) fn box_blur(src: &[f32], w: usize, h: usize, r: usize, passes: usize) -> Vec<f32> {
    let mut a = src.to_vec();
    if r == 0 || w == 0 || h == 0 {
        return a;
    }
    let mut line = Vec::new();
    let mut blurred = Vec::new();
    for _ in 0..passes {
        for z in 0..h {
            line.clear();
            line.extend_from_slice(&a[z * w..(z + 1) * w]);
            blur_line(&line, r, &mut blurred);
            a[z * w..(z + 1) * w].copy_from_slice(&blurred);
        }
        for x in 0..w {
            line.clear();
            line.extend((0..h).map(|z| a[z * w + x]));
            blur_line(&line, r, &mut blurred);
            for (z, v) in blurred.iter().enumerate() {
                a[z * w + x] = *v;
            }
        }
    }
    a
}

/// One running-sum box blur of `line` into `out`, ends clamped.
fn blur_line(line: &[f32], r: usize, out: &mut Vec<f32>) {
    out.clear();
    let n = line.len() as i64;
    let get = |i: i64| line[i.clamp(0, n - 1) as usize];
    let ri = r as i64;
    let inv = 1.0 / (2 * r + 1) as f32;
    let mut sum: f32 = (-ri..=ri).map(get).sum();
    for i in 0..n {
        out.push(sum * inv);
        sum += get(i + ri + 1) - get(i - ri);
    }
}

/// Pull every `free` cell of `data` back toward `original` until no slope it
/// takes part in is steeper than `degrees`, anchored on the cells that are not
/// free (the ground around the brush, which the dab did not move).
///
/// A cell only ever moves toward where it was before the dab, so the limit
/// never makes ground steeper, never overshoots the original, and leaves a
/// slope that was already steep (a cliff the brush only grazed) as it was.
/// Because each cell is clamped against its neighbours' new values, a stroke
/// held over the same spot does not ratchet the shoulder steeper frame by
/// frame: the anchors outside the brush never move.
///
/// One-sided on purpose: a lowered cell is only ever raised (to no more than
/// `k` below its highest neighbour), a raised one only ever lowered. That is a
/// distance transform, which settles in a few raster sweeps; clamping both
/// ways at once made each cell fight the neighbours not yet visited.
fn limit_slope(data: &mut [f32], original: &[f32], free: &[bool], w: usize, h: usize, spacing: f32, degrees: f32) {
    let k = degrees.clamp(1.0, 89.0).to_radians().tan() * spacing;
    let kd = k * std::f32::consts::SQRT_2;
    const NEIGHBOURS: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
    let visit = |data: &mut [f32], x: usize, z: usize| {
        let i = z * w + x;
        let (v, o) = (data[i], original[i]);
        if !free[i] || v == o {
            return;
        }
        let lowered = v < o;
        let mut bound = v;
        for (n, (dx, dz)) in NEIGHBOURS.iter().enumerate() {
            let (nx, nz) = (x as i64 + dx, z as i64 + dz);
            if nx < 0 || nz < 0 || nx >= w as i64 || nz >= h as i64 {
                continue;
            }
            let nv = data[nz as usize * w + nx as usize];
            let lim = if n < 4 { k } else { kd };
            bound = if lowered { bound.max(nv - lim) } else { bound.min(nv + lim) };
        }
        data[i] = if lowered { bound.min(o) } else { bound.max(o) };
    };
    for _ in 0..3 {
        for z in 0..h {
            for x in 0..w {
                visit(data, x, z);
            }
        }
        for z in (0..h).rev() {
            for x in (0..w).rev() {
                visit(data, x, z);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2x2 chunks of 64 m at 1 m spacing, heights 0..100 m, filled from `f`
    /// (terrain-local metres to metres).
    fn terrain_with(f: impl Fn(f32, f32) -> f32) -> (TerrainData, Vec<TerrainChunkData>) {
        let t = TerrainData { chunks_x: 2, chunks_z: 2, chunk_size: 64.0, chunk_resolution: 65, min_height: 0.0, max_height: 100.0, ..Default::default() };
        let res = t.chunk_resolution;
        let mut out = Vec::new();
        for cz in 0..2 {
            for cx in 0..2 {
                let mut c = TerrainChunkData::new(cx, cz, res, 0.0);
                for z in 0..res {
                    for x in 0..res {
                        let wx = cx as f32 * 64.0 + x as f32;
                        let wz = cz as f32 * 64.0 + z as f32;
                        c.base_heights[(z * res + x) as usize] = f(wx, wz) / 100.0;
                    }
                }
                out.push(c);
            }
        }
        (t, out)
    }

    fn dab(t: &TerrainData, cs: &mut [TerrainChunkData], settings: &TerrainSettings, state: &TerrainSculptState, at: Vec2, times: usize) {
        let mut refs: Vec<&mut TerrainChunkData> = cs.iter_mut().collect();
        for _ in 0..times {
            apply_brush_dab(t, &mut refs, settings, state, at.x, at.y, 0.5, false);
        }
    }

    fn grid(t: &TerrainData, cs: &[TerrainChunkData]) -> HeightWindow {
        let refs: Vec<&TerrainChunkData> = cs.iter().collect();
        HeightWindow::gather_all(t, &refs)
    }

    /// Steepest slope (rise over run) between 4-neighbours anywhere in `w`.
    fn steepest(w: &HeightWindow) -> f32 {
        let mut s = 0.0f32;
        for z in 0..w.h {
            for x in 0..w.w {
                let v = w.data[z * w.w + x];
                if x + 1 < w.w {
                    s = s.max((w.data[z * w.w + x + 1] - v).abs());
                }
                if z + 1 < w.h {
                    s = s.max((w.data[(z + 1) * w.w + x] - v).abs());
                }
            }
        }
        s / w.spacing
    }

    #[test]
    fn eroded_noise_stays_in_range_and_has_relief() {
        let (mut lo, mut hi, mut sum) = (f32::MAX, f32::MIN, 0.0);
        let n = 10_000;
        for i in 0..n {
            let (x, y) = ((i % 100) as f32 * 0.137, (i / 100) as f32 * 0.137);
            let v = eroded_noise(x, y, 6, 2.0, 0.5, 1337, 0.5);
            assert!((0.0..=1.0).contains(&v) && v.is_finite(), "{v} at {x},{y}");
            lo = lo.min(v);
            hi = hi.max(v);
            sum += v;
        }
        let mean = sum / n as f32;
        assert!(hi - lo > 0.5, "flat: {lo}..{hi}");
        assert!((0.2..0.7).contains(&mean), "mean {mean}");
    }

    /// `1 - |n|` creases at every zero crossing, which is what made ridged
    /// peaks sharp pyramids. A crease shows as curvature that grows without
    /// bound as the measuring step shrinks (a kink measured over a step `e`
    /// reads as `slope jump / e`), where a rounded crest reads the same at any
    /// step. So: the worst curvature (99.5th percentile) at a step, over the
    /// same at twice the step. About 2 for a creased field, about 1 for a
    /// smooth one.
    #[test]
    fn eroded_crests_are_rounded_not_creased() {
        let crease = |f: &dyn Fn(f32, f32) -> f32| {
            let worst = |e: f32| {
                let mut curv: Vec<f32> = (0..40_000)
                    .map(|i| {
                        // Near the origin, where f32 has the precision for
                        // differences this small.
                        let (x, y) = (0.37 + (i % 200) as f32 * 0.0193, 0.37 + (i / 200) as f32 * 0.0193);
                        let c = f(x, y);
                        ((f(x - e, y) + f(x + e, y) + f(x, y - e) + f(x, y + e) - 4.0 * c) / (e * e)).abs()
                    })
                    .collect();
                curv.sort_by(f32::total_cmp);
                curv[curv.len() * 995 / 1000]
            };
            worst(1e-3) / worst(2e-3)
        };
        // Four octaves, so the finest detail is still many steps across and
        // the index measures creases, not undersampling.
        let eroded = crease(&|x, y| eroded_noise(x, y, 4, 2.0, 0.5, 7, 0.5));
        let ridged = crease(&|x, y| ridge_noise(x, y, 4, 2.0, 0.5, 7));
        println!("crease index (2 = creased, 1 = smooth): eroded {eroded:.2}, ridge {ridged:.2}");
        assert!(eroded < 1.15, "eroded {eroded}");
        assert!(ridged > eroded + 0.25, "eroded {eroded} vs ridge {ridged}");
    }

    /// A 10 m wall, sitting on a chunk seam, smoothed by a 55 m brush: one dab
    /// at full strength must leave a slope, not a wall, on both sides of the
    /// seam.
    #[test]
    fn a_large_smooth_brush_softens_a_large_feature_across_a_seam() {
        let (t, mut cs) = terrain_with(|x, _| if x < 64.0 { 20.0 } else { 30.0 });
        let settings = TerrainSettings { brush_type: TerrainBrushType::Smooth, brush_radius: 55.0, brush_strength: 1.0, falloff: 0.5, ..Default::default() };
        dab(&t, &mut cs, &settings, &TerrainSculptState::default(), Vec2::new(64.0, 64.0), 1);
        let w = grid(&t, &cs);
        let row = 64 * w.w;
        let slope = (54..74).map(|x| (w.data[row + x + 1] - w.data[row + x]).abs()).fold(0.0f32, f32::max);
        println!("steepest step across the wall after one dab: {slope:.2} m per m (was 10)");
        assert!(slope < 1.0, "still {slope} m per m");
        // The seam vertex is the same in both chunks.
        let res = t.chunk_resolution;
        assert_eq!(cs[0].get_base_height(64, 10, res), cs[1].get_base_height(0, 10, res));
    }

    #[test]
    fn relax_removes_bumps_but_keeps_the_hill() {
        let hill = |x: f32, z: f32| 40.0 * (-((x - 64.0).powi(2) + (z - 64.0).powi(2)) / 900.0).exp();
        let bumps = |x: f32, z: f32| if (x as i32 + z as i32) % 2 == 0 { 0.5 } else { -0.5 };
        let (t, mut cs) = terrain_with(|x, z| 20.0 + hill(x, z) + bumps(x, z));
        let settings = TerrainSettings { brush_type: TerrainBrushType::Relax, brush_radius: 40.0, brush_strength: 1.0, ..Default::default() };
        dab(&t, &mut cs, &settings, &TerrainSculptState::default(), Vec2::new(64.0, 64.0), 3);
        let w = grid(&t, &cs);
        let at = |x: usize, z: usize| w.data[z * w.w + x];
        // Second differences: the checkerboard, not the hill's own slope.
        let rough = (58..70).map(|x| (at(x - 1, 64) - 2.0 * at(x, 64) + at(x + 1, 64)).abs()).fold(0.0f32, f32::max);
        println!("relax: bump curvature {rough:.3} (was 2.0), peak {:.1} m (was 60)", at(64, 64));
        assert!(rough < 0.4, "bumps left: {rough}");
        assert!(at(64, 64) > 20.0 + 40.0 * 0.85, "the hill sank to {}", at(64, 64));
    }

    /// The quarry: Set Height digging 30 m down with a 55 m brush must leave
    /// sides no steeper than the shoulder angle, however long the stroke is
    /// held, and still reach the target in the middle.
    #[test]
    fn set_height_leaves_a_shoulder_not_a_wall() {
        let (t, mut cs) = terrain_with(|_, _| 30.0);
        let settings = TerrainSettings {
            brush_type: TerrainBrushType::SetHeight,
            brush_radius: 55.0,
            brush_strength: 1.0,
            falloff: 0.2,
            target_height: 0.0,
            ..Default::default()
        };
        let limit = SHOULDER_DEGREES.to_radians().tan();
        dab(&t, &mut cs, &settings, &TerrainSculptState::default(), Vec2::new(64.0, 64.0), 5);
        let after_five = steepest(&grid(&t, &cs));
        dab(&t, &mut cs, &settings, &TerrainSculptState::default(), Vec2::new(64.0, 64.0), 45);
        let w = grid(&t, &cs);
        let after_fifty = steepest(&w);
        println!("steepest side: {after_five:.2} after 5 dabs, {after_fifty:.2} after 50 (limit {limit:.2}, was a 30 m wall)");
        assert!(after_fifty <= limit * 1.02, "{after_fifty}");
        assert!(w.data[64 * w.w + 64] < 1.0, "floor at {}", w.data[64 * w.w + 64]);
    }

    #[test]
    fn flatten_on_a_hillside_cuts_and_fills_with_shoulders() {
        let (t, mut cs) = terrain_with(|x, _| x * 0.5);
        let settings = TerrainSettings { brush_type: TerrainBrushType::Flatten, brush_radius: 30.0, brush_strength: 1.0, falloff: 0.1, ..Default::default() };
        let state = TerrainSculptState { flatten_start_height: Some(0.32), ..Default::default() };
        dab(&t, &mut cs, &settings, &state, Vec2::new(64.0, 64.0), 30);
        let w = grid(&t, &cs);
        assert!(steepest(&w) <= SHOULDER_DEGREES.to_radians().tan() * 1.02);
        assert!((w.data[64 * w.w + 64] - 32.0).abs() < 0.5);
    }

    /// Terraces have level treads and risers that are steep but never walls.
    #[test]
    fn terraces_have_treads_and_sloped_risers() {
        let (t, mut cs) = terrain_with(|x, _| 10.0 + x * 0.55);
        let settings = TerrainSettings {
            brush_type: TerrainBrushType::Terrace,
            brush_radius: 40.0,
            brush_strength: 1.0,
            falloff: 0.2,
            terrace_height: 5.0,
            terrace_sharpness: 0.8,
            ..Default::default()
        };
        dab(&t, &mut cs, &settings, &TerrainSculptState::default(), Vec2::new(64.0, 64.0), 20);
        let w = grid(&t, &cs);
        let row = 64 * w.w;
        let steps: Vec<f32> = (44..84).map(|x| w.data[row + x + 1] - w.data[row + x]).collect();
        let flat = steps.iter().filter(|s| s.abs() < 0.1).count();
        let steepest = steps.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        println!("terrace: {flat} of {} cells level, steepest riser {:.0} degrees", steps.len(), steepest.atan().to_degrees());
        assert!(flat * 3 > steps.len(), "only {flat} level cells");
        assert!(steepest > 1.0, "risers too gentle: {steepest}");
        assert!(steepest < 75f32.to_radians().tan(), "a wall: {steepest}");
    }

    /// Cliff strata follow the rock, across the slope, not the fall line: along
    /// a contour the result varies far less than down the slope, and no face
    /// is vertical.
    #[test]
    fn cliff_makes_strata_not_stripes() {
        let (t, mut cs) = terrain_with(|x, _| 10.0 + x * 0.7);
        let settings = TerrainSettings { brush_type: TerrainBrushType::Cliff, brush_radius: 40.0, brush_strength: 1.0, falloff: 0.2, ..Default::default() };
        dab(&t, &mut cs, &settings, &TerrainSculptState::default(), Vec2::new(64.0, 64.0), 20);
        let w = grid(&t, &cs);
        let at = |x: usize, z: usize| w.data[z * w.w + x];
        // Second differences: what a groove or a ledge leaves.
        let curv = |a: f32, b: f32, c: f32| (a - 2.0 * b + c).powi(2);
        let (mut across, mut down, mut n) = (0.0, 0.0, 0.0);
        for z in 56..72 {
            for x in 56..72 {
                across += curv(at(x, z - 1), at(x, z), at(x, z + 1));
                down += curv(at(x - 1, z), at(x, z), at(x + 1, z));
                n += 1.0;
            }
        }
        let (across, down) = ((across / n as f32).sqrt(), (down / n as f32).sqrt());
        println!("cliff curvature rms: along the contour {across:.3}, down the slope {down:.3}");
        assert!(down > 0.2, "no strata: {down}");
        assert!(across < down * 0.35, "stripes: {across} vs {down}");
        assert!(steepest(&w) < 80f32.to_radians().tan());
    }
}

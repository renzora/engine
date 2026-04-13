//! Terrain sculpt brush algorithms — 16 brush types with noise utilities.
//!
//! Pure data operations on heightmaps. No systems here — those live in the editor crate.

use crate::data::*;

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
    let fx = x.fract();
    let fy = y.fract();
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
pub fn fbm(
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
        value += value_noise(x * frequency, y * frequency, oct_seed) * amplitude;
        max_amp += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    if max_amp > 0.0 { value / max_amp } else { 0.0 }
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

    if max_amp > 0.0 { value / max_amp } else { 0.0 }
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

    if max_amp > 0.0 { value / max_amp } else { 0.0 }
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
    let wy = fbm(x + 5.2, y + 1.3, octaves.min(3), lacunarity, persistence, warp_seed.wrapping_add(1)) * 2.0 - 1.0;
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
        NoiseMode::Warped => warped_fbm(x, y, octaves, lacunarity, persistence, seed, warp_strength),
        NoiseMode::Hybrid => hybrid_noise(x, y, octaves, lacunarity, persistence, seed),
    }
}

// ── Whole-terrain noise generation ───────────────────────────────────────────

/// Fill a chunk's heightmap with procedural noise using the current noise settings.
///
/// `additive`: if true, adds noise on top of existing heights; if false, replaces.
pub fn generate_noise_for_chunk(
    chunk: &mut TerrainChunkData,
    terrain: &TerrainData,
    settings: &TerrainSettings,
    additive: bool,
) {
    let resolution = terrain.chunk_resolution;
    let spacing = terrain.vertex_spacing();
    let chunk_origin_x = chunk.chunk_x as f32 * terrain.chunk_size;
    let chunk_origin_z = chunk.chunk_z as f32 * terrain.chunk_size;
    let scale = settings.noise_scale.max(0.1);
    let strength = settings.brush_strength;

    for vz in 0..resolution {
        for vx in 0..resolution {
            let world_x = chunk_origin_x + vx as f32 * spacing;
            let world_z = chunk_origin_z + vz as f32 * spacing;

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

            let idx = (vz * resolution + vx) as usize;
            if idx < chunk.heights.len() {
                if additive {
                    let current = chunk.heights[idx];
                    chunk.heights[idx] = (current + (n - 0.5) * strength).clamp(0.0, 1.0);
                } else {
                    chunk.heights[idx] = (n * strength).clamp(0.0, 1.0);
                }
            }
        }
    }
    chunk.dirty = true;
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

            if u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0 {
                continue;
            }

            // stamp pixel (0-1) * height_scale * strength = normalized height delta
            let stamp_sample = stamp.sample(u, v);
            let stamp_value = stamp_sample * height_scale * strength;
            let current = chunk.get_height(vx, vz, resolution);

            // Edge falloff so the stamp doesn't create hard edges
            let t = dist / brush_radius;
            let edge_blend = compute_brush_falloff(t, settings.falloff, settings.falloff_type);

            let new_h = match settings.stamp_blend_mode {
                StampBlendMode::Add => {
                    current + stamp_value * edge_blend
                }
                StampBlendMode::Subtract => {
                    current - stamp_value * edge_blend
                }
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
        TerrainBrushType::SetHeight => {
            let current = chunk.get_height(vx, vz, resolution);
            let target = settings.target_height;
            let new_h = current + (target - current) * (effect * 3.0).min(1.0);
            chunk.set_height(vx, vz, resolution, new_h);
        }
        TerrainBrushType::Erase => {
            let current = chunk.get_height(vx, vz, resolution);
            let new_h = current + (0.2 - current) * (effect * 2.0).min(1.0);
            chunk.set_height(vx, vz, resolution, new_h);
        }
        TerrainBrushType::Smooth => {
            let current = chunk.get_height(vx, vz, resolution);
            const KERNEL: &[(i32, i32, f32)] = &[
                (-1, -1, 0.0625), (0, -1, 0.125), (1, -1, 0.0625),
                (-1,  0, 0.125),  (0,  0, 0.25),  (1,  0, 0.125),
                (-1,  1, 0.0625), (0,  1, 0.125), (1,  1, 0.0625),
            ];
            let mut weighted = 0.0f32;
            let mut total_w = 0.0f32;
            for &(kx, kz, w) in KERNEL {
                let nx = vx as i32 + kx;
                let nz = vz as i32 + kz;
                if nx >= 0 && nx < resolution as i32 && nz >= 0 && nz < resolution as i32 {
                    weighted += chunk.get_height(nx as u32, nz as u32, resolution) * w;
                    total_w += w;
                }
            }
            let avg = weighted / total_w;
            let new_h = current + (avg - current) * (effect * 2.0).min(1.0);
            chunk.set_height(vx, vz, resolution, new_h);
        }
        TerrainBrushType::Flatten => {
            if let Some(target) = sculpt_state.flatten_start_height {
                let current = chunk.get_height(vx, vz, resolution);
                let should_apply = match settings.flatten_mode {
                    FlattenMode::Both => true,
                    FlattenMode::Raise => current < target,
                    FlattenMode::Lower => current > target,
                };
                if should_apply {
                    let new_h = current + (target - current) * (effect * 2.0).min(1.0);
                    chunk.set_height(vx, vz, resolution, new_h);
                }
            }
        }
        TerrainBrushType::Noise => {
            if shift {
                // Shift: simple box smooth
                let current = chunk.get_height(vx, vz, resolution);
                let mut sum = 0.0;
                let mut count = 0.0;
                for nz in vz.saturating_sub(1)..=(vz + 1).min(resolution - 1) {
                    for nx in vx.saturating_sub(1)..=(vx + 1).min(resolution - 1) {
                        sum += chunk.get_height(nx, nz, resolution);
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
            let current = chunk.get_height(vx, vz, resolution);
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
                    let diff = current - chunk.get_height(nx, nz, resolution);
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
            let current = chunk.get_height(vx, vz, resolution);
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
                    let drop = current - chunk.get_height(nx, nz, resolution);
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
        TerrainBrushType::Ramp => {
            let t = if shift {
                dist / brush_radius
            } else {
                1.0 - dist / brush_radius
            };
            let target = sculpt_state.flatten_start_height.unwrap_or(0.5);
            let current = chunk.get_height(vx, vz, resolution);
            let ramp_h = current + (target - current) * t;
            let new_h = current + (ramp_h - current) * (effect * 2.0).min(1.0);
            chunk.set_height(vx, vz, resolution, new_h);
        }
        TerrainBrushType::Retop => {
            // Wide 5x5 kernel aggressive smooth
            let current = chunk.get_height(vx, vz, resolution);
            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            for nz in vz.saturating_sub(2)..=(vz + 2).min(resolution - 1) {
                for nx in vx.saturating_sub(2)..=(vx + 2).min(resolution - 1) {
                    sum += chunk.get_height(nx, nz, resolution);
                    count += 1.0;
                }
            }
            let avg = sum / count;
            let new_h = current + (avg - current) * (effect * 3.0).min(1.0);
            chunk.set_height(vx, vz, resolution, new_h);
        }
        TerrainBrushType::Terrace => {
            let current = chunk.get_height(vx, vz, resolution);
            let steps = settings.terrace_steps.max(1) as f32;
            let sharpness = settings.terrace_sharpness.clamp(0.0, 1.0);
            let stepped = (current * steps).round() / steps;
            let blend = effect * (0.5 + sharpness * 1.5).min(1.0);
            let new_h = current + (stepped - current) * blend;
            chunk.set_height(vx, vz, resolution, new_h);
        }
        TerrainBrushType::Pinch => {
            // Amplify deviation from local average (Shift = smooth towards average)
            let current = chunk.get_height(vx, vz, resolution);
            let left = if vx > 0 { chunk.get_height(vx - 1, vz, resolution) } else { current };
            let right = if vx < resolution - 1 { chunk.get_height(vx + 1, vz, resolution) } else { current };
            let up = if vz < resolution - 1 { chunk.get_height(vx, vz + 1, resolution) } else { current };
            let down = if vz > 0 { chunk.get_height(vx, vz - 1, resolution) } else { current };
            let avg = (left + right + up + down) * 0.25;
            let deviation = current - avg;
            let target = if shift {
                current - deviation * effect
            } else {
                current + deviation * effect * 0.5
            };
            chunk.set_height(vx, vz, resolution, target.clamp(0.0, 1.0));
        }
        TerrainBrushType::Relax => {
            // Laplacian relaxation
            let current = chunk.get_height(vx, vz, resolution);
            let left = if vx > 0 { chunk.get_height(vx - 1, vz, resolution) } else { current };
            let right = if vx < resolution - 1 { chunk.get_height(vx + 1, vz, resolution) } else { current };
            let up = if vz < resolution - 1 { chunk.get_height(vx, vz + 1, resolution) } else { current };
            let down = if vz > 0 { chunk.get_height(vx, vz - 1, resolution) } else { current };
            let laplacian = (left + right + up + down) * 0.25 - current;
            let new_h = current + laplacian * (effect * 2.5).min(1.0);
            chunk.set_height(vx, vz, resolution, new_h);
        }
        TerrainBrushType::Stamp => {
            // Stamp brush uses apply_stamp() directly, not the per-vertex brush loop.
        }
        TerrainBrushType::Cliff => {
            // Amplify local slope gradient (Shift = soften)
            let current = chunk.get_height(vx, vz, resolution);
            let left = if vx > 0 { chunk.get_height(vx - 1, vz, resolution) } else { current };
            let right = if vx < resolution - 1 { chunk.get_height(vx + 1, vz, resolution) } else { current };
            let up = if vz < resolution - 1 { chunk.get_height(vx, vz + 1, resolution) } else { current };
            let down = if vz > 0 { chunk.get_height(vx, vz - 1, resolution) } else { current };
            let dh_dx = (right - left) * 0.5;
            let dh_dz = (up - down) * 0.5;
            let slope = (dh_dx * dh_dx + dh_dz * dh_dz).sqrt();
            if slope > 0.001 {
                let delta = if shift {
                    -slope * effect * 0.4
                } else {
                    slope * effect * 0.4
                };
                chunk.modify_height(vx, vz, resolution, delta);
            }
        }
    }
}

//! Baked grass mesh generation — produces one combined mesh per chunk per foliage type.
//!
//! Per-blade attributes are packed into standard Bevy vertex attributes:
//! - `UV_0`: (u_across, v_along_blade)  — v=0 at base, v=1 at tip
//! - `UV_1`: (phase, blade_height)
//! - `COLOR`: (bend, lean_x, lean_z, color_variation)

use bevy::prelude::*;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;

use super::data::{FoliageDensityMap, FoliageType};

/// Number of segments per grass blade (more = smoother curve, more verts).
const BLADE_SEGMENTS: usize = 4;
/// Vertices per blade = (segments + 1) * 2 sides.
const VERTS_PER_BLADE: usize = (BLADE_SEGMENTS + 1) * 2;
/// Indices per blade = segments * 6.
const INDICES_PER_BLADE: usize = BLADE_SEGMENTS * 6;

/// Deterministic hash for reproducible random from grid position.
fn hash_pos(x: u32, z: u32, seed: u32) -> f32 {
    let mut h = x
        .wrapping_mul(2654435761)
        .wrapping_add(z.wrapping_mul(2246822519))
        .wrapping_add(seed);
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    (h as f32) / (u32::MAX as f32)
}

/// Generate a single grass blade template (unit height, centered at origin).
/// Returns (positions, uvs) for the template vertices.
fn blade_template() -> (Vec<[f32; 3]>, Vec<[f32; 2]>) {
    let mut positions = Vec::with_capacity(VERTS_PER_BLADE);
    let mut uvs = Vec::with_capacity(VERTS_PER_BLADE);
    for s in 0..=BLADE_SEGMENTS {
        let t = s as f32 / BLADE_SEGMENTS as f32;
        let w = 0.03 * (1.0 - t * 0.85); // taper toward tip
        let curve = t * t * 0.02; // slight forward curve
        positions.push([-w, t, curve]);
        positions.push([w, t, curve]);
        uvs.push([0.0, t]);
        uvs.push([1.0, t]);
    }
    (positions, uvs)
}

/// Blade index template (offsets from 0; caller adds base vertex offset).
fn blade_indices() -> Vec<u32> {
    let mut indices = Vec::with_capacity(INDICES_PER_BLADE);
    for s in 0..BLADE_SEGMENTS {
        let i = (s * 2) as u32;
        indices.extend_from_slice(&[i, i + 1, i + 2, i + 1, i + 3, i + 2]);
    }
    indices
}

/// Generate a baked foliage mesh for one chunk and one foliage type.
///
/// # Arguments
/// - `foliage_type`: Configuration (density, height/width ranges, etc.)
/// - `type_index`: Which foliage type slot in the density map.
/// - `density_map`: Per-chunk density weights.
/// - `heights`: Terrain chunk heightmap (normalized 0..1).
/// - `chunk_resolution`: Heightmap vertices per side.
/// - `chunk_size`: World-space size of the chunk.
/// - `min_height`: Terrain minimum height.
/// - `height_range`: max_height - min_height.
/// - `seed`: Deterministic seed for this chunk.
pub fn generate_foliage_chunk_mesh(
    foliage_type: &FoliageType,
    type_index: usize,
    density_map: &FoliageDensityMap,
    heights: &[f32],
    chunk_resolution: u32,
    chunk_size: f32,
    min_height: f32,
    height_range: f32,
    seed: u32,
) -> Option<Mesh> {
    if !foliage_type.enabled || foliage_type.density <= 0.0 {
        return None;
    }

    let (template_pos, template_uv) = blade_template();
    let template_idx = blade_indices();

    let spacing = 1.0 / foliage_type.density.sqrt();
    let grid_count = (chunk_size / spacing).ceil() as u32;

    // Pre-allocate for estimated blade count
    let est_blades = (grid_count * grid_count) as usize / 2; // ~50% coverage estimate
    let mut positions = Vec::with_capacity(est_blades * VERTS_PER_BLADE);
    let mut normals = Vec::with_capacity(est_blades * VERTS_PER_BLADE);
    let mut uvs_0 = Vec::with_capacity(est_blades * VERTS_PER_BLADE);
    let mut uvs_1 = Vec::with_capacity(est_blades * VERTS_PER_BLADE);
    let mut colors = Vec::with_capacity(est_blades * VERTS_PER_BLADE);
    let mut indices: Vec<u32> = Vec::with_capacity(est_blades * INDICES_PER_BLADE);

    let vert_spacing = chunk_size / (chunk_resolution - 1) as f32;

    for gz in 0..grid_count {
        for gx in 0..grid_count {
            let seed_val = seed.wrapping_add(gx * 7919 + gz * 6271);

            // Grid position with jitter
            let jitter_x = hash_pos(gx, gz, seed_val) - 0.5;
            let jitter_z = hash_pos(gz, gx, seed_val.wrapping_add(1)) - 0.5;
            let local_x = (gx as f32 + 0.5 + jitter_x * 0.8) * spacing;
            let local_z = (gz as f32 + 0.5 + jitter_z * 0.8) * spacing;

            if local_x < 0.0 || local_x >= chunk_size || local_z < 0.0 || local_z >= chunk_size {
                continue;
            }

            // Sample density weight from the painted density map
            let uv_x = local_x / chunk_size;
            let uv_z = local_z / chunk_size;
            let density = density_map.sample(uv_x, uv_z, type_index);
            if density < 0.01 {
                continue;
            }

            // Bilinear height interpolation
            let fx = local_x / vert_spacing;
            let fz = local_z / vert_spacing;
            let vx0 = (fx.floor() as u32).min(chunk_resolution - 1);
            let vz0 = (fz.floor() as u32).min(chunk_resolution - 1);
            let vx1 = (vx0 + 1).min(chunk_resolution - 1);
            let vz1 = (vz0 + 1).min(chunk_resolution - 1);
            let tx = fx.fract();
            let tz = fz.fract();

            let get_h = |x: u32, z: u32| -> f32 {
                heights
                    .get((z * chunk_resolution + x) as usize)
                    .copied()
                    .unwrap_or(0.0)
            };
            let h_norm = get_h(vx0, vz0) * (1.0 - tx) * (1.0 - tz)
                + get_h(vx1, vz0) * tx * (1.0 - tz)
                + get_h(vx0, vz1) * (1.0 - tx) * tz
                + get_h(vx1, vz1) * tx * tz;
            let y = min_height + h_norm * height_range;

            // Per-blade random attributes
            let h_rand = hash_pos(gx * 13, gz * 17, seed_val.wrapping_add(2));
            let blade_height = foliage_type.height_range.x
                + (foliage_type.height_range.y - foliage_type.height_range.x) * h_rand;

            let w_rand = hash_pos(gx * 23, gz * 29, seed_val.wrapping_add(3));
            let blade_width = foliage_type.width_range.x
                + (foliage_type.width_range.y - foliage_type.width_range.x) * w_rand;

            let phase = hash_pos(gx * 37, gz * 41, seed_val.wrapping_add(4))
                * std::f32::consts::TAU;

            let bend = ((blade_height - foliage_type.height_range.x)
                / (foliage_type.height_range.y - foliage_type.height_range.x).max(0.01)
                * 0.7
                + hash_pos(gx * 47, gz * 53, seed_val.wrapping_add(6)).abs() * 0.3)
                .clamp(0.0, 1.0);

            let lean_x =
                (hash_pos(gx * 59, gz * 61, seed_val.wrapping_add(7)) - 0.5) * 0.06;
            let lean_z =
                (hash_pos(gx * 67, gz * 71, seed_val.wrapping_add(8)) - 0.5) * 0.06;

            let color_var =
                (phase * 3.7).sin() * 0.12;

            // Y-axis rotation
            let angle = phase * 2.5;
            let cos_a = angle.cos();
            let sin_a = angle.sin();

            let width_scale = blade_width / 0.03; // template uses 0.03 base width

            // Emit blade vertices
            let base_vertex = positions.len() as u32;
            for v in 0..VERTS_PER_BLADE {
                let tp = template_pos[v];
                // Scale by blade dimensions
                let px = tp[0] * width_scale * blade_height * 1.2;
                let py = tp[1] * blade_height;
                let pz = tp[2] * blade_height;
                // Rotate around Y
                let rx = px * cos_a - pz * sin_a;
                let rz = px * sin_a + pz * cos_a;
                // Translate to world-local position
                positions.push([rx + local_x, py + y, rz + local_z]);
                normals.push([0.0, 1.0, 0.0]); // recomputed in vertex shader
                uvs_0.push(template_uv[v]);
                uvs_1.push([phase, blade_height]);
                colors.push([bend, lean_x * 10.0 + 0.5, lean_z * 10.0 + 0.5, color_var + 0.5]);
            }

            // Emit indices (offset by base vertex)
            for &idx in &template_idx {
                indices.push(base_vertex + idx);
            }
        }
    }

    if positions.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs_0);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs_1);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

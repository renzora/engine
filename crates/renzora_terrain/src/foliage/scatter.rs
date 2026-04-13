//! Terrain foliage auto-scatter — instance generation from splatmap weights.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for foliage scattering on a specific terrain layer.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct TerrainFoliageConfig {
    /// Which paint layer drives placement.
    pub layer_index: usize,
    /// Instances per square unit.
    pub density: f32,
    /// Minimum splatmap weight threshold for placement.
    pub min_weight: f32,
    /// Asset path to the foliage mesh.
    pub mesh_path: String,
    /// Asset path to the foliage material.
    pub material_path: String,
    /// Min/max blade height.
    pub height_range: Vec2,
    /// Min/max blade width.
    pub width_range: Vec2,
    /// Random Y-axis rotation.
    pub random_rotation: bool,
    /// Align instance to surface normal.
    pub align_to_normal: bool,
    /// Whether this config is enabled.
    pub enabled: bool,
}

impl Default for TerrainFoliageConfig {
    fn default() -> Self {
        Self {
            layer_index: 0,
            density: 4.0,
            min_weight: 0.3,
            mesh_path: String::new(),
            material_path: String::new(),
            height_range: Vec2::new(0.3, 0.8),
            width_range: Vec2::new(0.05, 0.15),
            random_rotation: true,
            align_to_normal: false,
            enabled: false,
        }
    }
}

/// Marker component for foliage instance batch entities.
#[derive(Component)]
pub struct FoliageBatch {
    pub config_entity: Entity,
    pub chunk_x: u32,
    pub chunk_z: u32,
}

/// Simple hash for deterministic random from grid position.
fn hash_position(x: u32, z: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(2654435761).wrapping_add(z.wrapping_mul(2246822519)).wrapping_add(seed);
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    (h as f32) / (u32::MAX as f32)
}

/// Generate instance transforms for a single chunk from splatmap weights.
pub fn generate_foliage_instances(
    config: &TerrainFoliageConfig,
    weights: &[[f32; 8]],
    splatmap_resolution: u32,
    heights: &[f32],
    chunk_resolution: u32,
    chunk_size: f32,
    min_height: f32,
    height_range: f32,
    seed: u32,
) -> Vec<Transform> {
    if !config.enabled || config.density <= 0.0 {
        return Vec::new();
    }

    let spacing = 1.0 / config.density.sqrt();
    let grid_count = (chunk_size / spacing).ceil() as u32;
    let layer = config.layer_index;

    let mut instances = Vec::new();

    for gz in 0..grid_count {
        for gx in 0..grid_count {
            let seed_val = seed.wrapping_add(gx * 7919 + gz * 6271);

            // Grid position with random jitter
            let jitter_x = hash_position(gx, gz, seed_val) - 0.5;
            let jitter_z = hash_position(gz, gx, seed_val.wrapping_add(1)) - 0.5;
            let local_x = (gx as f32 + 0.5 + jitter_x * 0.8) * spacing;
            let local_z = (gz as f32 + 0.5 + jitter_z * 0.8) * spacing;

            if local_x < 0.0 || local_x >= chunk_size || local_z < 0.0 || local_z >= chunk_size {
                continue;
            }

            // Sample splatmap weight at this position
            let uv_x = local_x / chunk_size;
            let uv_z = local_z / chunk_size;
            let sx = (uv_x * (splatmap_resolution - 1) as f32).round() as u32;
            let sz = (uv_z * (splatmap_resolution - 1) as f32).round() as u32;
            let si = (sz.min(splatmap_resolution - 1) * splatmap_resolution + sx.min(splatmap_resolution - 1)) as usize;

            if si >= weights.len() || layer >= 8 {
                continue;
            }

            let weight = weights[si][layer];
            if weight < config.min_weight {
                continue;
            }

            // Sample height via bilinear interpolation
            let vert_spacing = chunk_size / (chunk_resolution - 1) as f32;
            let fx = local_x / vert_spacing;
            let fz = local_z / vert_spacing;
            let vx0 = (fx.floor() as u32).min(chunk_resolution - 1);
            let vz0 = (fz.floor() as u32).min(chunk_resolution - 1);
            let vx1 = (vx0 + 1).min(chunk_resolution - 1);
            let vz1 = (vz0 + 1).min(chunk_resolution - 1);
            let tx = fx.fract();
            let tz = fz.fract();

            let get_h = |x: u32, z: u32| -> f32 {
                heights.get((z * chunk_resolution + x) as usize).copied().unwrap_or(0.0)
            };

            let h_norm = get_h(vx0, vz0) * (1.0 - tx) * (1.0 - tz)
                + get_h(vx1, vz0) * tx * (1.0 - tz)
                + get_h(vx0, vz1) * (1.0 - tx) * tz
                + get_h(vx1, vz1) * tx * tz;

            let y = min_height + h_norm * height_range;

            // Random scale
            let scale_rand = hash_position(gx * 13, gz * 17, seed_val.wrapping_add(2));
            let h_scale = config.height_range.x + (config.height_range.y - config.height_range.x) * scale_rand;
            let w_rand = hash_position(gx * 23, gz * 29, seed_val.wrapping_add(3));
            let w_scale = config.width_range.x + (config.width_range.y - config.width_range.x) * w_rand;

            // Random rotation
            let rot_y = if config.random_rotation {
                hash_position(gx * 31, gz * 37, seed_val.wrapping_add(4)) * std::f32::consts::TAU
            } else {
                0.0
            };

            let translation = Vec3::new(local_x, y, local_z);
            let rotation = Quat::from_rotation_y(rot_y);
            let scale = Vec3::new(w_scale, h_scale, w_scale);

            instances.push(Transform {
                translation,
                rotation,
                scale,
            });
        }
    }

    instances
}

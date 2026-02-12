use std::sync::Arc;

use bevy::prelude::*;
use bevy::platform::collections::HashMap;
use bevy_voxel_world::prelude::*;
use noise::{HybridMulti, MultiFractal, NoiseFn, Perlin, Simplex};

use crate::component_system::components::voxel_world::{VoxelNoiseType, VoxelWorldData};

/// Renzora's VoxelWorldConfig implementation.
/// Holds a cached copy of VoxelWorldData that drives all config methods.
#[derive(Resource, Clone)]
pub struct RenzoraVoxelConfig {
    pub data: VoxelWorldData,
}

impl Default for RenzoraVoxelConfig {
    fn default() -> Self {
        Self {
            data: VoxelWorldData::default(),
        }
    }
}

impl VoxelWorldConfig for RenzoraVoxelConfig {
    type MaterialIndex = u8;
    type ChunkUserBundle = ();

    fn spawning_distance(&self) -> u32 {
        self.data.spawning_distance
    }

    fn min_despawn_distance(&self) -> u32 {
        self.data.min_despawn_distance
    }

    fn max_spawn_per_frame(&self) -> usize {
        self.data.max_spawn_per_frame
    }

    fn debug_draw_chunks(&self) -> bool {
        self.data.debug_draw_chunks
    }

    fn attach_chunks_to_root(&self) -> bool {
        true
    }

    fn voxel_texture(&self) -> Option<(String, u32)> {
        Some(("example_voxel_texture.png".into(), 4))
    }

    fn texture_index_mapper(&self) -> Arc<dyn Fn(Self::MaterialIndex) -> [u32; 3] + Send + Sync> {
        Arc::new(|mat| match mat {
            0 => [0, 0, 0],
            1 => [1, 1, 1],
            2 => [2, 2, 2],
            3 => [3, 3, 3],
            _ => [0, 0, 0],
        })
    }

    fn voxel_lookup_delegate(&self) -> VoxelLookupDelegate<Self::MaterialIndex> {
        if !self.data.enabled {
            return Box::new(|_, _, _| Box::new(|_, _| WorldVoxel::Unset));
        }

        let seed = self.data.seed;
        let noise_type = self.data.noise_type;
        let frequency = self.data.noise_frequency as f64;
        let octaves = (self.data.noise_octaves as usize).min(6);
        let amplitude = self.data.noise_amplitude as f64;
        let base_height = self.data.base_height;
        let caves_enabled = self.data.caves_enabled;
        let cave_threshold = self.data.cave_threshold as f64;

        Box::new(move |_chunk_pos, _lod, _previous| {
            let voxel_fn: VoxelLookupFn<u8> = match noise_type {
                VoxelNoiseType::Perlin => {
                    let mut noise = HybridMulti::<Perlin>::new(seed as u32);
                    noise.octaves = octaves;
                    noise.frequency = frequency;
                    make_voxel_fn(noise, amplitude, base_height, caves_enabled, cave_threshold, seed)
                }
                VoxelNoiseType::Simplex => {
                    let mut noise = HybridMulti::<Simplex>::new(seed as u32);
                    noise.octaves = octaves;
                    noise.frequency = frequency;
                    make_voxel_fn(noise, amplitude, base_height, caves_enabled, cave_threshold, seed)
                }
            };
            voxel_fn
        })
    }
}

fn make_voxel_fn<N: NoiseFn<f64, 2> + NoiseFn<f64, 3> + Send + Sync + 'static>(
    noise_2d: N,
    amplitude: f64,
    base_height: i32,
    caves_enabled: bool,
    cave_threshold: f64,
    seed: u64,
) -> VoxelLookupFn<u8> {
    let mut cache = HashMap::<(i32, i32), f64>::new();

    // Create a separate 3D noise for caves if enabled
    let cave_noise: Option<HybridMulti<Perlin>> = if caves_enabled {
        let mut n = HybridMulti::<Perlin>::new(seed.wrapping_add(12345) as u32);
        n.octaves = 3;
        n.frequency = 0.05;
        Some(n)
    } else {
        None
    };

    Box::new(move |pos: IVec3, _previous| {
        let [x, y, z] = pos.as_dvec3().to_array();

        // Height-based terrain
        let height = match cache.get(&(pos.x, pos.z)) {
            Some(sample) => *sample,
            None => {
                let sample = NoiseFn::<f64, 2>::get(&noise_2d, [x / 1000.0, z / 1000.0]) * amplitude + base_height as f64;
                cache.insert((pos.x, pos.z), sample);
                sample
            }
        };

        let is_ground = y < height;

        if !is_ground {
            // Sea level fill
            if pos.y < base_height {
                return WorldVoxel::Solid(3); // Water material
            }
            return WorldVoxel::Air;
        }

        // Cave carving
        if let Some(ref cave_n) = cave_noise {
            let cave_val = NoiseFn::<f64, 3>::get(cave_n, [x / 100.0, y / 100.0, z / 100.0]);
            if cave_val.abs() < cave_threshold {
                return WorldVoxel::Air;
            }
        }

        // Material based on depth
        let depth = height - y;
        if depth < 1.0 {
            WorldVoxel::Solid(0) // Grass/top
        } else if depth < 4.0 {
            WorldVoxel::Solid(1) // Dirt
        } else {
            WorldVoxel::Solid(2) // Stone
        }
    })
}

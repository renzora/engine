//! Foliage runtime systems — mesh rebuilding and uniform updates.

// `bevy::platform::time::Instant`, never `std`'s — std's panics on wasm.
use bevy::platform::time::Instant;
use bevy::prelude::*;
use std::sync::Arc;

use crate::data::TerrainData;

use super::blades::scatter_foliage_chunk;
use super::data::{FoliageBatch, FoliageConfig, FoliageDensityMap};
use super::instance::{BladeSetId, GrassChunk};

/// Share of wall-clock time a live foliage preview may spend rebuilding.
const LIVE_BUDGET: f32 = 0.15;
/// Never rebuild more often than this while a stroke is in progress — past
/// roughly 20 Hz the extra rebuilds are invisible and only cost frames.
const MIN_LIVE_INTERVAL: f32 = 0.05;
/// Never leave a stroke without feedback for longer than this, however
/// expensive the chunk is.
const MAX_LIVE_INTERVAL: f32 = 0.5;

/// Rolling cost of rescattering one chunk's foliage, in seconds.
///
/// A rebuild rescatters the chunk's *entire* blade set, and that cost spans
/// orders of magnitude with how much of the chunk is painted — which is why the
/// brush originally deferred every rebuild to mouse-release and you saw nothing
/// until you let go. The editor's brush now previews as you drag, paced by this
/// measurement rather than by a guessed interval: a bare chunk redraws at the
/// cap, a heavily grassed one backs off on its own.
///
/// Instancing made this a lot cheaper — the scatter no longer builds vertices —
/// so in practice most strokes now sit at the fastest interval.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct FoliageRebuildCost {
    /// Exponential moving average of recent rebuilds' wall time, in seconds.
    /// Zero until the first rebuild has been measured.
    pub seconds: f32,
}

impl FoliageRebuildCost {
    /// Seconds a live preview should leave between rebuilds.
    pub fn live_interval(&self) -> f32 {
        (self.seconds / LIVE_BUDGET).clamp(MIN_LIVE_INTERVAL, MAX_LIVE_INTERVAL)
    }

    /// Fold one rebuild's measured duration into the average.
    fn record(&mut self, elapsed: f32) {
        // Seeded rather than averaged from zero: the first rebuild of a stroke
        // is the one whose pacing matters most, so it must not be judged against
        // a "free" history that doesn't exist yet.
        self.seconds = if self.seconds <= 0.0 {
            elapsed
        } else {
            self.seconds * 0.7 + elapsed * 0.3
        };
    }
}

/// Rescatters a chunk's blades when its density map is marked dirty.
pub fn foliage_scatter_rebuild_system(
    mut commands: Commands,
    foliage_config: Res<FoliageConfig>,
    mut density_query: Query<(
        Entity,
        &mut FoliageDensityMap,
        &crate::data::TerrainChunkData,
        &GlobalTransform,
    )>,
    terrain_query: Query<&TerrainData>,
    existing_batches: Query<(Entity, &FoliageBatch)>,
    mut cost: ResMut<FoliageRebuildCost>,
) {
    for (chunk_entity, mut density_map, chunk_data, chunk_transform) in density_query.iter_mut() {
        if !density_map.dirty {
            continue;
        }
        density_map.dirty = false;
        let started = Instant::now();

        // Find parent terrain data
        let terrain = terrain_query.iter().next();
        let Some(terrain_data) = terrain else {
            continue;
        };

        // Remove existing foliage batches for this chunk
        for (batch_entity, batch) in existing_batches.iter() {
            if batch.chunk_entity == chunk_entity {
                commands.entity(batch_entity).despawn();
            }
        }

        let chunk_world = chunk_transform.translation();

        // Scatter blades for each foliage type
        for (type_idx, foliage_type) in foliage_config.types.iter().enumerate() {
            let blades = scatter_foliage_chunk(
                foliage_type,
                type_idx,
                &density_map,
                &chunk_data.heights,
                terrain_data.chunk_resolution,
                terrain_data.chunk_size,
                terrain_data.min_height,
                terrain_data.max_height - terrain_data.min_height,
                chunk_data.chunk_x * 1000 + chunk_data.chunk_z,
            );

            let Some(blades) = blades else {
                continue;
            };

            // The blade set is bounded by the terrain chunk in x/z and by the
            // tallest blade in y. Spelled out because there is no `Mesh3d` on
            // this path for Bevy to derive an `Aabb` from, and without one the
            // chunk is never frustum-culled.
            let size = terrain_data.chunk_size;
            let (mut low, mut high) = (f32::MAX, f32::MIN);
            for blade in &blades {
                low = low.min(blade.position_height[1]);
                high = high.max(blade.position_height[1] + blade.position_height[3]);
            }
            let aabb = bevy::camera::primitives::Aabb::from_min_max(
                Vec3::new(0.0, low, 0.0),
                Vec3::new(size, high, size),
            );

            commands.spawn((
                GrassChunk {
                    id: BladeSetId::next(),
                    blades: Arc::from(blades),
                    color_base: foliage_type.color_base,
                    color_tip: foliage_type.color_tip,
                    wind_strength: foliage_type.wind_strength,
                },
                Transform::from_translation(chunk_world),
                aabb,
                FoliageBatch {
                    foliage_type_index: type_idx,
                    chunk_entity,
                },
            ));
        }

        // Measured around the scatter only — the spawn is a deferred command
        // and the GPU upload happens in the render world, but the scatter is
        // what dominates and what scales with the amount of grass painted.
        cost.record(started.elapsed().as_secs_f32());
    }
}

/// When terrain chunks are sculpted (heightmap changes), mark their foliage
/// density maps as dirty so the grass mesh rebuilds at the new heights.
pub fn foliage_follow_terrain_system(
    mut query: Query<
        (&crate::data::TerrainChunkData, &mut FoliageDensityMap),
        Changed<crate::data::TerrainChunkData>,
    >,
) {
    for (chunk, mut density_map) in query.iter_mut() {
        // `mesh_stale` (composition → mesh-rebuild hand-off), not `dirty`:
        // this system is ordered inside that window, so it reliably sees
        // every height change regardless of where the writer ran.
        if chunk.mesh_stale {
            density_map.dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Before anything has been measured the brush still has to pick a rate.
    /// The floor is the right guess: an unmeasured chunk is usually an empty
    /// one, and previewing too eagerly costs a frame, not a stroke.
    #[test]
    fn unmeasured_cost_previews_at_the_fastest_rate() {
        assert_eq!(
            FoliageRebuildCost::default().live_interval(),
            MIN_LIVE_INTERVAL
        );
    }

    /// The whole point of measuring: an expensive chunk must slow the preview
    /// down rather than rebuild every frame and stall the drag.
    #[test]
    fn expensive_chunks_back_off_but_never_go_silent() {
        let mut cost = FoliageRebuildCost::default();
        cost.record(0.020); // a 20 ms rebuild
        let interval = cost.live_interval();
        assert!(
            interval > MIN_LIVE_INTERVAL,
            "20 ms rebuild should not preview at the cap"
        );

        cost.seconds = 10.0; // absurdly expensive
        assert_eq!(
            cost.live_interval(),
            MAX_LIVE_INTERVAL,
            "a stroke must never be left without feedback"
        );
    }

    /// The first measurement seeds the average outright. Blending it against a
    /// zero history would report a chunk as ~3x cheaper than it is, on exactly
    /// the rebuild whose pacing matters most.
    #[test]
    fn first_measurement_seeds_rather_than_blends() {
        let mut cost = FoliageRebuildCost::default();
        cost.record(0.040);
        assert_eq!(cost.seconds, 0.040);
        cost.record(0.040);
        assert!(
            (cost.seconds - 0.040).abs() < 1e-6,
            "a steady cost must stay put"
        );
    }
}

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
///
/// Raised from 0.15 once the scatter went parallel (`blades::ScatterCtx`). The
/// budget is what converts rebuild cost into a tick rate, so with the cost cut
/// several-fold a stricter share was just leaving the preview slower than the
/// machine could afford.
const LIVE_BUDGET: f32 = 0.25;
/// Never rebuild more often than once a frame. Anything tighter is a second
/// rebuild nobody can see, since only one of them reaches the screen.
///
/// This used to be 0.05 — 20 Hz — on the reasoning that faster was invisible.
/// It isn't: at 20 Hz the grass trails the cursor by up to 50 ms on top of
/// whatever the pacing adds, and that is exactly the "small delay" a stroke
/// feels.
const MIN_LIVE_INTERVAL: f32 = 0.016;
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
    mut existing_batches: Query<(
        Entity,
        &FoliageBatch,
        &mut GrassChunk,
        &mut Transform,
        &mut bevy::camera::primitives::Aabb,
    )>,
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

        // This chunk's existing batch entities, by foliage type. They are
        // *updated*, not replaced — see the reuse note where they are written.
        let existing: Vec<(usize, Entity)> = existing_batches
            .iter()
            .filter(|(_, batch, ..)| batch.chunk_entity == chunk_entity)
            .map(|(entity, batch, ..)| (batch.foliage_type_index, entity))
            .collect();
        let mut reused: Vec<Entity> = Vec::new();

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

            let scattered = GrassChunk {
                id: BladeSetId::next(),
                blades: Arc::from(blades),
                color_base: foliage_type.color_base,
                color_tip: foliage_type.color_tip,
                wind_strength: foliage_type.wind_strength,
            };

            // Update this type's batch entity in place if it already has one.
            //
            // Despawning and respawning it instead costs a **blank frame**, and
            // during a paint stroke that lands as the whole chunk's grass
            // strobing: `queue_grass` only enqueues chunks that already carry a
            // `GrassInstanceBuffer`, and that buffer is created in the render
            // schedule's Prepare — which runs *after* Queue. So a batch entity
            // is invisible for the entire first frame of its life, and a preview
            // ticking at 20 Hz respawned one three times a second.
            //
            // Reuse also restores what the render world was built to do: it
            // keeps its GPU buffer per chunk and re-uploads only when
            // `BladeSetId` changes. A fresh entity every rebuild threw that
            // buffer away and reallocated instead of refilling it.
            match existing
                .iter()
                .find(|(idx, _)| *idx == type_idx)
                .map(|(_, entity)| *entity)
            {
                Some(entity) => {
                    if let Ok((_, _, mut slot, mut slot_transform, mut slot_aabb)) =
                        existing_batches.get_mut(entity)
                    {
                        *slot = scattered;
                        *slot_aabb = aabb;
                        slot_transform.translation = chunk_world;
                        reused.push(entity);
                    }
                }
                None => {
                    commands.spawn((
                        scattered,
                        Transform::from_translation(chunk_world),
                        aabb,
                        FoliageBatch {
                            foliage_type_index: type_idx,
                            chunk_entity,
                        },
                    ));
                }
            }
        }

        // Batches whose type scattered nothing this time — erased back to bare
        // ground, or the type was disabled — have nothing left to update.
        for (_, entity) in existing {
            if !reused.contains(&entity) {
                commands.entity(entity).despawn();
            }
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

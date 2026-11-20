//! Whether the terrain systems have anything to do this frame.
//!
//! The executor charges a few microseconds for every system it starts, even
//! one that returns at once, and terrain owns dozens of them. An editor with
//! no terrain open was paying for all of them every frame to find empty
//! queries. [`TerrainPresence`] is decided once a frame, in `First`, and every
//! terrain system set runs only while it is active.
//!
//! **Decided once, not per set.** The sets hand chunks to each other inside a
//! frame (composition raises `mesh_stale`, the LOD and foliage systems read it,
//! the mesh rebuild clears it). A condition that looked at the world itself
//! would be evaluated at each set's first system, so a terrain spawned midway
//! through `Update` could wake the mesh rebuild while the LOD rebuild before it
//! had already been skipped, and that chunk's LODs would never hear of the
//! change. Read from one resource, every set agrees for the whole frame; a
//! terrain spawned during a frame starts being processed the next one.
//!
//! **What counts as present** is anything a terrain system still has to act
//! on, not only a terrain root: chunks (which a scene can hold before its root
//! is rehydrated), a landscape material probe whose terrain is gone (despawned
//! by `follow_terrain_materials`) and a blade set whose chunk is gone (a root
//! entity, despawned by `foliage_despawn_orphaned_batches_system`). Each of
//! those keeps the systems awake until the system that clears it has run.
//!
//! **And a few frames after,** for the readers of `RemovedComponents`
//! (`undress_chunks`, `follow_terrain_materials`), whose messages live two
//! frames, and for the systems that put something back when the last terrain
//! goes: the sun's cascades return to Bevy's reach, and the progress card is
//! taken down.

use bevy::prelude::*;

use crate::data::{TerrainChunkData, TerrainData};
use crate::foliage::data::FoliageBatch;
use crate::landscape::instance::TerrainMaterialProbe;

/// Frames the systems keep running after the last terrain entity is gone.
/// Two covers a `RemovedComponents` message's lifetime; one more is margin.
const GRACE_FRAMES: u8 = 3;

/// Whether the terrain systems run this frame. See the module docs.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct TerrainPresence {
    /// Frames left to run. Topped up to [`GRACE_FRAMES`] while anything is
    /// present, counted down once nothing is.
    frames_left: u8,
}

impl TerrainPresence {
    pub fn active(&self) -> bool {
        self.frames_left > 0
    }
}

/// Register the resource and the system that keeps it. Both plugins that gate
/// on it call this, so either works alone; the second call does nothing.
pub fn install(app: &mut App) {
    if app.world().contains_resource::<TerrainPresence>() {
        return;
    }
    app.init_resource::<TerrainPresence>().add_systems(First, track_terrain_presence);
}

#[allow(clippy::type_complexity)]
fn track_terrain_presence(
    present: Query<
        (),
        Or<(With<TerrainData>, With<TerrainChunkData>, With<TerrainMaterialProbe>, With<FoliageBatch>)>,
    >,
    mut presence: ResMut<TerrainPresence>,
) {
    let next = if !present.is_empty() {
        GRACE_FRAMES
    } else {
        presence.frames_left.saturating_sub(1)
    };
    if presence.frames_left != next {
        presence.frames_left = next;
    }
}

/// Run condition: the terrain systems have work this frame. With no
/// [`TerrainPresence`] (a test app that adds a system set by hand) it says
/// yes, which is what the system did before it was gated.
pub fn terrain_in_scene(presence: Option<Res<TerrainPresence>>) -> bool {
    presence.is_none_or(|p| p.active())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_while_present_and_for_a_grace_after() {
        let mut app = App::new();
        install(&mut app);
        app.update();
        assert!(!app.world().resource::<TerrainPresence>().active(), "nothing in the scene");

        let terrain = app.world_mut().spawn(TerrainData::default()).id();
        app.update();
        assert!(app.world().resource::<TerrainPresence>().active());

        app.world_mut().despawn(terrain);
        for frame in 0..GRACE_FRAMES {
            app.update();
            let active = app.world().resource::<TerrainPresence>().active();
            assert_eq!(active, frame + 1 < GRACE_FRAMES, "frame {frame} after the terrain went");
        }
    }
}

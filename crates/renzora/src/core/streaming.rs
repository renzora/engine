//! World-streaming helpers shared by every crate that loads/unloads content
//! by camera distance (streamed `SceneInstance` expansion in `renzora_engine`,
//! terrain chunk residency in `renzora_terrain`, texture tiers in the rmip
//! streamer). Centralized here so all streamers agree on *when* streaming is
//! in effect and *whose* position drives it — divergent answers would make
//! one system unload what another just loaded.

use super::{DedicatedServer, DefaultCamera, EditorCamera, EditorSession, PlayModeState, PlayState, SceneCamera};
use bevy::ecs::query::Has;
use bevy::prelude::*;

/// Whether distance-based streaming should run this frame.
///
/// - **Dedicated server** — never: there is no camera, and gameplay
///   (physics, scripts, navmesh) needs the whole world resident.
/// - **Shipped game** — always.
/// - **Editor** — only while playing/paused/simulating. In edit mode the
///   world stays fully expanded so designers see and select everything.
pub fn world_streaming_active(world: &World) -> bool {
    if world.get_resource::<DedicatedServer>().is_some() {
        return false;
    }
    let is_editor = world
        .get_resource::<EditorSession>()
        .is_some_and(|session| session.is_editor());
    if !is_editor {
        return true;
    }
    world
        .get_resource::<PlayModeState>()
        .is_some_and(|play| !matches!(play.state, PlayState::Editing))
}

/// The world-space position streaming distances are measured from.
///
/// Prefers the active gameplay camera (`DefaultCamera` first, then any active
/// `SceneCamera`); falls back to the active editor camera — Simulate mode
/// keeps the editor camera as the user's eye, and streaming should follow
/// what they're looking at. `None` (no active camera at all) means callers
/// should leave residency untouched rather than unload everything.
pub fn streaming_camera_pos(world: &mut World) -> Option<Vec3> {
    let mut fallback: Option<Vec3> = None;
    {
        let mut q = world
            .query_filtered::<(&GlobalTransform, &Camera, Has<DefaultCamera>), With<SceneCamera>>();
        for (transform, camera, is_default) in q.iter(world) {
            if !camera.is_active {
                continue;
            }
            if is_default {
                return Some(transform.translation());
            }
            if fallback.is_none() {
                fallback = Some(transform.translation());
            }
        }
    }
    if fallback.is_some() {
        return fallback;
    }
    let mut q = world.query_filtered::<(&GlobalTransform, &Camera), With<EditorCamera>>();
    for (transform, camera) in q.iter(world) {
        if camera.is_active {
            return Some(transform.translation());
        }
    }
    None
}

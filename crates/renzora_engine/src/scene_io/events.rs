//! Scene load state + the events a load emits.

use bevy::prelude::*;

/// Coarse phase of the most recent scene load.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SceneLoadPhase {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed,
}

/// Tracks the state of scene loading so UI can reflect progress.
///
/// `progress` is 0..1. Scene loading is currently synchronous, so the value
/// jumps from 0 → 1 in a single frame; a future async split can make this a
/// true progress readout without changing this resource's shape.
#[derive(Resource, Default)]
pub struct SceneLoadState {
    pub phase: SceneLoadPhase,
    pub current_path: Option<String>,
    pub progress: f32,
}

#[derive(Event, Clone, Debug)]
pub struct SceneLoaded {
    pub path: String,
}

#[derive(Event, Clone, Debug)]
pub struct SceneLoadFailed {
    pub path: String,
    pub error: String,
}

/// Fired after a scene loads when one or more component/resource types
/// were skipped because they aren't registered in the type registry.
/// The editor turns this into a toast; the runtime just logs it.
///
/// Most-common cause: an editor-only component (e.g.
/// `renzora_camera::OrbitCameraState`) was serialized into the scene and
/// then loaded by a runtime build that doesn't register editor types.
#[derive(Event, Clone, Debug)]
pub struct SceneLoadedWithSkippedTypes {
    pub path: String,
    pub skipped: Vec<String>,
}

//! Scene loading resources and commands for scripting
//!
//! Provides event-based scene and prefab loading for scripts.

use bevy::prelude::*;
use std::path::PathBuf;

/// Command to load a scene (replaces current scene content)
#[derive(Debug, Clone)]
pub struct SceneLoadRequest {
    /// Path to the scene file (relative to project)
    pub path: PathBuf,
}

/// Command to unload/despawn entities
#[derive(Debug, Clone)]
pub struct SceneUnloadRequest {
    /// Entity to despawn (if Some), or despawn all runtime prefabs (if None)
    pub entity: Option<Entity>,
}

/// Command to spawn a prefab at a location
#[derive(Debug, Clone)]
pub struct PrefabSpawnRequest {
    /// Path to the prefab/scene file (relative to project)
    pub path: PathBuf,
    /// Position to spawn at
    pub position: Vec3,
    /// Rotation in euler angles (degrees)
    pub rotation: Vec3,
    /// Optional parent entity
    pub parent: Option<Entity>,
}

/// Queue of pending scene operations
#[derive(Resource, Default)]
pub struct SceneCommandQueue {
    /// Scenes to load
    pub load_requests: Vec<SceneLoadRequest>,
    /// Scenes/entities to unload
    pub unload_requests: Vec<SceneUnloadRequest>,
    /// Prefabs to spawn
    pub spawn_requests: Vec<PrefabSpawnRequest>,
}

impl SceneCommandQueue {
    /// Queue a scene load request
    pub fn load_scene(&mut self, path: impl Into<PathBuf>) {
        self.load_requests.push(SceneLoadRequest {
            path: path.into(),
        });
    }

    /// Queue an unload request for a specific entity
    pub fn unload_entity(&mut self, entity: Entity) {
        self.unload_requests.push(SceneUnloadRequest {
            entity: Some(entity),
        });
    }

    /// Queue an unload request to despawn all runtime prefabs
    pub fn unload_all_prefabs(&mut self) {
        self.unload_requests.push(SceneUnloadRequest {
            entity: None,
        });
    }

    /// Queue a prefab spawn request
    pub fn spawn_prefab(&mut self, path: impl Into<PathBuf>, position: Vec3, rotation: Vec3) {
        self.spawn_requests.push(PrefabSpawnRequest {
            path: path.into(),
            position,
            rotation,
            parent: None,
        });
    }

    /// Queue a prefab spawn with parent
    pub fn spawn_prefab_with_parent(&mut self, path: impl Into<PathBuf>, position: Vec3, rotation: Vec3, parent: Entity) {
        self.spawn_requests.push(PrefabSpawnRequest {
            path: path.into(),
            position,
            rotation,
            parent: Some(parent),
        });
    }

    /// Take all pending load requests
    pub fn take_load_requests(&mut self) -> Vec<SceneLoadRequest> {
        std::mem::take(&mut self.load_requests)
    }

    /// Take all pending unload requests
    pub fn take_unload_requests(&mut self) -> Vec<SceneUnloadRequest> {
        std::mem::take(&mut self.unload_requests)
    }

    /// Take all pending spawn requests
    pub fn take_spawn_requests(&mut self) -> Vec<PrefabSpawnRequest> {
        std::mem::take(&mut self.spawn_requests)
    }

    /// Check if there are any pending requests
    pub fn has_pending(&self) -> bool {
        !self.load_requests.is_empty() || !self.spawn_requests.is_empty() || !self.unload_requests.is_empty()
    }
}

/// Marker component for entities spawned from prefabs at runtime
#[derive(Component, Debug, Clone)]
pub struct RuntimePrefabInstance {
    /// Path to the source prefab file
    pub source_path: PathBuf,
    /// When this prefab was spawned
    pub spawn_time: f32,
}

/// Marker for a prefab that is currently being loaded
#[derive(Component, Debug)]
pub struct PendingPrefabSpawn {
    /// The original spawn request
    pub request: PrefabSpawnRequest,
}

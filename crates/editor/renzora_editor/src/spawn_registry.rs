//! Spawn registry — holds entity presets that can be spawned from the hierarchy overlay.

use bevy::prelude::*;

/// A spawnable entity template.
pub struct EntityPreset {
    pub id: &'static str,
    pub display_name: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    pub spawn_fn: fn(&mut World) -> Entity,
}

/// Registry of entity presets available for spawning.
#[derive(Resource, Default)]
pub struct SpawnRegistry {
    presets: Vec<EntityPreset>,
}

impl SpawnRegistry {
    /// Register a new entity preset.
    pub fn register(&mut self, preset: EntityPreset) {
        self.presets.push(preset);
    }

    /// Iterate over all registered presets.
    pub fn iter(&self) -> impl Iterator<Item = &EntityPreset> {
        self.presets.iter()
    }
}

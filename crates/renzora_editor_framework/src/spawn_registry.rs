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

// ── Component Icon Registry ────────────────────────────────────────────────

/// Entry mapping a component to an icon + color in the hierarchy tree.
pub struct ComponentIconEntry {
    /// Bevy component TypeId — used for runtime archetype checks.
    pub type_id: std::any::TypeId,
    /// Icon string (egui-phosphor).
    pub icon: &'static str,
    /// Icon color in the hierarchy.
    pub color: [u8; 3],
    /// Priority — higher values are checked first. Allows cameras to take
    /// precedence over meshes when an entity has both.
    pub priority: i32,
    /// Optional: for UI widgets that have per-type icons, a function that
    /// returns a dynamic icon based on the entity's state.
    pub dynamic_icon_fn: Option<fn(&bevy::ecs::world::World, bevy::ecs::entity::Entity) -> Option<(&'static str, [u8; 3])>>,
}

/// Registry of component → icon mappings. Plugins register their own icons
/// so the hierarchy tree doesn't need to import domain crates.
#[derive(Resource, Default)]
pub struct ComponentIconRegistry {
    entries: Vec<ComponentIconEntry>,
}

impl ComponentIconRegistry {
    /// Register a component icon mapping.
    pub fn register(&mut self, entry: ComponentIconEntry) {
        self.entries.push(entry);
        // Keep sorted by priority (descending) so higher-priority icons win
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Look up the icon for an entity by checking its archetype against
    /// all registered component types. Returns the first (highest priority) match.
    pub fn entity_icon(&self, world: &bevy::ecs::world::World, entity: bevy::ecs::entity::Entity) -> Option<(&'static str, [u8; 3])> {
        // Check dynamic icons first (for things like per-widget-type icons)
        for entry in &self.entries {
            if let Some(dynamic_fn) = entry.dynamic_icon_fn {
                if let Some(result) = dynamic_fn(world, entity) {
                    return Some(result);
                }
            }
        }
        // Then check static component-based icons
        for entry in &self.entries {
            if entry.dynamic_icon_fn.is_some() {
                continue; // already checked above
            }
            if let Some(component_id) = world.components().get_id(entry.type_id) {
                let er = world.entity(entity);
                if er.contains_id(component_id) {
                    return Some((entry.icon, entry.color));
                }
            }
        }
        None
    }
}

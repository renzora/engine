#![allow(unused_mut, dead_code, unused_variables)]

//! Exclusive system that applies generic reflection-based component field writes.
//!
//! The reflection read/write/enumerate helpers themselves live in the `renzora`
//! contract crate (`renzora::reflection`) so scripting, blueprints and the
//! animation system all share one implementation. This module re-exports the
//! ones scripting callers use and hosts the script-queue-draining system.

use bevy::prelude::*;

use super::execution::ScriptReflectionQueue;

pub use renzora::reflection::{
    get_all_component_fields, get_entity_component_names, get_reflected_field,
};

/// Exclusive system that drains the [`ScriptReflectionQueue`] and applies each
/// set operation via the shared reflection writer.
pub fn apply_reflection_sets(world: &mut World) {
    // Drain the queue
    let sets = {
        let Some(mut queue) = world.get_resource_mut::<ScriptReflectionQueue>() else {
            return;
        };
        std::mem::take(&mut queue.sets)
    };

    if sets.is_empty() {
        return;
    }

    // Build name → entity map for entity_name lookups (lazy).
    let mut name_map: Option<std::collections::HashMap<String, Entity>> = None;

    for set_op in &sets {
        // Resolve target entity
        let target = if let Some(name) = &set_op.entity_name {
            if name_map.is_none() {
                let mut map = std::collections::HashMap::new();
                let mut query = world.query::<(Entity, &Name)>();
                for (e, n) in query.iter(world) {
                    map.insert(n.as_str().to_string(), e);
                }
                name_map = Some(map);
            }
            match name_map.as_ref().unwrap().get(name) {
                Some(&e) => e,
                None => {
                    warn!("[Script] set: entity '{}' not found", name);
                    continue;
                }
            }
        } else if let Some(id) = set_op.entity_id {
            Entity::from_bits(id)
        } else {
            set_op.source_entity
        };

        if !renzora::reflection::set_reflected_field(
            world,
            target,
            &set_op.component_type,
            &set_op.field_path,
            &set_op.value,
        ) {
            warn!(
                "[Script] set: failed to set field '{}.{}' on '{}'",
                set_op.component_type,
                set_op.field_path,
                set_op.entity_name.as_deref().unwrap_or("self")
            );
        }
    }
}

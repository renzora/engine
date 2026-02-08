//! ECS (Entity Component System) API functions for Rhai scripts

use rhai::{Engine, Map, ImmutableString};
use super::super::rhai_commands::RhaiCommand;

/// Register ECS functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Entity Spawning
    // ===================

    // spawn_entity(name) - Spawn a new entity
    engine.register_fn("spawn_entity", |name: ImmutableString| {
        super::push_command(RhaiCommand::SpawnEntity { name: name.to_string() });
    });

    // despawn_entity(entity_id) - Despawn an entity by ID
    engine.register_fn("despawn_entity", |entity_id: i64| {
        super::push_command(RhaiCommand::DespawnEntity { entity_id: entity_id as u64 });
    });

    // ===================
    // Primitive Spawning
    // ===================

    // spawn_cube(name) - Spawn a cube mesh
    engine.register_fn("spawn_cube", |name: ImmutableString| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "cube".into(), position: None, scale: None });
    });

    // spawn_cube_at(name, x, y, z) - Spawn a cube at a position
    engine.register_fn("spawn_cube_at", |name: ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "cube".into(), position: Some(bevy::prelude::Vec3::new(x as f32, y as f32, z as f32)), scale: None });
    });

    // spawn_sphere(name) - Spawn a sphere mesh
    engine.register_fn("spawn_sphere", |name: ImmutableString| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "sphere".into(), position: None, scale: None });
    });

    // spawn_sphere_at(name, x, y, z) - Spawn a sphere at a position
    engine.register_fn("spawn_sphere_at", |name: ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "sphere".into(), position: Some(bevy::prelude::Vec3::new(x as f32, y as f32, z as f32)), scale: None });
    });

    // spawn_plane(name) - Spawn a plane mesh
    engine.register_fn("spawn_plane", |name: ImmutableString| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "plane".into(), position: None, scale: None });
    });

    // spawn_plane_at(name, x, y, z) - Spawn a plane at a position
    engine.register_fn("spawn_plane_at", |name: ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "plane".into(), position: Some(bevy::prelude::Vec3::new(x as f32, y as f32, z as f32)), scale: None });
    });

    // spawn_cylinder(name) - Spawn a cylinder mesh
    engine.register_fn("spawn_cylinder", |name: ImmutableString| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "cylinder".into(), position: None, scale: None });
    });

    // spawn_cylinder_at(name, x, y, z) - Spawn a cylinder at a position
    engine.register_fn("spawn_cylinder_at", |name: ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "cylinder".into(), position: Some(bevy::prelude::Vec3::new(x as f32, y as f32, z as f32)), scale: None });
    });

    // spawn_capsule(name) - Spawn a capsule mesh
    engine.register_fn("spawn_capsule", |name: ImmutableString| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "capsule".into(), position: None, scale: None });
    });

    // spawn_capsule_at(name, x, y, z) - Spawn a capsule at a position
    engine.register_fn("spawn_capsule_at", |name: ImmutableString, x: f64, y: f64, z: f64| {
        super::push_command(RhaiCommand::SpawnPrimitive { name: name.to_string(), primitive_type: "capsule".into(), position: Some(bevy::prelude::Vec3::new(x as f32, y as f32, z as f32)), scale: None });
    });

    // despawn_self() - Despawn the current entity
    engine.register_fn("despawn_self", || {
        super::push_command(RhaiCommand::DespawnSelf);
    });

    // ===================
    // Entity Names
    // ===================

    // set_entity_name(entity_id, name)
    engine.register_fn("set_entity_name", |entity_id: i64, name: ImmutableString| {
        super::push_command(RhaiCommand::SetEntityName { entity_id: entity_id as u64, name: name.to_string() });
    });

    // ===================
    // Tags
    // ===================

    // add_tag(tag) - Add tag to self
    engine.register_fn("add_tag", |tag: ImmutableString| {
        super::push_command(RhaiCommand::AddTag { entity_id: None, tag: tag.to_string() });
    });

    // add_tag_to(entity_id, tag) - Add tag to entity
    engine.register_fn("add_tag_to", |entity_id: i64, tag: ImmutableString| {
        super::push_command(RhaiCommand::AddTag { entity_id: Some(entity_id as u64), tag: tag.to_string() });
    });

    // remove_tag(tag) - Remove tag from self
    engine.register_fn("remove_tag", |tag: ImmutableString| {
        super::push_command(RhaiCommand::RemoveTag { entity_id: None, tag: tag.to_string() });
    });

    // remove_tag_from(entity_id, tag) - Remove tag from entity
    engine.register_fn("remove_tag_from", |entity_id: i64, tag: ImmutableString| {
        super::push_command(RhaiCommand::RemoveTag { entity_id: Some(entity_id as u64), tag: tag.to_string() });
    });

    // ===================
    // Entity Queries (these use pre-populated maps in scope)
    // ===================

    // find_entity_by_name(entities_map, name) - Get entity ID by name
    engine.register_fn("find_entity_by_name", |entities_map: Map, name: ImmutableString| -> i64 {
        entities_map.get(name.as_str())
            .and_then(|v| v.clone().try_cast::<i64>())
            .unwrap_or(-1)
    });

    // entity_exists(entities_map, name) - Check if entity exists by name
    engine.register_fn("entity_exists", |entities_map: Map, name: ImmutableString| -> bool {
        entities_map.contains_key(name.as_str())
    });

    // get_entities_by_tag(tag_map, tag) - Get array of entity IDs with the given tag
    engine.register_fn("get_entities_by_tag", |tag_map: Map, tag: ImmutableString| -> rhai::Array {
        tag_map.get(tag.as_str())
            .and_then(|v| v.clone().try_cast::<rhai::Array>())
            .unwrap_or_else(rhai::Array::new)
    });

    // has_entities_with_tag(tag_map, tag) - Check if any entities have the given tag
    engine.register_fn("has_entities_with_tag", |tag_map: Map, tag: ImmutableString| -> bool {
        tag_map.contains_key(tag.as_str())
    });

    // count_entities_by_tag(tag_map, tag) - Get count of entities with the given tag
    engine.register_fn("count_entities_by_tag", |tag_map: Map, tag: ImmutableString| -> i64 {
        tag_map.get(tag.as_str())
            .and_then(|v| v.clone().try_cast::<rhai::Array>())
            .map(|arr| arr.len() as i64)
            .unwrap_or(0)
    });
}

//! ECS (Entity Component System) API functions for Rhai scripts

use rhai::{Dynamic, Engine, Map, ImmutableString};

/// Register ECS functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Entity Spawning
    // ===================

    // spawn_entity(name) - Spawn a new entity
    engine.register_fn("spawn_entity", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_entity"));
        m.insert("name".into(), Dynamic::from(name));
        m
    });

    // despawn_entity(entity_id) - Despawn an entity by ID
    engine.register_fn("despawn_entity", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("despawn_entity"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m
    });

    // ===================
    // Primitive Spawning
    // ===================

    // spawn_cube(name) - Spawn a cube mesh
    engine.register_fn("spawn_cube", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("cube"));
        m
    });

    // spawn_cube_at(name, x, y, z) - Spawn a cube at a position
    engine.register_fn("spawn_cube_at", |name: ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("cube"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // spawn_sphere(name) - Spawn a sphere mesh
    engine.register_fn("spawn_sphere", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("sphere"));
        m
    });

    // spawn_sphere_at(name, x, y, z) - Spawn a sphere at a position
    engine.register_fn("spawn_sphere_at", |name: ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("sphere"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // spawn_plane(name) - Spawn a plane mesh
    engine.register_fn("spawn_plane", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("plane"));
        m
    });

    // spawn_plane_at(name, x, y, z) - Spawn a plane at a position
    engine.register_fn("spawn_plane_at", |name: ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("plane"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // spawn_cylinder(name) - Spawn a cylinder mesh
    engine.register_fn("spawn_cylinder", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("cylinder"));
        m
    });

    // spawn_cylinder_at(name, x, y, z) - Spawn a cylinder at a position
    engine.register_fn("spawn_cylinder_at", |name: ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("cylinder"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // spawn_capsule(name) - Spawn a capsule mesh
    engine.register_fn("spawn_capsule", |name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("capsule"));
        m
    });

    // spawn_capsule_at(name, x, y, z) - Spawn a capsule at a position
    engine.register_fn("spawn_capsule_at", |name: ImmutableString, x: f64, y: f64, z: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("spawn_primitive"));
        m.insert("name".into(), Dynamic::from(name));
        m.insert("primitive_type".into(), Dynamic::from("capsule"));
        m.insert("x".into(), Dynamic::from(x));
        m.insert("y".into(), Dynamic::from(y));
        m.insert("z".into(), Dynamic::from(z));
        m
    });

    // despawn_self() - Despawn the current entity
    engine.register_fn("despawn_self", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("despawn_self"));
        m
    });

    // ===================
    // Entity Names
    // ===================

    // set_entity_name(entity_id, name)
    engine.register_fn("set_entity_name", |entity_id: i64, name: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_entity_name"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("name".into(), Dynamic::from(name));
        m
    });

    // ===================
    // Tags
    // ===================

    // add_tag(tag) - Add tag to self
    engine.register_fn("add_tag", |tag: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("add_tag"));
        m.insert("tag".into(), Dynamic::from(tag));
        m
    });

    // add_tag_to(entity_id, tag) - Add tag to entity
    engine.register_fn("add_tag_to", |entity_id: i64, tag: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("add_tag"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("tag".into(), Dynamic::from(tag));
        m
    });

    // remove_tag(tag) - Remove tag from self
    engine.register_fn("remove_tag", |tag: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("remove_tag"));
        m.insert("tag".into(), Dynamic::from(tag));
        m
    });

    // remove_tag_from(entity_id, tag) - Remove tag from entity
    engine.register_fn("remove_tag_from", |entity_id: i64, tag: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("remove_tag"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("tag".into(), Dynamic::from(tag));
        m
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

    // ===================
    // Self Entity
    // ===================

    // get_self_entity_id() is provided via scope variable: self_entity_id
    // get_self_entity_name() is provided via scope variable: self_entity_name
}

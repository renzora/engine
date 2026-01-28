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

    // ===================
    // Self Entity
    // ===================

    // get_self_entity_id() is provided via scope variable: self_entity_id
    // get_self_entity_name() is provided via scope variable: self_entity_name
}

//! Component API functions for Rhai scripts
//!
//! Provides access to component data on entities.

use rhai::{Dynamic, Engine, Map, ImmutableString};

/// Register component functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Health Component
    // ===================

    // get_health() - Get current health of self
    // Returns the health value from scope variable (populated by runtime)
    engine.register_fn("get_health", || -> f64 {
        // This is a placeholder - actual value comes from scope
        0.0
    });

    // set_health(value) - Set health of self
    engine.register_fn("set_health", |value: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_health"));
        m.insert("value".into(), Dynamic::from(value));
        m
    });

    // set_health_of(entity_id, value) - Set health of entity
    engine.register_fn("set_health_of", |entity_id: i64, value: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_health"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("value".into(), Dynamic::from(value));
        m
    });

    // set_max_health(value) - Set max health of self
    engine.register_fn("set_max_health", |value: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_max_health"));
        m.insert("value".into(), Dynamic::from(value));
        m
    });

    // set_max_health_of(entity_id, value) - Set max health of entity
    engine.register_fn("set_max_health_of", |entity_id: i64, value: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_max_health"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("value".into(), Dynamic::from(value));
        m
    });

    // damage(amount) - Deal damage to self
    engine.register_fn("damage", |amount: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("damage"));
        m.insert("amount".into(), Dynamic::from(amount));
        m
    });

    // damage_entity(entity_id, amount) - Deal damage to entity
    engine.register_fn("damage_entity", |entity_id: i64, amount: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("damage"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("amount".into(), Dynamic::from(amount));
        m
    });

    // heal(amount) - Heal self
    engine.register_fn("heal", |amount: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("heal"));
        m.insert("amount".into(), Dynamic::from(amount));
        m
    });

    // heal_entity(entity_id, amount) - Heal entity
    engine.register_fn("heal_entity", |entity_id: i64, amount: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("heal"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("amount".into(), Dynamic::from(amount));
        m
    });

    // set_invincible(invincible) - Set invincibility of self
    engine.register_fn("set_invincible", |invincible: bool| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_invincible"));
        m.insert("invincible".into(), Dynamic::from(invincible));
        m
    });

    // set_invincible_of(entity_id, invincible) - Set invincibility of entity
    engine.register_fn("set_invincible_of", |entity_id: i64, invincible: bool| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_invincible"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("invincible".into(), Dynamic::from(invincible));
        m
    });

    // set_invincible_duration(invincible, duration) - Set invincibility of self with duration
    engine.register_fn("set_invincible_duration", |invincible: bool, duration: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_invincible"));
        m.insert("invincible".into(), Dynamic::from(invincible));
        m.insert("duration".into(), Dynamic::from(duration));
        m
    });

    // set_invincible_of_duration(entity_id, invincible, duration) - Set invincibility of entity with duration
    engine.register_fn("set_invincible_of_duration", |entity_id: i64, invincible: bool, duration: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_invincible"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("invincible".into(), Dynamic::from(invincible));
        m.insert("duration".into(), Dynamic::from(duration));
        m
    });

    // kill() - Instantly kill self (set health to 0)
    engine.register_fn("kill", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("kill"));
        m
    });

    // kill_entity(entity_id) - Instantly kill entity
    engine.register_fn("kill_entity", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("kill"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m
    });

    // revive() - Revive self (restore to max health)
    engine.register_fn("revive", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("revive"));
        m
    });

    // revive_entity(entity_id) - Revive entity
    engine.register_fn("revive_entity", |entity_id: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("revive"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m
    });

    // is_dead() - Check if self is dead (health <= 0)
    // Uses scope variable self_health
    engine.register_fn("is_dead", |self_health: f64| -> bool {
        self_health <= 0.0
    });

    // ===================
    // Light Component
    // ===================

    // set_light_color(r, g, b) - Set light color of self
    engine.register_fn("set_light_color", |r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_light_color"));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m
    });

    // set_light_color_of(entity_id, r, g, b) - Set light color of entity
    engine.register_fn("set_light_color_of", |entity_id: i64, r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_light_color"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m
    });

    // set_light_intensity(intensity) - Set light intensity of self
    engine.register_fn("set_light_intensity", |intensity: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_light_intensity"));
        m.insert("intensity".into(), Dynamic::from(intensity));
        m
    });

    // set_light_intensity_of(entity_id, intensity) - Set light intensity of entity
    engine.register_fn("set_light_intensity_of", |entity_id: i64, intensity: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_light_intensity"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("intensity".into(), Dynamic::from(intensity));
        m
    });

    // ===================
    // Material Component
    // ===================

    // set_material_color(r, g, b, a) - Set material base color of self
    engine.register_fn("set_material_color", |r: f64, g: f64, b: f64, a: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_material_color"));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(a));
        m
    });

    // set_material_color_of(entity_id, r, g, b, a) - Set material base color of entity
    engine.register_fn("set_material_color_of", |entity_id: i64, r: f64, g: f64, b: f64, a: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_material_color"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m.insert("a".into(), Dynamic::from(a));
        m
    });

    // set_material_emissive(r, g, b) - Set emissive color of self
    engine.register_fn("set_material_emissive", |r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_material_emissive"));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m
    });

    // set_material_emissive_of(entity_id, r, g, b) - Set emissive color of entity
    engine.register_fn("set_material_emissive_of", |entity_id: i64, r: f64, g: f64, b: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_material_emissive"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("r".into(), Dynamic::from(r));
        m.insert("g".into(), Dynamic::from(g));
        m.insert("b".into(), Dynamic::from(b));
        m
    });

    // ===================
    // Generic Component Access
    // ===================

    // set_component(component_type, field, value) - Set a component field on self
    engine.register_fn("set_component_float", |component_type: ImmutableString, field: ImmutableString, value: f64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_component_field"));
        m.insert("component_type".into(), Dynamic::from(component_type));
        m.insert("field".into(), Dynamic::from(field));
        m.insert("value".into(), Dynamic::from(value));
        m.insert("value_type".into(), Dynamic::from("float"));
        m
    });

    engine.register_fn("set_component_int", |component_type: ImmutableString, field: ImmutableString, value: i64| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_component_field"));
        m.insert("component_type".into(), Dynamic::from(component_type));
        m.insert("field".into(), Dynamic::from(field));
        m.insert("value".into(), Dynamic::from(value));
        m.insert("value_type".into(), Dynamic::from("int"));
        m
    });

    engine.register_fn("set_component_bool", |component_type: ImmutableString, field: ImmutableString, value: bool| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_component_field"));
        m.insert("component_type".into(), Dynamic::from(component_type));
        m.insert("field".into(), Dynamic::from(field));
        m.insert("value".into(), Dynamic::from(value));
        m.insert("value_type".into(), Dynamic::from("bool"));
        m
    });

    engine.register_fn("set_component_string", |component_type: ImmutableString, field: ImmutableString, value: ImmutableString| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_component_field"));
        m.insert("component_type".into(), Dynamic::from(component_type));
        m.insert("field".into(), Dynamic::from(field));
        m.insert("value".into(), Dynamic::from(value));
        m.insert("value_type".into(), Dynamic::from("string"));
        m
    });

    // ===================
    // Visibility
    // ===================

    // set_visible(visible) - Set visibility of self
    engine.register_fn("set_visible", |visible: bool| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_visibility"));
        m.insert("visible".into(), Dynamic::from(visible));
        m
    });

    // set_visible_of(entity_id, visible) - Set visibility of entity
    engine.register_fn("set_visible_of", |entity_id: i64, visible: bool| -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_visibility"));
        m.insert("entity_id".into(), Dynamic::from(entity_id));
        m.insert("visible".into(), Dynamic::from(visible));
        m
    });

    // show() - Make self visible
    engine.register_fn("show", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_visibility"));
        m.insert("visible".into(), Dynamic::from(true));
        m
    });

    // hide() - Make self invisible
    engine.register_fn("hide", || -> Map {
        let mut m = Map::new();
        m.insert("_cmd".into(), Dynamic::from("set_visibility"));
        m.insert("visible".into(), Dynamic::from(false));
        m
    });
}

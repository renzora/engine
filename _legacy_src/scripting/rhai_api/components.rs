//! Component API functions for Rhai scripts

use rhai::{Engine, ImmutableString};
use super::super::rhai_commands::{RhaiCommand, ComponentValue};

/// Register component functions
pub fn register(engine: &mut Engine) {
    // ===================
    // Health Component
    // ===================

    engine.register_fn("get_health", || -> f64 { 0.0 });

    engine.register_fn("set_health", |value: f64| {
        super::push_command(RhaiCommand::SetHealth { entity_id: None, value: value as f32 });
    });

    engine.register_fn("set_health_of", |entity_id: i64, value: f64| {
        super::push_command(RhaiCommand::SetHealth { entity_id: Some(entity_id as u64), value: value as f32 });
    });

    engine.register_fn("set_max_health", |value: f64| {
        super::push_command(RhaiCommand::SetMaxHealth { entity_id: None, value: value as f32 });
    });

    engine.register_fn("set_max_health_of", |entity_id: i64, value: f64| {
        super::push_command(RhaiCommand::SetMaxHealth { entity_id: Some(entity_id as u64), value: value as f32 });
    });

    engine.register_fn("damage", |amount: f64| {
        super::push_command(RhaiCommand::Damage { entity_id: None, amount: amount as f32 });
    });

    engine.register_fn("damage_entity", |entity_id: i64, amount: f64| {
        super::push_command(RhaiCommand::Damage { entity_id: Some(entity_id as u64), amount: amount as f32 });
    });

    engine.register_fn("heal", |amount: f64| {
        super::push_command(RhaiCommand::Heal { entity_id: None, amount: amount as f32 });
    });

    engine.register_fn("heal_entity", |entity_id: i64, amount: f64| {
        super::push_command(RhaiCommand::Heal { entity_id: Some(entity_id as u64), amount: amount as f32 });
    });

    engine.register_fn("set_invincible", |invincible: bool| {
        super::push_command(RhaiCommand::SetInvincible { entity_id: None, invincible, duration: 0.0 });
    });

    engine.register_fn("set_invincible_of", |entity_id: i64, invincible: bool| {
        super::push_command(RhaiCommand::SetInvincible { entity_id: Some(entity_id as u64), invincible, duration: 0.0 });
    });

    engine.register_fn("set_invincible_duration", |invincible: bool, duration: f64| {
        super::push_command(RhaiCommand::SetInvincible { entity_id: None, invincible, duration: duration as f32 });
    });

    engine.register_fn("set_invincible_of_duration", |entity_id: i64, invincible: bool, duration: f64| {
        super::push_command(RhaiCommand::SetInvincible { entity_id: Some(entity_id as u64), invincible, duration: duration as f32 });
    });

    engine.register_fn("kill", || {
        super::push_command(RhaiCommand::Kill { entity_id: None });
    });

    engine.register_fn("kill_entity", |entity_id: i64| {
        super::push_command(RhaiCommand::Kill { entity_id: Some(entity_id as u64) });
    });

    engine.register_fn("revive", || {
        super::push_command(RhaiCommand::Revive { entity_id: None });
    });

    engine.register_fn("revive_entity", |entity_id: i64| {
        super::push_command(RhaiCommand::Revive { entity_id: Some(entity_id as u64) });
    });

    engine.register_fn("is_dead", |self_health: f64| -> bool {
        self_health <= 0.0
    });

    // ===================
    // Light Component (duplicates from rendering — kept for backwards compatibility)
    // ===================

    engine.register_fn("set_light_color", |r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::SetLightColor { entity_id: None, color: [r as f32, g as f32, b as f32] });
    });

    engine.register_fn("set_light_color_of", |entity_id: i64, r: f64, g: f64, b: f64| {
        super::push_command(RhaiCommand::SetLightColor { entity_id: Some(entity_id as u64), color: [r as f32, g as f32, b as f32] });
    });

    engine.register_fn("set_light_intensity", |intensity: f64| {
        super::push_command(RhaiCommand::SetLightIntensity { entity_id: None, intensity: intensity as f32 });
    });

    engine.register_fn("set_light_intensity_of", |entity_id: i64, intensity: f64| {
        super::push_command(RhaiCommand::SetLightIntensity { entity_id: Some(entity_id as u64), intensity: intensity as f32 });
    });

    // ===================
    // Material Component
    // ===================

    engine.register_fn("set_material_color", |r: f64, g: f64, b: f64, a: f64| {
        super::push_command(RhaiCommand::SetMaterialColor { entity_id: None, color: [r as f32, g as f32, b as f32, a as f32] });
    });

    engine.register_fn("set_material_color_of", |entity_id: i64, r: f64, g: f64, b: f64, a: f64| {
        super::push_command(RhaiCommand::SetMaterialColor { entity_id: Some(entity_id as u64), color: [r as f32, g as f32, b as f32, a as f32] });
    });

    engine.register_fn("set_material_emissive", |_r: f64, _g: f64, _b: f64| {
        // Emissive handled through component system
    });

    engine.register_fn("set_material_emissive_of", |_entity_id: i64, _r: f64, _g: f64, _b: f64| {
        // Emissive handled through component system
    });

    // ===================
    // Generic Component Access
    // ===================

    engine.register_fn("set_component_float", |component_type: ImmutableString, field: ImmutableString, value: f64| {
        super::push_command(RhaiCommand::SetComponentField {
            entity_id: None, component_type: component_type.to_string(),
            field_name: field.to_string(), value: ComponentValue::Float(value as f32),
        });
    });

    engine.register_fn("set_component_int", |component_type: ImmutableString, field: ImmutableString, value: i64| {
        super::push_command(RhaiCommand::SetComponentField {
            entity_id: None, component_type: component_type.to_string(),
            field_name: field.to_string(), value: ComponentValue::Int(value),
        });
    });

    engine.register_fn("set_component_bool", |component_type: ImmutableString, field: ImmutableString, value: bool| {
        super::push_command(RhaiCommand::SetComponentField {
            entity_id: None, component_type: component_type.to_string(),
            field_name: field.to_string(), value: ComponentValue::Bool(value),
        });
    });

    engine.register_fn("set_component_string", |component_type: ImmutableString, field: ImmutableString, value: ImmutableString| {
        super::push_command(RhaiCommand::SetComponentField {
            entity_id: None, component_type: component_type.to_string(),
            field_name: field.to_string(), value: ComponentValue::String(value.to_string()),
        });
    });

    // ===================
    // Visibility
    // ===================

    engine.register_fn("set_visible", |visible: bool| {
        super::push_command(RhaiCommand::SetVisibility { entity_id: None, visible });
    });

    engine.register_fn("set_visible_of", |entity_id: i64, visible: bool| {
        super::push_command(RhaiCommand::SetVisibility { entity_id: Some(entity_id as u64), visible });
    });

    engine.register_fn("show", || {
        super::push_command(RhaiCommand::SetVisibility { entity_id: None, visible: true });
    });

    engine.register_fn("hide", || {
        super::push_command(RhaiCommand::SetVisibility { entity_id: None, visible: false });
    });
}

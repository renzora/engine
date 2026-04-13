//! Gauge script integration — handles gauge actions from scripts.
//!
//! Scripts use the generic `action()` function for gauge operations:
//!
//! ```lua
//! action("gauge_set", { name = "health", value = 100 })
//! action("gauge_damage", { name = "health", amount = 25 })
//! action("gauge_heal", { name = "health", amount = 10 })
//! action("gauge_set_target", { target = entity_id, name = "health", value = 50 })
//! ```

use bevy::prelude::*;
use renzora_core::ScriptAction;

use bevy_gauge::prelude::InstantExt;
use crate::{AttributesMut, Modifier, InstantModifierSet};

/// Observer that handles gauge-related ScriptAction events.
pub fn handle_gauge_script_actions(
    trigger: On<ScriptAction>,
    mut attrs: AttributesMut,
) {
    use renzora_core::ScriptActionValue;
    let action = trigger.event();

    let get_str = |key: &str| -> String {
        match action.args.get(key) {
            Some(ScriptActionValue::String(s)) => s.clone(),
            _ => String::new(),
        }
    };
    let get_f32 = |key: &str| -> f32 {
        match action.args.get(key) {
            Some(ScriptActionValue::Float(v)) => *v,
            Some(ScriptActionValue::Int(v)) => *v as f32,
            _ => 0.0,
        }
    };
    let get_target = || -> Entity {
        match action.args.get("target") {
            Some(ScriptActionValue::Int(id)) => Entity::from_bits(*id as u64),
            Some(ScriptActionValue::Float(id)) => Entity::from_bits(*id as u64),
            _ => action.entity,
        }
    };

    match action.name.as_str() {
        "gauge_set" => {
            let name = get_str("name");
            let value = get_f32("value");
            let target = get_target();
            attrs.set(target, &name, value);
        }
        "gauge_add_modifier" => {
            let name = get_str("name");
            let value = get_f32("value");
            let target = get_target();
            attrs.add_modifier(target, &name, Modifier::Flat(value));
        }
        "gauge_remove_modifier" => {
            let name = get_str("name");
            let value = get_f32("value");
            let target = get_target();
            attrs.remove_modifier(target, &name, &Modifier::Flat(value));
        }
        "gauge_damage" => {
            let name = get_str("name");
            let amount = get_f32("amount");
            let target = get_target();
            let mut instant = InstantModifierSet::new();
            instant.push_sub(&name, amount);
            attrs.apply_instant(&instant, &[], target);
        }
        "gauge_heal" => {
            let name = get_str("name");
            let amount = get_f32("amount");
            let target = get_target();
            let mut instant = InstantModifierSet::new();
            instant.push_add(&name, amount);
            attrs.apply_instant(&instant, &[], target);
        }
        "gauge_instant" => {
            let name = get_str("name");
            let op = get_str("op");
            let value = get_f32("value");
            let target = get_target();
            let mut instant = InstantModifierSet::new();
            match op.as_str() {
                "add" => instant.push_add(&name, value),
                "subtract" | "sub" => instant.push_sub(&name, value),
                "set" => instant.push_set(&name, value),
                _ => {
                    warn!("Unknown gauge instant op '{}', use add/subtract/set", op);
                }
            }
            attrs.apply_instant(&instant, &[], target);
        }
        _ => {} // Not a gauge action
    }
}

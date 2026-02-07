//! Scripting component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::scripting::ScriptComponent;
use crate::ui::property_row;

use egui_phosphor::regular::CODE;

// ScriptComponent doesn't derive Serialize/Deserialize/Default,
// so we keep the static definition pattern instead of register_component! macro.

pub static SCRIPT: ComponentDefinition = ComponentDefinition {
    type_id: "script",
    display_name: "Script",
    category: ComponentCategory::Scripting,
    icon: CODE,
    priority: 0,
    add_fn: add_script,
    remove_fn: remove_script,
    has_fn: has_script,
    serialize_fn: serialize_script,
    deserialize_fn: deserialize_script,
    inspector_fn: inspect_script,
    conflicts_with: &[],
    requires: &[],
};

pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&SCRIPT);
}

fn add_script(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(ScriptComponent {
        script_id: String::new(),
        script_path: None,
        enabled: true,
        variables: Default::default(),
        runtime_state: Default::default(),
    });
}

fn remove_script(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<ScriptComponent>();
}

fn has_script(world: &World, entity: Entity) -> bool {
    world.get::<ScriptComponent>(entity).is_some()
}

fn serialize_script(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let script = world.get::<ScriptComponent>(entity)?;
    Some(json!({
        "script_id": script.script_id,
        "script_path": script.script_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        "enabled": script.enabled
    }))
}

fn deserialize_script(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    entity_commands.insert(ScriptComponent {
        script_id: data.get("script_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        script_path: data.get("script_path").and_then(|v| v.as_str()).map(std::path::PathBuf::from),
        enabled: data.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
        variables: Default::default(),
        runtime_state: Default::default(),
    });
}

fn inspect_script(
    ui: &mut egui::Ui, world: &mut World, entity: Entity,
    _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut script) = world.get_mut::<ScriptComponent>(entity) else {
        return false;
    };
    let mut changed = false;

    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Script ID");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut id = script.script_id.clone();
                if ui.text_edit_singleline(&mut id).changed() {
                    script.script_id = id;
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Path");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut path_str = script.script_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                if ui.text_edit_singleline(&mut path_str).changed() {
                    script.script_path = if path_str.is_empty() {
                        None
                    } else {
                        Some(std::path::PathBuf::from(path_str))
                    };
                    changed = true;
                }
            });
        });
    });

    changed
}

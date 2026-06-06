//! Inspector entry for the ScriptComponent.
//!
//! Registered automatically when the `editor` feature is enabled.
//!
//! The native (bevy_ui / ember) drawer lives in `renzora_inspector`; this module
//! only registers the component's `InspectorEntry`.

use bevy::prelude::*;
use egui_phosphor::regular;
use renzora_editor::InspectorEntry;

use crate::component::ScriptComponent;

/// Register the script inspector entry via `AppEditorExt`.
pub fn register_script_inspector(app: &mut App) {
    use renzora_editor::AppEditorExt;
    app.register_inspector(script_component_entry());
}

fn script_component_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "script_component",
        display_name: "Scripts",
        icon: regular::CODE,
        category: "scripting",
        has_fn: |world, entity| world.get::<ScriptComponent>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(ScriptComponent::new());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<ScriptComponent>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

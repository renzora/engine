//! Editor-only half of `renzora_scripting` — the inspector entry for the
//! `ScriptComponent` (a renzora editor-contract `InspectorEntry` with a Phosphor
//! icon).
//!
//! `renzora_scripting` compiles lean (no `editor` feature, no egui-phosphor). This
//! crate holds the inspector entry (which reads/writes the `pub`
//! `renzora_scripting::ScriptComponent` runtime component), registered
//! `renzora::add!(ScriptingEditorPlugin, Editor)`, linked only by the editor
//! bundle. The native (bevy_ui / ember) drawer lives in `renzora_inspector`.

use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};

use renzora_scripting::ScriptComponent;

// ============================================================================
// Inspector entry
// ============================================================================

fn script_component_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "script_component",
        display_name: "Scripts",
        icon: "scroll",
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

// ============================================================================
// Plugin
// ============================================================================

/// Editor-scope companion to `renzora_scripting::ScriptingPlugin`. Reproduces the
/// inspector registration the runtime plugin did under `#[cfg(feature = "editor")]`.
#[derive(Default)]
pub struct ScriptingEditorPlugin;

impl Plugin for ScriptingEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ScriptingEditorPlugin");
        app.register_inspector(script_component_entry());
    }
}

renzora::add!(ScriptingEditorPlugin, Editor);

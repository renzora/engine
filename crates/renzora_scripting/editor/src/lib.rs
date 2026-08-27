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

// ============================================================================
// Inspector entry
// ============================================================================

/// Scripts read as **inherent to every entity** (`has_fn` is unconditionally
/// true) rather than as an Add-Component entry, the same way `renzora_light2d`
/// makes 2D lighting inherent to a `Camera2d`. Attaching a script is one of the
/// most common things anyone does in the editor, and routing it through
/// Add Component → search "Scripts" → then drop the file was two clicks of pure
/// ceremony in front of the actual action.
///
/// The component itself is still **absent until a script is attached** — the
/// drawer's add-bar inserts it on the first drop, and the last removal takes it
/// away again (see `renzora_inspector::scripts`). That matters: the component is
/// registered for reflection, so a materialised-but-empty one on every entity
/// would serialise into every saved scene and put every entity into the queries
/// that drive script execution and hot-reload. Always-visible UI over an
/// absent component gives the same feel for nothing.
///
/// `add_fn`/`remove_fn` are therefore `None`. An always-true `has_fn` already
/// hides the entry from the Add Component menu, which skips anything already
/// present, and a component-level remove button would be a second, confusing
/// way to do what removing the last script entry does.
fn script_component_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "script_component",
        display_name: "Scripts",
        icon: "scroll",
        category: "scripting",
        has_fn: |_world, _entity| true,
        add_fn: None,
        remove_fn: None,
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

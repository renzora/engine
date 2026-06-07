//! Inspector panel — shows and edits component properties for the selected entity.
//!
//! The panel is bevy_ui (ember) native; see [`native`]. Component drawers are
//! registered into the [`renzora_editor_framework::NativeInspectorRegistry`] (e.g. the
//! script drawer in [`scripts`]); the reusable [`native::asset_drop_field`] is
//! re-exported for drawers in other crates.

mod native;
mod scripts;

pub use native::asset_drop_field;

use bevy::prelude::*;
use renzora_editor_framework::InspectorRegistry;

/// Plugin that registers the native inspector panel and built-in component
/// inspectors.
#[derive(Default)]
pub struct InspectorPanelPlugin;

impl Plugin for InspectorPanelPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] InspectorPanelPlugin");
        // Inspector entries are now self-registered by their owning crates:
        // - Bevy built-ins: renzora_editor_framework::bevy_inspectors
        // - Physics: renzora_physics::inspector (editor feature)
        // - Scripts: renzora_scripting::inspector (editor feature)
        // - Material: renzora_material_editor::material_inspector
        app.init_resource::<InspectorRegistry>();

        // The bevy_ui-native inspector panel + the native script drawer.
        native::register_native_inspector(app);
        scripts::register(app);
    }
}

renzora::add!(InspectorPanelPlugin, Editor);

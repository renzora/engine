//! Inspector panel — shows and edits component properties for the selected entity.
//!
//! The panel itself lives in [`panel`]. Component drawers are registered into
//! the [`renzora_editor_framework::NativeInspectorRegistry`] (e.g. the script
//! drawer in [`scripts`]); the reusable [`panel::asset_drop_field`] is
//! re-exported for drawers in other crates.

mod camera_presets;
mod entity_header;
mod panel;
pub mod reflect_source;
mod resources;
mod richtext;
mod scripts;
mod textfont;

pub use panel::asset_drop_field;

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

        // The inspector panel + the script drawer.
        panel::register(app);
        scripts::register(app);
        camera_presets::register(app);
        textfont::register(app);
        resources::register(app);
        richtext::register(app);
    }

    /// `register_settings_section` needs `&mut App`, and the plugin list it
    /// renders is only complete once every loader's `build` has run. `finish`
    /// is the one hook that satisfies both constraints.
    fn finish(&self, app: &mut App) {
        plugin_manager::register(app);
    }
}

renzora::add!(InspectorPanelPlugin, Editor);

pub mod plugin_manager;

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
        plugin_resources::register(app);
        resources::register(app);
        richtext::register(app);
        // Plugin components only exist after `load_global_plugins` has run, so
        // their sections cannot be registered at plugin-build time. A startup
        // system picks them up once everything is loaded.
        app.add_systems(
            Startup,
            |world: &mut World| plugin_fields::register_plugin_component_sections(world),
        );
    }

    /// Panels need `&mut App` — `register_panel_content` is an `App` extension —
    /// so they cannot wait for a startup system the way component sections do.
    /// `finish` runs after every plugin's `build`, including the loader's, which
    /// is the one hook that satisfies both constraints.
    fn finish(&self, app: &mut App) {
        plugin_panels::register_plugin_panels(app);
        // Same hook, same reason: `register_settings_section` needs `&mut App`,
        // and the list it renders is only complete once every loader's `build`
        // has run.
        plugin_manager::register(app);
    }
}

renzora::add!(InspectorPanelPlugin, Editor);

pub mod plugin_fields;
pub mod plugin_manager;
pub mod plugin_panels;
pub mod plugin_resources;

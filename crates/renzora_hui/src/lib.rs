//! Renzora HUI — author bevy_ui as hot-reloadable pseudo-HTML/XML templates.
//!
//! Wraps the `bevy_hui` crate so UI can be written as markup (à la Unity's UI
//! Toolkit / React components) instead of Rust spawn code. Templates live under
//! `assets/ui/*.html`; reusable component templates under `assets/ui/components/`
//! are auto-registered by [`bevy_hui::HuiAutoLoadPlugin`].

use bevy::prelude::*;

pub mod lua_bridge;

#[cfg(feature = "editor")]
pub mod editor;

/// Folders (relative to the asset root) scanned by [`bevy_hui::HuiAutoLoadPlugin`]
/// for reusable component templates that register themselves by file stem.
const AUTOLOAD_DIRS: &[&str] = &["ui/components"];

#[derive(Default)]
pub struct HuiPlugin;

impl Plugin for HuiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            bevy_hui::prelude::HuiPlugin,
            bevy_hui::prelude::HuiAutoLoadPlugin::new(AUTOLOAD_DIRS),
        ))
        // Markup callbacks with no Rust binding fall through to scripts'
        // `on_ui` hook; this inbox carries them to `renzora_scripting`.
        .init_resource::<renzora::ScriptUiInbox>()
        .add_systems(Update, lua_bridge::register_lua_forwarders)
        .add_observer(lua_bridge::handle_hui_spawn);

        // Editor-only: make template entities show up as draggable widgets in
        // the canvas preview and persist drag positions across hot-reloads.
        #[cfg(feature = "editor")]
        app.add_plugins(editor::HuiEditorPlugin);
    }
}

renzora::add!(HuiPlugin);

//! Plugin Core System
//!
//! This module provides the infrastructure for loading and managing editor plugins.
//! It uses stabby for ABI stability, allowing plugins to be compiled separately
//! from the main editor.

pub mod abi;
pub mod api;
pub mod dependency;
pub mod host;
pub mod registry;
pub mod traits;

pub use abi::*;
pub use api::*;
pub use host::PluginHost;
pub use registry::PluginRegistry;
pub use traits::*;

use bevy::prelude::*;

/// Plugin that manages the plugin host lifecycle
pub struct PluginCorePlugin;

impl Plugin for PluginCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PluginHost>()
            .init_resource::<PluginRegistry>()
            .add_systems(Startup, initialize_plugin_host)
            .add_systems(Update, update_plugins);
    }
}

fn initialize_plugin_host(mut plugin_host: ResMut<PluginHost>) {
    if let Err(e) = plugin_host.discover_and_load_plugins() {
        error!("Failed to load plugins: {}", e);
    }
}

fn update_plugins(mut plugin_host: ResMut<PluginHost>, time: Res<Time>) {
    plugin_host.update(time.delta_secs());
}

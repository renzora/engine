//! Seeds the editable disable list the Settings UI binds to.
//!
//! The UI that renders it lives in `renzora_settings` (Settings → Editor →
//! Plugins) and reads only contract-crate types, so it needs no dependency on
//! the loader.
//!
//! # Where the inventory comes from
//!
//! `renzora_native_plugin`'s one scan finds every plugin and records it in
//! [`renzora::PluginInventory`] as it runs, so a panel never has to re-derive
//! the loader's rules. Nothing here scans anything.

use bevy::prelude::*;

use renzora::{DisabledPlugins, PluginInventory};

pub fn register(app: &mut App) {
    app.init_resource::<PluginInventory>();
    // Seeded from disk once. The loader has already read the same file for
    // itself — it runs before any resource exists — and this is the editable
    // mirror the settings UI binds to.
    app.insert_resource(DisabledPlugins(renzora::load_disabled_plugins()));
}

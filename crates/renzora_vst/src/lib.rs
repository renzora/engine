//! Audio plugin (VST/CLAP) host scaffolding for Renzora.
//!
//! Today this crate provides:
//!   - The internal data model for plugin descriptors and per-bus insert
//!     chains (`PluginDescriptor`, `PluginInstanceSlot`, `PluginInsertChain`,
//!     `BusInserts`, `PluginRegistry`).
//!   - A background scanner that walks the platform's standard CLAP plugin
//!     paths and lists every `.clap` file found.
//!   - Bridge systems that mirror the private state to read-only resources
//!     in `renzora_audio` (`BusInsertsSummary`, `PluginCatalog`) and consume
//!     `MixerFxCommand` messages, so other crates (the mixer panel, the DAW
//!     panel) never need to link against `renzora_vst` directly.
//!
//! What this crate does NOT yet do (planned, gated behind the `clap-host`
//! feature in Cargo.toml):
//!   - Actually loading a `.clap` bundle and reading its plugin descriptors
//!     (needs `clack-host` from git — see the memory note
//!     `reference_clap_hosting_in_rust.md`).
//!   - Instantiating plugins or processing audio through them.
//!   - Hosting plugin GUIs.

use bevy::prelude::*;

pub mod bridge;
pub mod host;
#[cfg(feature = "clap-host")]
pub mod host_impl;
pub mod registry;
pub mod scan;
pub mod insert;

pub use insert::{BusInserts, PluginInsertChain, PluginInstanceSlot};
pub use registry::{PluginDescriptor, PluginId, PluginRegistry};

/// Bevy plugin: registers `PluginRegistry`, kicks off a background scan,
/// and wires the bridge that exposes state to the rest of the editor via
/// `renzora_audio`'s public resources.
pub struct PluginHostPlugin;

impl Default for PluginHostPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for PluginHostPlugin {
    fn build(&self, app: &mut App) {
        info!("[vst] PluginHostPlugin");

        app.init_resource::<PluginRegistry>();
        app.init_resource::<BusInserts>();
        // PluginInstances is NonSend because clack-host plugin instances
        // are not Send and must be created/used from the main thread.
        app.insert_non_send_resource(host::PluginInstances::default());

        // Mark the host as present in the catalog so panels can show
        // "scanning…" instead of "no plugin host" when nothing is found yet.
        if let Some(mut catalog) =
            app.world_mut()
                .get_resource_mut::<renzora_audio::PluginCatalog>()
        {
            catalog.host_present = true;
        }

        // Trigger a scan on startup. The scan runs on a worker thread; the
        // registry's results land asynchronously.
        app.add_systems(Startup, scan::start_initial_scan);

        // Per-frame work, in order: drain scan results → apply incoming FX
        // commands → pump live plugin messages (so user-closed editor
        // windows flip the editor_open flag) → publish current state to
        // the bridge mirrors so the UI sees the latest editor_open /
        // instance_loaded values.
        app.add_systems(
            Update,
            (
                scan::poll_scan_results,
                bridge::apply_fx_commands,
                host::pump_plugin_messages,
                bridge::mirror_plugin_catalog,
                bridge::mirror_bus_inserts,
            )
                .chain(),
        );
    }
}


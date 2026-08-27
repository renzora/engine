//! Bridges `renzora_plugin`'s load report into the shared plugin inventory, and
//! seeds the editable disable list.
//!
//! The Settings UI that renders all this lives in `renzora_settings`
//! (Settings → Editor → Plugins). It reads only contract-crate types, so it
//! needs no dependency on either loader. This module is the one piece that does.
//!
//! # Why a bridge exists at all
//!
//! Two loaders find plugins, and they report to the same place —
//! [`renzora::PluginInventory`] — so a panel never has to re-derive either one's
//! rules. `renzora_native_plugin` writes there directly. `renzora_plugin` cannot:
//! it is published to crates.io so a third-party author can `cargo add` it, and
//! a path dependency on the contract crate would make that impossible. So it
//! records into its own dependency-free `PluginLoadReport` and this copies it
//! across.
//!
//! This crate is the seam because it is one of the few that depends on both.

use bevy::prelude::*;

use renzora::{DisabledPlugins, PluginInventory, PluginKind, PluginState};

pub fn register(app: &mut App) {
    app.init_resource::<PluginInventory>();
    // Seeded from disk once. Both loaders have already read the same file for
    // themselves — they run before any resource exists — and this is the
    // editable mirror the settings UI binds to.
    app.insert_resource(DisabledPlugins(renzora::load_disabled_plugins()));
    // `Startup`, not `build`: the C-ABI host plugin is added to the app after
    // this crate, so its report does not exist yet while plugins are still
    // being installed.
    app.add_systems(Startup, adopt_standalone_report);
}

/// Copy the C-ABI loader's report into the shared inventory.
///
/// Runs once. That loader does all its work during `App` assembly and never adds
/// to the report afterwards — a hot-reloaded plugin replaces an image behind an
/// entry that is already there.
fn adopt_standalone_report(world: &mut World) {
    use renzora_plugin::host::loader::{LoadOutcome, PluginLoadReport};

    // Taken out and put back so the `record_plugin` calls below are not made
    // while a borrow of the report is live.
    let Some(report) = world.remove_resource::<PluginLoadReport>() else {
        return;
    };
    for (id, outcome) in &report.entries {
        let state = match outcome {
            LoadOutcome::Loaded => PluginState::Loaded,
            LoadOutcome::Disabled => PluginState::Disabled,
            LoadOutcome::Failed(why) => PluginState::Failed(why.clone()),
            LoadOutcome::VersionTooOld => {
                PluginState::Skipped("built for a newer plugin ABI than this engine".to_string())
            }
            // Phrased as what it means rather than as the enum: "Editor-scope,
            // so it does not load here" is something a person can act on;
            // `WrongScope(Editor)` is not.
            LoadOutcome::WrongScope(scope) => {
                PluginState::Skipped(format!("{scope:?}-scope, so it does not load here"))
            }
            // Filtered by the loader before it reaches the report.
            LoadOutcome::NotAPlugin => continue,
        };
        renzora::record_plugin(world, id.clone(), PluginKind::Standalone, state);
    }
    world.insert_resource(report);
}

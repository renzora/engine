//! Bridges `renzora_plugin`'s load report into the shared plugin inventory, and
//! seeds the editable disable list.
//!
//! The Settings UI that renders all this lives in `renzora_settings`
//! (Settings → Editor → Plugins). It reads only contract-crate types, so it
//! needs no dependency on either loader. This module is the one piece that does.
//!
//! # Where the inventory comes from
//!
//! One scan finds every plugin and records it in [`renzora::PluginInventory`],
//! so a panel never has to re-derive either loader's rules. That scan lives in
//! `renzora_native_plugin`, which is the only crate that can see both loaders —
//! `renzora_plugin` is published to crates.io so a third-party author can
//! `cargo add` it, and a path dependency on the contract crate would make that
//! impossible, so the translation happens on the side that can name both.
//!
//! There used to be a second report here, copied across at `Startup`, because
//! the two loaders ran at different points and reported in different
//! vocabularies. One scan removed the need for it.

use bevy::prelude::*;

use renzora::{DisabledPlugins, PluginInventory};

pub fn register(app: &mut App) {
    app.init_resource::<PluginInventory>();
    // Seeded from disk once. Both loaders have already read the same file for
    // themselves — they run before any resource exists — and this is the
    // editable mirror the settings UI binds to.
    app.insert_resource(DisabledPlugins(renzora::load_disabled_plugins()));
}

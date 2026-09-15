//! What the marketplace currently publishes for the plugins installed here.
//!
//! One crate does the asking (`renzora_marketplace`, which owns the catalogue
//! client and the installer) and four surfaces draw the answer: the updater
//! overlay beside the engine's own update, the marketplace's Updates view, the
//! Settings plugin grid and the exporter's plugin picker. None of those four can
//! depend on the marketplace, so the answer lands here.
//!
//! Nothing in this module talks to the network or the disk. It is the shape of
//! the answer, and the question of whether to *nag* about it
//! ([`load_plugin_update_reminders`](crate::core::load_plugin_update_reminders)),
//! and that is all.
//!
//! # Why the keys are both an id and an asset id
//!
//! The two halves of this are keyed differently and cannot be reconciled into
//! one. Settings and the exporter list plugins by their **directory name** under
//! `plugins/` (see [`PluginEntry::id`](super::PluginEntry::id)), which is the
//! only string stable across platforms. The marketplace knows an asset by its
//! **asset id**, which is what an install is keyed on, because two listings can
//! ship a crate of the same name. Each entry carries both so either side can
//! look itself up without guessing.

use bevy::prelude::*;

/// One installed plugin with something newer published for it.
#[derive(Debug, Clone)]
pub struct PluginUpdate {
    /// Directory name under `plugins/`, matching [`PluginEntry::id`](super::PluginEntry::id).
    pub id: String,
    /// Marketplace identity, which is what an install is keyed on.
    pub asset_id: String,
    pub slug: String,
    /// Display name, as the listing has it. The directory name is derived from
    /// the crate name and is routinely not what the store calls the plugin.
    pub name: String,
    pub installed_version: String,
    pub available_version: String,
    /// The newer release exists but this editor cannot run it.
    ///
    /// Shown rather than hidden: "update the editor" is the actionable answer,
    /// and silence would make a maintained plugin look abandoned.
    pub needs_newer_engine: bool,
    /// The engine release the update asks for, when the marketplace named one.
    /// Empty is a real answer from an older marketplace, which is why this is
    /// not what [`needs_newer_engine`](Self::needs_newer_engine) is derived from.
    pub requires_engine: String,
}

/// A marketplace plugin found on disk, whether or not it has an update.
///
/// Carried alongside the updates because the store grid needs it: a card with
/// nothing newer published still has to say **Installed** rather than offering
/// the install again, and that is a different question from "is it stale".
#[derive(Debug, Clone)]
pub struct InstalledPluginAsset {
    pub id: String,
    pub asset_id: String,
    pub version: String,
}

/// What the last check found. Empty and `checked == false` until it runs.
#[derive(Resource, Default)]
pub struct PluginUpdates {
    pub entries: Vec<PluginUpdate>,
    pub installed: Vec<InstalledPluginAsset>,
    /// A check has completed at least once.
    ///
    /// The difference between "nothing to update" and "we have not looked yet"
    /// matters to every surface that draws this: a green "everything is current"
    /// before the answer is in is a claim nobody made.
    pub checked: bool,
}

impl PluginUpdates {
    /// The update for a plugin directory, if it has one.
    pub fn for_plugin(&self, id: &str) -> Option<&PluginUpdate> {
        self.entries.iter().find(|u| u.id == id)
    }

    /// The update for a marketplace listing, if the installed copy is stale.
    pub fn for_asset(&self, asset_id: &str) -> Option<&PluginUpdate> {
        self.entries.iter().find(|u| u.asset_id == asset_id)
    }

    /// The installed copy of a listing, if this editor has one.
    pub fn installed_asset(&self, asset_id: &str) -> Option<&InstalledPluginAsset> {
        self.installed.iter().find(|p| p.asset_id == asset_id)
    }

    /// Updates that can be installed right now.
    pub fn ready(&self) -> usize {
        self.entries.iter().filter(|u| !u.needs_newer_engine).count()
    }

    /// Updates waiting on a newer editor.
    pub fn blocked(&self) -> usize {
        self.entries.iter().filter(|u| u.needs_newer_engine).count()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The live, editable mirror of the persisted reminder preference.
///
/// Same arrangement as [`DisabledPlugins`](super::DisabledPlugins): the file is
/// the record and this is the copy the Settings toggle edits and the check reads,
/// saved back on every change. A resource because the getter behind a settings
/// toggle runs every frame, and a preference read off disk at that rate is a
/// file the editor never stops opening.
///
/// Inserted by whoever does the checking, which is the only crate that knows the
/// preference matters. Everything else treats its absence as "on", because that
/// is the default and an editor with no update checker has nothing to suppress.
#[derive(Resource)]
pub struct PluginUpdateReminders(pub bool);

impl Default for PluginUpdateReminders {
    fn default() -> Self {
        Self(true)
    }
}

/// "Show me the plugins that need updating."
///
/// Written by the updater overlay, the Settings plugin section and the
/// exporter's plugin tab; consumed by the marketplace, which opens its store
/// overlay on the Updates view and removes the resource.
///
/// A resource rather than a direct call because none of the three writers can
/// link the marketplace, and a resource rather than a message because it is a
/// one-shot request with no payload and no ordering to get wrong: whoever
/// notices it first serves it.
#[derive(Resource)]
pub struct PluginUpdatesRequested;

/// How a card in the marketplace grid should present itself.
///
/// Decided from [`PluginUpdates`] at snapshot time and handed to the card
/// builder, which has no world to ask. It is here rather than in the store
/// because the exporter and Settings draw the same three states with the same
/// two colours, and a fourth definition of "amber means stale" is how they drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PluginInstallState {
    /// Not installed, or not something this editor installs.
    #[default]
    Absent,
    /// Installed and current.
    Installed,
    /// Installed, with something newer published that this editor can run.
    UpdateAvailable,
    /// Installed, with something newer published that needs a newer editor.
    UpdateBlocked,
}

impl PluginInstallState {
    /// Work out a listing's state from a check result.
    pub fn of(updates: &PluginUpdates, asset_id: &str) -> Self {
        match updates.for_asset(asset_id) {
            Some(u) if u.needs_newer_engine => Self::UpdateBlocked,
            Some(_) => Self::UpdateAvailable,
            None if updates.installed_asset(asset_id).is_some() => Self::Installed,
            None => Self::Absent,
        }
    }

    pub fn is_installed(self) -> bool {
        !matches!(self, Self::Absent)
    }

    pub fn is_stale(self) -> bool {
        matches!(self, Self::UpdateAvailable | Self::UpdateBlocked)
    }
}

/// Amber, for anything carrying a pending plugin update.
///
/// One constant so the store pill, the store badge, the Settings status line,
/// the exporter's note and the updater's list are the same colour. They were
/// five separate literals away from being five slightly different ambers.
pub const PLUGIN_UPDATE_AMBER: (u8, u8, u8) = (235, 178, 58);

/// Green, for "installed and current".
pub const PLUGIN_INSTALLED_GREEN: (u8, u8, u8) = (82, 186, 112);

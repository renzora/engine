//! Decoupling layer between the mixer panel and the (optional) audio plugin
//! host (`renzora_vst`).
//!
//! Why this lives here: every UI panel that wants to show or change FX
//! inserts already depends on `renzora_audio`, so by parking the public
//! event/resource types in this base crate the panels avoid taking a hard
//! dynamic-link dependency on `renzora_vst`. The plugin host fills in the
//! resources and consumes the events; if it isn't loaded, the resources
//! stay empty and the events go nowhere.
//!
//! Only POD-ish types are exposed (Strings, ints, bools). No plugin handles,
//! no internal IDs from `renzora_vst` — those are private to that crate.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;

/// Action a UI panel asks the plugin host to perform on a bus's insert chain.
#[derive(Clone, Debug)]
pub enum MixerFxOp {
    /// Append a new slot loading the plugin identified by
    /// `plugin_catalog_id` (matches a [`PluginCatalogEntry::id`]).
    Add { plugin_catalog_id: String },
    /// Remove the slot whose local id matches.
    Remove { local_id: u64 },
    /// Move the slot one step earlier in the chain.
    MoveUp { local_id: u64 },
    /// Move the slot one step later in the chain.
    MoveDown { local_id: u64 },
    /// Flip the slot's bypass flag.
    ToggleBypass { local_id: u64 },
    /// Open this plugin's native (floating) editor window. CLAP plugin GUIs
    /// can't be drawn into an egui texture — they render into their own OS
    /// window. The host opens the window; the user manages it like any
    /// other application window.
    OpenEditor { local_id: u64 },
    /// Close the plugin's editor window if currently open.
    CloseEditor { local_id: u64 },
}

/// Message a UI panel fires to request an FX-chain mutation on a bus.
///
/// `bus` is the same string the rest of the audio engine uses ("Master",
/// "Sfx", "Music", "Ambient", or any custom-bus / timeline-track name).
#[derive(Message, Clone, Debug)]
pub struct MixerFxCommand {
    pub bus: String,
    pub op: MixerFxOp,
}

/// Read-only snapshot of one slot in an insert chain. Mirrored from
/// `renzora_vst`'s `PluginInstanceSlot` for cross-crate display.
#[derive(Clone, Debug, Default)]
pub struct FxSlotSummary {
    /// Stable identifier for this slot (used as the target of subsequent
    /// `Remove` / `MoveUp` / `ToggleBypass` commands).
    pub local_id: u64,
    /// Human-friendly label — usually the plugin's display name.
    pub display_name: String,
    /// Plugin catalog id this slot was created from (empty if the plugin
    /// has since been unloaded from disk).
    pub plugin_catalog_id: String,
    pub bypass: bool,
    /// True while the plugin's floating editor window is open. Drives the
    /// state of the Open/Close toggle in the FX popup.
    pub editor_open: bool,
    /// True when the host has actually instantiated this plugin (i.e.
    /// `clack-host` is compiled in and the bundle loaded). False ⇒ the
    /// editor button is shown disabled with a hint.
    pub instance_loaded: bool,
}

/// Read-only mirror of every bus's FX insert chain. Populated by
/// `renzora_vst` when the host plugin is loaded; stays empty otherwise.
#[derive(Resource, Default, Clone, Debug)]
pub struct BusInsertsSummary {
    /// Keyed by bus name.
    pub by_bus: HashMap<String, Vec<FxSlotSummary>>,
}

impl BusInsertsSummary {
    pub fn slots(&self, bus: &str) -> &[FxSlotSummary] {
        self.by_bus.get(bus).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn slot_count(&self, bus: &str) -> usize {
        self.by_bus.get(bus).map(|v| v.len()).unwrap_or(0)
    }
}

/// One audio plugin discovered on disk by the plugin host's scanner.
#[derive(Clone, Debug)]
pub struct PluginCatalogEntry {
    /// Stable identifier (today: the bundle path as a string). Used as
    /// `plugin_catalog_id` in `MixerFxOp::Add`.
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub bundle_path: PathBuf,
}

/// Read-only mirror of the available plugin catalog. Mirrored from
/// `renzora_vst`'s internal `PluginRegistry`.
#[derive(Resource, Default, Clone, Debug)]
pub struct PluginCatalog {
    pub plugins: Vec<PluginCatalogEntry>,
    /// True while a background scan is running. UI shows a "scanning…"
    /// indicator while this is set and `plugins` is empty.
    pub scanning: bool,
    /// Number of filesystem roots searched on the most recent scan.
    pub last_scan_root_count: usize,
    /// Whether the plugin host crate is loaded at all. False ⇒ "no plugin
    /// support compiled in / VST plugin not loaded"; UI can show a
    /// distinct message in that case.
    pub host_present: bool,
}

//! Per-bus / per-track plugin insert chain data model.
//!
//! Lives in `renzora_plugins` (not in the audio or mixer crate) so the
//! insert chain can later carry a live `clack-host` plugin instance handle
//! without forcing the audio crate to take that dep.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;

use crate::registry::PluginId;

/// One slot in an insert chain — a reference to a plugin descriptor plus a
/// bypass flag, a wet/dry mix, and a `local_id` we can use to identify the
/// slot across UI frames without depending on its index (so drag-reorder
/// doesn't drop user-set parameters).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PluginInstanceSlot {
    pub local_id: u64,
    pub plugin: PluginId,
    pub bypass: bool,
    /// Linear wet mix from 0.0 (dry only) to 1.0 (wet only). Plugins that
    /// already have their own dry/wet usually leave this at 1.0.
    pub mix: f32,
    /// Display name override. Falls back to the plugin descriptor's name
    /// when empty.
    pub label: String,
    /// `true` when a live `clack-host` plugin instance has been allocated
    /// for this slot. Mirrored to `FxSlotSummary::instance_loaded` so the
    /// UI knows whether the Open Editor button can do anything.
    #[serde(skip)]
    pub instance_loaded: bool,
    /// `true` while the floating editor window for this plugin is open.
    /// Mirrored to the UI; flipped by `apply_fx_commands` when the host
    /// actually opens / closes a window.
    #[serde(skip)]
    pub editor_open: bool,
}

impl PluginInstanceSlot {
    pub fn new(plugin: PluginId) -> Self {
        Self {
            local_id: fresh_local_id(),
            plugin,
            bypass: false,
            mix: 1.0,
            label: String::new(),
            instance_loaded: false,
            editor_open: false,
        }
    }
}

/// Ordered chain of plugin slots applied to a bus or track.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PluginInsertChain {
    pub slots: Vec<PluginInstanceSlot>,
}

impl PluginInsertChain {
    pub fn push(&mut self, plugin: PluginId) -> u64 {
        let slot = PluginInstanceSlot::new(plugin);
        let id = slot.local_id;
        self.slots.push(slot);
        id
    }

    pub fn remove(&mut self, local_id: u64) {
        self.slots.retain(|s| s.local_id != local_id);
    }

    pub fn move_up(&mut self, local_id: u64) {
        if let Some(pos) = self.slots.iter().position(|s| s.local_id == local_id) {
            if pos > 0 {
                self.slots.swap(pos - 1, pos);
            }
        }
    }

    pub fn move_down(&mut self, local_id: u64) {
        if let Some(pos) = self.slots.iter().position(|s| s.local_id == local_id) {
            if pos + 1 < self.slots.len() {
                self.slots.swap(pos, pos + 1);
            }
        }
    }
}

/// Monotonically-increasing local id counter shared across all chains so
/// that ids stay unique within one editor session. Persistence loads will
/// just renumber on import.
static LOCAL_ID: AtomicU64 = AtomicU64::new(1);

fn fresh_local_id() -> u64 {
    LOCAL_ID.fetch_add(1, Ordering::Relaxed)
}

/// Resource holding per-bus and per-track insert chains.
///
/// Keys are the bus / track names ("Master", "Music", custom bus names, or
/// timeline track names — whatever the caller decides to use). Lookups for
/// missing keys return an empty chain, so callers don't need to bootstrap.
#[derive(Resource, Default, Clone, Debug)]
pub struct BusInserts {
    pub chains: HashMap<String, PluginInsertChain>,
}

impl BusInserts {
    /// Read-only borrow of the chain for a key, or `None` if no slots.
    pub fn get(&self, key: &str) -> Option<&PluginInsertChain> {
        self.chains.get(key).filter(|c| !c.slots.is_empty())
    }

    /// Mutable borrow, creating an empty chain if needed.
    pub fn entry(&mut self, key: &str) -> &mut PluginInsertChain {
        self.chains
            .entry(key.to_string())
            .or_default()
    }

    /// Discard chains that have become empty so the map doesn't grow
    /// unbounded with renames.
    pub fn prune_empty(&mut self) {
        self.chains.retain(|_, c| !c.slots.is_empty());
    }

    /// Total number of plugin slots across every chain. UI uses this to
    /// decide whether to show the "no plugins inserted" placeholder.
    pub fn total_slot_count(&self) -> usize {
        self.chains.values().map(|c| c.slots.len()).sum()
    }
}

//! Bridge between the private state in this crate and the public types in
//! `renzora_audio::fx_bridge`. UI crates (mixer, daw) only ever see the
//! public types — this module is the only place that touches both sides.

use bevy::prelude::*;
use renzora_audio::{
    BusInsertsSummary, FxSlotSummary, MixerFxCommand, MixerFxOp, PluginCatalog,
    PluginCatalogEntry,
};

use crate::host::PluginInstances;
use crate::insert::BusInserts;
use crate::registry::{PluginId, PluginRegistry};

/// Drain incoming `MixerFxCommand` messages and apply them to `BusInserts`
/// and the live `PluginInstances`. Runs on the main thread because
/// `PluginInstances` is `NonSend`.
pub fn apply_fx_commands(
    mut messages: MessageReader<MixerFxCommand>,
    mut inserts: ResMut<BusInserts>,
    registry: Res<PluginRegistry>,
    mut instances: NonSendMut<PluginInstances>,
) {
    for msg in messages.read() {
        match &msg.op {
            MixerFxOp::Add { plugin_catalog_id } => {
                // Catalog ids are the plugin's bundle path stringified —
                // matches `PluginId(String)` exactly.
                let plugin = PluginId(plugin_catalog_id.clone());
                let Some(descriptor) = registry.get(&plugin) else {
                    warn!(
                        "[vst] MixerFxCommand::Add for unknown plugin id '{}'",
                        plugin_catalog_id
                    );
                    continue;
                };
                let bundle_path = descriptor.bundle_path.clone();
                let new_local_id = inserts.entry(&msg.bus).push(plugin);

                let outcome = instances.add(&msg.bus, new_local_id, &bundle_path);
                let loaded = matches!(
                    outcome,
                    crate::host::HostOpOutcome::Ok | crate::host::HostOpOutcome::HostDisabled
                );
                if let Some(slot) = inserts
                    .entry(&msg.bus)
                    .slots
                    .iter_mut()
                    .find(|s| s.local_id == new_local_id)
                {
                    slot.instance_loaded = loaded;
                }
            }
            MixerFxOp::Remove { local_id } => {
                instances.remove(&msg.bus, *local_id);
                inserts.entry(&msg.bus).remove(*local_id);
                inserts.prune_empty();
            }
            MixerFxOp::MoveUp { local_id } => {
                inserts.entry(&msg.bus).move_up(*local_id);
            }
            MixerFxOp::MoveDown { local_id } => {
                inserts.entry(&msg.bus).move_down(*local_id);
            }
            MixerFxOp::ToggleBypass { local_id } => {
                let chain = inserts.entry(&msg.bus);
                if let Some(slot) = chain.slots.iter_mut().find(|s| s.local_id == *local_id) {
                    slot.bypass = !slot.bypass;
                }
            }
            MixerFxOp::OpenEditor { local_id } => {
                let outcome = instances.open_editor(&msg.bus, *local_id);
                debug!("[vst] OpenEditor outcome: {:?}", outcome);
                if let Some(slot) = inserts
                    .entry(&msg.bus)
                    .slots
                    .iter_mut()
                    .find(|s| s.local_id == *local_id)
                {
                    slot.editor_open = instances.is_editor_open(&msg.bus, *local_id);
                }
            }
            MixerFxOp::CloseEditor { local_id } => {
                let outcome = instances.close_editor(&msg.bus, *local_id);
                debug!("[vst] CloseEditor outcome: {:?}", outcome);
                if let Some(slot) = inserts
                    .entry(&msg.bus)
                    .slots
                    .iter_mut()
                    .find(|s| s.local_id == *local_id)
                {
                    slot.editor_open = instances.is_editor_open(&msg.bus, *local_id);
                }
            }
        }
    }
}

/// Copy the current `PluginRegistry` into the public `PluginCatalog` mirror
/// any time the registry changes. Cheap: only runs on Changed, and the
/// data is small (path strings, not plugin handles).
pub fn mirror_plugin_catalog(
    registry: Res<PluginRegistry>,
    mut catalog: ResMut<PluginCatalog>,
) {
    let scanning = registry.is_scanning();
    let root_count = registry.last_scan_roots.len();

    // Track scanning-flag changes too so the panel updates while the
    // background worker is still running.
    let scanning_changed = catalog.scanning != scanning
        || catalog.last_scan_root_count != root_count;
    if !registry.is_changed() && !scanning_changed {
        return;
    }

    catalog.plugins = registry
        .plugins
        .iter()
        .map(|d| PluginCatalogEntry {
            id: d.id.0.clone(),
            name: d.name.clone(),
            vendor: d.vendor.clone(),
            bundle_path: d.bundle_path.clone(),
        })
        .collect();
    catalog.scanning = scanning;
    catalog.last_scan_root_count = root_count;
    catalog.host_present = true;
}

/// Copy `BusInserts` into the public `BusInsertsSummary`, looking up
/// display names from the registry on the fly.
pub fn mirror_bus_inserts(
    inserts: Res<BusInserts>,
    registry: Res<PluginRegistry>,
    mut summary: ResMut<BusInsertsSummary>,
) {
    if !inserts.is_changed() && !registry.is_changed() {
        return;
    }

    summary.by_bus.clear();
    for (bus, chain) in &inserts.chains {
        if chain.slots.is_empty() {
            continue;
        }
        let slots: Vec<FxSlotSummary> = chain
            .slots
            .iter()
            .map(|slot| {
                let display_name = registry
                    .get(&slot.plugin)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "(missing plugin)".to_string());
                FxSlotSummary {
                    local_id: slot.local_id,
                    display_name,
                    plugin_catalog_id: slot.plugin.0.clone(),
                    bypass: slot.bypass,
                    editor_open: slot.editor_open,
                    instance_loaded: slot.instance_loaded,
                }
            })
            .collect();
        summary.by_bus.insert(bus.clone(), slots);
    }
}

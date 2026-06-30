//! Networking editor panels for the Renzora editor.
//!
//! Panels: Network Monitor, Network Entities, Network Settings — all rendered
//! natively (bevy_ui / ember) in [`native`]. This crate also registers the
//! `Networked` and `NetworkTransform` inspector entries (declarative fields, no
//! egui).

mod native;

use bevy::prelude::*;

use renzora::{bool_field, AppEditorExt, InspectorEntry};
use renzora_network::{NetworkTransform, Networked};

// ============================================================================
// Inspector entries (attachable components)
// ============================================================================

/// Resolve a localized string and leak it to satisfy the `&'static str`
/// inspector-field contract. Inspector entries are constructed once at plugin
/// registration, so this one-time leak per label is bounded (it does not grow
/// per frame). The trade-off: labels won't re-localize on a live language swap.
fn loc(key: &str) -> &'static str {
    Box::leak(renzora::lang::t(key).into_boxed_str())
}

/// `Networked` — the "replicate this entity" marker. Adding it makes the
/// server replicate the entity (and its `Transform`) to every client.
fn networked_inspector() -> InspectorEntry {
    InspectorEntry {
        type_id: "networked",
        display_name: loc("comp.networked"),
        icon: "share-network",
        category: "networking",
        has_fn: |world, entity| world.get::<Networked>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(Networked);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<Networked>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

/// `NetworkTransform` — tunes how the entity's transform replicates.
fn network_transform_inspector() -> InspectorEntry {
    InspectorEntry {
        type_id: "network_transform",
        display_name: loc("comp.network_transform"),
        icon: "arrows-out-cardinal",
        category: "networking",
        has_fn: |world, entity| world.get::<NetworkTransform>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NetworkTransform::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<NetworkTransform>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            bool_field!(loc("comp.interpolate"), NetworkTransform, interpolate),
            bool_field!(loc("comp.sync_rotation"), NetworkTransform, sync_rotation),
            bool_field!(loc("comp.sync_scale"), NetworkTransform, sync_scale),
        ],
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct NetworkEditorPlugin;

impl Plugin for NetworkEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] NetworkEditorPlugin");
        app.add_plugins(native::NativeNetworkPanels);
        app.register_inspector(networked_inspector());
        app.register_inspector(network_transform_inspector());
    }
}

renzora::add!(NetworkEditorPlugin, Editor);

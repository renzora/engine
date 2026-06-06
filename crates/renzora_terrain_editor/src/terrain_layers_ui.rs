//! "Layers" section of the terrain inspector, backed by the `Painter`
//! component on the terrain entity.
//!
//! The egui inspector body was removed in the bevy_ui migration; the native
//! terrain panel (see `native.rs`) is the live layer UI. This module keeps the
//! inspector registration entry (now without a custom drawer) and the legacy
//! `ActiveBrushLayer` resource that the brush-layer paint path still links.

use bevy::prelude::*;
use egui_phosphor::regular as icons;
use renzora_editor::InspectorEntry;
use renzora_terrain::data::TerrainData;

/// `ActiveBrushLayer` moved into the `Painter` itself (`active_layer`), but
/// during the transition we still register this type so the old
/// brush-layer paint path links. It's no longer written by the new UI.
#[derive(Resource, Default)]
// kept during the brush-layer transition so the old paint path still links
pub struct ActiveBrushLayer(#[allow(dead_code)] pub Option<Entity>);

pub fn terrain_layers_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "terrain_layers",
        display_name: "Layers",
        icon: icons::STACK,
        category: "component",
        has_fn: |world, entity| world.get::<TerrainData>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
    }
}

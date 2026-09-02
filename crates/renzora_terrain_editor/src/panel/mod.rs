//! The Terrain Tools panel (panel id "terrain_tools"): an enable toggle over a
//! Sculpt / Paint tab bar.
//!
//! * **Sculpt** ([`sculpt`]) — a 17-tool brush grid plus collapsibles for Tool
//!   Settings (strength + per-brush Flatten / Noise / Terrace / Stamp controls),
//!   Brush Settings (size / falloff / shape / falloff-type) and Heightmap Import.
//! * **Paint** ([`paint`]) — a 4-tool brush grid plus collapsibles for Layers
//!   (selectable list + per-active-layer material drop-zone + Add Layer), Brush
//!   Settings (size / strength / falloff + shape) and Foliage (info text).
//!
//! Every control writes back into the resources the terrain systems read:
//! [`TerrainToolState`], [`TerrainSettings`], [`SurfacePaintSettings`] and
//! [`SurfacePaintState`] (the last via its `pending_commands` queue). Nothing
//! here holds state of its own, which is what lets the viewport toolbar and this
//! panel drive the same tools without either being authoritative.

use bevy::prelude::*;

use renzora_editor_framework::SplashState;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::Rx;

use renzora_terrain::data::{
    BrushFalloffType, BrushShape, TerrainBrushType, TerrainSettings, TerrainTab, TerrainToolState,
};
use renzora_terrain::paint::{PaintBrushType, SurfacePaintSettings};

pub(crate) mod build;
pub(crate) mod paint;
pub(crate) mod sculpt;
pub(crate) mod systems;
pub(crate) mod widgets;

pub(super) const LABEL_W: f32 = 100.0;
pub(super) const MATERIAL_EXTS: &[&str] = &["material"];

pub struct TerrainToolsPanel;

impl Plugin for TerrainToolsPanel {
    fn build(&self, app: &mut App) {
        app.register_panel_content("terrain_tools", true, build::build)
            .systems(
            Update,
            (
                systems::enable_toggle_click,
                systems::tab_click,
                systems::follow_active_tool,
                systems::sculpt_tool_click,
                systems::paint_tool_click,
                systems::shape_btn_click,
                systems::falloff_type_btn_click,
                systems::flatten_mode_combo_open,
                systems::noise_mode_combo_open,
                systems::stamp_preset_combo_open,
                systems::stamp_blend_combo_open,
                systems::stamp_load_click,
                systems::layer_row_click,
                systems::add_layer_click,
                systems::heightmap_import_click,
                systems::heightmap_export_click,
                systems::material_drop,
                systems::material_clear_click,
                systems::material_drop_highlight,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── State accessors ──────────────────────────────────────────────────────────

pub(super) fn tool_active(w: &Rx) -> bool {
    w.get_resource::<TerrainToolState>()
        .map(|t| t.active)
        .unwrap_or_default()
}

pub(super) fn settings_tab(w: &Rx) -> TerrainTab {
    w.get_resource::<TerrainSettings>()
        .map(|s| s.tab)
        .unwrap_or_default()
}

pub(super) fn brush_type(w: &Rx) -> TerrainBrushType {
    w.get_resource::<TerrainSettings>()
        .map(|s| s.brush_type)
        .unwrap_or_default()
}

pub(super) fn paint_brush_type(w: &Rx) -> PaintBrushType {
    w.get_resource::<SurfacePaintSettings>()
        .map(|s| s.brush_type)
        .unwrap_or_default()
}

pub(super) fn set_settings(w: &mut World, f: impl FnOnce(&mut TerrainSettings)) {
    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() {
        f(&mut s);
    }
}

pub(super) fn set_paint(w: &mut World, f: impl FnOnce(&mut SurfacePaintSettings)) {
    if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() {
        f(&mut s);
    }
}

pub(super) fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
pub(super) struct EnableToggle;

#[derive(Component)]
pub(super) struct TabBtn {
    pub(super) tab: TerrainTab,
}

#[derive(Component)]
pub(super) struct SculptToolBtn {
    pub(super) brush: TerrainBrushType,
}

#[derive(Component)]
pub(super) struct PaintToolBtn {
    pub(super) brush: PaintBrushType,
}

#[derive(Component)]
pub(super) struct FlattenModeCombo;
#[derive(Component)]
pub(super) struct NoiseModeCombo;
#[derive(Component)]
pub(super) struct StampPresetCombo;
#[derive(Component)]
pub(super) struct StampBlendCombo;
#[derive(Component)]
pub(super) struct StampLoadBtn;

#[derive(Component)]
pub(super) struct HeightmapImportBtn;
#[derive(Component)]
pub(super) struct HeightmapExportBtn;

#[derive(Component)]
pub(super) struct LayerRow {
    pub(super) index: usize,
}
#[derive(Component)]
pub(super) struct AddLayerBtn;

#[derive(Component)]
pub(super) struct MaterialDropZone {
    pub(super) layer: usize,
}
#[derive(Component)]
pub(super) struct MaterialClearBtn {
    pub(super) layer: usize,
}

/// Which settings resource a shape button writes to. Sculpt and paint keep
/// separate brush shapes, and the button that sets one must not touch the other.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum ShapeTarget {
    Sculpt,
    Paint,
}

#[derive(Component)]
pub(super) struct ShapeBtn {
    pub(super) target: ShapeTarget,
    pub(super) shape: BrushShape,
}

#[derive(Component)]
pub(super) struct FalloffTypeBtn {
    pub(super) ft: BrushFalloffType,
}

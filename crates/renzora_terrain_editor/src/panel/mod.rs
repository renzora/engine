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
use renzora::AppEditorExt;
use renzora_ember::reactive::Rx;

use renzora_terrain::data::{
    BrushFalloffType, BrushShape, TerrainBrushType, TerrainSettings, TerrainTab,
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

/// The tools as the **Terrain component's** inspector body.
///
/// This tree used to be a dock panel of its own ("terrain_tools"), which meant
/// the brushes and their settings existed in three places at once: here, as icon
/// grids on the viewport shelf, and as a context group in the viewport toolbar.
/// Three surfaces over one set of resources is why the shelf came out cluttered
/// -- it was the copy with no room for labels.
///
/// The component is the one that earns it. These controls act on *the terrain
/// you have selected*, selecting it is already how you get into terrain editing,
/// and the inspector has the width for named rows instead of 21 unlabelled
/// glyphs. It is where Unity puts the same thing, for the same reason.
///
/// Nothing inside changed: `build` is the panel's own builder, and every system
/// below finds its widgets by component rather than by where they are parented,
/// so they drive this exactly as they drove the panel.
fn terrain_tools_native(world: &mut World, _entity: Entity) -> Entity {
    renzora_ember::inspector::inspector_body(world, build::build)
}

impl Plugin for TerrainToolsPanel {
    fn build(&self, app: &mut App) {
        // Registered against `terrain_data`, the inspector entry that already
        // carried the terrain's Size / Edit Terrain… / height rows.
        app.register_native_inspector_ui("terrain_data", terrain_tools_native);
        app.add_systems(
            Update,
            (
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

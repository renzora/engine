//! The Terrain component's inspector body: a material slot over four mode
//! tabs (Manage, Sculpt, Paint, Foliage), each showing only what that mode
//! needs.
//!
//! * [`material_slot`]: the landscape material the terrain wears.
//! * [`manage`]: the chunk map ([`chunk_map`]), size and streaming with
//!   chunk size and resolution batched until Apply, the generator graph, the
//!   whole-terrain operations and heightmap import and export.
//! * [`sculpt`]: the brushes in two groups, one card of the chosen brush's
//!   settings, one shared brush card with a drawn falloff curve ([`falloff`]).
//! * [`paint`]: the material's layers and the weight brush with its masks.
//! * Foliage is built by `renzora_foliage_editor::panel`.
//!
//! Every control writes the resources and components the terrain systems
//! read ([`TerrainSettings`], [`SurfacePaintSettings`], `TerrainData`,
//! `TerrainGenerator`, `TerrainMaterial`). Nothing here holds state of its
//! own except the staged Apply edit ([`manage::ManageDraft`]), which is the
//! point of it.

use bevy::prelude::*;

use renzora::AppEditorExt;
use renzora_editor_framework::{EditorSelection, SplashState};
use renzora_ember::reactive::Rx;

use renzora_terrain::data::{TerrainBrushType, TerrainData, TerrainSettings};
use renzora_terrain::paint::SurfacePaintSettings;

use crate::terrain_inspector::{ShownTerrainTab, TerrainInspectorTab};

pub(crate) mod build;
pub(crate) mod canvas;
pub(crate) mod chunk_map;
pub(crate) mod falloff;
pub(crate) mod manage;
pub(crate) mod material_slot;
pub(crate) mod paint;
pub(crate) mod sculpt;
pub(crate) mod systems;
pub(crate) mod widgets;

/// Width of a row's label column.
pub(super) const LABEL_W: f32 = 92.0;

pub struct TerrainToolsPanel;

/// The Terrain component's body, built for the terrain the inspector shows.
fn terrain_tools_native(world: &mut World, entity: Entity) -> Entity {
    renzora_ember::inspector::inspector_body(world, move |c, f| build::build(c, f, entity))
}

impl Plugin for TerrainToolsPanel {
    fn build(&self, app: &mut App) {
        app.register_native_inspector_ui("terrain_data", terrain_tools_native);
        app.init_resource::<manage::ManageDraft>()
            .init_resource::<chunk_map::ChunkMap>()
            .add_systems(
                Update,
                (
                    (
                        systems::mode_tab_click,
                        systems::sculpt_tool_click,
                        systems::layer_row_click,
                        systems::stamp_preset_combo_open,
                        systems::stamp_load_click,
                        systems::height_pick_click,
                        systems::manage_action_click,
                        systems::asset_slot_drop,
                        systems::asset_slot_highlight,
                    ),
                    (
                        chunk_map::refresh_chunk_map,
                        chunk_map::chunk_map_click,
                        chunk_map::draw_chunk_map,
                    )
                        .chain(),
                    falloff::draw_falloff_curves,
                    material_slot::material_thumbnails,
                )
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}

// ── State accessors ──────────────────────────────────────────────────────────

pub(super) fn brush_type(w: &Rx) -> TerrainBrushType {
    w.get_resource::<TerrainSettings>().map(|s| s.brush_type).unwrap_or_default()
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

/// `terrain/highlands.tgraph` -> `highlands`.
pub(super) fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Arm `tab`'s tool and show its pane, the way a mode tab click does.
///
/// Through the armed tab rather than `ActiveTool` directly:
/// `sync_active_tool_system` re-derives the tool from the tab every frame while
/// a terrain is selected, so a tool set behind its back is overwritten next
/// frame.
pub(super) fn arm(world: &mut World, tab: TerrainInspectorTab) {
    if world.get_resource::<TerrainInspectorTab>() != Some(&tab) {
        world.insert_resource(tab);
    }
    if world.get_resource::<ShownTerrainTab>().map(|s| s.0) != Some(tab) {
        world.insert_resource(ShownTerrainTab(tab));
    }
}

/// The terrain the panel acts on: the selected one. The body only exists
/// while a terrain is selected, so this is that terrain.
pub(super) fn panel_terrain(world: &World) -> Option<Entity> {
    world
        .get_resource::<EditorSelection>()
        .and_then(|s| s.get())
        .filter(|e| world.get::<TerrainData>(*e).is_some())
}

// ── Markers ──────────────────────────────────────────────────────────────────

/// A mode tab.
#[derive(Component)]
pub(super) struct ModeTabBtn(pub TerrainInspectorTab);

#[derive(Component)]
pub(super) struct SculptToolBtn {
    pub(super) brush: TerrainBrushType,
}

/// A row of the Paint layer list; `index` into `TerrainLandscape::layers`.
#[derive(Component)]
pub(super) struct LayerRow {
    pub(super) index: usize,
}

#[derive(Component)]
pub(super) struct StampPresetCombo;
#[derive(Component)]
pub(super) struct StampLoadBtn;
/// Set Height's Pick: the next click on the ground sets the target height.
#[derive(Component)]
pub(super) struct HeightPickBtn;

/// The buttons that act on the whole terrain or its assets.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ManageAction {
    Apply,
    Revert,
    ChooseGraph,
    EditGraph,
    Reroll,
    Build,
    CancelBuild,
    ErodeAll,
    FlattenAll,
    ImportHeightmap,
    ExportHeightmap,
    EditMaterial,
    UseAlpine,
    ChooseMaterial,
}

/// A slot an asset can be dropped on from the Assets panel.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum AssetSlot {
    Material,
    Graph,
}

impl AssetSlot {
    pub(super) fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Material => &["material"],
            Self::Graph => &[renzora_terrain_graph::EXTENSION],
        }
    }
}

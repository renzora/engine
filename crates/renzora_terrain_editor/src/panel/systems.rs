//! Everything the panel's widgets do once clicked: tool/tab switching, the
//! enum combos' screen-menus, the heightmap and stamp file dialogs, and the
//! layer material drop target.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::core::CurrentProject;
use renzora_editor_framework::{ActiveTool, AssetDragPayload, EditorSelection};
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::*;
use renzora_ember::widgets::{menu_item, screen_menu};

use renzora_terrain::data::{
    FlattenMode, NoiseMode, StampBlendMode, StampBrushData, StampPreset, TerrainBrushType,
    TerrainData, TerrainSettings, TerrainTab, TerrainToolState,
};
use renzora_terrain::paint::{SurfacePaintCommand, SurfacePaintSettings, SurfacePaintState};

use crate::terrain_inspector::TerrainInspectorTab;

use super::sculpt::{flatten_mode_label, stamp_blend_label};
use super::{
    set_settings, AddLayerBtn, EnableToggle, FalloffTypeBtn, FlattenModeCombo, HeightmapExportBtn,
    HeightmapImportBtn, LayerRow, MaterialClearBtn, MaterialDropZone, NoiseModeCombo, PaintToolBtn,
    SculptToolBtn, ShapeBtn, ShapeTarget, StampBlendCombo, StampLoadBtn, StampPresetCombo, TabBtn,
    MATERIAL_EXTS,
};

pub(super) fn enable_toggle_click(
    q: Query<&Interaction, (With<EnableToggle>, Changed<Interaction>)>,
    mut tool: Option<ResMut<TerrainToolState>>,
    settings: Option<Res<TerrainSettings>>,
    mut inspector_tab: Option<ResMut<TerrainInspectorTab>>,
    selection: Option<Res<EditorSelection>>,
    terrains: Query<Entity, With<TerrainData>>,
) {
    let Some(tool) = tool.as_mut() else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    tool.active = !tool.active;

    // The toggle is not just cosmetic: turning it on arms the current tab's
    // tool (via the inspector-tab resource `sync_active_tool_system` reads),
    // turning it off drops back to Select.
    let Some(tab) = inspector_tab.as_mut() else { return };
    if tool.active {
        **tab = match settings.map(|s| s.tab).unwrap_or_default() {
            TerrainTab::Sculpt => TerrainInspectorTab::Sculpt,
            TerrainTab::Paint => TerrainInspectorTab::Paint,
        };
        select_first_terrain_if_needed(selection.as_deref(), &terrains);
    } else {
        **tab = TerrainInspectorTab::Size;
    }
}

pub(super) fn tab_click(
    q: Query<(&Interaction, &TabBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<TerrainSettings>>,
    mut inspector_tab: Option<ResMut<TerrainInspectorTab>>,
    selection: Option<Res<EditorSelection>>,
    terrains: Query<Entity, With<TerrainData>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            settings.tab = btn.tab;
            // Switching the panel tab also switches the active tool — the
            // panel and the viewport toolbar drive the same state.
            if let Some(tab) = inspector_tab.as_mut() {
                **tab = match btn.tab {
                    TerrainTab::Sculpt => TerrainInspectorTab::Sculpt,
                    TerrainTab::Paint => TerrainInspectorTab::Paint,
                };
            }
            select_first_terrain_if_needed(selection.as_deref(), &terrains);
        }
    }
}

/// `sync_active_tool_system` only arms terrain tools while a terrain is
/// selected — so panel interactions select the first terrain when the current
/// selection isn't one (same behaviour as the viewport toolbar buttons).
fn select_first_terrain_if_needed(
    selection: Option<&EditorSelection>,
    terrains: &Query<Entity, With<TerrainData>>,
) {
    let Some(selection) = selection else { return };
    let selected_is_terrain = selection.get().is_some_and(|e| terrains.contains(e));
    if !selected_is_terrain {
        if let Some(first) = terrains.iter().next() {
            selection.set(Some(first));
        }
    }
}

/// Keep the panel following the viewport toolbar: when a terrain tool
/// activates (toolbar click, inspector tab), reveal the panel body and switch
/// its Sculpt/Paint tab to match, so the two never show contradictory state.
pub(super) fn follow_active_tool(
    active: Option<Res<ActiveTool>>,
    mut last: Local<Option<ActiveTool>>,
    mut tool: Option<ResMut<TerrainToolState>>,
    mut settings: Option<ResMut<TerrainSettings>>,
) {
    let Some(active) = active.map(|a| *a) else { return };
    if *last == Some(active) {
        return;
    }
    *last = Some(active);
    if !matches!(
        active,
        ActiveTool::TerrainSculpt | ActiveTool::TerrainPaint | ActiveTool::FoliagePaint
    ) {
        return;
    }
    if let Some(tool) = tool.as_mut() {
        if !tool.active {
            tool.active = true;
        }
    }
    let want = match active {
        ActiveTool::TerrainSculpt => Some(TerrainTab::Sculpt),
        ActiveTool::TerrainPaint => Some(TerrainTab::Paint),
        _ => None,
    };
    if let (Some(settings), Some(want)) = (settings.as_mut(), want) {
        if settings.tab != want {
            settings.tab = want;
        }
    }
}

pub(super) fn sculpt_tool_click(
    q: Query<(&Interaction, &SculptToolBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<TerrainSettings>>,
    mut stamp: Option<ResMut<StampBrushData>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            settings.brush_type = btn.brush;
            // Picking Stamp with nothing loaded gets a default shape so the
            // brush works immediately instead of silently no-opping.
            if btn.brush == TerrainBrushType::Stamp {
                if let Some(stamp) = stamp.as_mut() {
                    if !stamp.is_loaded() {
                        **stamp = StampBrushData::generate(StampPreset::Dome, 256);
                    }
                }
            }
        }
    }
}

pub(super) fn paint_tool_click(
    q: Query<(&Interaction, &PaintToolBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<SurfacePaintSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            settings.brush_type = btn.brush;
        }
    }
}

pub(super) fn shape_btn_click(
    q: Query<(&Interaction, &ShapeBtn), Changed<Interaction>>,
    mut terrain: Option<ResMut<TerrainSettings>>,
    mut paint: Option<ResMut<SurfacePaintSettings>>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn.target {
            ShapeTarget::Sculpt => {
                if let Some(t) = terrain.as_mut() {
                    t.brush_shape = btn.shape;
                }
            }
            ShapeTarget::Paint => {
                if let Some(p) = paint.as_mut() {
                    p.brush_shape = btn.shape;
                }
            }
        }
    }
}

pub(super) fn falloff_type_btn_click(
    q: Query<(&Interaction, &FalloffTypeBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<TerrainSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            settings.falloff_type = btn.ft;
        }
    }
}

pub(super) fn flatten_mode_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (With<FlattenModeCombo>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let modes = [FlattenMode::Both, FlattenMode::Raise, FlattenMode::Lower];
    let kids: Vec<Entity> = modes
        .iter()
        .map(|&mode| {
            menu_item(&mut commands, &fonts, "dot", &flatten_mode_label(mode), move |w| {
                set_settings(w, |s| s.flatten_mode = mode);
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

pub(super) fn noise_mode_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (With<NoiseModeCombo>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let kids: Vec<Entity> = NoiseMode::all()
        .iter()
        .map(|&mode| {
            menu_item(&mut commands, &fonts, "dot", mode.display_name(), move |w| {
                set_settings(w, |s| s.noise_mode = mode);
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

pub(super) fn stamp_preset_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (With<StampPresetCombo>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let presets = [
        StampPreset::Dome,
        StampPreset::Cone,
        StampPreset::Bell,
        StampPreset::Mesa,
        StampPreset::Ridge,
        StampPreset::Crater,
        StampPreset::Noise,
    ];
    let kids: Vec<Entity> = presets
        .iter()
        .map(|&preset| {
            menu_item(&mut commands, &fonts, "dot", preset.display_name(), move |w| {
                if let Some(mut stamp) = w.get_resource_mut::<StampBrushData>() {
                    *stamp = StampBrushData::generate(preset, 256);
                }
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

pub(super) fn stamp_blend_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (With<StampBlendCombo>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let modes = [
        StampBlendMode::Add,
        StampBlendMode::Subtract,
        StampBlendMode::Replace,
        StampBlendMode::Max,
        StampBlendMode::Min,
    ];
    let kids: Vec<Entity> = modes
        .iter()
        .map(|&mode| {
            menu_item(&mut commands, &fonts, "dot", &stamp_blend_label(mode), move |w| {
                set_settings(w, |s| s.stamp_blend_mode = mode);
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

pub(super) fn stamp_load_click(
    q: Query<&Interaction, (With<StampLoadBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|world: &mut World| {
            let _ = world.run_system_once(run_stamp_load);
        });
    }
}

fn run_stamp_load(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Stamp image", &["png"])
            .pick_file()
        else {
            return;
        };
        let Ok(bytes) = std::fs::read(&path) else {
            bevy::log::error!("Failed to read stamp file {path:?}");
            return;
        };
        match StampBrushData::load_from_png(&bytes) {
            Ok((width, height, pixels)) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Stamp".to_string());
                world.insert_resource(StampBrushData {
                    pixels,
                    width,
                    height,
                    name,
                });
            }
            Err(e) => bevy::log::error!("Stamp PNG load failed: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = world;
}

pub(super) fn layer_row_click(
    q: Query<(&Interaction, &LayerRow), Changed<Interaction>>,
    mut settings: Option<ResMut<SurfacePaintSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, row) in &q {
        if *interaction == Interaction::Pressed {
            settings.active_layer = row.index;
        }
    }
}

pub(super) fn add_layer_click(
    q: Query<&Interaction, (With<AddLayerBtn>, Changed<Interaction>)>,
    mut state: Option<ResMut<SurfacePaintState>>,
) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        state.pending_commands.push(SurfacePaintCommand::AddLayer);
    }
}

pub(super) fn heightmap_import_click(
    q: Query<&Interaction, (With<HeightmapImportBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|world: &mut World| {
            let _ = world.run_system_once(run_heightmap_import);
        });
    }
}

pub(super) fn heightmap_export_click(
    q: Query<&Interaction, (With<HeightmapExportBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|world: &mut World| {
            let _ = world.run_system_once(run_heightmap_export);
        });
    }
}

/// Import a heightmap file into every chunk.
fn run_heightmap_import(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Heightmap", &["png", "r16", "raw"])
            .pick_file()
        else {
            return;
        };
        let import_settings = renzora_terrain::heightmap_import::HeightmapImportSettings::default();
        let mut terrain_query = world.query::<&renzora_terrain::data::TerrainData>();
        let Some(terrain_data) = terrain_query.iter(world).next().cloned() else {
            return;
        };
        match renzora_terrain::heightmap_import::import_heightmap(&path, &import_settings, &terrain_data) {
            Ok(imported) => {
                let mut chunk_query = world.query::<&mut renzora_terrain::data::TerrainChunkData>();
                for mut chunk in chunk_query.iter_mut(world) {
                    if let Some((_, _, heights)) = imported
                        .iter()
                        .find(|(cx, cz, _)| *cx == chunk.chunk_x && *cz == chunk.chunk_z)
                    {
                        chunk.base_heights = heights.clone();
                        chunk.dirty = true;
                    }
                }
            }
            Err(e) => bevy::log::error!("Heightmap import failed: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = world;
}

/// Export the composed heightmap to a PNG.
fn run_heightmap_export(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).save_file() else {
            return;
        };
        let mut terrain_query = world.query::<&renzora_terrain::data::TerrainData>();
        let Some(terrain_data) = terrain_query.iter(world).next().cloned() else {
            return;
        };
        let mut chunk_query = world.query::<&renzora_terrain::data::TerrainChunkData>();
        let chunks: Vec<&renzora_terrain::data::TerrainChunkData> = chunk_query.iter(world).collect();
        match renzora_terrain::heightmap_import::export_heightmap_png16(&terrain_data, &chunks) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, &data) {
                    bevy::log::error!("Failed to write heightmap: {e}");
                }
            }
            Err(e) => bevy::log::error!("Heightmap export failed: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = world;
}

/// Drop a dragged `.material` asset onto the hovered layer zone → queue an
/// `AssignMaterial` command.
pub(super) fn material_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    project: Option<Res<CurrentProject>>,
    zones: Query<(&RelativeCursorPosition, &MaterialDropZone)>,
    mut state: Option<ResMut<SurfacePaintState>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(state)) = (payload, state.as_mut()) else {
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(MATERIAL_EXTS) {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        let path = project
            .as_ref()
            .map(|p| p.make_asset_relative(&payload.path))
            .unwrap_or_else(|| payload.path.to_string_lossy().to_string());
        state.pending_commands.push(SurfacePaintCommand::AssignMaterial {
            layer: zone.layer,
            path,
        });
        break;
    }
}

pub(super) fn material_clear_click(
    q: Query<(&Interaction, &MaterialClearBtn), Changed<Interaction>>,
    mut state: Option<ResMut<SurfacePaintState>>,
) {
    let Some(state) = state.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            state
                .pending_commands
                .push(SurfacePaintCommand::ClearMaterial(btn.layer));
        }
    }
}

/// Accent the zone border while a compatible `.material` asset is dragged over.
pub(super) fn material_drop_highlight(
    payload: Option<Res<AssetDragPayload>>,
    mut zones: Query<(&RelativeCursorPosition, &mut BorderColor), With<MaterialDropZone>>,
) {
    for (rcp, mut bc) in &mut zones {
        let active = payload
            .as_ref()
            .is_some_and(|p| p.is_detached && rcp.cursor_over && p.matches_extensions(MATERIAL_EXTS));
        let want = BorderColor::all(rgb(if active { accent() } else { border() }));
        if *bc != want {
            *bc = want;
        }
    }
}

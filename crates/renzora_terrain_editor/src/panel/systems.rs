//! What the component's controls do once clicked: mode tabs, brush picks,
//! layer rows, the stamp menu and file dialogs, the Manage buttons, and the
//! two asset slots' drag and drop.
//!
//! Anything that touches files, the undo stack or many chunks at once is
//! queued as an exclusive command rather than done inside the click system:
//! a file dialog blocks the thread, and a resize or a whole-terrain edit is
//! not something to do while holding queries over the chunks.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::core::CurrentProject;
use renzora_editor_framework::{AssetDragPayload, DocTabKind};
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::*;
use renzora_ember::widgets::{menu_item, screen_menu};
use renzora_ember::UiInteraction;

use renzora_shader::material::landscape_preset::ALPINE_LANDSCAPE_PATH;
use renzora_terrain::data::{StampBrushData, StampPreset, TerrainBrushType, TerrainData, TerrainSettings};
use renzora_terrain::paint::SurfacePaintSettings;
use renzora_terrain_graph::TerrainGenerator;

use crate::systems::HeightPick;
use crate::terrain_inspector::{ShownTerrainTab, TerrainInspectorTab};

use super::manage::{apply_draft, ManageDraft};
use super::{arm, panel_terrain, AssetSlot, HeightPickBtn, LayerRow, ManageAction, ModeTabBtn, SculptToolBtn, StampLoadBtn, StampPresetCombo};

fn pressed(i: &UiInteraction) -> bool {
    *i == UiInteraction::Pressed
}

/// A mode tab shows its pane and arms its tool (Manage arms Select).
pub(super) fn mode_tab_click(
    q: Query<(&UiInteraction, &ModeTabBtn), Changed<UiInteraction>>,
    mut armed: ResMut<TerrainInspectorTab>,
    mut shown: ResMut<ShownTerrainTab>,
) {
    for (i, tab) in &q {
        if pressed(i) {
            if *armed != tab.0 {
                *armed = tab.0;
            }
            if shown.0 != tab.0 {
                shown.0 = tab.0;
            }
        }
    }
}

/// Picking a sculpt brush also **arms sculpt mode**: clicking a tool is the
/// whole gesture. A brush that set what would be sculpted without arming the
/// sculpting was a click that visibly did nothing.
pub(super) fn sculpt_tool_click(
    q: Query<(&UiInteraction, &SculptToolBtn), Changed<UiInteraction>>,
    mut settings: Option<ResMut<TerrainSettings>>,
    mut stamp: Option<ResMut<StampBrushData>>,
    mut commands: Commands,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (i, btn) in &q {
        if !pressed(i) {
            continue;
        }
        settings.brush_type = btn.brush;
        commands.queue(|w: &mut World| arm(w, TerrainInspectorTab::Sculpt));
        // Picking Stamp with nothing loaded gets a default shape, so the brush
        // works at once instead of silently doing nothing.
        if btn.brush == TerrainBrushType::Stamp {
            if let Some(stamp) = stamp.as_mut() {
                if !stamp.is_loaded() {
                    **stamp = StampBrushData::generate(StampPreset::Dome, 256);
                }
            }
        }
    }
}

/// Picking a layer makes it the brush, and picks up the paint brush.
pub(super) fn layer_row_click(
    q: Query<(&UiInteraction, &LayerRow), Changed<UiInteraction>>,
    mut settings: Option<ResMut<SurfacePaintSettings>>,
    mut commands: Commands,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (i, row) in &q {
        if pressed(i) {
            settings.active_layer = row.index;
            commands.queue(|w: &mut World| arm(w, TerrainInspectorTab::Paint));
        }
    }
}

type ComboPress<'a> = (&'a UiInteraction, &'a RelativeCursorPosition, &'a ComputedNode);

pub(super) fn stamp_preset_combo_open(
    q: Query<ComboPress, (With<StampPresetCombo>, Changed<UiInteraction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| pressed(i)) else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    // Root-spawned by `screen_menu`, so the inspector's clipping can't eat it.
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
            menu_item(&mut commands, &fonts, "stamp", preset.display_name(), move |w| {
                if let Some(mut stamp) = w.get_resource_mut::<StampBrushData>() {
                    *stamp = StampBrushData::generate(preset, 256);
                }
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

pub(super) fn stamp_load_click(q: Query<&UiInteraction, (With<StampLoadBtn>, Changed<UiInteraction>)>, mut commands: Commands) {
    if q.iter().any(pressed) {
        commands.queue(load_stamp);
    }
}

fn load_stamp(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new().add_filter("Stamp image", &["png"]).pick_file() else { return };
        let Ok(bytes) = std::fs::read(&path) else {
            error!("Failed to read stamp file {path:?}");
            return;
        };
        match StampBrushData::load_from_png(&bytes) {
            Ok((width, height, pixels)) => {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Stamp".into());
                world.insert_resource(StampBrushData { pixels, width, height, name });
            }
            Err(e) => error!("Stamp PNG load failed: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = world;
}

/// Pick toggles: armed, the next click on the ground sets Set Height's target
/// instead of sculpting (see `crate::systems::height_pick_system`).
pub(super) fn height_pick_click(
    q: Query<&UiInteraction, (With<HeightPickBtn>, Changed<UiInteraction>)>,
    mut pick: ResMut<HeightPick>,
    mut commands: Commands,
) {
    if q.iter().any(pressed) {
        pick.armed = !pick.armed;
        if pick.armed {
            commands.queue(|w: &mut World| arm(w, TerrainInspectorTab::Sculpt));
        }
    }
}

pub(super) fn manage_action_click(q: Query<(&UiInteraction, &ManageAction), Changed<UiInteraction>>, mut commands: Commands) {
    for (i, action) in &q {
        if pressed(i) {
            let action = *action;
            commands.queue(move |w: &mut World| run_action(w, action));
        }
    }
}

fn run_action(world: &mut World, action: ManageAction) {
    let Some(terrain) = panel_terrain(world) else { return };
    match action {
        ManageAction::Apply => apply_draft(world),
        ManageAction::Revert => {
            if let Some(mut d) = world.get_resource_mut::<ManageDraft>() {
                d.revert();
            }
        }
        ManageAction::Reroll => {
            if let Some(mut g) = world.get_mut::<TerrainGenerator>(terrain) {
                g.seed = renzora_terrain::generate::next_seed(g.seed as u32) as u64;
            }
        }
        ManageAction::Build => renzora_terrain_graph_editor::request_build(world, terrain),
        ManageAction::CancelBuild => renzora_terrain_graph_editor::cancel_build(world, terrain),
        ManageAction::EditGraph => {
            if let Some(path) = world.get::<TerrainGenerator>(terrain).map(|g| g.graph.clone()).filter(|p| !p.is_empty()) {
                open_project_file(world, &path, DocTabKind::TerrainGraph);
            }
        }
        ManageAction::EditMaterial => {
            if let Some(path) = world.get::<renzora_terrain::landscape::TerrainMaterial>(terrain).map(|m| m.path.clone()).filter(|p| !p.is_empty()) {
                open_project_file(world, &path, DocTabKind::Material);
            }
        }
        ManageAction::UseAlpine => assign_material(world, terrain, ALPINE_LANDSCAPE_PATH),
        ManageAction::ChooseMaterial => {
            if let Some(path) = pick_project_file(world, "Landscape material", &["material"]) {
                assign_material(world, terrain, &path);
            }
        }
        ManageAction::ChooseGraph => {
            if let Some(path) = pick_project_file(world, "Terrain graph", &[renzora_terrain_graph::EXTENSION]) {
                assign_graph(world, terrain, &path);
            }
        }
        ManageAction::ErodeAll => erode_all(world, terrain),
        ManageAction::FlattenAll => crate::generate_tool::reset_now(world, terrain),
        ManageAction::ImportHeightmap => import_heightmap(world, terrain),
        ManageAction::ExportHeightmap => export_heightmap(world, terrain),
    }
}

fn open_project_file(world: &mut World, path: &str, kind: DocTabKind) {
    let Some(abs) = world.get_resource::<CurrentProject>().map(|p| p.resolve_path(path)) else { return };
    renzora_editor_framework::open_asset_tab(world, &abs, kind);
}

fn assign_material(world: &mut World, terrain: Entity, path: &str) {
    if world.get::<renzora_terrain::landscape::TerrainMaterial>(terrain).is_some_and(|m| m.path == path) {
        return;
    }
    if let Err(e) = renzora_terrain::landscape::assign_material(world, terrain, path) {
        warn!("[terrain] {e}");
    }
}

fn assign_graph(world: &mut World, terrain: Entity, path: &str) {
    match world.get_mut::<TerrainGenerator>(terrain) {
        Some(mut g) => {
            if g.graph != path {
                g.graph = path.to_string();
            }
        }
        None => {
            world.entity_mut(terrain).insert(TerrainGenerator::new(path, 1337));
        }
    }
}

/// A file dialog for a project asset, answered project-relative.
fn pick_project_file(world: &World, what: &str, exts: &[&str]) -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let project = world.get_resource::<CurrentProject>()?;
        let path = rfd::FileDialog::new().add_filter(what, exts).set_directory(&project.path).pick_file()?;
        Some(project.make_asset_relative(&path))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (world, what, exts);
        None
    }
}

/// Weather the whole terrain through the same operation the MCP's
/// erode_terrain uses, so it is one undo step and erodes the same way.
fn erode_all(world: &mut World, terrain: Entity) {
    let Some(tools) = world.get_resource::<renzora::terrain_tools::TerrainTools>().copied() else { return };
    // The default rain count suits a 512 m terrain; scale it with the area so
    // every terrain gets the same density.
    let area = world
        .get::<TerrainData>(terrain)
        .map_or(1.0, |t| t.total_width() * t.total_depth() / (512.0 * 512.0));
    let base = renzora::terrain_tools::TerrainErode::default().droplets;
    let droplets = ((base as f32 * area.max(0.25)) as u32).min(2_000_000);
    let request = renzora::terrain_tools::TerrainErode { terrain: Some(terrain), droplets, ..default() };
    if let Err(e) = (tools.erode)(world, &request) {
        warn!("[terrain] {e}");
    }
}

/// Import a heightmap file into this terrain's chunks.
fn import_heightmap(world: &mut World, terrain: Entity) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf};
        let Some(path) = rfd::FileDialog::new().add_filter("Heightmap", &["png", "r16", "raw"]).pick_file() else { return };
        let Some(data) = world.get::<TerrainData>(terrain).cloned() else { return };
        let settings = renzora_terrain::heightmap_import::HeightmapImportSettings::default();
        match renzora_terrain::heightmap_import::import_heightmap(&path, &settings, &data) {
            Ok(imported) => {
                let mut q = world.query::<(&mut TerrainChunkData, &TerrainChunkOf)>();
                for (mut chunk, of) in q.iter_mut(world) {
                    if of.0 != terrain {
                        continue;
                    }
                    if let Some((_, _, heights)) = imported.iter().find(|(cx, cz, _)| *cx == chunk.chunk_x && *cz == chunk.chunk_z) {
                        chunk.base_heights = heights.clone();
                        chunk.dirty = true;
                    }
                }
            }
            Err(e) => error!("Heightmap import failed: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (world, terrain);
}

/// Export this terrain's composed heights to a 16-bit PNG.
fn export_heightmap(world: &mut World, terrain: Entity) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf};
        let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).save_file() else { return };
        let Some(data) = world.get::<TerrainData>(terrain).cloned() else { return };
        let mut q = world.query::<(&TerrainChunkData, &TerrainChunkOf)>();
        let chunks: Vec<&TerrainChunkData> = q.iter(world).filter(|(_, of)| of.0 == terrain).map(|(c, _)| c).collect();
        match renzora_terrain::heightmap_import::export_heightmap_png16(&data, &chunks) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&path, &bytes) {
                    error!("Failed to write heightmap: {e}");
                }
            }
            Err(e) => error!("Heightmap export failed: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (world, terrain);
}

/// Drop a `.material` on the material slot or a `.tgraph` on the generator
/// slot.
pub(super) fn asset_slot_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    project: Option<Res<CurrentProject>>,
    slots: Query<(&RelativeCursorPosition, &AssetSlot)>,
    mut commands: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(payload) = payload.filter(|p| p.is_detached) else { return };
    for (rcp, slot) in &slots {
        if !rcp.cursor_over || !payload.matches_extensions(slot.extensions()) {
            continue;
        }
        let path = project
            .as_ref()
            .map(|p| p.make_asset_relative(&payload.path))
            .unwrap_or_else(|| payload.path.to_string_lossy().to_string());
        let slot = *slot;
        commands.queue(move |w: &mut World| {
            let Some(terrain) = panel_terrain(w) else { return };
            match slot {
                AssetSlot::Material => assign_material(w, terrain, &path),
                AssetSlot::Graph => assign_graph(w, terrain, &path),
            }
        });
        break;
    }
}

/// Accent a slot's border while an asset it takes is dragged over it.
pub(super) fn asset_slot_highlight(
    payload: Option<Res<AssetDragPayload>>,
    mut slots: Query<(&RelativeCursorPosition, &AssetSlot, &mut BorderColor)>,
) {
    for (rcp, slot, mut bc) in &mut slots {
        let hot = payload
            .as_ref()
            .is_some_and(|p| p.is_detached && rcp.cursor_over && p.matches_extensions(slot.extensions()));
        let want = BorderColor::all(rgb(if hot { accent() } else { border() }));
        if *bc != want {
            *bc = want;
        }
    }
}

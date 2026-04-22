#![allow(deprecated)] // egui API rename pending; will migrate at next bevy_egui bump.

//! Terrain inspector — renders the full terrain editing UI (tabs + content) as a
//! single custom `InspectorEntry` section. Replaces the former Tool Settings panel.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Rounding, CursorIcon, RichText};
use egui_phosphor::regular::*;

use renzora_editor_framework::{ActiveTool, EditorCommands, EditorSelection, inline_property};
use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use renzora_theme::Theme;

use crate::tool_panel::{render_foliage, render_paint, render_sculpt};

// ── Tab state ───────────────────────────────────────────────────────────────

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainInspectorTab {
    #[default]
    Size,
    Sculpt,
    Paint,
    Foliage,
    Heightmap,
}

impl TerrainInspectorTab {
    fn label(&self) -> (&'static str, &'static str) {
        match self {
            Self::Size => (RULER, "Size"),
            Self::Sculpt => (MOUNTAINS, "Sculpt"),
            Self::Paint => (PAINT_BRUSH, "Paint"),
            Self::Foliage => (TREE, "Foliage"),
            Self::Heightmap => (IMAGE, "Heightmap"),
        }
    }

    fn all() -> [Self; 5] {
        [Self::Size, Self::Sculpt, Self::Paint, Self::Foliage, Self::Heightmap]
    }

    /// Which ActiveTool this tab drives when selected and a terrain is picked.
    pub fn active_tool(&self) -> ActiveTool {
        match self {
            Self::Sculpt => ActiveTool::TerrainSculpt,
            Self::Paint => ActiveTool::TerrainPaint,
            Self::Foliage => ActiveTool::FoliagePaint,
            Self::Size | Self::Heightmap => ActiveTool::Select,
        }
    }
}

// ── Main render entry (registered via InspectorEntry::custom_ui_fn) ────────

pub fn render_terrain_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(terrain) = world.get::<TerrainData>(entity).cloned() else { return };
    let active_tab = world
        .get_resource::<TerrainInspectorTab>()
        .copied()
        .unwrap_or_default();

    render_tab_bar(ui, active_tab, theme, cmds);
    ui.add_space(6.0);

    match active_tab {
        TerrainInspectorTab::Size => render_size_tab(ui, world, entity, &terrain, theme, cmds),
        TerrainInspectorTab::Sculpt => render_sculpt(ui, world, theme, cmds),
        TerrainInspectorTab::Paint => render_paint(ui, world, theme, cmds),
        TerrainInspectorTab::Foliage => render_foliage(ui, world, theme, cmds),
        TerrainInspectorTab::Heightmap => {
            render_heightmap_tab(ui, entity, &terrain, theme, cmds)
        }
    }
}

// ── Tab bar ─────────────────────────────────────────────────────────────────

fn render_tab_bar(
    ui: &mut egui::Ui,
    active: TerrainInspectorTab,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let accent = theme.semantic.accent.to_color32();
    let inactive_bg = theme.widgets.inactive_bg.to_color32();
    let hovered_bg = theme.widgets.hovered_bg.to_color32();
    let text_primary = theme.text.primary.to_color32();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let avail = ui.available_width();
        let tab_count = TerrainInspectorTab::all().len() as f32;
        let tab_w = ((avail - 2.0 * (tab_count - 1.0)) / tab_count).max(48.0);
        for tab in TerrainInspectorTab::all() {
            let (icon, label) = tab.label();
            let is_active = tab == active;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(tab_w, 40.0), egui::Sense::click());
            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if ui.is_rect_visible(rect) {
                let bg = if is_active {
                    accent
                } else if response.hovered() {
                    hovered_bg
                } else {
                    inactive_bg
                };
                let fg = if is_active { Color32::WHITE } else { text_primary };
                ui.painter().rect_filled(rect, Rounding::same(4), bg);
                let icon_c = egui::pos2(rect.center().x, rect.center().y - 7.0);
                ui.painter().text(
                    icon_c,
                    egui::Align2::CENTER_CENTER,
                    icon,
                    egui::FontId::proportional(16.0),
                    fg,
                );
                let label_c = egui::pos2(rect.center().x, rect.max.y - 8.0);
                ui.painter().text(
                    label_c,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(10.0),
                    fg,
                );
            }
            if response.clicked() {
                cmds.push(move |w: &mut World| {
                    w.insert_resource(tab);
                });
            }
        }
    });
}

// ── Size tab ────────────────────────────────────────────────────────────────

fn render_size_tab(
    ui: &mut egui::Ui,
    _world: &World,
    entity: Entity,
    terrain: &TerrainData,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let text_secondary = theme.text.secondary.to_color32();

    // Info header
    ui.label(
        RichText::new(format!(
            "Size: {} × {} tiles  ({}m × {}m)",
            terrain.chunks_x,
            terrain.chunks_z,
            (terrain.chunks_x as f32 * terrain.chunk_size) as i32,
            (terrain.chunks_z as f32 * terrain.chunk_size) as i32,
        ))
        .size(11.0)
        .color(text_secondary),
    );
    ui.add_space(6.0);

    // Add Neighbor buttons
    ui.label(RichText::new("Add Tile").size(11.0).color(text_secondary));
    ui.add_space(2.0);
    let avail = ui.available_width();
    let btn_w = ((avail - 12.0) / 4.0).max(40.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (label, dir) in [
            ("↑ N", ExpandDirection::North),
            ("↓ S", ExpandDirection::South),
            ("← W", ExpandDirection::West),
            ("→ E", ExpandDirection::East),
        ] {
            if ui
                .add_sized(
                    egui::vec2(btn_w, 28.0),
                    egui::Button::new(RichText::new(label).size(12.0)),
                )
                .clicked()
            {
                let entity = entity;
                cmds.push(move |w: &mut World| expand_terrain(w, entity, dir));
            }
        }
    });
    ui.add_space(8.0);

    // Advanced collapsible
    let id = egui::Id::new(("terrain_size_advanced", entity));
    let mut open = ui.data_mut(|d| d.get_persisted::<bool>(id)).unwrap_or(false);
    let header = RichText::new(if open {
        format!("{}  Advanced", CARET_DOWN)
    } else {
        format!("{}  Advanced", CARET_RIGHT)
    })
    .size(11.0)
    .color(text_secondary);
    if ui
        .add(egui::Button::new(header).frame(false))
        .clicked()
    {
        open = !open;
        ui.data_mut(|d| d.insert_persisted(id, open));
    }
    if open {
        ui.add_space(4.0);
        ui.indent(id, |ui| {
            render_advanced_fields(ui, entity, terrain, theme, cmds);
        });
    }
}

fn render_advanced_fields(
    ui: &mut egui::Ui,
    entity: Entity,
    terrain: &TerrainData,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let mut chunks_x = terrain.chunks_x as f32;
    let mut chunks_z = terrain.chunks_z as f32;
    let mut chunk_size = terrain.chunk_size;
    let mut chunk_res = terrain.chunk_resolution as f32;
    let mut max_h = terrain.max_height;
    let mut min_h = terrain.min_height;

    let mut row = 0;
    inline_property(ui, row, "Chunks X", theme, |ui| {
        ui.add(egui::DragValue::new(&mut chunks_x).speed(1.0).range(1.0..=32.0))
    });
    row += 1;
    inline_property(ui, row, "Chunks Z", theme, |ui| {
        ui.add(egui::DragValue::new(&mut chunks_z).speed(1.0).range(1.0..=32.0))
    });
    row += 1;
    inline_property(ui, row, "Chunk Size", theme, |ui| {
        ui.add(egui::DragValue::new(&mut chunk_size).speed(1.0).range(8.0..=256.0))
    });
    row += 1;
    inline_property(ui, row, "Resolution", theme, |ui| {
        ui.add(egui::DragValue::new(&mut chunk_res).speed(1.0).range(3.0..=257.0))
    });
    row += 1;
    inline_property(ui, row, "Max Height", theme, |ui| {
        ui.add(egui::DragValue::new(&mut max_h).speed(1.0).range(0.0..=1000.0))
    });
    row += 1;
    inline_property(ui, row, "Min Height", theme, |ui| {
        ui.add(egui::DragValue::new(&mut min_h).speed(1.0).range(-500.0..=0.0))
    });
    let _ = row;

    if (chunks_x as u32) != terrain.chunks_x
        || (chunks_z as u32) != terrain.chunks_z
        || (chunk_size - terrain.chunk_size).abs() > f32::EPSILON
        || (chunk_res as u32) != terrain.chunk_resolution
        || (max_h - terrain.max_height).abs() > f32::EPSILON
        || (min_h - terrain.min_height).abs() > f32::EPSILON
    {
        cmds.push(move |w: &mut World| {
            if let Some(mut t) = w.get_mut::<TerrainData>(entity) {
                t.chunks_x = (chunks_x as u32).max(1);
                t.chunks_z = (chunks_z as u32).max(1);
                t.chunk_size = chunk_size.max(8.0);
                t.chunk_resolution = (chunk_res as u32).max(3);
                t.max_height = max_h;
                t.min_height = min_h;
            }
        });
    }
}

// ── Heightmap tab ───────────────────────────────────────────────────────────

fn render_heightmap_tab(
    ui: &mut egui::Ui,
    _entity: Entity,
    _terrain: &TerrainData,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();

    ui.label(
        RichText::new("Import a heightmap PNG or RAW16 file to reshape the terrain.")
            .size(11.0)
            .color(text_secondary),
    );
    ui.add_space(8.0);

    if ui
        .add(
            egui::Button::new(
                RichText::new(format!("{}  Import Heightmap…", PLUS))
                    .size(11.0)
                    .color(text_primary),
            )
            .min_size(egui::vec2(ui.available_width(), 26.0)),
        )
        .clicked()
    {
        cmds.push(|world: &mut World| import_heightmap_action(world));
    }
    ui.add_space(4.0);
    if ui
        .add(
            egui::Button::new(
                RichText::new("Export Heightmap…")
                    .size(11.0)
                    .color(text_secondary),
            )
            .min_size(egui::vec2(ui.available_width(), 24.0)),
        )
        .clicked()
    {
        cmds.push(|world: &mut World| export_heightmap_action(world));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn import_heightmap_action(world: &mut World) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Heightmap", &["png", "r16", "raw"])
        .pick_file()
    else {
        return;
    };
    let settings = renzora_terrain::heightmap_import::HeightmapImportSettings::default();
    let terrain_data = {
        let mut q = world.query::<&TerrainData>();
        q.iter(world).next().cloned()
    };
    let Some(terrain_data) = terrain_data else { return };
    match renzora_terrain::heightmap_import::import_heightmap(&path, &settings, &terrain_data) {
        Ok(imported) => {
            let mut q = world.query::<&mut TerrainChunkData>();
            for mut chunk in q.iter_mut(world) {
                if let Some((_, _, heights)) = imported
                    .iter()
                    .find(|(cx, cz, _)| *cx == chunk.chunk_x && *cz == chunk.chunk_z)
                {
                    chunk.heights = heights.clone();
                    chunk.dirty = true;
                }
            }
        }
        Err(e) => bevy::log::error!("Heightmap import failed: {e}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn import_heightmap_action(_world: &mut World) {}

#[cfg(not(target_arch = "wasm32"))]
fn export_heightmap_action(world: &mut World) {
    let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).save_file() else {
        return;
    };
    let terrain_data = {
        let mut q = world.query::<&TerrainData>();
        q.iter(world).next().cloned()
    };
    let Some(terrain_data) = terrain_data else { return };
    let chunks: Vec<TerrainChunkData> = {
        let mut q = world.query::<&TerrainChunkData>();
        q.iter(world).cloned().collect()
    };
    let chunk_refs: Vec<&TerrainChunkData> = chunks.iter().collect();
    match renzora_terrain::heightmap_import::export_heightmap_png16(&terrain_data, &chunk_refs) {
        Ok(data) => {
            if let Err(e) = std::fs::write(&path, &data) {
                bevy::log::error!("Failed to write heightmap: {e}");
            }
        }
        Err(e) => bevy::log::error!("Heightmap export failed: {e}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn export_heightmap_action(_world: &mut World) {}

// ── Terrain expansion ───────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub enum ExpandDirection {
    North, // -Z (shift existing + translate root)
    South, // +Z (just increment chunks_z)
    West,  // -X (shift existing + translate root)
    East,  // +X (just increment chunks_x)
}

fn expand_terrain(world: &mut World, entity: Entity, dir: ExpandDirection) {
    // For N / W we must shift every existing chunk coord before bumping the count,
    // otherwise the rebuild system would place old chunks at the new grid's origin
    // corner instead of preserving their world positions.
    match dir {
        ExpandDirection::East => {
            if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                t.chunks_x = t.chunks_x.saturating_add(1).min(64);
            }
        }
        ExpandDirection::South => {
            if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                t.chunks_z = t.chunks_z.saturating_add(1).min(64);
            }
        }
        ExpandDirection::West => {
            shift_chunks(world, entity, 1, 0);
            if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                t.chunks_x = t.chunks_x.saturating_add(1).min(64);
            }
            translate_root(world, entity, Vec3::new(-chunk_size(world, entity), 0.0, 0.0));
        }
        ExpandDirection::North => {
            shift_chunks(world, entity, 0, 1);
            if let Some(mut t) = world.get_mut::<TerrainData>(entity) {
                t.chunks_z = t.chunks_z.saturating_add(1).min(64);
            }
            translate_root(world, entity, Vec3::new(0.0, 0.0, -chunk_size(world, entity)));
        }
    }
}

fn chunk_size(world: &World, entity: Entity) -> f32 {
    world
        .get::<TerrainData>(entity)
        .map(|t| t.chunk_size)
        .unwrap_or(64.0)
}

fn shift_chunks(world: &mut World, terrain_entity: Entity, dx: u32, dz: u32) {
    let mut q = world.query::<(&mut TerrainChunkData, &TerrainChunkOf)>();
    for (mut chunk, owner) in q.iter_mut(world) {
        if owner.0 == terrain_entity {
            chunk.chunk_x += dx;
            chunk.chunk_z += dz;
        }
    }
}

fn translate_root(world: &mut World, entity: Entity, delta: Vec3) {
    if let Some(mut tf) = world.get_mut::<Transform>(entity) {
        tf.translation += delta;
    }
}

// ── ActiveTool <-> tab sync ────────────────────────────────────────────────

/// Sync `ActiveTool` to follow the selected terrain's inspector tab.
/// Resets to `Select` when no terrain is selected.
pub fn sync_active_tool_system(
    selection: Res<EditorSelection>,
    tab: Option<Res<TerrainInspectorTab>>,
    terrain_query: Query<&TerrainData>,
    mut active: ResMut<ActiveTool>,
) {
    let terrain_selected = selection
        .get()
        .map(|e| terrain_query.get(e).is_ok())
        .unwrap_or(false);

    let desired = if terrain_selected {
        tab.map(|t| t.active_tool()).unwrap_or(ActiveTool::Select)
    } else {
        // If the user switched away from a terrain while a terrain tool was
        // active, drop back to Select so brush gizmos stop rendering.
        if active.is_terrain() || *active == ActiveTool::FoliagePaint {
            ActiveTool::Select
        } else {
            *active
        }
    };

    if *active != desired {
        *active = desired;
    }
}

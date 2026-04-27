//! "Layers" section of the terrain inspector, backed by the `Painter`
//! component on the terrain entity.
//!
//! Reads the per-frame `PainterRegistry` cache for display, pushes
//! deferred closures for mutations, and supports drag-to-reorder.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, Vec2};
use egui_phosphor::regular as icons;
use renzora::core::CurrentProject;
use renzora_editor::{
    asset_drop_target, AssetDragPayload, EditorCommands, InspectorEntry,
};
use renzora_terrain::data::TerrainData;
use renzora_terrain::painter::{
    LayerPreview, PaintLayer, Painter, PainterLayerMesh, PainterPreview, PainterRegistry,
};
use renzora_theme::Theme;

const MAX_LAYERS: usize = 16;

/// `ActiveBrushLayer` moved into the `Painter` itself (`active_layer`), but
/// during the transition we still register this type so the old
/// brush-layer paint path links. It's no longer written by the new UI.
#[derive(Resource, Default)]
pub struct ActiveBrushLayer(pub Option<Entity>);

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
        custom_ui_fn: Some(render_layer_list),
    }
}

fn render_layer_list(
    ui: &mut egui::Ui,
    world: &World,
    terrain_entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let text_secondary = theme.text.secondary.to_color32();

    let empty_preview = PainterPreview::default();
    let preview = world
        .get_resource::<PainterRegistry>()
        .and_then(|r| r.painters.get(&terrain_entity))
        .unwrap_or(&empty_preview);

    if preview.layers.is_empty() {
        ui.label(
            RichText::new("No layers yet — click Add Layer to create one.")
                .size(11.0)
                .color(text_secondary),
        );
    }

    // Track drag source / hover target for reorder.
    let drag_state_id = egui::Id::new(("painter_drag", terrain_entity));
    let mut dragged: Option<usize> = ui
        .ctx()
        .data(|d| d.get_temp::<Option<usize>>(drag_state_id))
        .flatten();

    let mut drop_target: Option<usize> = None;
    let mut drop_fired: Option<(usize, usize)> = None;

    for (i, layer) in preview.layers.iter().enumerate() {
        let row_response = render_layer_row(
            ui,
            i,
            layer,
            Some(i) == preview.active_layer,
            terrain_entity,
            theme,
            world,
            cmds,
        );

        // Drag source: pointer-down on the drag handle starts a drag.
        if row_response.drag_handle_clicked_down {
            dragged = Some(i);
        }
        if row_response.hovered && dragged.is_some() && dragged != Some(i) {
            drop_target = Some(i);
        }

        if dragged.is_some() && drop_target == Some(i) {
            // Drop indicator: a thin accent line above the hovered row.
            let line_y = row_response.row_rect.min.y;
            let stroke = egui::Stroke::new(2.0, theme.widgets.active_bg.to_color32());
            ui.painter().line_segment(
                [
                    egui::pos2(row_response.row_rect.min.x, line_y),
                    egui::pos2(row_response.row_rect.max.x, line_y),
                ],
                stroke,
            );
        }

        ui.add_space(4.0);
    }

    // Finalize drag on pointer release.
    let pointer_released = ui.ctx().input(|i| i.pointer.any_released());
    if pointer_released {
        if let (Some(from), Some(to)) = (dragged, drop_target) {
            if from != to {
                drop_fired = Some((from, to));
            }
        }
        dragged = None;
        drop_target = None;
    }

    ui.ctx()
        .data_mut(|d| d.insert_temp::<Option<usize>>(drag_state_id, dragged));

    if let Some((from, to)) = drop_fired {
        let target = terrain_entity;
        cmds.push(move |world: &mut World| {
            move_layer(world, target, from, to);
        });
    }

    ui.add_space(2.0);
    if preview.layers.len() < MAX_LAYERS
        && ui
            .add(
                egui::Button::new(
                    RichText::new(format!("{}  Add Layer", icons::PLUS))
                        .size(11.0)
                        .color(text_secondary),
                )
                .min_size(Vec2::new(ui.available_width(), 22.0)),
            )
            .clicked()
    {
        let target = terrain_entity;
        cmds.push(move |world: &mut World| {
            add_layer(world, target);
        });
    }
}

struct RowResponse {
    row_rect: egui::Rect,
    hovered: bool,
    drag_handle_clicked_down: bool,
}

#[allow(clippy::too_many_arguments)]
fn render_layer_row(
    ui: &mut egui::Ui,
    idx: usize,
    layer: &LayerPreview,
    is_active: bool,
    painter_entity: Entity,
    theme: &Theme,
    world: &World,
    cmds: &EditorCommands,
) -> RowResponse {
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let accent = theme.widgets.active_bg.to_color32();
    let bg = if is_active {
        accent
    } else {
        theme.widgets.inactive_bg.to_color32()
    };
    let fg = if is_active { Color32::WHITE } else { text_primary };

    let start_y = ui.cursor().min.y;
    let mut drag_handle_clicked_down = false;

    ui.horizontal(|ui| {
        // Drag handle
        let drag_response = ui.add(
            egui::Button::new(
                RichText::new(icons::DOTS_SIX_VERTICAL)
                    .size(12.0)
                    .color(text_secondary),
            )
            .frame(false)
            .min_size(Vec2::new(18.0, 22.0)),
        );
        if drag_response.is_pointer_button_down_on() {
            drag_handle_clicked_down = true;
        }

        // Enable toggle
        let enabled_icon = if layer.enabled { icons::EYE } else { icons::EYE_SLASH };
        if ui
            .add(
                egui::Button::new(RichText::new(enabled_icon).size(12.0).color(text_secondary))
                    .frame(false)
                    .min_size(Vec2::new(22.0, 22.0)),
            )
            .on_hover_text("Toggle layer visibility")
            .clicked()
        {
            let target = painter_entity;
            let layer_idx = idx;
            cmds.push(move |world: &mut World| {
                if let Some(mut painter) = world.get_mut::<Painter>(target) {
                    if let Some(l) = painter.layers.get_mut(layer_idx) {
                        l.enabled = !l.enabled;
                        l.mesh_dirty = true;
                    }
                }
            });
        }

        // Name (active selector)
        let label = RichText::new(format!("{}  {}", idx + 1, layer.name))
            .size(12.0)
            .color(fg);
        if ui
            .add(
                egui::Button::new(label)
                    .fill(bg)
                    .min_size(Vec2::new(ui.available_width() - 30.0, 22.0)),
            )
            .clicked()
        {
            let target = painter_entity;
            let layer_idx = idx;
            cmds.push(move |world: &mut World| {
                if let Some(mut painter) = world.get_mut::<Painter>(target) {
                    painter.active_layer = Some(layer_idx);
                }
            });
        }

        // Delete
        if ui
            .add(
                egui::Button::new(RichText::new(icons::TRASH).size(12.0).color(text_secondary))
                    .frame(false)
                    .min_size(Vec2::new(22.0, 22.0)),
            )
            .clicked()
        {
            let target = painter_entity;
            let layer_idx = idx;
            cmds.push(move |world: &mut World| {
                delete_layer(world, target, layer_idx);
            });
        }
    });

    // Material drop
    let drag_payload = world.get_resource::<AssetDragPayload>();
    let drop_result = asset_drop_target(
        ui,
        egui::Id::new(("painter_layer_mat", painter_entity, idx)),
        layer.material_path.as_deref(),
        &["material"],
        "Drop .material file",
        theme,
        drag_payload,
    );
    if let Some(ref dropped) = drop_result.dropped_path {
        let path = if let Some(project) = world.get_resource::<CurrentProject>() {
            project.make_asset_relative(dropped)
        } else {
            dropped.to_string_lossy().to_string()
        };
        let target = painter_entity;
        let layer_idx = idx;
        cmds.push(move |world: &mut World| {
            if let Some(mut painter) = world.get_mut::<Painter>(target) {
                if let Some(l) = painter.layers.get_mut(layer_idx) {
                    l.material_path = Some(path);
                    l.material_dirty = true;
                }
            }
        });
    }
    if drop_result.cleared {
        let target = painter_entity;
        let layer_idx = idx;
        cmds.push(move |world: &mut World| {
            if let Some(mut painter) = world.get_mut::<Painter>(target) {
                if let Some(l) = painter.layers.get_mut(layer_idx) {
                    l.material_path = None;
                    l.material_dirty = true;
                }
            }
        });
    }

    // Offset slider
    let mut offset = layer.height_offset;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Offset").size(11.0).color(text_secondary));
        if ui
            .add(
                egui::Slider::new(&mut offset, -5.0..=5.0)
                    .fixed_decimals(2)
                    .clamping(egui::SliderClamping::Always),
            )
            .changed()
        {
            let target = painter_entity;
            let layer_idx = idx;
            let new_offset = offset;
            cmds.push(move |world: &mut World| {
                if let Some(mut painter) = world.get_mut::<Painter>(target) {
                    if let Some(l) = painter.layers.get_mut(layer_idx) {
                        l.height_offset = new_offset;
                        l.mesh_dirty = true;
                    }
                }
            });
        }
    });

    let end_y = ui.cursor().min.y;
    let row_rect = egui::Rect::from_min_max(
        egui::pos2(ui.min_rect().min.x, start_y),
        egui::pos2(ui.min_rect().max.x, end_y),
    );
    let pointer = ui.ctx().pointer_hover_pos();
    let hovered = pointer.map_or(false, |p| row_rect.contains(p));

    RowResponse {
        row_rect,
        hovered,
        drag_handle_clicked_down,
    }
}

// ── Deferred-command helpers ─────────────────────────────────────────────────

fn add_layer(world: &mut World, terrain_entity: Entity) {
    let Some(terrain) = world.get::<TerrainData>(terrain_entity) else {
        return;
    };
    let grid = terrain.chunks_x * (terrain.chunk_resolution - 1) + 1;
    let grid = grid.max(terrain.chunks_z * (terrain.chunk_resolution - 1) + 1);

    // Ensure the terrain has a Painter.
    if world.get::<Painter>(terrain_entity).is_none() {
        world.entity_mut(terrain_entity).insert(Painter::default());
    }

    let name = {
        let painter = world.get::<Painter>(terrain_entity).unwrap();
        format!("Layer {}", painter.layers.len() + 1)
    };

    if let Some(mut painter) = world.get_mut::<Painter>(terrain_entity) {
        let layer = PaintLayer::empty(name, grid);
        painter.layers.push(layer);
        painter.active_layer = Some(painter.layers.len() - 1);
    }
}

fn delete_layer(world: &mut World, painter_entity: Entity, idx: usize) {
    // Collect markers + indices first so we can re-enter world for despawns.
    let markers: Vec<(Entity, usize)> = {
        let mut q = world.query::<(Entity, &PainterLayerMesh)>();
        q.iter(world)
            .filter(|(_, m)| m.painter == painter_entity)
            .map(|(e, m)| (e, m.layer_index))
            .collect()
    };
    // Mutate Painter data.
    let still_alive = {
        if let Some(mut painter) = world.get_mut::<Painter>(painter_entity) {
            if idx >= painter.layers.len() {
                return;
            }
            painter.layers.remove(idx);
            painter.active_layer = match painter.active_layer {
                Some(a) if a == idx => {
                    if painter.layers.is_empty() {
                        None
                    } else {
                        Some(a.min(painter.layers.len() - 1))
                    }
                }
                Some(a) if a > idx => Some(a - 1),
                other => other,
            };
            for l in painter.layers.iter_mut() {
                l.mesh_dirty = true;
                l.material_dirty = true;
            }
            true
        } else {
            false
        }
    };
    if !still_alive {
        return;
    }
    // Despawn/re-index child meshes.
    for (entity, old_idx) in markers {
        if old_idx == idx {
            world.entity_mut(entity).despawn();
        } else if old_idx > idx {
            world.entity_mut(entity).insert(PainterLayerMesh {
                painter: painter_entity,
                layer_index: old_idx - 1,
            });
        }
    }
}

fn move_layer(world: &mut World, painter_entity: Entity, from: usize, to: usize) {
    if from == to {
        return;
    }
    // Re-order layers in Painter.
    {
        let Some(mut painter) = world.get_mut::<Painter>(painter_entity) else {
            return;
        };
        if from >= painter.layers.len() || to >= painter.layers.len() {
            return;
        }
        let layer = painter.layers.remove(from);
        painter.layers.insert(to, layer);
        painter.active_layer = painter.active_layer.map(|a| {
            if a == from {
                to
            } else if from < a && a <= to {
                a - 1
            } else if to <= a && a < from {
                a + 1
            } else {
                a
            }
        });
        for l in painter.layers.iter_mut() {
            l.mesh_dirty = true;
        }
    }

    // Update child-mesh marker indices.
    let markers: Vec<(Entity, usize)> = {
        let mut q = world.query::<(Entity, &PainterLayerMesh)>();
        q.iter(world)
            .filter(|(_, m)| m.painter == painter_entity)
            .map(|(e, m)| (e, m.layer_index))
            .collect()
    };
    for (entity, old_idx) in markers {
        let new_idx = if old_idx == from {
            to
        } else if from < to && old_idx > from && old_idx <= to {
            old_idx - 1
        } else if to < from && old_idx >= to && old_idx < from {
            old_idx + 1
        } else {
            old_idx
        };
        if new_idx != old_idx {
            world.entity_mut(entity).insert(PainterLayerMesh {
                painter: painter_entity,
                layer_index: new_idx,
            });
        }
    }
}

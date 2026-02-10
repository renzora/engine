//! Component registration for PaintableSurfaceData.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::PAINT_BRUSH;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::ui::inline_property;
use crate::{console_info, console_success, console_warn};

use super::brush::{create_splatmap_image, SplatmapEntry, SplatmapHandles, SplatmapMaterialMarker};
use super::data::{MaterialLayer, PaintableSurfaceData};
use super::material::SplatmapMaterial;

// ---------------------------------------------------------------------------
// Custom add: initialise splatmap image + material, attach to entity
// ---------------------------------------------------------------------------

fn add_paintable_surface(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    // Insert default component; the init system will create the GPU resources
    commands.entity(entity).insert(PaintableSurfaceData::default());
}

/// Startup / reactive system: for every PaintableSurfaceData **without** a splatmap entry,
/// create the GPU image and material, and attach `MeshMaterial3d<SplatmapMaterial>`.
pub(crate) fn init_splatmap_for_new_surfaces(
    mut commands: Commands,
    query: Query<(Entity, &PaintableSurfaceData), Without<SplatmapMaterialMarker>>,
    mut images: ResMut<Assets<Image>>,
    mut splatmap_materials: ResMut<Assets<SplatmapMaterial>>,
    mut splatmap_handles: ResMut<SplatmapHandles>,
) {
    for (entity, surface) in query.iter() {
        console_info!("Surface Paint", "Initializing splatmap for entity {:?}", entity);
        console_info!("Surface Paint", "  layers: {}, resolution: {}x{}, weights: {}",
            surface.layers.len(), surface.splatmap_resolution, surface.splatmap_resolution, surface.splatmap_weights.len());

        let image = create_splatmap_image(surface);
        let image_handle = images.add(image);

        let mat = SplatmapMaterial {
            layer_colors_0: layer_color(&surface.layers, 0),
            layer_colors_1: layer_color(&surface.layers, 1),
            layer_colors_2: layer_color(&surface.layers, 2),
            layer_colors_3: layer_color(&surface.layers, 3),
            layer_props_0: layer_props(&surface.layers, 0),
            layer_props_1: layer_props(&surface.layers, 1),
            layer_props_2: layer_props(&surface.layers, 2),
            layer_props_3: layer_props(&surface.layers, 3),
            splatmap: image_handle.clone(),
        };
        let material_handle = splatmap_materials.add(mat);

        // Remove StandardMaterial to avoid dual-material rendering conflict,
        // then attach splatmap material
        commands.entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(material_handle.clone()))
            .insert(SplatmapMaterialMarker);

        console_success!("Surface Paint", "Attached splatmap material to entity {:?}", entity);

        splatmap_handles.entries.push(SplatmapEntry {
            entity,
            image_handle,
            material_handle,
        });
    }
}

fn layer_color(_layers: &[MaterialLayer], _idx: usize) -> Vec4 {
    // Colors are no longer used (layers are shader-driven)
    Vec4::ZERO
}

fn layer_props(layers: &[MaterialLayer], idx: usize) -> Vec4 {
    layers.get(idx).map_or(Vec4::new(0.0, 0.5, 1.0, 1.0), |l| {
        Vec4::new(l.metallic, l.roughness, l.uv_scale.x, l.uv_scale.y)
    })
}

// ---------------------------------------------------------------------------
// Custom remove: clean up splatmap material and handles
// ---------------------------------------------------------------------------

fn remove_paintable_surface(commands: &mut Commands, entity: Entity) {
    commands.entity(entity)
        .remove::<PaintableSurfaceData>()
        .remove::<MeshMaterial3d<SplatmapMaterial>>()
        .remove::<SplatmapMaterialMarker>()
        // Restore a default StandardMaterial so the mesh is visible again
        .insert(MeshMaterial3d(Handle::<StandardMaterial>::default()));
}

/// Ensure splatmap entities never have a StandardMaterial (the Mesh Renderer or
/// scene deserialization may re-add one).  Runs every frame, cheap query.
pub(crate) fn enforce_splatmap_material(
    mut commands: Commands,
    query: Query<Entity, (With<SplatmapMaterialMarker>, With<MeshMaterial3d<StandardMaterial>>)>,
) {
    for entity in query.iter() {
        console_warn!("Surface Paint", "Entity {:?} still has StandardMaterial — removing it", entity);
        commands.entity(entity).remove::<MeshMaterial3d<StandardMaterial>>();
    }
}

/// Cleanup system: remove stale SplatmapHandles entries when entities lose the component.
pub(crate) fn cleanup_splatmap_handles(
    mut splatmap_handles: ResMut<SplatmapHandles>,
    query: Query<Entity, With<SplatmapMaterialMarker>>,
) {
    splatmap_handles.entries.retain(|entry| query.contains(entry.entity));
}

// ---------------------------------------------------------------------------
// Custom inspector
// ---------------------------------------------------------------------------

fn inspect_paintable_surface(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    // Read dragging state from InspectorPanelRenderState (captured before panel loop,
    // so it survives cross-panel clearing of AssetBrowserState.dragging_asset)
    let dragging_material: Option<String> = world
        .get_resource::<crate::core::InspectorPanelRenderState>()
        .and_then(|state| state.dragging_asset_path.as_ref())
        .and_then(|p| {
            let s = p.to_string_lossy();
            if s.ends_with(".wgsl") || s.ends_with(".material_bp") {
                Some(s.to_string())
            } else {
                None
            }
        });

    let Some(mut data) = world.get_mut::<PaintableSurfaceData>(entity) else {
        return false;
    };
    let mut changed = false;
    let mut row = 0;

    // Resolution (read-only display)
    inline_property(ui, row, "Resolution", |ui| {
        ui.label(format!("{}x{}", data.splatmap_resolution, data.splatmap_resolution));
        false
    });
    row += 1;

    // Layer count
    let layer_count = data.layers.len();
    inline_property(ui, row, "Layers", |ui| {
        ui.label(format!("{}/4", layer_count));
        false
    });
    row += 1;

    ui.separator();

    // Per-layer editing
    let mut layers_to_remove: Option<usize> = None;
    let mut mark_dirty = false;
    let mut material_drop: Option<(usize, String)> = None;
    let mut material_clear: Option<usize> = None;

    for i in 0..data.layers.len() {
        let layer_label = format!("Layer {}", i);
        ui.push_id(i, |ui| {
            ui.label(egui::RichText::new(&layer_label).size(11.0).strong());

            let name_label = format!("  Name {}", i);
            changed |= inline_property(ui, row, &name_label, |ui| {
                let resp = ui.text_edit_singleline(&mut data.layers[i].name).changed();
                resp
            });
            row += 1;

            // Material source (drop zone)
            ui.add_space(2.0);
            if let Some(src) = &data.layers[i].texture_path {
                // Assigned material: show filename in a styled box
                let filename = src.rsplit(['/', '\\']).next().unwrap_or(src);
                let frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgba_premultiplied(40, 80, 120, 60))
                    .rounding(4.0)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 140, 200)));
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("\u{f1c9}").size(12.0).color(egui::Color32::from_rgb(100, 180, 255)));
                        ui.label(egui::RichText::new(filename).size(10.0).color(egui::Color32::from_rgb(180, 210, 240)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("\u{00d7}").clicked() {
                                material_clear = Some(i);
                            }
                        });
                    });
                });
            } else {
                // Empty drop zone
                let is_dragging = dragging_material.is_some();
                let hover_color = if is_dragging {
                    egui::Color32::from_rgb(80, 160, 240)
                } else {
                    egui::Color32::from_gray(80)
                };

                let frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgba_premultiplied(30, 30, 30, 100))
                    .rounding(4.0)
                    .inner_margin(egui::Margin::symmetric(8, 6));

                let drop_resp = frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.vertical_centered(|ui| {
                        if is_dragging {
                            ui.label(egui::RichText::new("Drop material here").size(10.0).color(egui::Color32::from_rgb(100, 180, 255)));
                        } else {
                            ui.label(egui::RichText::new("Drag .wgsl / .material_bp").size(10.0).color(egui::Color32::from_gray(100)));
                        }
                    });
                });

                // Check drop via pointer position (more reliable than egui hover during cross-panel drags)
                let drop_rect = drop_resp.response.rect;
                let pointer_pos = ui.input(|inp| inp.pointer.hover_pos());
                let pointer_released = ui.input(|inp| inp.pointer.any_released());
                let pointer_in_zone = pointer_pos.map_or(false, |pos| drop_rect.contains(pos));

                // Draw border (dashed effect via stroke)
                let border_color = if is_dragging && pointer_in_zone {
                    egui::Color32::from_rgb(100, 200, 255)
                } else {
                    hover_color
                };
                let stroke_width = if is_dragging && pointer_in_zone { 2.0 } else { 1.0 };
                ui.painter().rect_stroke(
                    drop_rect,
                    4.0,
                    egui::Stroke::new(stroke_width, border_color),
                    egui::StrokeKind::Outside,
                );

                if is_dragging && pointer_in_zone && pointer_released {
                    if let Some(path) = &dragging_material {
                        material_drop = Some((i, path.clone()));
                    }
                }
            }
            ui.add_space(2.0);

            let roughness_label = format!("  Roughness {}", i);
            changed |= inline_property(ui, row, &roughness_label, |ui| {
                let resp = ui.add(egui::DragValue::new(&mut data.layers[i].roughness).speed(0.01).range(0.0..=1.0)).changed();
                if resp { mark_dirty = true; }
                resp
            });
            row += 1;

            let metallic_label = format!("  Metallic {}", i);
            changed |= inline_property(ui, row, &metallic_label, |ui| {
                let resp = ui.add(egui::DragValue::new(&mut data.layers[i].metallic).speed(0.01).range(0.0..=1.0)).changed();
                if resp { mark_dirty = true; }
                resp
            });
            row += 1;

            // Remove button (only if > 1 layer)
            if data.layers.len() > 1 {
                if ui.small_button("Remove").clicked() {
                    layers_to_remove = Some(i);
                }
            }

            ui.add_space(4.0);
        });
    }

    // Apply material drops
    if let Some((idx, path)) = material_drop {
        data.layers[idx].texture_path = Some(path);
        mark_dirty = true;
        changed = true;
    }

    // Apply material clears
    if let Some(idx) = material_clear {
        data.layers[idx].texture_path = None;
        mark_dirty = true;
        changed = true;
    }

    // Add layer button
    if data.layers.len() < 4 {
        if ui.button("+ Add Layer").clicked() {
            data.layers.push(MaterialLayer::default());
            mark_dirty = true;
            changed = true;
        }
    }

    // Apply removals
    if let Some(idx) = layers_to_remove {
        data.layers.remove(idx);
        mark_dirty = true;
        changed = true;
    }

    if mark_dirty {
        data.dirty = true;
    }

    changed
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(PaintableSurfaceData {
        type_id: "paintable_surface",
        display_name: "Paintable Surface",
        category: ComponentCategory::Rendering,
        icon: PAINT_BRUSH,
        custom_inspector: inspect_paintable_surface,
        custom_add: add_paintable_surface,
        custom_remove: remove_paintable_surface,
    }));
}

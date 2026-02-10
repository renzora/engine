//! Surface paint brush systems — hover, paint, and GPU sync.

use bevy::prelude::*;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::window::PrimaryWindow;

use crate::core::{InputFocusState, ViewportCamera, ViewportState};
use crate::gizmo::{EditorTool, GizmoState};
use crate::terrain::{BrushFalloffType, BrushShape};

use crate::{console_info, console_warn};

use super::data::{PaintBrushType, PaintableSurfaceData, SurfacePaintSettings, SurfacePaintState};
use super::material::SplatmapMaterial;

/// System to handle P key shortcut for surface paint tool.
pub fn surface_paint_shortcut_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut gizmo_state: ResMut<GizmoState>,
    input_focus: Res<InputFocusState>,
) {
    if input_focus.egui_wants_keyboard {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        gizmo_state.tool = EditorTool::SurfacePaint;
    }
}

/// Marker for the splatmap material entity spawned alongside a PaintableSurfaceData.
#[derive(Component)]
pub struct SplatmapMaterialMarker;

/// Resource tracking per-entity splatmap handles.
#[derive(Resource, Default)]
pub struct SplatmapHandles {
    pub entries: Vec<SplatmapEntry>,
}

pub struct SplatmapEntry {
    pub entity: Entity,
    pub image_handle: Handle<Image>,
    pub material_handle: Handle<SplatmapMaterial>,
}

// ---------------------------------------------------------------------------
// Hover system — raycast to find UV on paintable meshes
// ---------------------------------------------------------------------------

pub fn surface_paint_hover_system(
    gizmo_state: Res<GizmoState>,
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<ViewportCamera>>,
    paintable_query: Query<Entity, With<PaintableSurfaceData>>,
    mut paint_state: ResMut<SurfacePaintState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mesh_ray_cast: MeshRayCast,
    meshes: Res<Assets<Mesh>>,
    mesh_handles: Query<&Mesh3d>,
) {
    if gizmo_state.tool != EditorTool::SurfacePaint {
        paint_state.hover_position = None;
        paint_state.hover_uv = None;
        paint_state.active_entity = None;
        paint_state.brush_visible = false;
        return;
    }

    let Ok(window) = window_query.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else {
        paint_state.hover_position = None;
        paint_state.hover_uv = None;
        return;
    };

    if !viewport.contains_point(cursor_pos.x, cursor_pos.y) {
        paint_state.hover_position = None;
        paint_state.hover_uv = None;
        return;
    }

    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        paint_state.hover_position = None;
        return;
    };

    let viewport_pos = Vec2::new(
        cursor_pos.x - viewport.position[0],
        cursor_pos.y - viewport.position[1],
    );

    let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_pos) else {
        paint_state.hover_position = None;
        return;
    };

    let hits = mesh_ray_cast.cast_ray(ray, &MeshRayCastSettings::default());

    // Find closest hit on a paintable entity
    let mut best: Option<(Entity, Vec3, Vec2, f32)> = None;

    for (hit_entity, hit) in hits.iter() {
        if !paintable_query.contains(*hit_entity) {
            continue;
        }

        // Compute UV from barycentric coords + triangle index
        if let Some(uv) = compute_hit_uv(*hit_entity, hit.triangle_index, &hit.barycentric_coords, &mesh_handles, &meshes) {
            if best.is_none() || hit.distance < best.as_ref().unwrap().3 {
                best = Some((*hit_entity, hit.point, uv, hit.distance));
            }
        }
    }

    if let Some((entity, point, uv, _)) = best {
        paint_state.hover_position = Some(point);
        paint_state.hover_uv = Some(uv);
        paint_state.active_entity = Some(entity);
        paint_state.brush_visible = true;

        if mouse_button.just_pressed(MouseButton::Left) {
            paint_state.is_painting = true;
            console_info!("Surface Paint", "Started painting on entity {:?} at UV ({:.3}, {:.3})", entity, uv.x, uv.y);
        }
    } else {
        paint_state.hover_position = None;
        paint_state.hover_uv = None;
        paint_state.active_entity = None;
        paint_state.brush_visible = false;
    }

    if mouse_button.just_released(MouseButton::Left) {
        if paint_state.is_painting {
            console_info!("Surface Paint", "Stopped painting");
        }
        paint_state.is_painting = false;
    }
}

/// Extract the UV at a ray hit using barycentric interpolation.
fn compute_hit_uv(
    entity: Entity,
    triangle_index: Option<usize>,
    barycentric: &Vec3,
    mesh_handles: &Query<&Mesh3d>,
    meshes: &Assets<Mesh>,
) -> Option<Vec2> {
    let tri_idx = triangle_index?;
    let mesh_handle = mesh_handles.get(entity).ok()?;
    let mesh = meshes.get(&mesh_handle.0)?;

    // Get UV attribute
    let uvs = mesh.attribute(Mesh::ATTRIBUTE_UV_0)?;
    let uv_values: Vec<[f32; 2]> = match uvs {
        bevy::mesh::VertexAttributeValues::Float32x2(v) => v.clone(),
        _ => return None,
    };

    // Get indices for the triangle
    let indices = mesh.indices()?;
    let idx_base = tri_idx * 3;
    let (i0, i1, i2) = match indices {
        bevy::mesh::Indices::U16(v) => {
            if idx_base + 2 >= v.len() { return None; }
            (v[idx_base] as usize, v[idx_base + 1] as usize, v[idx_base + 2] as usize)
        }
        bevy::mesh::Indices::U32(v) => {
            if idx_base + 2 >= v.len() { return None; }
            (v[idx_base] as usize, v[idx_base + 1] as usize, v[idx_base + 2] as usize)
        }
    };

    if i0 >= uv_values.len() || i1 >= uv_values.len() || i2 >= uv_values.len() {
        return None;
    }

    let uv0 = Vec2::from(uv_values[i0]);
    let uv1 = Vec2::from(uv_values[i1]);
    let uv2 = Vec2::from(uv_values[i2]);

    // Barycentric interpolation: bary = (w, u, v) where w = weight of v0, u = v1, v = v2
    let uv = uv0 * barycentric.x + uv1 * barycentric.y + uv2 * barycentric.z;
    Some(uv)
}

// ---------------------------------------------------------------------------
// Paint system — modify splatmap weights under brush
// ---------------------------------------------------------------------------

pub fn surface_paint_system(
    gizmo_state: Res<GizmoState>,
    paint_state: Res<SurfacePaintState>,
    settings: Res<SurfacePaintSettings>,
    time: Res<Time>,
    mut paintable_query: Query<&mut PaintableSurfaceData>,
    viewport: Res<ViewportState>,
) {
    if gizmo_state.tool != EditorTool::SurfacePaint {
        return;
    }

    if !paint_state.is_painting {
        return;
    }
    if !viewport.hovered {
        console_warn!("Surface Paint", "Paint blocked: viewport not hovered");
        return;
    }

    let Some(uv_center) = paint_state.hover_uv else {
        console_warn!("Surface Paint", "Paint blocked: no hover UV");
        return;
    };
    let Some(entity) = paint_state.active_entity else {
        console_warn!("Surface Paint", "Paint blocked: no active entity");
        return;
    };
    let Ok(mut surface) = paintable_query.get_mut(entity) else {
        console_warn!("Surface Paint", "Paint blocked: entity {:?} not found in query", entity);
        return;
    };

    // (per-frame painting logic — no log spam)

    let resolution = surface.splatmap_resolution as f32;
    let brush_radius_uv = settings.brush_radius;
    let strength = settings.brush_strength * time.delta_secs() * 4.0;
    let active_layer = settings.active_layer.min(3);

    // Iterate over texels within brush radius
    let min_u = ((uv_center.x - brush_radius_uv) * resolution).floor().max(0.0) as u32;
    let max_u = ((uv_center.x + brush_radius_uv) * resolution).ceil().min(resolution - 1.0) as u32;
    let min_v = ((uv_center.y - brush_radius_uv) * resolution).floor().max(0.0) as u32;
    let max_v = ((uv_center.y + brush_radius_uv) * resolution).ceil().min(resolution - 1.0) as u32;

    let res = surface.splatmap_resolution;

    for v in min_v..=max_v {
        for u in min_u..=max_u {
            let texel_uv = Vec2::new(
                (u as f32 + 0.5) / resolution,
                (v as f32 + 0.5) / resolution,
            );
            let delta = texel_uv - uv_center;

            // Distance based on brush shape
            let dist = match settings.brush_shape {
                BrushShape::Circle => delta.length(),
                BrushShape::Square => delta.x.abs().max(delta.y.abs()),
                BrushShape::Diamond => delta.x.abs() + delta.y.abs(),
            };

            if dist > brush_radius_uv {
                continue;
            }

            let t = dist / brush_radius_uv;
            let inner_t = 1.0 - settings.brush_falloff;

            let falloff = if t <= inner_t {
                1.0
            } else {
                let outer_t = (t - inner_t) / (1.0 - inner_t).max(0.001);
                match settings.falloff_type {
                    BrushFalloffType::Smooth => {
                        (1.0 + (outer_t * std::f32::consts::PI).cos()) * 0.5
                    }
                    BrushFalloffType::Linear => 1.0 - outer_t,
                    BrushFalloffType::Spherical => (1.0 - outer_t * outer_t).sqrt().max(0.0),
                    BrushFalloffType::Tip => (1.0 - outer_t).powi(3),
                    BrushFalloffType::Flat => 1.0,
                }
            };

            let effect = strength * falloff;
            let idx = (v * res + u) as usize;
            if idx >= surface.splatmap_weights.len() {
                continue;
            }

            match settings.brush_type {
                PaintBrushType::Paint => {
                    let current = surface.splatmap_weights[idx][active_layer];
                    let new_val = (current + effect).min(1.0);
                    let added = new_val - current;
                    surface.splatmap_weights[idx][active_layer] = new_val;

                    let others_sum: f32 = (0..4)
                        .filter(|&i| i != active_layer)
                        .map(|i| surface.splatmap_weights[idx][i])
                        .sum();
                    if others_sum > 0.001 {
                        for i in 0..4 {
                            if i != active_layer {
                                let ratio = surface.splatmap_weights[idx][i] / others_sum;
                                surface.splatmap_weights[idx][i] -= added * ratio;
                                surface.splatmap_weights[idx][i] = surface.splatmap_weights[idx][i].max(0.0);
                            }
                        }
                    }
                }
                PaintBrushType::Erase => {
                    let current = surface.splatmap_weights[idx][active_layer];
                    let new_val = (current - effect).max(0.0);
                    let removed = current - new_val;
                    surface.splatmap_weights[idx][active_layer] = new_val;

                    let others_count = 3.0;
                    for i in 0..4 {
                        if i != active_layer {
                            surface.splatmap_weights[idx][i] += removed / others_count;
                        }
                    }
                }
                PaintBrushType::Smooth => {
                    // Collect neighbor average first (immutable borrow)
                    let res_i = res as i32;
                    let ui = u as i32;
                    let vi = v as i32;
                    let mut avg = [0.0f32; 4];
                    let mut count = 0.0;
                    for dv in -1i32..=1 {
                        for du in -1i32..=1 {
                            let nu = ui + du;
                            let nv = vi + dv;
                            if nu >= 0 && nu < res_i && nv >= 0 && nv < res_i {
                                let ni = (nv * res_i + nu) as usize;
                                if ni < surface.splatmap_weights.len() {
                                    let nw = surface.splatmap_weights[ni];
                                    for c in 0..4 { avg[c] += nw[c]; }
                                    count += 1.0;
                                }
                            }
                        }
                    }
                    // Apply (mutable borrow)
                    if count > 0.0 {
                        for c in 0..4 {
                            avg[c] /= count;
                            let cur = surface.splatmap_weights[idx][c];
                            surface.splatmap_weights[idx][c] = (cur + (avg[c] - cur) * effect).max(0.0);
                        }
                    }
                }
                PaintBrushType::Fill => {
                    for i in 0..4 {
                        surface.splatmap_weights[idx][i] = if i == active_layer { 1.0 } else { 0.0 };
                    }
                }
            }
        }
    }

    surface.dirty = true;
}

// ---------------------------------------------------------------------------
// Brush preview gizmo
// ---------------------------------------------------------------------------

pub fn surface_paint_gizmo_system(
    gizmo_state: Res<GizmoState>,
    paint_state: Res<SurfacePaintState>,
    settings: Res<SurfacePaintSettings>,
    viewport: Res<ViewportState>,
    mut gizmos: Gizmos,
) {
    if gizmo_state.tool != EditorTool::SurfacePaint {
        return;
    }

    if !viewport.hovered {
        return;
    }

    let Some(hover_pos) = paint_state.hover_position else { return };

    let color = match settings.brush_type {
        PaintBrushType::Paint => Color::srgba(0.2, 0.6, 1.0, 0.8),
        PaintBrushType::Erase => Color::srgba(1.0, 0.3, 0.2, 0.8),
        PaintBrushType::Smooth => Color::srgba(0.2, 0.8, 0.5, 0.8),
        PaintBrushType::Fill => Color::srgba(1.0, 0.8, 0.2, 0.8),
    };

    // Draw a world-space circle at the hover point
    // Convert UV brush radius to approximate world-space (this is an approximation)
    let world_radius = settings.brush_radius * 10.0; // rough scale factor

    let segments = 32;
    let mut points: Vec<Vec3> = Vec::with_capacity(segments);
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin_a, cos_a) = angle.sin_cos();
        points.push(Vec3::new(
            hover_pos.x + cos_a * world_radius,
            hover_pos.y + 0.05,
            hover_pos.z + sin_a * world_radius,
        ));
    }

    for i in 0..segments {
        let next = (i + 1) % segments;
        gizmos.line(points[i], points[next], color);
    }
}

// ---------------------------------------------------------------------------
// Sync system — upload dirty splatmap to GPU Image + update material uniforms
// ---------------------------------------------------------------------------

pub fn surface_paint_sync_system(
    mut images: ResMut<Assets<Image>>,
    mut splatmap_materials: ResMut<Assets<SplatmapMaterial>>,
    splatmap_handles: ResMut<SplatmapHandles>,
    mut paintable_query: Query<(Entity, &mut PaintableSurfaceData)>,
) {
    for (entity, mut surface) in paintable_query.iter_mut() {
        if !surface.dirty {
            continue;
        }

        // Find or skip this entity's handles
        let entry = splatmap_handles.entries.iter().find(|e| e.entity == entity);
        let Some(entry) = entry else { continue };

        // Upload weight data to GPU image
        if let Some(image) = images.get_mut(&entry.image_handle) {
            let res = surface.splatmap_resolution;
            let pixel_count = (res * res) as usize;
            let mut data = vec![0u8; pixel_count * 4];
            for (i, w) in surface.splatmap_weights.iter().enumerate().take(pixel_count) {
                let base = i * 4;
                data[base] = (w[0].clamp(0.0, 1.0) * 255.0) as u8;
                data[base + 1] = (w[1].clamp(0.0, 1.0) * 255.0) as u8;
                data[base + 2] = (w[2].clamp(0.0, 1.0) * 255.0) as u8;
                data[base + 3] = (w[3].clamp(0.0, 1.0) * 255.0) as u8;
            }
            image.data = Some(data);
        }

        // Update material uniforms from layer definitions
        if let Some(mat) = splatmap_materials.get_mut(&entry.material_handle) {
            update_material_from_layers(mat, &surface.layers);
        }

        surface.dirty = false;
    }
}

fn update_material_from_layers(mat: &mut SplatmapMaterial, layers: &[super::data::MaterialLayer]) {
    let get_props = |idx: usize| -> Vec4 {
        if let Some(l) = layers.get(idx) {
            Vec4::new(l.metallic, l.roughness, l.uv_scale.x, l.uv_scale.y)
        } else {
            Vec4::new(0.0, 0.5, 1.0, 1.0)
        }
    };

    // Colors are no longer used (layers are shader-driven), zero them out
    mat.layer_colors_0 = Vec4::ZERO;
    mat.layer_colors_1 = Vec4::ZERO;
    mat.layer_colors_2 = Vec4::ZERO;
    mat.layer_colors_3 = Vec4::ZERO;
    mat.layer_props_0 = get_props(0);
    mat.layer_props_1 = get_props(1);
    mat.layer_props_2 = get_props(2);
    mat.layer_props_3 = get_props(3);
}

// ---------------------------------------------------------------------------
// Helper: create a splatmap Image from PaintableSurfaceData
// ---------------------------------------------------------------------------

pub fn create_splatmap_image(surface: &PaintableSurfaceData) -> Image {
    let res = surface.splatmap_resolution;
    let pixel_count = (res * res) as usize;
    let mut data = vec![0u8; pixel_count * 4];
    for (i, w) in surface.splatmap_weights.iter().enumerate().take(pixel_count) {
        let base = i * 4;
        data[base] = (w[0].clamp(0.0, 1.0) * 255.0) as u8;
        data[base + 1] = (w[1].clamp(0.0, 1.0) * 255.0) as u8;
        data[base + 2] = (w[2].clamp(0.0, 1.0) * 255.0) as u8;
        data[base + 3] = (w[3].clamp(0.0, 1.0) * 255.0) as u8;
    }

    let mut image = Image::new(
        Extent3d {
            width: res,
            height: res,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::MAIN_WORLD
            | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image
}

// ---------------------------------------------------------------------------
// Command processing — apply pending UI commands to PaintableSurfaceData
// ---------------------------------------------------------------------------

pub fn surface_paint_command_system(
    mut paint_state: ResMut<SurfacePaintState>,
    selection: Res<crate::core::SelectionState>,
    mut paintable_query: Query<&mut PaintableSurfaceData>,
) {
    if paint_state.pending_commands.is_empty() {
        return;
    }

    let entity = paint_state.active_entity
        .or(selection.selected_entity);

    let Some(entity) = entity else {
        paint_state.pending_commands.clear();
        return;
    };

    let Ok(mut surface) = paintable_query.get_mut(entity) else {
        paint_state.pending_commands.clear();
        return;
    };

    let commands: Vec<_> = paint_state.pending_commands.drain(..).collect();
    for cmd in commands {
        match cmd {
            super::data::SurfacePaintCommand::AddLayer => {
                if surface.layers.len() < 4 {
                    surface.layers.push(super::data::MaterialLayer::default());
                    surface.dirty = true;
                }
            }
            super::data::SurfacePaintCommand::RemoveLayer(idx) => {
                if idx < surface.layers.len() && surface.layers.len() > 1 {
                    surface.layers.remove(idx);
                    surface.dirty = true;
                }
            }
            super::data::SurfacePaintCommand::AssignMaterial { layer, path } => {
                if let Some(l) = surface.layers.get_mut(layer) {
                    l.texture_path = Some(path);
                    l.cached_shader_source = None; // Force re-load
                    surface.dirty = true;
                    surface.shader_dirty = true;
                }
            }
            super::data::SurfacePaintCommand::ClearMaterial(idx) => {
                if let Some(l) = surface.layers.get_mut(idx) {
                    l.texture_path = Some(super::data::DEFAULT_LAYER_SHADER.to_string());
                    l.cached_shader_source = None;
                    surface.dirty = true;
                    surface.shader_dirty = true;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Layer preview sync — copy layer info from active entity into paint state for UI
// ---------------------------------------------------------------------------

pub fn surface_paint_layer_preview_system(
    mut paint_state: ResMut<SurfacePaintState>,
    selection: Res<crate::core::SelectionState>,
    paintable_query: Query<&PaintableSurfaceData>,
) {
    // Use the active hover entity if available, otherwise fall back to editor selection
    let entity = paint_state.active_entity
        .or(selection.selected_entity);

    let Some(entity) = entity else {
        if !paint_state.layers_preview.is_empty() {
            paint_state.layers_preview.clear();
            paint_state.layer_count = 0;
        }
        return;
    };

    let Ok(surface) = paintable_query.get(entity) else {
        if !paint_state.layers_preview.is_empty() {
            paint_state.layers_preview.clear();
            paint_state.layer_count = 0;
        }
        return;
    };

    // Only update if layer count changed or we need to refresh
    let needs_update = paint_state.layer_count != surface.layers.len()
        || paint_state.layers_preview.iter().zip(surface.layers.iter()).any(|(p, l)| {
            p.name != l.name || p.material_source != l.texture_path
        });

    if needs_update {
        paint_state.layers_preview = surface.layers.iter().map(|l| {
            super::data::LayerPreview {
                name: l.name.clone(),
                material_source: l.texture_path.clone(),
            }
        }).collect();
        paint_state.layer_count = surface.layers.len();
    }
}

//! VR controller interaction with floating panels.
//!
//! Both controllers cast rays against VR panel quads. The closest hit from
//! either hand gets egui hover routing. Both hands can independently grab
//! panels using the trigger button (index finger).
//!
//! Controller poses are in OpenXR tracking space (from `VrControllerState`).
//! Panels are in Bevy world space. We transform rays using `XrTrackingRoot`'s
//! `GlobalTransform` before intersection. Tracking-space rays are stored in
//! `VrPointerHit` for the laser pointers (children of XrTrackingRoot).

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureUsages};

use renzora_xr::reexports::XrTrackingRoot;
use renzora_xr::VrControllerState;
use renzora_xr::input::ControllerHandState;

use crate::{
    VrEditorState, VrPanel, VrPanelBacking, VrPanelInput, VrPointerHit,
    HandGrab, ResizeEdge, PANEL_DEPTH,
};

/// Result of a hand ray cast against panels.
struct HandRayResult {
    tracking_origin: Vec3,
    tracking_dir: Vec3,
    hit: Option<(Entity, f32, Vec2)>,
    hit_world: Vec3,
    edge: Option<ResizeEdge>,
}

/// Threshold in UV space for edge detection (~4cm on a 0.5m panel).
const EDGE_THRESHOLD: f32 = 0.08;

/// Detect which resize edge (if any) a UV coordinate is near.
fn detect_edge(u: f32, v: f32) -> Option<ResizeEdge> {
    let left = u < EDGE_THRESHOLD;
    let right = u > 1.0 - EDGE_THRESHOLD;
    let top = v < EDGE_THRESHOLD;
    let bottom = v > 1.0 - EDGE_THRESHOLD;

    match (left, right, top, bottom) {
        (true, _, true, _) => Some(ResizeEdge::TopLeft),
        (_, true, true, _) => Some(ResizeEdge::TopRight),
        (true, _, _, true) => Some(ResizeEdge::BottomLeft),
        (_, true, _, true) => Some(ResizeEdge::BottomRight),
        (true, _, _, _) => Some(ResizeEdge::Left),
        (_, true, _, _) => Some(ResizeEdge::Right),
        (_, _, true, _) => Some(ResizeEdge::Top),
        (_, _, _, true) => Some(ResizeEdge::Bottom),
        _ => None,
    }
}

/// Cast a ray from one hand and return the closest panel hit with edge info.
fn cast_hand_ray(
    hand: &ControllerHandState,
    root_tf: &GlobalTransform,
    panels: &Query<(Entity, &GlobalTransform, &VrPanel)>,
) -> HandRayResult {
    let tracking_ray_origin = hand.aim_position;
    let tracking_ray_dir = (hand.aim_rotation * Vec3::NEG_Z).normalize();

    if !hand.tracked {
        return HandRayResult {
            tracking_origin: tracking_ray_origin,
            tracking_dir: tracking_ray_dir,
            hit: None,
            hit_world: Vec3::ZERO,
            edge: None,
        };
    }

    let ray_origin = root_tf.transform_point(tracking_ray_origin);
    let ray_dir = (root_tf.affine().matrix3 * tracking_ray_dir).normalize();

    let mut closest: Option<(Entity, f32, Vec2)> = None;
    let mut closest_hit_world = Vec3::ZERO;
    let mut closest_edge: Option<ResizeEdge> = None;

    for (entity, global_tf, panel) in panels.iter() {
        let panel_normal = global_tf.forward().as_vec3();
        let panel_center = global_tf.translation();

        let denom = ray_dir.dot(panel_normal);
        if denom.abs() < 1e-6 {
            continue;
        }

        let t = (panel_center - ray_origin).dot(panel_normal) / denom;
        if t < 0.0 || t > 10.0 {
            continue;
        }

        let hit_world = ray_origin + ray_dir * t;
        let inv_tf = global_tf.affine().inverse();
        let hit_local = inv_tf.transform_point3(hit_world);

        let half_w = panel.width_meters / 2.0;
        let half_h = panel.height_meters / 2.0;

        if hit_local.x.abs() > half_w || hit_local.y.abs() > half_h {
            continue;
        }

        let u = (hit_local.x + half_w) / panel.width_meters;
        let v = 1.0 - (hit_local.y + half_h) / panel.height_meters;

        if closest.is_none() || t < closest.unwrap().1 {
            closest = Some((entity, t, Vec2::new(u, v)));
            closest_hit_world = hit_world;
            closest_edge = detect_edge(u, v);
        }
    }

    HandRayResult {
        tracking_origin: tracking_ray_origin,
        tracking_dir: tracking_ray_dir,
        hit: closest,
        hit_world: closest_hit_world,
        edge: closest_edge,
    }
}

/// System: cast rays from both controllers, write pointer state to `VrPanelInput`
/// resource (consumed by `render_vr_panel_exclusive` for proper egui event timing),
/// and update backing material colors for edge hover feedback.
pub fn vr_panel_interaction(
    controllers: Option<Res<VrControllerState>>,
    panels: Query<(Entity, &GlobalTransform, &VrPanel)>,
    mut pointer_hit: ResMut<VrPointerHit>,
    mut panel_input: ResMut<VrPanelInput>,
    tracking_root: Query<&GlobalTransform, With<XrTrackingRoot>>,
    vr_state: Res<VrEditorState>,
    mut prev_trigger: Local<bool>,
    // Backing material color change
    children_query: Query<&Children>,
    backings: Query<Entity, With<VrPanelBacking>>,
    backing_mats: Query<&MeshMaterial3d<StandardMaterial>>,
    mut mat_assets: ResMut<Assets<StandardMaterial>>,
) {
    let Some(controllers) = controllers else { return };
    let root_tf = tracking_root.single().copied().unwrap_or(GlobalTransform::IDENTITY);

    let l_result = cast_hand_ray(&controllers.left, &root_tf, &panels);
    let r_result = cast_hand_ray(&controllers.right, &root_tf, &panels);

    // Store per-hand ray data for laser pointers
    pointer_hit.left.ray_origin = l_result.tracking_origin;
    pointer_hit.left.ray_direction = l_result.tracking_dir;
    pointer_hit.left.hit_distance = l_result.hit.as_ref().map(|(_, t, _)| *t);
    pointer_hit.left.hit_entity = l_result.hit.as_ref().map(|(e, _, _)| *e);
    pointer_hit.left.hovered_edge = l_result.edge;

    pointer_hit.right.ray_origin = r_result.tracking_origin;
    pointer_hit.right.ray_direction = r_result.tracking_dir;
    pointer_hit.right.hit_distance = r_result.hit.as_ref().map(|(_, t, _)| *t);
    pointer_hit.right.hit_entity = r_result.hit.as_ref().map(|(e, _, _)| *e);
    pointer_hit.right.hovered_edge = r_result.edge;

    // Pick the closest overall hit from either hand.
    // Button mapping is fixed: right trigger = click, left trigger = scroll.
    let best_hit: Option<(Entity, f32, Vec2)> =
        match (&l_result.hit, &r_result.hit) {
            (Some(l), Some(r)) => {
                if l.1 <= r.1 { l_result.hit.clone() } else { r_result.hit.clone() }
            }
            (Some(_), None) => l_result.hit.clone(),
            (None, Some(_)) => r_result.hit.clone(),
            (None, None) => None,
        };

    let any_resize = vr_state.left_resize.active || vr_state.right_resize.active;

    // Determine which panel entity has edge hover (for backing color)
    let edge_hovered_entity: Option<Entity> = if !any_resize {
        pointer_hit.left.hovered_edge.and_then(|_| pointer_hit.left.hit_entity)
            .or_else(|| pointer_hit.right.hovered_edge.and_then(|_| pointer_hit.right.hit_entity))
    } else {
        // During active resize, highlight the resize target
        vr_state.left_resize.panel_entity
            .filter(|_| vr_state.left_resize.active)
            .or_else(|| vr_state.right_resize.panel_entity.filter(|_| vr_state.right_resize.active))
    };

    // Update backing material colors: yellow when edge-hovered, dark otherwise
    let normal_color = Color::srgb(0.12, 0.12, 0.14);
    let hover_color = Color::srgb(0.8, 0.7, 0.0);
    for (panel_entity, _, _) in panels.iter() {
        let is_hovered = edge_hovered_entity == Some(panel_entity);
        let target_color = if is_hovered { hover_color } else { normal_color };
        if let Ok(kids) = children_query.get(panel_entity) {
            for child in kids.iter() {
                if backings.get(child).is_ok() {
                    if let Ok(mat_handle) = backing_mats.get(child) {
                        if let Some(mat) = mat_assets.get_mut(&mat_handle.0) {
                            mat.base_color = target_color;
                        }
                    }
                }
            }
        }
    }

    // Write to VrPanelInput for consumption by inject_vr_panel_egui_input.
    // Right trigger = click/drag, left trigger = scroll (always, regardless of pointing hand).
    if let Some((hit_entity, _t, uv)) = best_hit {
        let Ok(panel) = panels.get(hit_entity).map(|(_, _, p)| p) else {
            panel_input.hovered_context = None;
            return;
        };

        let px_w = panel.width_meters * 512.0;
        let px_h = panel.height_meters * 512.0;

        let click_now = controllers.right.trigger_pressed;
        let click_prev = *prev_trigger;

        panel_input.prev_pointer_pos = panel_input.pointer_pos;
        panel_input.hovered_context = Some(panel.context_entity);
        panel_input.pointer_pos = Vec2::new(uv.x * px_w, uv.y * px_h);

        // Right trigger = click/drag
        panel_input.click_pressed = click_now;
        panel_input.click_just_pressed = click_now && !click_prev;
        panel_input.click_just_released = !click_now && click_prev;

        // Left trigger = scroll (hold + move pointer to drag-scroll)
        panel_input.scroll_active = controllers.left.trigger_pressed;
        panel_input.scroll_trigger_value = controllers.left.trigger;

        *prev_trigger = click_now;
    } else {
        let click_prev = *prev_trigger;
        panel_input.prev_pointer_pos = panel_input.pointer_pos;
        panel_input.hovered_context = None;
        panel_input.click_pressed = false;
        panel_input.click_just_pressed = false;
        panel_input.click_just_released = click_prev;
        panel_input.scroll_active = false;
        panel_input.scroll_trigger_value = 0.0;
        *prev_trigger = false;
    }
}

/// Helper: process grab for a single hand.
/// Grab uses the **grip** (squeeze) button, leaving trigger free for panel content.
fn process_hand_grab(
    hand: &ControllerHandState,
    grab: &mut HandGrab,
    panels: &Query<(Entity, &mut Transform, &VrPanel)>,
    root_tf: &GlobalTransform,
    resize_active: bool,
) -> Option<(Entity, Vec3, Quat)> {
    if !hand.tracked {
        return None;
    }

    let grip_position_ws = root_tf.transform_point(hand.grip_position);
    let grip_rotation_ws = root_tf.to_isometry().rotation * hand.grip_rotation;
    let aim_position_ws = root_tf.transform_point(hand.aim_position);
    let aim_rotation_ws = root_tf.to_isometry().rotation * hand.aim_rotation;
    let aim_dir_ws = (aim_rotation_ws * Vec3::NEG_Z).normalize();

    if hand.grip_pressed {
        if grab.grabbed_panel.is_none() && !resize_active {
            let mut best: Option<(Entity, f32)> = None;
            for (entity, tf, panel) in panels.iter() {
                let panel_normal = tf.forward().as_vec3();
                let denom = aim_dir_ws.dot(panel_normal);
                if denom.abs() < 1e-6 { continue; }
                let t = (tf.translation - aim_position_ws).dot(panel_normal) / denom;
                if t < 0.0 || t > 5.0 { continue; }
                let hit = aim_position_ws + aim_dir_ws * t;
                let half_w = panel.width_meters / 2.0;
                let half_h = panel.height_meters / 2.0;
                let inv = tf.compute_affine().inverse();
                let local = inv.transform_point3(hit);
                if local.x.abs() <= half_w && local.y.abs() <= half_h {
                    if best.is_none() || t < best.unwrap().1 {
                        best = Some((entity, t));
                    }
                }
            }

            if let Some((entity, _)) = best {
                if let Ok((_, tf, _)) = panels.get(entity) {
                    let controller_tf = Transform {
                        translation: grip_position_ws,
                        rotation: grip_rotation_ws,
                        ..default()
                    };
                    let inv_controller = controller_tf.compute_affine().inverse();
                    let relative = Transform::from_matrix(
                        Mat4::from(inv_controller) * Mat4::from(tf.compute_affine()),
                    );
                    grab.grab_offset = relative;
                    grab.grabbed_panel = Some(entity);
                }
            }
        }

        if grab.grabbed_panel.is_some() {
            return Some((grab.grabbed_panel.unwrap(), grip_position_ws, grip_rotation_ws));
        }
    } else {
        grab.grabbed_panel = None;
    }

    None
}

/// System: trigger button grabs/releases panels, both hands independently.
pub fn vr_panel_grab(
    controllers: Option<Res<VrControllerState>>,
    mut vr_state: ResMut<VrEditorState>,
    mut panels: Query<(Entity, &mut Transform, &VrPanel)>,
    tracking_root: Query<&GlobalTransform, With<XrTrackingRoot>>,
) {
    let Some(controllers) = controllers else { return };
    let root_tf = tracking_root.single().copied().unwrap_or(GlobalTransform::IDENTITY);

    let mut left_grab = vr_state.left.clone();
    let mut right_grab = vr_state.right.clone();
    let left_resizing = vr_state.left_resize.active;
    let right_resizing = vr_state.right_resize.active;

    if let Some((entity, grip_pos, grip_rot)) =
        process_hand_grab(&controllers.left, &mut left_grab, &panels, &root_tf, left_resizing)
    {
        if let Ok((_, mut tf, _)) = panels.get_mut(entity) {
            let controller_mat = Mat4::from_rotation_translation(grip_rot, grip_pos);
            let offset_mat = Mat4::from(left_grab.grab_offset.compute_affine());
            let new_tf = Transform::from_matrix(controller_mat * offset_mat);
            tf.translation = new_tf.translation;
            tf.rotation = new_tf.rotation;
        }
    }

    if let Some((entity, grip_pos, grip_rot)) =
        process_hand_grab(&controllers.right, &mut right_grab, &panels, &root_tf, right_resizing)
    {
        if let Ok((_, mut tf, _)) = panels.get_mut(entity) {
            let controller_mat = Mat4::from_rotation_translation(grip_rot, grip_pos);
            let offset_mat = Mat4::from(right_grab.grab_offset.compute_affine());
            let new_tf = Transform::from_matrix(controller_mat * offset_mat);
            tf.translation = new_tf.translation;
            tf.rotation = new_tf.rotation;
        }
    }

    vr_state.left = left_grab;
    vr_state.right = right_grab;
}

/// Compute new panel dimensions from resize delta.
fn compute_resize_dims(
    edge: ResizeEdge,
    start_width: f32,
    start_height: f32,
    delta_local: Vec3,
) -> (f32, f32) {
    const MIN_SIZE: f32 = 0.2;
    const MAX_SIZE: f32 = 1.5;

    let mut new_width = start_width;
    let mut new_height = start_height;

    match edge {
        ResizeEdge::Right | ResizeEdge::TopRight | ResizeEdge::BottomRight => {
            new_width = (start_width + delta_local.x).clamp(MIN_SIZE, MAX_SIZE);
        }
        ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft => {
            new_width = (start_width - delta_local.x).clamp(MIN_SIZE, MAX_SIZE);
        }
        _ => {}
    }
    match edge {
        ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight => {
            new_height = (start_height + delta_local.y).clamp(MIN_SIZE, MAX_SIZE);
        }
        ResizeEdge::Bottom | ResizeEdge::BottomLeft | ResizeEdge::BottomRight => {
            new_height = (start_height - delta_local.y).clamp(MIN_SIZE, MAX_SIZE);
        }
        _ => {}
    }

    (new_width, new_height)
}

/// Project the current aim ray onto a panel's plane and return the local-space
/// delta from the resize start point.
fn resize_delta_local(
    hand: &ControllerHandState,
    root_tf: &GlobalTransform,
    global_tf: &GlobalTransform,
    start_hit_world: Vec3,
) -> Option<Vec3> {
    let aim_pos = root_tf.transform_point(hand.aim_position);
    let aim_dir = (root_tf.affine().matrix3 * (hand.aim_rotation * Vec3::NEG_Z)).normalize();

    let panel_normal = global_tf.forward().as_vec3();
    let denom = aim_dir.dot(panel_normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (global_tf.translation() - aim_pos).dot(panel_normal) / denom;
    let current_hit = aim_pos + aim_dir * t;

    let inv = global_tf.affine().inverse();
    let start_local = inv.transform_point3(start_hit_world);
    let current_local = inv.transform_point3(current_hit);
    Some(current_local - start_local)
}

/// System: resize panels by dragging edges. Runs after `vr_panel_interaction`.
///
/// Creates a new render target image each frame the dimensions change,
/// updates material texture + camera render target to match.
pub fn vr_panel_resize(
    controllers: Option<Res<VrControllerState>>,
    mut vr_state: ResMut<VrEditorState>,
    pointer_hit: Res<VrPointerHit>,
    mut panels: Query<(Entity, &GlobalTransform, &mut VrPanel)>,
    tracking_root: Query<&GlobalTransform, With<XrTrackingRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut mesh_handles: Query<&mut Mesh3d>,
    children_query: Query<&Children>,
    backings: Query<Entity, With<VrPanelBacking>>,
    mut materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut mat_assets: ResMut<Assets<StandardMaterial>>,
    mut render_targets: Query<&mut bevy::camera::RenderTarget>,
) {
    let Some(controllers) = controllers else { return };
    let root_tf = tracking_root.single().copied().unwrap_or(GlobalTransform::IDENTITY);

    let hands: [(
        &renzora_xr::input::ControllerHandState,
        &crate::HandRay,
        bool,
    ); 2] = [
        (&controllers.left, &pointer_hit.left, true),
        (&controllers.right, &pointer_hit.right, false),
    ];

    for (hand, hand_ray, is_left) in hands {
        let resize = if is_left {
            &mut vr_state.left_resize
        } else {
            &mut vr_state.right_resize
        };

        if !hand.tracked {
            resize.active = false;
            continue;
        }

        if hand.button_a {
            if !resize.active {
                // Start resize if hovering an edge
                if let Some(edge) = hand_ray.hovered_edge {
                    if let Some(entity) = hand_ray.hit_entity {
                        if let Ok((_, _, panel)) = panels.get(entity) {
                            let aim_pos = root_tf.transform_point(hand.aim_position);
                            let aim_dir = (root_tf.affine().matrix3
                                * (hand.aim_rotation * Vec3::NEG_Z))
                            .normalize();
                            let panel_normal = panels.get(entity).unwrap().1.forward().as_vec3();
                            let denom = aim_dir.dot(panel_normal);
                            let t = if denom.abs() > 1e-6 {
                                (panels.get(entity).unwrap().1.translation() - aim_pos).dot(panel_normal) / denom
                            } else {
                                0.0
                            };

                            resize.active = true;
                            resize.edge = Some(edge);
                            resize.panel_entity = Some(entity);
                            resize.start_width = panel.width_meters;
                            resize.start_height = panel.height_meters;
                            resize.start_hit_world = aim_pos + aim_dir * t;
                        }
                    }
                }
            } else {
                // Continue resizing
                let Some(panel_entity) = resize.panel_entity else {
                    resize.active = false;
                    continue;
                };
                let Some(edge) = resize.edge else {
                    resize.active = false;
                    continue;
                };

                let Ok((_, global_tf, _)) = panels.get(panel_entity) else {
                    resize.active = false;
                    continue;
                };
                let global_tf = *global_tf;

                let Some(delta) = resize_delta_local(hand, &root_tf, &global_tf, resize.start_hit_world) else {
                    continue;
                };

                let (new_w, new_h) = compute_resize_dims(edge, resize.start_width, resize.start_height, delta);

                if let Ok((_, _, mut panel)) = panels.get_mut(panel_entity) {
                    if (panel.width_meters - new_w).abs() > 0.001
                        || (panel.height_meters - new_h).abs() > 0.001
                    {
                        panel.width_meters = new_w;
                        panel.height_meters = new_h;

                        // Update front face mesh
                        let new_mesh = meshes.add(
                            Plane3d::new(Vec3::Z, Vec2::new(new_w / 2.0, new_h / 2.0)).mesh(),
                        );
                        if let Ok(mut m) = mesh_handles.get_mut(panel_entity) {
                            m.0 = new_mesh;
                        }

                        // Create new render target image at correct resolution
                        let px_w = (new_w * 512.0) as u32;
                        let px_h = (new_h * 512.0) as u32;
                        let new_image = images.add({
                            let size = Extent3d {
                                width: px_w,
                                height: px_h,
                                depth_or_array_layers: 1,
                            };
                            let mut img = Image {
                                data: Some(vec![0; (px_w * px_h * 4) as usize]),
                                ..default()
                            };
                            img.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
                            img.texture_descriptor.size = size;
                            img
                        });

                        // Update material texture
                        if let Ok(mat_handle) = materials.get(panel_entity) {
                            if let Some(mat) = mat_assets.get_mut(&mat_handle.0) {
                                mat.base_color_texture = Some(new_image.clone());
                            }
                        }

                        // Update camera render target
                        if let Ok(mut rt) = render_targets.get_mut(panel.context_entity) {
                            *rt = bevy::camera::RenderTarget::Image(new_image.clone().into());
                        }

                        panel.image_handle = new_image;

                        // Resize backing cuboid
                        if let Ok(kids) = children_query.get(panel_entity) {
                            for child in kids.iter() {
                                if backings.get(child).is_ok() {
                                    let backing_mesh = meshes.add(Cuboid::new(new_w, new_h, PANEL_DEPTH));
                                    if let Ok(mut m) = mesh_handles.get_mut(child) {
                                        m.0 = backing_mesh;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if resize.active {
            resize.active = false;
        }
    }
}

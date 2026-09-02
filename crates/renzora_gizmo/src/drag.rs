//! Dragging a handle.
//!
//! Translate is *cursor-locked*: the world point grabbed at press time is
//! re-projected onto the axis every frame and the object moves by the absolute
//! difference, so the handle can never drift off the pointer. Rotate and scale
//! accumulate a screen-space delta instead, because there is no equivalent
//! point to pin.
//!
//! Everything is written back through the entity's parent frame (captured once
//! at drag start, since the parent does not move during a gesture), so a nested
//! object moves along the gizmo's axis rather than a parent-rotated one.

use bevy::ecs::system::SystemParam;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use renzora::core::resize::{resize_in_flight, ResizeBusy};
use renzora::core::viewport_types::{SnapSettings, ViewportSettings, ViewportState};
use renzora_editor_framework::{EditorCamera, EditorLocked, EditorSelection};

use crate::pivot::compute_gizmo_pivot;
use crate::ray::{translate_cursor_point, viewport_cursor_ray};
use crate::transform_space;
use crate::types::{
    gizmo_basis, GizmoAxis, GizmoMesh, GizmoMode, GizmoRoot, GizmoSpace, GizmoState,
};

fn acquire_drag_cursor(cursor_q: &mut Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursor_q.single_mut() {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
}

fn release_drag_cursor(cursor_q: &mut Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursor_q.single_mut() {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    }
}

/// Geometry queries shared by the drag system, bundled so `gizmo_drag` stays
/// under Bevy's 16-parameter system limit.
#[derive(SystemParam)]
pub(crate) struct DragGeom<'w, 's> {
    global: Query<'w, 's, &'static GlobalTransform, Without<EditorCamera>>,
    aabb: Query<'w, 's, &'static bevy::camera::primitives::Aabb>,
    pivot_aabbs: Query<
        'w,
        's,
        (
            Option<&'static bevy::camera::primitives::Aabb>,
            &'static GlobalTransform,
        ),
        With<Mesh3d>,
    >,
    children: Query<'w, 's, &'static Children>,
}

pub(crate) fn gizmo_drag(
    mut gizmo_state: ResMut<GizmoState>,
    mode: Res<GizmoMode>,
    space: Res<GizmoSpace>,
    selection: Res<EditorSelection>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    viewport: Option<Res<ViewportState>>,
    viewport_settings: Option<Res<ViewportSettings>>,
    camera_q: Query<(&GlobalTransform, &Projection), With<EditorCamera>>,
    mut transform_q: Query<
        &mut Transform,
        (
            Without<EditorCamera>,
            Without<EditorLocked>,
            Without<GizmoRoot>,
            Without<GizmoMesh>,
        ),
    >,
    geom: DragGeom,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut cursor_options: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut commands: Commands,
    resizing: Option<Res<ResizeBusy>>,
) {
    let pivot_bottom = viewport_settings
        .as_ref()
        .map(|s| s.gizmo_pivot_bottom)
        .unwrap_or(true);
    let snap: SnapSettings = viewport_settings
        .as_deref()
        .map(|s| s.snap)
        .unwrap_or_default();
    if matches!(*mode, GizmoMode::Select | GizmoMode::None) {
        mouse_motion.clear();
        return;
    }
    if collider_edit.map(|c| c.active).unwrap_or(false) {
        if gizmo_state.active_axis.is_some() {
            release_drag_cursor(&mut cursor_options);
        }
        gizmo_state.active_axis = None;
        gizmo_state.drag_starts.clear();
        mouse_motion.clear();
        return;
    }

    let selected_entities = selection.get_all();
    if selected_entities.is_empty() {
        if gizmo_state.active_axis.is_some() {
            release_drag_cursor(&mut cursor_options);
        }
        gizmo_state.active_axis = None;
        gizmo_state.drag_starts.clear();
        mouse_motion.clear();
        return;
    }

    if mouse_button.pressed(MouseButton::Right) || mouse_button.pressed(MouseButton::Middle) {
        mouse_motion.clear();
        return;
    }

    // Start drag. Never off the press that started a resize: the handle
    // overhangs the viewport, so a divider or the bottom panel's grip can sit
    // over a gizmo axis and the press would move the selection instead of the
    // seam (the same overhang that armed the selection box in
    // `entity_pick_system`).
    if mouse_button.just_pressed(MouseButton::Left)
        && gizmo_state.active_axis.is_none()
        && !resize_in_flight(&resizing)
    {
        if let Some(axis) = gizmo_state.hovered_axis {
            let mut starts = Vec::new();
            let mut parents = Vec::new();
            for &entity in &selected_entities {
                if let Ok(t) = transform_q.get(entity) {
                    starts.push((entity, t.translation, t.rotation, t.scale));
                    // Parent frame = world * local⁻¹, captured now (the parent is
                    // stationary for the gesture). Identity when unparented.
                    let parent = geom.global
                        .get(entity)
                        .map(|gt| transform_space::parent_affine(gt, t))
                        .unwrap_or(bevy::math::Affine3A::IDENTITY);
                    parents.push(parent);
                }
            }
            // Capture the handle orientation and the world pivot now, so both
            // stay fixed for the whole gesture (in Local space the live basis
            // would otherwise drift as the object rotates).
            let sel_world_rot = selection
                .get()
                .and_then(|e| geom.global.get(e).ok())
                .map(|gt| gt.rotation())
                .unwrap_or(Quat::IDENTITY);
            let mut pivot_sum = Vec3::ZERO;
            let mut pivot_n = 0u32;
            for &e in &selected_entities {
                if let Ok(gt) = geom.global.get(e) {
                    pivot_sum += compute_gizmo_pivot(
                        e,
                        &geom.pivot_aabbs,
                        &geom.children,
                        gt,
                        pivot_bottom,
                    );
                    pivot_n += 1;
                }
            }
            gizmo_state.drag_basis = gizmo_basis(*space, *mode, sel_world_rot);
            gizmo_state.drag_pivot = if pivot_n > 0 {
                pivot_sum / pivot_n as f32
            } else {
                Vec3::ZERO
            };
            // Reference point under the cursor on the dragged axis/plane, so
            // translate can keep it pinned to the pointer (cursor-locked drag).
            let pivot0 = gizmo_state.drag_pivot;
            let basis = gizmo_state.drag_basis;
            gizmo_state.drag_grab = camera_q
                .single()
                .ok()
                .zip(window_q.single().ok())
                .and_then(|((cam_gt, projection), window)| {
                    let vp = viewport.as_ref()?;
                    let ray = viewport_cursor_ray(window, vp, cam_gt, projection)?;
                    translate_cursor_point(&ray, pivot0, basis, axis)
                })
                .unwrap_or(pivot0);
            gizmo_state.active_axis = Some(axis);
            gizmo_state.drag_starts = starts;
            gizmo_state.drag_parents = parents;
            gizmo_state.drag_offset = Vec3::ZERO;
            gizmo_state.drag_angle = 0.0;
            gizmo_state.drag_angle_snapped = 0.0;
            gizmo_state.drag_scale_factor = 0.0;
            // Leave the cursor visible and free while dragging — the drag tracks
            // raw mouse motion either way, and locking it in place feels frozen.
            mouse_motion.clear();
            return;
        }
    }

    // End drag
    if mouse_button.just_released(MouseButton::Left) && gizmo_state.active_axis.is_some() {
        let mut records: Vec<(Entity, Transform, Transform)> = Vec::new();
        for (entity, old_t, old_r, old_s) in &gizmo_state.drag_starts {
            let Ok(t) = transform_q.get(*entity) else {
                continue;
            };
            let old = Transform {
                translation: *old_t,
                rotation: *old_r,
                scale: *old_s,
            };
            let new = *t;
            if old.translation == new.translation
                && old.rotation == new.rotation
                && old.scale == new.scale
            {
                continue;
            }
            records.push((*entity, old, new));
        }
        if !records.is_empty() {
            commands.queue(move |world: &mut World| {
                for (entity, old, new) in records {
                    renzora_undo::record(
                        world,
                        renzora_undo::UndoContext::Scene,
                        Box::new(renzora_undo::TransformCmd { entity, old, new }),
                    );
                }
            });
        }
        gizmo_state.active_axis = None;
        gizmo_state.drag_starts.clear();
        release_drag_cursor(&mut cursor_options);
        mouse_motion.clear();
        return;
    }

    let Some(axis) = gizmo_state.active_axis else {
        mouse_motion.clear();
        return;
    };

    if !mouse_button.pressed(MouseButton::Left) {
        gizmo_state.active_axis = None;
        gizmo_state.drag_starts.clear();
        release_drag_cursor(&mut cursor_options);
        mouse_motion.clear();
        return;
    }

    let Ok((cam_gt, projection)) = camera_q.single() else {
        mouse_motion.clear();
        return;
    };
    let Some(viewport) = viewport.as_ref() else {
        mouse_motion.clear();
        return;
    };

    let mut total_delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        total_delta += ev.delta;
    }
    if total_delta.length_squared() < 1e-6 {
        return;
    }

    // Drag-start positions are local-space (so writes go back into local
    // Transform). For camera-distance scaling we need the world-space pivot,
    // so average GlobalTransform translations of the selected entities.
    let center = if gizmo_state.drag_starts.is_empty() {
        Vec3::ZERO
    } else {
        let sum: Vec3 = gizmo_state.drag_starts.iter().map(|(_, t, _, _)| *t).sum();
        sum / gizmo_state.drag_starts.len() as f32
    };
    let world_center = if selected_entities.is_empty() {
        center
    } else {
        let mut sum = Vec3::ZERO;
        let mut n = 0u32;
        for &e in &selected_entities {
            if let Ok(gt) = geom.global.get(e) {
                sum += compute_gizmo_pivot(
                    e,
                    &geom.pivot_aabbs,
                    &geom.children,
                    gt,
                    pivot_bottom,
                );
                n += 1;
            }
        }
        if n > 0 {
            sum / n as f32
        } else {
            center
        }
    };
    let distance = (cam_gt.translation() - world_center).length();

    match *mode {
        GizmoMode::Select | GizmoMode::None => unreachable!(),
        GizmoMode::Translate => {
            // Cursor-locked: pin the grabbed point under the pointer. Project the
            // cursor ray onto the active axis/plane and move by how far that
            // point has travelled from the grab reference captured at drag start.
            // Absolute (not accumulated), so the handle never drifts off-cursor.
            let Ok(window) = window_q.single() else {
                return;
            };
            let Some(ray) = viewport_cursor_ray(window, viewport, cam_gt, projection) else {
                return;
            };
            let Some(cur) =
                translate_cursor_point(&ray, gizmo_state.drag_pivot, gizmo_state.drag_basis, axis)
            else {
                return;
            };
            let total_offset = cur - gizmo_state.drag_grab;
            for (i, &entity) in selected_entities.iter().enumerate() {
                if let Ok(mut t) = transform_q.get_mut(entity) {
                    let (start_t, start_r, start_s) = gizmo_state
                        .drag_starts
                        .get(i)
                        .map(|(_, p, r, s)| (*p, *r, *s))
                        .unwrap_or((t.translation, t.rotation, t.scale));
                    let parent = gizmo_state
                        .drag_parents
                        .get(i)
                        .copied()
                        .unwrap_or(bevy::math::Affine3A::IDENTITY);
                    // Convert the world-space drag into the entity's parent frame
                    // so it moves along the gizmo's axis, not a parent-rotated one.
                    let mut new_pos =
                        transform_space::world_translation(start_t, total_offset, &parent);
                    if snap.translate_enabled && snap.translate_snap > 0.0 {
                        let step = snap.translate_snap;
                        // For edge-snap, snap the world-space AABB min corner
                        // (computed from the drag-start transform since rot/scale
                        // don't change during translate) to the grid, then derive
                        // the required pivot position.
                        let min_offset = if snap.translate_edge_snap {
                            geom.aabb.get(entity).ok().map(|aabb| {
                                world_aabb_min(aabb, start_t, start_r, start_s) - start_t
                            })
                        } else {
                            None
                        };
                        if let Some(off) = min_offset {
                            let target = new_pos + off;
                            let snapped = Vec3::new(
                                (target.x / step).round() * step,
                                (target.y / step).round() * step,
                                (target.z / step).round() * step,
                            );
                            new_pos = snapped - off;
                        } else {
                            new_pos = Vec3::new(
                                (new_pos.x / step).round() * step,
                                (new_pos.y / step).round() * step,
                                (new_pos.z / step).round() * step,
                            );
                        }
                    }
                    t.translation = new_pos;
                }
            }
        }
        GizmoMode::Rotate => {
            // Rotation axis in world space (world or the object's own axis).
            let world_axis = gizmo_state.drag_basis * axis.direction();
            let delta_angle = screen_delta_to_angle(total_delta, world_axis, cam_gt);
            gizmo_state.drag_angle += delta_angle;

            // Apply the delta needed to reach the snapped value from starts.
            let effective_angle = snap_rotation(gizmo_state.drag_angle, &snap);
            gizmo_state.drag_angle_snapped = effective_angle;
            let world_rot = Quat::from_axis_angle(world_axis, effective_angle);
            let pivot = gizmo_state.drag_pivot;
            for (i, &entity) in selected_entities.iter().enumerate() {
                if let Ok(mut t) = transform_q.get_mut(entity) {
                    let (start_t, start_r, start_s) = gizmo_state
                        .drag_starts
                        .get(i)
                        .map(|(_, p, r, s)| (*p, *r, *s))
                        .unwrap_or((t.translation, t.rotation, t.scale));
                    let parent = gizmo_state
                        .drag_parents
                        .get(i)
                        .copied()
                        .unwrap_or(bevy::math::Affine3A::IDENTITY);
                    // Rotate about the shared world pivot so single and group
                    // rotations both pivot in place.
                    transform_space::pivot_rotation(
                        &mut t, start_t, start_r, start_s, world_rot, pivot, &parent,
                    );
                }
            }
        }
        GizmoMode::Scale => {
            // Scale is always along the object's own axes; the handle's world
            // direction is what the screen delta projects onto.
            let handle_dir = gizmo_state.drag_basis * axis.direction();
            let delta_scale = screen_delta_to_scale(total_delta, handle_dir, cam_gt);
            gizmo_state.drag_scale_factor += delta_scale;
            let snap_step = if snap.scale_enabled && snap.scale_snap > 0.0 {
                Some(snap.scale_snap)
            } else {
                None
            };
            let apply = |v: f32, step: Option<f32>| -> f32 {
                let v = v.max(0.01);
                match step {
                    Some(s) => ((v / s).round() * s).max(s.min(0.01)),
                    None => v,
                }
            };
            let f = gizmo_state.drag_scale_factor;
            let pivot = gizmo_state.drag_pivot;
            for (i, &entity) in selected_entities.iter().enumerate() {
                if let Ok(mut t) = transform_q.get_mut(entity) {
                    let (start_t, start_r, start_scale) = gizmo_state
                        .drag_starts
                        .get(i)
                        .map(|(_, p, r, s)| (*p, *r, *s))
                        .unwrap_or((t.translation, t.rotation, t.scale));
                    let parent = gizmo_state
                        .drag_parents
                        .get(i)
                        .copied()
                        .unwrap_or(bevy::math::Affine3A::IDENTITY);
                    let mut new_scale = start_scale;
                    match axis {
                        GizmoAxis::X => new_scale.x = apply(start_scale.x + f, snap_step),
                        GizmoAxis::Y => new_scale.y = apply(start_scale.y + f, snap_step),
                        GizmoAxis::Z => new_scale.z = apply(start_scale.z + f, snap_step),
                        _ => {}
                    }
                    // Scale about the world pivot so the object stays in place
                    // (translation is compensated through the parent frame).
                    transform_space::pivot_scale(
                        &mut t, start_t, start_r, start_scale, new_scale, pivot, &parent,
                    );
                }
            }
        }
    }
}

/// Minimum-corner of a local-space AABB transformed by (translation, rotation,
/// scale) into world space. Used by the translate/scale gizmo for edge-snap
/// and bottom-anchor behaviors.
fn world_aabb_min(
    aabb: &bevy::camera::primitives::Aabb,
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
) -> Vec3 {
    let c = Vec3::from(aabb.center);
    let h = Vec3::from(aabb.half_extents);
    let mut min = Vec3::splat(f32::INFINITY);
    for dx in [-1.0_f32, 1.0] {
        for dy in [-1.0_f32, 1.0] {
            for dz in [-1.0_f32, 1.0] {
                let local = c + Vec3::new(dx * h.x, dy * h.y, dz * h.z);
                let world = translation + rotation * (local * scale);
                min = min.min(world);
            }
        }
    }
    min
}

/// Format the rotate HUD's degrees readout.
///
/// Carries no `°` suffix on purpose. Bevy's gizmo stroke font only holds
/// glyphs for printable ASCII (32-126) and U+00B0 falls outside that range, so
/// the lookup misses and the character draws as a blank advance. It was never
/// visible at any angle — all the suffix bought was a trailing gap.
///
/// Values within ±0.05° of zero collapse to one `0.0` so the label doesn't
/// flicker between `0.0` and `-0.0` as the rotation crosses zero. Which sign
/// you land on depends on the direction of travel, so dragging back and forth
/// over zero made it alternate.
pub(crate) fn angle_label_text(radians: f32) -> String {
    let degrees = radians.to_degrees();
    let degrees = if degrees.abs() < 0.05 { 0.0 } else { degrees };
    format!("{degrees:.1}")
}

/// Round `radians` to the rotate-snap step, or pass it through when the snap
/// pill is off.
///
/// Three places have to agree on this exactly: the ring drag below, the modal
/// `R` shortcut's `apply_rotate`, and the modal overlay that draws the readout.
/// While the overlay was the odd one out, the HUD counted through every
/// intermediate degree for an object that was moving in steps.
pub(crate) fn snap_rotation(radians: f32, snap: &SnapSettings) -> f32 {
    if snap.rotate_enabled && snap.rotate_snap > 0.0 {
        let step = snap.rotate_snap.to_radians();
        (radians / step).round() * step
    } else {
        radians
    }
}

fn screen_delta_to_angle(mouse_delta: Vec2, axis_world: Vec3, cam: &GlobalTransform) -> f32 {
    let cam_fwd = cam.forward().as_vec3();
    let dot = axis_world.dot(cam_fwd).abs();
    let sens = 0.005;
    if dot > 0.7 {
        (mouse_delta.x - mouse_delta.y) * sens
    } else {
        let cr = cam.right().as_vec3();
        let cu = cam.up().as_vec3();
        let sa = Vec2::new(axis_world.dot(cr), -axis_world.dot(cu));
        let sp = Vec2::new(-sa.y, sa.x);
        let len = sp.length();
        if len < 1e-4 {
            0.0
        } else {
            mouse_delta.dot(sp / len) * sens
        }
    }
}

fn screen_delta_to_scale(mouse_delta: Vec2, axis_world: Vec3, cam: &GlobalTransform) -> f32 {
    let cr = cam.right().as_vec3();
    let cu = cam.up().as_vec3();
    let sa = Vec2::new(axis_world.dot(cr), -axis_world.dot(cu));
    let len = sa.length();
    if len < 1e-4 {
        0.0
    } else {
        mouse_delta.dot(sa / len) * 0.005
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::primitives::Aabb;
    use std::f32::consts::PI;

    // ── screen-delta helpers ────────────────────────────────────────────────

    #[test]
    fn screen_delta_to_angle_front_facing_uses_combined_delta() {
        // Identity camera looks down -Z; the Z axis faces the camera
        // (|dot| = 1 > 0.7) → angle = (dx - dy) * 0.005.
        let cam = GlobalTransform::IDENTITY;
        let a = screen_delta_to_angle(Vec2::new(10.0, 4.0), Vec3::Z, &cam);
        assert!((a - 0.03).abs() < 1e-6, "got {a}");
    }

    #[test]
    fn screen_delta_to_angle_edge_on_projects_perpendicular() {
        // X axis is edge-on to the identity camera: screen axis is (1,0),
        // its perpendicular is (0,1) → only the vertical delta contributes.
        let cam = GlobalTransform::IDENTITY;
        let a = screen_delta_to_angle(Vec2::new(3.0, 8.0), Vec3::X, &cam);
        assert!((a - 0.04).abs() < 1e-6, "got {a}");
    }

    #[test]
    fn screen_delta_to_scale_projects_onto_axis() {
        let cam = GlobalTransform::IDENTITY;
        // X axis maps to screen (1, 0): only the horizontal delta counts.
        let s = screen_delta_to_scale(Vec2::new(10.0, 99.0), Vec3::X, &cam);
        assert!((s - 0.05).abs() < 1e-6, "got {s}");
        // Z axis has no screen projection on the identity camera → 0.
        let s = screen_delta_to_scale(Vec2::new(10.0, 10.0), Vec3::Z, &cam);
        assert_eq!(s, 0.0);
    }

    // ── world-space AABB helper ─────────────────────────────────────────────

    #[test]
    fn world_aabb_min_applies_translation_and_scale() {
        let aabb = Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0));
        let min = world_aabb_min(
            &aabb,
            Vec3::new(10.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::new(2.0, 3.0, 1.0),
        );
        assert!((min - Vec3::new(8.0, -3.0, -1.0)).length() < 1e-4, "got {min}");
    }

    #[test]
    fn world_aabb_min_applies_rotation() {
        // 180° about X flips Y/Z, but a symmetric cube's min is unchanged.
        let aabb = Aabb::from_min_max(Vec3::ZERO, Vec3::ONE);
        let min = world_aabb_min(&aabb, Vec3::ZERO, Quat::from_rotation_x(PI), Vec3::ONE);
        // Local (0..1)³ rotated 180° about X → y/z in (-1..0).
        assert!((min - Vec3::new(0.0, -1.0, -1.0)).length() < 1e-4, "got {min}");
    }

    // ── rotate HUD readout ──────────────────────────────────────────────────

    #[test]
    fn angle_label_text_drops_the_degree_symbol() {
        // The stroke font is ASCII-only; a non-ASCII char would draw as a gap.
        let text = angle_label_text(std::f32::consts::FRAC_PI_4);
        assert!(text.is_ascii(), "got {text}");
        assert_eq!(text, "45.0");
    }

    #[test]
    fn angle_label_text_collapses_negative_zero() {
        // Both sides of zero, and the rounding noise just inside the clamp,
        // must render as one string or the label flickers as you cross zero.
        assert_eq!(angle_label_text(0.0), "0.0");
        assert_eq!(angle_label_text(-0.0), "0.0");
        assert_eq!(angle_label_text(-0.04_f32.to_radians()), "0.0");
        assert_eq!(angle_label_text(0.04_f32.to_radians()), "0.0");
    }

    #[test]
    fn angle_label_text_is_unbounded_past_three_digits() {
        // Nothing truncates this — a full turn and beyond must read out whole.
        assert_eq!(angle_label_text(100.0_f32.to_radians()), "100.0");
        assert_eq!(angle_label_text(450.0_f32.to_radians()), "450.0");
        assert_eq!(angle_label_text(-100.0_f32.to_radians()), "-100.0");
    }

    fn rotate_snap(step_degrees: f32) -> SnapSettings {
        SnapSettings {
            rotate_enabled: true,
            rotate_snap: step_degrees,
            ..Default::default()
        }
    }

    #[test]
    fn snap_rotation_rounds_to_the_nearest_step() {
        let snap = rotate_snap(15.0);
        let deg = |r: f32| snap_rotation(r.to_radians(), &snap).to_degrees();
        assert!((deg(22.0) - 15.0).abs() < 1e-3, "got {}", deg(22.0));
        assert!((deg(23.0) - 30.0).abs() < 1e-3, "got {}", deg(23.0));
        // Rounds symmetrically through zero, so dragging back retraces steps.
        assert!((deg(-23.0) + 30.0).abs() < 1e-3, "got {}", deg(-23.0));
        // Under half a step applies nothing — the readout guard relies on this.
        assert!(deg(7.0).abs() < 1e-3, "got {}", deg(7.0));
    }

    #[test]
    fn snap_rotation_passes_through_when_disabled() {
        let raw = 0.371_f32;
        assert_eq!(snap_rotation(raw, &SnapSettings::default()), raw);
        // Enabled but with a zero step would divide by zero — guarded off.
        assert_eq!(snap_rotation(raw, &rotate_snap(0.0)), raw);
    }
}

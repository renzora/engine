//! Edit-mode lifecycle, picking, grab translation, and bake-to-asset.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use renzora::core::EditorCamera;
use renzora::core::viewport_types::ViewportState;
use renzora::editor::{ActiveTool, EditorSelection};

use crate::edit_mesh::{EditMesh, VertexId};
use crate::operators;
use crate::selection::{MeshSelection, SelectMode};
use crate::undo::{EditMeshSnapshotCmd, SelectionSnapshot};

/// Pixels from the cursor at which a vertex or edge is considered picked.
const PICK_RADIUS_PX: f32 = 8.0;

// ── Lifecycle ───────────────────────────────────────────────────────────────

/// On entering Edit mode, promote the selected entity's Mesh into an
/// [`EditMesh`] component so operators can mutate topology. Idempotent.
pub fn enter_edit_mode(
    selection: Res<EditorSelection>,
    mut mesh_selection: ResMut<MeshSelection>,
    mut active_tool: ResMut<ActiveTool>,
    mut active_flag: ResMut<EditModeActive>,
    meshes: Res<Assets<Mesh>>,
    mesh_q: Query<&Mesh3d>,
    has_edit: Query<(), With<EditMesh>>,
    mut commands: Commands,
) {
    active_flag.0 = true;
    let Some(target) = selection.get() else {
        // No entity selected yet — let normal Scene-mode picking stay active
        // so the user can click a mesh to edit.
        if *active_tool == ActiveTool::None {
            *active_tool = ActiveTool::Select;
        }
        return;
    };
    // Entity is selected — disengage the scene gizmo / box-select so our
    // vert/edge/face picking owns the mouse.
    if *active_tool != ActiveTool::None {
        *active_tool = ActiveTool::None;
    }
    if mesh_selection.target == Some(target) {
        return;
    }
    if let Some(prev) = mesh_selection.target.take() {
        if has_edit.get(prev).is_ok() {
            commands.entity(prev).remove::<EditMesh>();
        }
    }
    mesh_selection.clear();
    mesh_selection.target = Some(target);

    if let Ok(mesh3d) = mesh_q.get(target) {
        if let Some(mesh) = meshes.get(&mesh3d.0) {
            if let Some(edit) = EditMesh::from_mesh(mesh) {
                commands.entity(target).insert(edit);
            } else {
                warn!("[mesh_edit] cannot edit non-triangle mesh");
            }
        }
    }
}

/// On leaving Edit mode, bake edits back to the Mesh asset and drop the
/// component.
/// Tracks whether the plugin is currently "inside" Edit mode so the exit
/// restore fires exactly once on the transition out.
#[derive(Resource, Default)]
pub struct EditModeActive(pub bool);

pub fn exit_edit_mode(
    mut mesh_selection: ResMut<MeshSelection>,
    mut active_tool: ResMut<ActiveTool>,
    mut active_flag: ResMut<EditModeActive>,
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_q: Query<&Mesh3d>,
    edit_q: Query<&EditMesh>,
    mut commands: Commands,
) {
    if !active_flag.0 {
        return;
    }
    active_flag.0 = false;
    // Transitioned out of Edit mode — restore the scene tool so picking +
    // the gizmo come back even when no target was edited.
    if *active_tool == ActiveTool::None {
        *active_tool = ActiveTool::Select;
    }
    if let Some(target) = mesh_selection.target.take() {
        mesh_selection.clear();
        if let (Ok(edit), Ok(mesh3d)) = (edit_q.get(target), mesh_q.get(target)) {
            if let Some(mesh) = meshes.get_mut(&mesh3d.0) {
                edit.bake_to_mesh(mesh);
            }
        }
        commands.entity(target).remove::<EditMesh>();
    }
}

// ── Mode keys (1=verts, 2=edges, 3=faces) ───────────────────────────────────

pub fn switch_select_mode(keys: Res<ButtonInput<KeyCode>>, mut sel: ResMut<MeshSelection>) {
    if keys.just_pressed(KeyCode::Digit1) {
        sel.mode = SelectMode::Vertex;
    } else if keys.just_pressed(KeyCode::Digit2) {
        sel.mode = SelectMode::Edge;
    } else if keys.just_pressed(KeyCode::Digit3) {
        sel.mode = SelectMode::Face;
    }
}

pub fn select_all_toggle(keys: Res<ButtonInput<KeyCode>>, mut sel: ResMut<MeshSelection>, edit_q: Query<&EditMesh>) {
    if !keys.just_pressed(KeyCode::KeyA) {
        return;
    }
    let Some(target) = sel.target else { return };
    let Ok(edit) = edit_q.get(target) else { return };
    let any_selected = !sel.is_empty();
    if any_selected {
        sel.clear();
    } else {
        match sel.mode {
            SelectMode::Vertex => sel.verts = (0..edit.vertices.len() as u32).map(VertexId).collect(),
            SelectMode::Edge => sel.edges = (0..edit.edges.len() as u32).map(crate::edit_mesh::EdgeId).collect(),
            SelectMode::Face => sel.faces = (0..edit.faces.len() as u32).map(crate::edit_mesh::FaceId).collect(),
        }
    }
}

// ── Picking ────────────────────────────────────────────────────────────────

pub fn pick_element(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    grab: Res<GrabState>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    edit_q: Query<(&EditMesh, &GlobalTransform)>,
    editor_selection: Res<EditorSelection>,
    mut sel: ResMut<MeshSelection>,
    mut active_tool: ResMut<ActiveTool>,
    mut commands: Commands,
) {
    if matches!(*grab, GrabState::Active { .. }) {
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor_vp) = viewport_cursor(&viewport, &window_q) else { return };
    let Ok((camera, cam_gt)) = camera_q.single() else { return };
    let Some(target) = sel.target else { return };
    let Ok((edit, gt)) = edit_q.get(target) else { return };

    let project = |p: Vec3| -> Option<Vec2> {
        camera.world_to_viewport(cam_gt, gt.transform_point(p)).ok()
    };

    let additive = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let mut hit_any = false;

    match sel.mode {
        SelectMode::Vertex => {
            let mut best: Option<(f32, VertexId)> = None;
            for (i, v) in edit.vertices.iter().enumerate() {
                if let Some(sp) = project(v.position) {
                    let d = (sp - cursor_vp).length();
                    if d <= PICK_RADIUS_PX && best.map_or(true, |(bd, _)| d < bd) {
                        best = Some((d, VertexId(i as u32)));
                    }
                }
            }
            hit_any = best.is_some();
            apply_pick(&mut sel.verts, best.map(|(_, id)| id), additive);
        }
        SelectMode::Edge => {
            let mut best: Option<(f32, crate::edit_mesh::EdgeId)> = None;
            for (i, e) in edit.edges.iter().enumerate() {
                let Some(a) = edit.vertices.get(e.verts[0].0 as usize).and_then(|v| project(v.position)) else { continue };
                let Some(b) = edit.vertices.get(e.verts[1].0 as usize).and_then(|v| project(v.position)) else { continue };
                let d = point_to_segment(cursor_vp, a, b);
                if d <= PICK_RADIUS_PX && best.map_or(true, |(bd, _)| d < bd) {
                    best = Some((d, crate::edit_mesh::EdgeId(i as u32)));
                }
            }
            hit_any = best.is_some();
            apply_pick(&mut sel.edges, best.map(|(_, id)| id), additive);
        }
        SelectMode::Face => {
            // World-space ray vs triangle. Closest hit wins.
            let Some((ray_origin, ray_dir)) = build_world_ray(camera, cam_gt, cursor_vp, &viewport) else { return };
            let inv = gt.to_matrix().inverse();
            let local_origin = inv.transform_point3(ray_origin);
            let local_dir = inv.transform_vector3(ray_dir).normalize_or_zero();
            let mut best: Option<(f32, crate::edit_mesh::FaceId)> = None;
            for (i, f) in edit.faces.iter().enumerate() {
                if f.verts.len() < 3 { continue; }
                let p0 = edit.vertices[f.verts[0].0 as usize].position;
                for w in f.verts.windows(2).skip(1) {
                    let p1 = edit.vertices[w[0].0 as usize].position;
                    let p2 = edit.vertices[w[1].0 as usize].position;
                    if let Some(t) = ray_triangle(local_origin, local_dir, p0, p1, p2) {
                        if best.map_or(true, |(bt, _)| t < bt) {
                            best = Some((t, crate::edit_mesh::FaceId(i as u32)));
                        }
                    }
                }
            }
            hit_any = best.is_some();
            apply_pick(&mut sel.faces, best.map(|(_, id)| id), additive);
        }
    }

    // Click on empty space (non-additive, no element hit) — release the edit
    // target so the user can click a different mesh to edit. Entity picking
    // takes over next frame once enter_edit_mode sees no selection.
    if !hit_any && !additive {
        sel.target = None;
        sel.clear();
        commands.entity(target).remove::<EditMesh>();
        // Clear the editor-wide selection too so enter_edit_mode doesn't
        // immediately re-promote the same entity next frame.
        editor_selection.set(None);
        if *active_tool == ActiveTool::None {
            *active_tool = ActiveTool::Select;
        }
    }
}

fn apply_pick<T: Copy + Eq + std::hash::Hash>(
    set: &mut std::collections::HashSet<T>,
    hit: Option<T>,
    additive: bool,
) {
    match (hit, additive) {
        (Some(id), true) => {
            if !set.insert(id) {
                set.remove(&id);
            }
        }
        (Some(id), false) => {
            set.clear();
            set.insert(id);
        }
        (None, false) => set.clear(),
        (None, true) => {}
    }
}

// ── Extrude (E) ────────────────────────────────────────────────────────────

pub fn extrude_system(
    keys: Res<ButtonInput<KeyCode>>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut edit_q: Query<(&mut EditMesh, &GlobalTransform)>,
    mut sel: ResMut<MeshSelection>,
    mut grab: ResMut<GrabState>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyE) { return; }
    if !matches!(*grab, GrabState::Idle) { return; }
    let Some(target) = sel.target else { return };
    let Ok((mut edit, gt)) = edit_q.get_mut(target) else { return };

    let before = edit.clone();
    let before_sel = SelectionSnapshot::from_selection(&sel);

    let Some(result) = operators::extrude(&mut edit, &sel) else { return };

    // Adopt the post-op selection.
    sel.verts = result.post_verts.clone();
    sel.edges = result.post_edges.clone();
    sel.faces = result.post_faces.clone();

    let after = edit.clone();
    let after_sel = SelectionSnapshot::from_selection(&sel);

    // Record the topology snapshot for undo.
    let cmd = EditMeshSnapshotCmd {
        entity: target,
        label: "Extrude",
        before,
        after,
        before_sel,
        after_sel,
    };
    commands.queue(move |world: &mut World| {
        renzora::undo::record(
            world,
            renzora::undo::UndoContext::Scene,
            Box::new(cmd),
        );
    });

    // Seed a grab so the user can immediately drag the new geometry.
    // Use face normal as the locked axis when available; otherwise
    // fall back to view-plane translation.
    let Some(cursor_vp) = viewport_cursor(&viewport, &window_q) else { return };
    let Ok((camera, cam_gt)) = camera_q.single() else { return };
    let Some((ray_origin, ray_dir)) = build_world_ray(camera, cam_gt, cursor_vp, &viewport) else { return };

    let starts: Vec<(u32, Vec3)> = result
        .new_verts
        .iter()
        .map(|&id| (id, edit.vertices[id as usize].position))
        .collect();
    if starts.is_empty() { return; }

    // Use the selection centroid in world space as the plane/axis anchor.
    let centroid_local: Vec3 = starts.iter().map(|(_, p)| *p).sum::<Vec3>() / starts.len() as f32;
    let centroid_world = gt.transform_point(centroid_local);

    let (axis, anchor_world, plane_normal) = if let Some(axis_local) = result.grab_axis {
        // Convert the local face normal into world space so the axis line
        // stays consistent as the user moves the mouse.
        let axis_world = (gt.affine().matrix3 * axis_local).normalize_or_zero();
        let anchor = closest_point_on_line(centroid_world, axis_world, ray_origin, ray_dir)
            .unwrap_or(centroid_world);
        (Some(axis_world), anchor, -cam_gt.forward().as_vec3())
    } else {
        let normal = -cam_gt.forward().as_vec3();
        let anchor = ray_plane(ray_origin, ray_dir, centroid_world, normal).unwrap_or(centroid_world);
        (None, anchor, normal)
    };

    *grab = GrabState::Active {
        anchor_world,
        plane_normal,
        plane_point: centroid_world,
        axis,
        starts,
        seeded_by_op: true,
    };
}

// ── Grab (G) — translate selected verts on the view plane ──────────────────

#[derive(Resource, Default)]
pub enum GrabState {
    #[default]
    Idle,
    Active {
        /// Origin of the total-delta measurement in world space. For
        /// view-plane grab this is the initial cursor hit; when an axis is
        /// locked it's re-anchored to the current closest-point on the axis
        /// line.
        anchor_world: Vec3,
        /// Plane used for view-plane grab (unused in axis mode).
        plane_normal: Vec3,
        plane_point: Vec3,
        /// World-space axis constraint (None = view plane).
        axis: Option<Vec3>,
        /// (vertex index, original local position).
        starts: Vec<(u32, Vec3)>,
        /// True when this grab was seeded by a topology op (extrude, inset,
        /// etc.) that already pushed a snapshot undo command. Cancelling
        /// must roll that op back by popping it off the undo stack.
        seeded_by_op: bool,
    },
}

pub fn grab_start(
    keys: Res<ButtonInput<KeyCode>>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    edit_q: Query<(&EditMesh, &GlobalTransform)>,
    sel: Res<MeshSelection>,
    mut grab: ResMut<GrabState>,
) {
    if !keys.just_pressed(KeyCode::KeyG) { return; }
    if !matches!(*grab, GrabState::Idle) { return; }
    let Some(target) = sel.target else { return };
    let Ok((edit, gt)) = edit_q.get(target) else { return };
    let vert_ids = selected_vert_ids(edit, &sel);
    if vert_ids.is_empty() { return; }

    let Some(cursor_vp) = viewport_cursor(&viewport, &window_q) else { return };
    let Ok((camera, cam_gt)) = camera_q.single() else { return };
    let Some((ray_origin, ray_dir)) = build_world_ray(camera, cam_gt, cursor_vp, &viewport) else { return };

    // Plane through the selection centroid, facing the camera.
    let centroid_local: Vec3 = vert_ids
        .iter()
        .map(|&id| edit.vertices[id as usize].position)
        .sum::<Vec3>()
        / vert_ids.len() as f32;
    let centroid_world = gt.transform_point(centroid_local);
    let normal = -cam_gt.forward().as_vec3();
    let Some(hit) = ray_plane(ray_origin, ray_dir, centroid_world, normal) else { return };

    let starts: Vec<(u32, Vec3)> = vert_ids
        .iter()
        .map(|&id| (id, edit.vertices[id as usize].position))
        .collect();

    *grab = GrabState::Active {
        anchor_world: hit,
        plane_normal: normal,
        plane_point: centroid_world,
        axis: None,
        starts,
        seeded_by_op: false,
    };
}

pub fn grab_update(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut edit_q: Query<(&mut EditMesh, &GlobalTransform)>,
    sel: Res<MeshSelection>,
    mut grab: ResMut<GrabState>,
    mut commands: Commands,
) {
    let (mut anchor_world, plane_normal, plane_point, mut axis, starts, seeded_by_op) = match &*grab {
        GrabState::Active { anchor_world, plane_normal, plane_point, axis, starts, seeded_by_op } => {
            (*anchor_world, *plane_normal, *plane_point, *axis, starts.clone(), *seeded_by_op)
        }
        GrabState::Idle => return,
    };
    let Some(target) = sel.target else { *grab = GrabState::Idle; return };
    let Ok((mut edit, gt)) = edit_q.get_mut(target) else { *grab = GrabState::Idle; return };

    // Cancel (RMB or Esc): restore and exit. If the grab was seeded by a
    // topology op (extrude, inset, …) we also need to roll that op back,
    // since simply restoring vertex positions would leave zero-length
    // duplicated geometry behind.
    if mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) {
        if seeded_by_op {
            commands.queue(|world: &mut World| {
                renzora::undo::undo_once(world);
            });
        } else {
            for (id, start) in &starts {
                edit.vertices[*id as usize].position = *start;
            }
            edit.dirty = true;
        }
        *grab = GrabState::Idle;
        return;
    }

    // Commit (LMB). Record an undo command with the net per-vertex deltas.
    if mouse.just_pressed(MouseButton::Left) {
        let deltas: Vec<(u32, Vec3, Vec3)> = starts
            .iter()
            .filter_map(|(id, old)| {
                let new = edit.vertices.get(*id as usize)?.position;
                if (new - *old).length_squared() > 1e-12 {
                    Some((*id, *old, new))
                } else {
                    None
                }
            })
            .collect();
        if !deltas.is_empty() {
            let cmd = crate::undo::VertexMoveCmd { entity: target, deltas };
            commands.queue(move |world: &mut World| {
                renzora::undo::record(
                    world,
                    renzora::undo::UndoContext::Scene,
                    Box::new(cmd),
                );
            });
        }
        edit.dirty = true;
        *grab = GrabState::Idle;
        return;
    }

    // Drag.
    let Some(cursor_vp) = viewport_cursor(&viewport, &window_q) else { return };
    let Ok((camera, cam_gt)) = camera_q.single() else { return };
    let Some((ray_origin, ray_dir)) = build_world_ray(camera, cam_gt, cursor_vp, &viewport) else { return };

    // Axis-constraint keys (tap to lock, tap same key again to release).
    let axis_toggle = if keys.just_pressed(KeyCode::KeyX) { Some(Vec3::X) }
        else if keys.just_pressed(KeyCode::KeyY) { Some(Vec3::Y) }
        else if keys.just_pressed(KeyCode::KeyZ) { Some(Vec3::Z) }
        else { None };
    if let Some(new_axis) = axis_toggle {
        // Toggle off when pressing the already-locked axis.
        let target_axis = if axis.map(|a| a.abs_diff_eq(new_axis, 1e-5)).unwrap_or(false) {
            None
        } else {
            Some(new_axis)
        };
        // Re-anchor so the current cursor position becomes the new zero.
        anchor_world = if let Some(a) = target_axis {
            closest_point_on_line(plane_point, a, ray_origin, ray_dir).unwrap_or(plane_point)
        } else {
            ray_plane(ray_origin, ray_dir, plane_point, plane_normal).unwrap_or(anchor_world)
        };
        axis = target_axis;
        *grab = GrabState::Active {
            anchor_world, plane_normal, plane_point, axis, starts: starts.clone(), seeded_by_op,
        };
        // Snap verts back to their start — subsequent frames will move
        // along the new constraint from zero.
        for (id, start) in &starts {
            edit.vertices[*id as usize].position = *start;
        }
        edit.dirty = true;
        return;
    }

    let delta_world = if let Some(a) = axis {
        let Some(hit) = closest_point_on_line(plane_point, a, ray_origin, ray_dir) else { return };
        hit - anchor_world
    } else {
        let Some(hit) = ray_plane(ray_origin, ray_dir, plane_point, plane_normal) else { return };
        hit - anchor_world
    };

    // Convert world delta into the edit mesh's local space.
    let inv_rot = gt.affine().matrix3.inverse();
    let delta_local = inv_rot * delta_world;
    for (id, start) in &starts {
        edit.vertices[*id as usize].position = *start + delta_local;
    }
    edit.dirty = true;
}

// ── Bake on dirty ──────────────────────────────────────────────────────────

pub fn bake_if_dirty(
    mut meshes: ResMut<Assets<Mesh>>,
    mut edit_q: Query<(&mut EditMesh, &Mesh3d)>,
) {
    for (mut edit, mesh3d) in &mut edit_q {
        if !edit.dirty { continue; }
        if let Some(mesh) = meshes.get_mut(&mesh3d.0) {
            edit.bake_to_mesh(mesh);
        }
        edit.dirty = false;
    }
}

// ── Overlay ────────────────────────────────────────────────────────────────

pub fn draw_overlay(
    mesh_selection: Res<MeshSelection>,
    edit_q: Query<(&EditMesh, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    let Some(target) = mesh_selection.target else { return };
    let Ok((edit, gt)) = edit_q.get(target) else { return };
    let to_world = |v: Vec3| gt.transform_point(v);

    // Edges: faint white unless selected.
    for (i, edge) in edit.edges.iter().enumerate() {
        let a = edit.vertices.get(edge.verts[0].0 as usize).map(|v| v.position);
        let b = edit.vertices.get(edge.verts[1].0 as usize).map(|v| v.position);
        let (Some(a), Some(b)) = (a, b) else { continue };
        let selected =
            mesh_selection.mode == SelectMode::Edge && mesh_selection.edges.contains(&crate::edit_mesh::EdgeId(i as u32));
        let color = if selected {
            Color::srgb(1.0, 0.55, 0.1)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.35)
        };
        gizmos.line(to_world(a), to_world(b), color);
    }

    // Vertex dots (only drawn in vertex mode to reduce clutter).
    if mesh_selection.mode == SelectMode::Vertex {
        for (i, v) in edit.vertices.iter().enumerate() {
            let selected = mesh_selection.verts.contains(&VertexId(i as u32));
            let color = if selected {
                Color::srgb(1.0, 0.55, 0.1)
            } else {
                Color::srgb(0.15, 0.55, 1.0)
            };
            gizmos.sphere(to_world(v.position), 0.03, color);
        }
    }

    // Face highlights: draw the triangles' outline tinted when selected.
    if mesh_selection.mode == SelectMode::Face {
        for (i, face) in edit.faces.iter().enumerate() {
            let selected = mesh_selection.faces.contains(&crate::edit_mesh::FaceId(i as u32));
            let color = if selected {
                Color::srgba(1.0, 0.55, 0.1, 0.9)
            } else {
                continue;
            };
            for w in face.verts.windows(2) {
                let a = edit.vertices[w[0].0 as usize].position;
                let b = edit.vertices[w[1].0 as usize].position;
                gizmos.line(to_world(a), to_world(b), color);
            }
            // Close the loop.
            if let (Some(first), Some(last)) = (face.verts.first(), face.verts.last()) {
                let a = edit.vertices[last.0 as usize].position;
                let b = edit.vertices[first.0 as usize].position;
                gizmos.line(to_world(a), to_world(b), color);
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn selected_vert_ids(edit: &EditMesh, sel: &MeshSelection) -> Vec<u32> {
    match sel.mode {
        SelectMode::Vertex => sel.verts.iter().map(|v| v.0).collect(),
        SelectMode::Edge => {
            let mut out: std::collections::HashSet<u32> = Default::default();
            for id in &sel.edges {
                if let Some(e) = edit.edges.get(id.0 as usize) {
                    out.insert(e.verts[0].0);
                    out.insert(e.verts[1].0);
                }
            }
            out.into_iter().collect()
        }
        SelectMode::Face => {
            let mut out: std::collections::HashSet<u32> = Default::default();
            for id in &sel.faces {
                if let Some(f) = edit.faces.get(id.0 as usize) {
                    for v in &f.verts {
                        out.insert(v.0);
                    }
                }
            }
            out.into_iter().collect()
        }
    }
}

fn viewport_cursor(
    viewport: &Option<Res<ViewportState>>,
    window_q: &Query<&Window, With<PrimaryWindow>>,
) -> Option<Vec2> {
    let vp = viewport.as_ref()?;
    let window = window_q.single().ok()?;
    let cursor = window.cursor_position()?;
    let vp_min = vp.screen_position;
    let vp_max = vp_min + vp.screen_size;
    if cursor.x < vp_min.x || cursor.y < vp_min.y || cursor.x > vp_max.x || cursor.y > vp_max.y {
        return None;
    }
    Some(Vec2::new(
        (cursor.x - vp_min.x) / vp.screen_size.x * vp.current_size.x as f32,
        (cursor.y - vp_min.y) / vp.screen_size.y * vp.current_size.y as f32,
    ))
}

fn build_world_ray(
    camera: &Camera,
    cam_gt: &GlobalTransform,
    viewport_pos: Vec2,
    _vp: &Option<Res<ViewportState>>,
) -> Option<(Vec3, Vec3)> {
    let ray = camera.viewport_to_world(cam_gt, viewport_pos).ok()?;
    Some((ray.origin, ray.direction.as_vec3()))
}

/// Closest point on an infinite line `(line_point + t * line_dir)` to the
/// ray `(ray_origin + s * ray_dir)`. Returns `None` if the two lines are
/// near-parallel (axis-on-view happens when you lock to an axis that's
/// pointing at the camera).
fn closest_point_on_line(
    line_point: Vec3,
    line_dir: Vec3,
    ray_origin: Vec3,
    ray_dir: Vec3,
) -> Option<Vec3> {
    let line_dir = line_dir.normalize_or_zero();
    let ray_dir = ray_dir.normalize_or_zero();
    if line_dir.length_squared() < 1e-6 || ray_dir.length_squared() < 1e-6 {
        return None;
    }
    let b = line_dir.dot(ray_dir);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-4 {
        return None;
    }
    let w = line_point - ray_origin;
    let d = line_dir.dot(w);
    let e = ray_dir.dot(w);
    let t = (b * e - d) / denom;
    Some(line_point + line_dir * t)
}

fn ray_plane(origin: Vec3, dir: Vec3, plane_point: Vec3, normal: Vec3) -> Option<Vec3> {
    let denom = normal.dot(dir);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = normal.dot(plane_point - origin) / denom;
    if t < 0.0 {
        return None;
    }
    Some(origin + dir * t)
}

fn ray_triangle(origin: Vec3, dir: Vec3, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<f32> {
    // Möller–Trumbore.
    let e1 = v1 - v0;
    let e2 = v2 - v0;
    let h = dir.cross(e2);
    let a = e1.dot(h);
    if a.abs() < 1e-6 { return None; }
    let f = 1.0 / a;
    let s = origin - v0;
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) { return None; }
    let q = s.cross(e1);
    let v = f * dir.dot(q);
    if v < 0.0 || u + v > 1.0 { return None; }
    let t = f * e2.dot(q);
    if t < 0.0 { return None; }
    Some(t)
}

fn point_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

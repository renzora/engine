//! Modeling tools: op dispatch (keyboard + panel), the modal loop-cut
//! preview, Tab mode switching, and the status-bar mode indicator.
//!
//! Topology operators run through one funnel — [`PendingOps`] — so the
//! keyboard shortcuts and the Modeling panel buttons share the exact same
//! snapshot/undo/selection bookkeeping in [`apply_pending_ops`].

use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use bevy::window::PrimaryWindow;
use renzora::core::viewport_types::{ViewportMode, ViewportSettings, ViewportState, ViewportView};
use renzora::core::InputFocusState;
use renzora::EditorSelection;

use crate::edit_mesh::{EdgeId, EditMesh, VertexId};
use crate::operators;
use crate::selection::{MeshSelection, SelectMode};
use crate::systems::GrabState;
use crate::undo::{EditMeshSnapshotCmd, SelectionSnapshot};

/// Tunables shared between the keyboard ops and the Modeling panel.
#[derive(Resource)]
pub struct ModelingSettings {
    /// Mirror interactive edits (grab, sculpt) across the local X plane.
    pub symmetry_x: bool,
    /// Relative inset distance toward the face centroid (0..1).
    pub inset_amount: f32,
    /// Merge-by-distance threshold.
    pub weld_dist: f32,
    pub array_count: u32,
    pub array_offset: Vec3,
    /// Interpret `array_offset` as a multiple of the mesh bounds per axis.
    pub array_relative: bool,
}

impl Default for ModelingSettings {
    fn default() -> Self {
        Self {
            symmetry_x: false,
            inset_amount: 0.25,
            weld_dist: 0.001,
            array_count: 2,
            array_offset: Vec3::new(1.1, 0.0, 0.0),
            array_relative: true,
        }
    }
}

/// A queued topology operation. Keyboard systems and panel buttons both push
/// here; [`apply_pending_ops`] executes with uniform undo handling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModelingOp {
    Delete,
    Dissolve,
    MergeAtCenter,
    RemoveDoubles,
    Inset,
    Subdivide,
    /// Cut by the local axis plane through the origin (0 = X, 1 = Y, 2 = Z).
    Bisect(usize),
    /// Symmetrize +axis → −axis.
    Mirror(usize),
    Array,
}

#[derive(Resource, Default)]
pub struct PendingOps(pub Vec<ModelingOp>);

// ── Tab: Scene ↔ Edit mode ─────────────────────────────────────────────────

/// Shortcut handler (default `Tab`): Scene → Edit when a mesh entity is
/// selected; Edit or Sculpt → back to Scene. Registered via
/// `register_shortcut` so it's rebindable and respects text-input focus.
pub fn toggle_edit_mode(world: &mut World) {
    // No mesh editing while the game is running in the viewport.
    if world
        .get_resource::<renzora::PlayModeState>()
        .is_some_and(|pm| pm.is_in_play_mode())
    {
        return;
    }
    let selected_mesh = {
        let Some(selection) = world.get_resource::<EditorSelection>() else {
            return;
        };
        selection
            .get()
            .filter(|e| world.get::<Mesh3d>(*e).is_some())
    };
    let Some(mut settings) = world.get_resource_mut::<ViewportSettings>() else {
        return;
    };
    // Mesh editing only makes sense in the 3D view.
    if settings.viewport_view != ViewportView::Three {
        return;
    }
    match settings.viewport_mode {
        ViewportMode::Scene if selected_mesh.is_some() => {
            settings.viewport_mode = ViewportMode::Edit;
        }
        ViewportMode::Edit | ViewportMode::Sculpt => {
            settings.viewport_mode = ViewportMode::Scene;
        }
        _ => {}
    }
}

// ── Status-bar mode indicator ──────────────────────────────────────────────

/// Mirrors the viewport mode into the status bar ("Ready" slot). Only
/// touches [`renzora::ShellReadyStatus`] while a modeling mode is active so
/// other overriders (autosave countdown) win the rest of the time.
pub fn update_mode_status(
    settings: Option<Res<ViewportSettings>>,
    status: Option<ResMut<renzora::ShellReadyStatus>>,
    mut we_set_it: Local<bool>,
) {
    let (Some(settings), Some(mut status)) = (settings, status) else {
        return;
    };
    match settings.viewport_mode {
        ViewportMode::Edit => {
            status.label = Some("Edit Mode — Tab to exit".into());
            status.color = Some([255, 150, 40]);
            *we_set_it = true;
        }
        ViewportMode::Sculpt => {
            status.label = Some("Sculpt Mode — Tab to exit".into());
            status.color = Some([190, 120, 255]);
            *we_set_it = true;
        }
        _ => {
            if *we_set_it {
                status.label = None;
                status.color = None;
                *we_set_it = false;
            }
        }
    }
}

// ── Keyboard op triggers (Edit mode) ───────────────────────────────────────

pub fn op_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input_focus: Res<InputFocusState>,
    grab: Res<GrabState>,
    loop_cut: Res<LoopCutState>,
    mut pending: ResMut<PendingOps>,
) {
    if input_focus.ui_wants_keyboard {
        return;
    }
    if !matches!(*grab, GrabState::Idle) || !matches!(*loop_cut, LoopCutState::Idle) {
        return;
    }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if keys.just_pressed(KeyCode::KeyX) && ctrl {
        pending.0.push(ModelingOp::Dissolve);
    } else if keys.just_pressed(KeyCode::KeyX) || keys.just_pressed(KeyCode::Delete) {
        pending.0.push(ModelingOp::Delete);
    }
    if keys.just_pressed(KeyCode::KeyM) && !ctrl {
        pending.0.push(ModelingOp::MergeAtCenter);
    }
    if keys.just_pressed(KeyCode::KeyI) && !ctrl {
        pending.0.push(ModelingOp::Inset);
    }
}

/// Drain [`PendingOps`]: snapshot → run operator → adopt post-selection →
/// record undo. One frame can carry several ops; each gets its own undo entry.
pub fn apply_pending_ops(
    mut pending: ResMut<PendingOps>,
    settings: Res<ModelingSettings>,
    mut sel: ResMut<MeshSelection>,
    mut edit_q: Query<&mut EditMesh>,
    mut commands: Commands,
) {
    if pending.0.is_empty() {
        return;
    }
    let ops = std::mem::take(&mut pending.0);
    let Some(target) = sel.target else { return };
    let Ok(mut edit) = edit_q.get_mut(target) else {
        return;
    };

    for op in ops {
        let before = edit.clone();
        let before_sel = SelectionSnapshot::from_selection(&sel);

        let (changed, label): (bool, &'static str) = match op {
            ModelingOp::Delete => {
                let did = operators::delete_selection(&mut edit, &sel);
                if did {
                    sel.clear();
                }
                (did, "Delete")
            }
            ModelingOp::Dissolve => match sel.mode {
                SelectMode::Edge => {
                    let did = operators::dissolve_edges(&mut edit, &sel.edges);
                    if did {
                        sel.clear();
                    }
                    (did, "Dissolve Edges")
                }
                SelectMode::Vertex => {
                    let did = operators::dissolve_verts(&mut edit, &sel.verts);
                    if did {
                        sel.clear();
                    }
                    (did, "Dissolve Vertices")
                }
                SelectMode::Face => (false, "Dissolve"),
            },
            ModelingOp::MergeAtCenter => {
                let verts: std::collections::HashSet<VertexId> =
                    selected_vert_id_set(&edit, &sel);
                let survivor = operators::merge_at_center(&mut edit, &verts);
                if let Some(v) = survivor {
                    sel.clear();
                    sel.mode = SelectMode::Vertex;
                    sel.verts.insert(v);
                }
                (survivor.is_some(), "Merge at Center")
            }
            ModelingOp::RemoveDoubles => {
                let removed = operators::remove_doubles(&mut edit, settings.weld_dist);
                if removed > 0 {
                    sel.clear();
                }
                (removed > 0, "Merge by Distance")
            }
            ModelingOp::Inset => {
                let post = operators::inset_faces(&mut edit, &sel.faces, settings.inset_amount);
                if let Some(post) = post {
                    sel.mode = SelectMode::Face;
                    sel.verts.clear();
                    sel.edges.clear();
                    sel.faces = post;
                    (true, "Inset Faces")
                } else {
                    (false, "Inset Faces")
                }
            }
            ModelingOp::Subdivide => {
                // Subdivide works on faces; in vert/edge mode use the faces
                // fully covered by the selection.
                let faces = faces_from_selection(&edit, &sel);
                let post = operators::subdivide_faces(&mut edit, &faces);
                if let Some(post) = post {
                    sel.mode = SelectMode::Face;
                    sel.verts.clear();
                    sel.edges.clear();
                    sel.faces = post;
                    (true, "Subdivide")
                } else {
                    (false, "Subdivide")
                }
            }
            ModelingOp::Bisect(axis) => {
                let mut normal = Vec3::ZERO;
                normal[axis.min(2)] = 1.0;
                let post = operators::bisect(&mut edit, Vec3::ZERO, normal, false);
                if let Some(post) = post {
                    let changed = !post.is_empty();
                    if changed {
                        sel.clear();
                        sel.mode = SelectMode::Edge;
                        sel.edges = post;
                    }
                    (changed, "Bisect")
                } else {
                    (false, "Bisect")
                }
            }
            ModelingOp::Mirror(axis) => {
                let did = operators::mirror_symmetrize(&mut edit, axis.min(2));
                if did {
                    sel.clear();
                }
                (did, "Mirror")
            }
            ModelingOp::Array => {
                let did = operators::array_duplicate(
                    &mut edit,
                    settings.array_count.max(2),
                    settings.array_offset,
                    settings.array_relative,
                    settings.weld_dist,
                );
                if did {
                    sel.clear();
                }
                (did, "Array")
            }
        };

        if !changed {
            continue;
        }
        edit.dirty = true;

        let cmd = EditMeshSnapshotCmd {
            entity: target,
            label,
            before,
            after: edit.clone(),
            before_sel,
            after_sel: SelectionSnapshot::from_selection(&sel),
        };
        commands.queue(move |world: &mut World| {
            renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
        });
    }
}

/// All selected verts regardless of select mode (edges/faces contribute
/// their corners) as a `HashSet<VertexId>`.
fn selected_vert_id_set(
    edit: &EditMesh,
    sel: &MeshSelection,
) -> std::collections::HashSet<VertexId> {
    let mut out: std::collections::HashSet<VertexId> = sel.verts.clone();
    for id in &sel.edges {
        if let Some(e) = edit.edges.get(id.0 as usize) {
            out.insert(e.verts[0]);
            out.insert(e.verts[1]);
        }
    }
    for id in &sel.faces {
        if let Some(f) = edit.faces.get(id.0 as usize) {
            out.extend(f.verts.iter().copied());
        }
    }
    out
}

/// Faces implied by the current selection: face mode → the faces themselves;
/// vert/edge mode → faces whose corners are fully selected.
fn faces_from_selection(
    edit: &EditMesh,
    sel: &MeshSelection,
) -> std::collections::HashSet<crate::edit_mesh::FaceId> {
    use crate::edit_mesh::FaceId;
    match sel.mode {
        SelectMode::Face => sel.faces.clone(),
        SelectMode::Vertex => (0..edit.faces.len())
            .filter(|&i| {
                let f = &edit.faces[i];
                !f.verts.is_empty() && f.verts.iter().all(|v| sel.verts.contains(v))
            })
            .map(|i| FaceId(i as u32))
            .collect(),
        SelectMode::Edge => {
            let pairs: std::collections::HashSet<(u32, u32)> = sel
                .edges
                .iter()
                .filter_map(|e| edit.edges.get(e.0 as usize))
                .map(|e| {
                    let (a, b) = (e.verts[0].0, e.verts[1].0);
                    if a < b {
                        (a, b)
                    } else {
                        (b, a)
                    }
                })
                .collect();
            (0..edit.faces.len())
                .filter(|&i| {
                    let f = &edit.faces[i];
                    let n = f.verts.len();
                    n >= 3
                        && (0..n).all(|k| {
                            let a = f.verts[k].0;
                            let b = f.verts[(k + 1) % n].0;
                            pairs.contains(&if a < b { (a, b) } else { (b, a) })
                        })
                })
                .map(|i| FaceId(i as u32))
                .collect()
        }
    }
}

// ── Loop cut (Ctrl+R, modal) ───────────────────────────────────────────────

/// Modal state for the loop-cut tool: Ctrl+R arms a preview that follows the
/// edge ring under the cursor; scroll changes the cut count; LMB commits;
/// Esc/RMB cancels.
#[derive(Resource, Default)]
pub enum LoopCutState {
    #[default]
    Idle,
    Preview {
        edge: Option<EdgeId>,
        cuts: u32,
    },
    /// Commit/cancel happened this frame — keeps `pick_element` (later in
    /// the chain) from consuming the same click. Back to `Idle` next frame.
    JustFinished,
}

pub fn loop_cut_modal(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    input_focus: Res<InputFocusState>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera>>,
    mut scroll: MessageReader<MouseWheel>,
    grab: Res<GrabState>,
    mut state: ResMut<LoopCutState>,
    mut sel: ResMut<MeshSelection>,
    mut edit_q: Query<(&mut EditMesh, &GlobalTransform)>,
    mut gizmos: Gizmos,
    mut commands: Commands,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    if matches!(*state, LoopCutState::JustFinished) {
        *state = LoopCutState::Idle;
        return;
    }

    // Arm.
    if matches!(*state, LoopCutState::Idle) {
        if input_focus.ui_wants_keyboard
            || !matches!(*grab, GrabState::Idle)
            || !(ctrl && keys.just_pressed(KeyCode::KeyR))
        {
            // Drain scroll events we're not consuming so they don't pile up.
            scroll.clear();
            return;
        }
        *state = LoopCutState::Preview {
            edge: None,
            cuts: 1,
        };
    }

    let LoopCutState::Preview { edge, cuts } = &mut *state else {
        return;
    };

    // Cancel.
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right) {
        *state = LoopCutState::JustFinished;
        return;
    }

    let Some(target) = sel.target else {
        *state = LoopCutState::Idle;
        return;
    };
    let Ok((mut edit, gt)) = edit_q.get_mut(target) else {
        *state = LoopCutState::Idle;
        return;
    };

    // Scroll adjusts the number of cuts.
    for ev in scroll.read() {
        let delta = if ev.y > 0.0 { 1i32 } else { -1i32 };
        *cuts = (*cuts as i32 + delta).clamp(1, 16) as u32;
    }

    // Track the edge ring under the cursor.
    if let (Some(cursor_vp), Ok((camera, cam_gt))) = (
        crate::systems::viewport_cursor(&viewport, &window_q),
        camera_q.single(),
    ) {
        let project = |p: Vec3| -> Option<Vec2> {
            camera
                .world_to_viewport(cam_gt, gt.transform_point(p))
                .ok()
        };
        let mut best: Option<(f32, EdgeId)> = None;
        for (i, e) in edit.edges.iter().enumerate() {
            let (Some(a), Some(b)) = (
                edit.vertices
                    .get(e.verts[0].0 as usize)
                    .and_then(|v| project(v.position)),
                edit.vertices
                    .get(e.verts[1].0 as usize)
                    .and_then(|v| project(v.position)),
            ) else {
                continue;
            };
            let d = crate::systems::point_to_segment(cursor_vp, a, b);
            if d <= 48.0 && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, EdgeId(i as u32)));
            }
        }
        if let Some((_, id)) = best {
            *edge = Some(id);
        }
    }

    // Draw the preview: for each ring quad, connect matching parameter
    // points on its two ring edges (in cycle order the edges run in
    // opposite directions, hence t vs 1−t).
    let Some(start) = *edge else { return };
    let ring = operators::walk_edge_ring(&edit, start);
    if ring.faces.is_empty() {
        return;
    }
    let ring_set: std::collections::HashSet<u32> = ring.edges.iter().map(|e| e.0).collect();
    let preview_color = Color::srgb(1.0, 0.85, 0.2);
    for fid in &ring.faces {
        let face = &edit.faces[fid.0 as usize];
        let n = face.verts.len();
        // Directed occurrences of the two ring edges in this face's cycle.
        let mut dirs: Vec<(Vec3, Vec3)> = Vec::new();
        for i in 0..n {
            let a = face.verts[i].0;
            let b = face.verts[(i + 1) % n].0;
            let is_ring = face.edges.get(i).map(|e| ring_set.contains(&e.0)).unwrap_or(false)
                || edit
                    .edges
                    .iter()
                    .enumerate()
                    .any(|(k, e)| {
                        ring_set.contains(&(k as u32))
                            && ((e.verts[0].0 == a && e.verts[1].0 == b)
                                || (e.verts[0].0 == b && e.verts[1].0 == a))
                    });
            if is_ring {
                dirs.push((
                    edit.vertices[a as usize].position,
                    edit.vertices[b as usize].position,
                ));
            }
        }
        if dirs.len() != 2 {
            continue;
        }
        for k in 1..=*cuts {
            let t = k as f32 / (*cuts + 1) as f32;
            let p0 = dirs[0].0.lerp(dirs[0].1, t);
            let p1 = dirs[1].0.lerp(dirs[1].1, 1.0 - t);
            gizmos.line(gt.transform_point(p0), gt.transform_point(p1), preview_color);
        }
    }

    // Commit.
    if mouse.just_pressed(MouseButton::Left) {
        let before = edit.clone();
        let before_sel = SelectionSnapshot::from_selection(&sel);
        if let Some(new_edges) = operators::loop_cut(&mut edit, start, *cuts) {
            sel.clear();
            sel.mode = SelectMode::Edge;
            sel.edges = new_edges;
            edit.dirty = true;
            let cmd = EditMeshSnapshotCmd {
                entity: target,
                label: "Loop Cut",
                before,
                after: edit.clone(),
                before_sel,
                after_sel: SelectionSnapshot::from_selection(&sel),
            };
            commands.queue(move |world: &mut World| {
                renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
            });
        }
        *state = LoopCutState::JustFinished;
    }
}

// ── Join (Ctrl+J, Scene mode) ──────────────────────────────────────────────

/// Join every selected mesh entity into the primary selection (Blender's
/// Ctrl+J): source geometry is transformed into the target's local space,
/// appended, and the source entities despawn. The result gets a fresh mesh
/// asset (never mutate a possibly-shared source handle) plus an
/// [`renzora::core::EditedMesh`] so it persists with the scene.
///
/// Not undoable — the despawned sources can't be restored by the mesh undo
/// stack. Deliberate for now; scene-level entity undo is a separate system.
pub fn join_selected(
    keys: Res<ButtonInput<KeyCode>>,
    input_focus: Res<InputFocusState>,
    selection: Res<EditorSelection>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<(&Mesh3d, &GlobalTransform)>,
    mut commands: Commands,
) {
    if input_focus.ui_wants_keyboard {
        return;
    }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !(ctrl && keys.just_pressed(KeyCode::KeyJ)) {
        return;
    }
    let all = selection.get_all();
    if all.len() < 2 {
        return;
    }
    let target = all[0];
    let Ok((target_mesh3d, target_gt)) = q.get(target) else {
        return;
    };
    let Some(mut combined) = meshes
        .get(&target_mesh3d.0)
        .and_then(renzora::core::EditedMesh::from_mesh)
    else {
        warn!("[mesh_edit] join: target mesh has no readable geometry");
        return;
    };
    let target_inv = target_gt.to_matrix().inverse();

    let mut joined_any = false;
    for &src in &all[1..] {
        let Ok((mesh3d, gt)) = q.get(src) else {
            continue;
        };
        let Some(data) = meshes
            .get(&mesh3d.0)
            .and_then(renzora::core::EditedMesh::from_mesh)
        else {
            continue;
        };
        let rel = target_inv * gt.to_matrix();
        // Normals transform by the inverse-transpose of the linear part.
        let normal_mat = Mat3::from_mat4(rel).inverse().transpose();
        let base = (combined.positions.len() / 3) as u32;
        for chunk in data.positions.chunks_exact(3) {
            let p = rel.transform_point3(Vec3::new(chunk[0], chunk[1], chunk[2]));
            combined.positions.extend_from_slice(&[p.x, p.y, p.z]);
        }
        if data.normals.len() == data.positions.len() {
            for chunk in data.normals.chunks_exact(3) {
                let n = (normal_mat * Vec3::new(chunk[0], chunk[1], chunk[2]))
                    .normalize_or_zero();
                combined.normals.extend_from_slice(&[n.x, n.y, n.z]);
            }
        } else {
            combined
                .normals
                .extend(std::iter::repeat_n(0.0, data.positions.len()));
        }
        if data.uvs.len() == data.positions.len() / 3 * 2 {
            combined.uvs.extend_from_slice(&data.uvs);
        } else {
            combined
                .uvs
                .extend(std::iter::repeat_n(0.0, data.positions.len() / 3 * 2));
        }
        combined
            .indices
            .extend(data.indices.iter().map(|&i| i + base));
        commands.entity(src).despawn();
        joined_any = true;
    }
    if !joined_any {
        return;
    }

    let handle = meshes.add(combined.to_mesh());
    commands.entity(target).try_insert((
        Mesh3d(handle),
        combined,
        renzora::core::EditedMeshApplied,
    ));
    selection.set(Some(target));
    info!("[mesh_edit] joined {} meshes", all.len());
}

// ── Alt+click edge-loop select ─────────────────────────────────────────────

/// Expand a picked edge into its whole edge loop. Runs *after*
/// `pick_element` in the same frame: when Alt was held and exactly one edge
/// just became selected, the walk replaces it with the loop.
pub fn loop_select(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut sel: ResMut<MeshSelection>,
    edit_q: Query<&EditMesh>,
    mut last_pick: Local<Option<EdgeId>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if !alt || sel.mode != SelectMode::Edge {
        *last_pick = None;
        return;
    }
    let Some(target) = sel.target else { return };
    let Ok(edit) = edit_q.get(target) else { return };
    // The freshly clicked edge is the one not present before this frame —
    // with plain Alt+click the set was replaced, so len == 1.
    if sel.edges.len() != 1 {
        return;
    }
    let seed = *sel.edges.iter().next().unwrap();
    if *last_pick == Some(seed) {
        return;
    }
    *last_pick = Some(seed);
    let loop_edges = operators::walk_edge_loop(edit, seed);
    sel.edges = loop_edges;
}

#![allow(unused_mut, dead_code, unused_variables, unused_assignments)]

//! Edit-mode lifecycle, picking, grab translation, and bake-to-asset.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use renzora::core::viewport_types::{ViewportSettings, ViewportState};
use renzora::core::EditorCamera;
use renzora::core::InputFocusState;
use renzora_editor_framework::{ActiveTool, EditorSelection};

use crate::edit_mesh::{EditMesh, VertexId};
use crate::operators;
use crate::selection::{MeshSelection, SelectMode};
use crate::undo::{EditMeshSnapshotCmd, SelectionSnapshot};

/// Pixels from the cursor at which a **vertex** is considered picked.
const PICK_RADIUS_PX_VERTEX: f32 = 8.0;

/// Pixels from the cursor at which an **edge** is considered picked. Looser
/// than the vertex radius because edges are 1-D and the cursor can otherwise
/// miss a thin line by a few pixels — Blender ships with a comparable pad
/// here.
const PICK_RADIUS_PX_EDGE: f32 = 12.0;

/// Distance the cursor must travel from the LMB-down point before a press
/// "becomes" a marquee drag instead of a single click. Below this, the
/// release commits a single-element pick (or releases the edit target on
/// empty space, preserving the historical click-to-de-select behaviour);
/// above this, the release commits a marquee hit-test.
const DRAG_THRESHOLD_PX: f32 = 4.0;

/// Drag-vs-click state for the mesh-edit picker. A press on empty space is
/// either a single click (released before [`DRAG_THRESHOLD_PX`]) or a marquee
/// drag (released after). Replaces the implicit "every LMB-down on empty
/// space deselects the target immediately" behaviour, which used to fight
/// the user's attempt to drag a box around multiple vertices.
#[derive(Resource, Default)]
pub struct MeshEditBoxSelect {
    pub state: MeshEditBoxSelectState,
}

/// Mode of the marquee drag. `Pressing` is the warmup before the cursor
/// has crossed the threshold; `Marqueeing` is the active rubber-band state
/// that draws the rectangle and commits on release.
#[derive(Default, Clone, Copy)]
pub enum MeshEditBoxSelectState {
    #[default]
    Idle,
    /// LMB is down on empty space but the cursor hasn't travelled past
    /// `DRAG_THRESHOLD_PX`. Promotes to `Marqueeing` on drag, or releases
    /// the edit target on LMB-up if the user just clicks.
    Pressing {
        anchor_vp: Vec2,
        mode: SelectMode,
        /// Was Shift held when the press started? Shift-promoted releases
        /// toggle individual elements instead of replacing the selection.
        additive: bool,
        target: Entity,
    },
    /// LMB-down + cursor past the drag threshold. The rubber-band runs
    /// from `anchor_vp` to `current_vp`; on LMB-up every
    /// vertex/edge/face-marker inside the rect is added to (Shift) or
    /// replaces the selection.
    Marqueeing {
        anchor_vp: Vec2,
        current_vp: Vec2,
        mode: SelectMode,
        additive: bool,
        target: Entity,
    },
}

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
            if let Some(mut mesh) = meshes.get_mut(&mesh3d.0) {
                edit.bake_to_mesh(&mut mesh);
                if let Some(snapshot) = renzora::core::EditedMesh::from_mesh(&mesh) {
                    commands
                        .entity(target)
                        .try_insert((snapshot, renzora::core::EditedMeshApplied));
                }
            }
        }
        commands.entity(target).remove::<EditMesh>();
    }
}

// ── Mode keys (1=verts, 2=edges, 3=faces) ───────────────────────────────────

pub fn switch_select_mode(
    keys: Res<ButtonInput<KeyCode>>,
    input_focus: Res<InputFocusState>,
    mut sel: ResMut<MeshSelection>,
    edit_q: Query<&EditMesh>,
) {
    // Don't hijack the digit keys while the user is typing into a UI field.
    if input_focus.ui_wants_keyboard {
        return;
    }
    let new_mode = if keys.just_pressed(KeyCode::Digit1) {
        SelectMode::Vertex
    } else if keys.just_pressed(KeyCode::Digit2) {
        SelectMode::Edge
    } else if keys.just_pressed(KeyCode::Digit3) {
        SelectMode::Face
    } else {
        return;
    };
    if new_mode == sel.mode {
        return;
    }
    let old_mode = sel.mode;
    sel.mode = new_mode;
    // Flush the selection across modes (Blender-style): the old mode's
    // selection converts into the new mode's terms.
    let Some(edit) = sel.target.and_then(|t| edit_q.get(t).ok()) else {
        return;
    };
    convert_selection(edit, &mut sel, old_mode, new_mode);
}

/// Switch the element select mode from `&mut World` context (toolbar and
/// panel buttons), with the same selection flush the 1/2/3 keys perform.
pub fn set_select_mode(world: &mut World, new_mode: SelectMode) {
    world.resource_scope(|world, mut sel: Mut<MeshSelection>| {
        if sel.mode == new_mode {
            return;
        }
        let old = sel.mode;
        sel.mode = new_mode;
        if let Some(edit) = sel.target.and_then(|t| world.get::<EditMesh>(t)) {
            convert_selection(edit, &mut sel, old, new_mode);
        }
    });
}

/// Blender's up/down selection flush: down (verts of edges/faces) is a
/// straight expansion; up (edges/faces from verts) requires full coverage.
fn convert_selection(edit: &EditMesh, sel: &mut MeshSelection, from: SelectMode, to: SelectMode) {
    use crate::edit_mesh::{EdgeId, FaceId};
    // Derive the vert set from whatever the old mode had selected.
    let verts: std::collections::HashSet<u32> = match from {
        SelectMode::Vertex => sel.verts.iter().map(|v| v.0).collect(),
        SelectMode::Edge => sel
            .edges
            .iter()
            .filter_map(|e| edit.edges.get(e.0 as usize))
            .flat_map(|e| [e.verts[0].0, e.verts[1].0])
            .collect(),
        SelectMode::Face => sel
            .faces
            .iter()
            .filter_map(|f| edit.faces.get(f.0 as usize))
            .flat_map(|f| f.verts.iter().map(|v| v.0))
            .collect(),
    };
    match to {
        SelectMode::Vertex => {
            sel.verts = verts.into_iter().map(VertexId).collect();
        }
        SelectMode::Edge => {
            sel.edges = edit
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| verts.contains(&e.verts[0].0) && verts.contains(&e.verts[1].0))
                .map(|(i, _)| EdgeId(i as u32))
                .collect();
        }
        SelectMode::Face => {
            sel.faces = edit
                .faces
                .iter()
                .enumerate()
                .filter(|(_, f)| {
                    !f.verts.is_empty() && f.verts.iter().all(|v| verts.contains(&v.0))
                })
                .map(|(i, _)| FaceId(i as u32))
                .collect();
        }
    }
}

pub fn select_all_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    input_focus: Res<InputFocusState>,
    mut sel: ResMut<MeshSelection>,
    edit_q: Query<&EditMesh>,
) {
    if input_focus.ui_wants_keyboard {
        return;
    }
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
            SelectMode::Vertex => {
                sel.verts = (0..edit.vertices.len() as u32).map(VertexId).collect()
            }
            SelectMode::Edge => {
                sel.edges = (0..edit.edges.len() as u32)
                    .map(crate::edit_mesh::EdgeId)
                    .collect()
            }
            SelectMode::Face => {
                sel.faces = (0..edit.faces.len() as u32)
                    .map(crate::edit_mesh::FaceId)
                    .collect()
            }
        }
    }
}

// ── Picking ────────────────────────────────────────────────────────────────

pub fn pick_element(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    grab: Res<GrabState>,
    loop_cut: Res<crate::tools::LoopCutState>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    edit_q: Query<(&EditMesh, &GlobalTransform)>,
    editor_selection: Res<EditorSelection>,
    viewport_settings: Res<ViewportSettings>,
    mut sel: ResMut<MeshSelection>,
    mut active_tool: ResMut<ActiveTool>,
    mut commands: Commands,
    mut box_select: ResMut<MeshEditBoxSelect>,
) {
    // Busy or just-finished grabs / loop cuts own the mouse — a commit click
    // must not double as a pick.
    if !matches!(*grab, GrabState::Idle) {
        return;
    }
    if !matches!(*loop_cut, crate::tools::LoopCutState::Idle) {
        return;
    }
    let Some(target) = sel.target else {
        // No edit target → drop any stale marquee state and bail so the
        // Scene-mode picker can take over.
        box_select.state = MeshEditBoxSelectState::Idle;
        return;
    };
    let Ok((edit, gt)) = edit_q.get(target) else {
        return;
    };
    let Some(cursor_vp) = viewport_cursor(&viewport, &window_q) else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };

    let project =
        |p: Vec3| -> Option<Vec2> { camera.world_to_viewport(cam_gt, gt.transform_point(p)).ok() };
    let additive = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    // ── LMB JUST PRESSED ────────────────────────────────────────────────────
    //
    // Defer the commit to release. Any LMB-down arms the marquee state
    // machine with the press point as the anchor — clicking on a vertex
    // and dragging works just the same as clicking in empty space and
    // dragging (Blender's marquee-from-anywhere UX). On release:
    //   - cursor moved past `DRAG_THRESHOLD_PX` → commit the rectangle,
    //   - cursor stayed put → single-pick at the press anchor.
    // Running the hit-test on press (the old behaviour) broke the latter
    // case: a single click followed by even a 2-pixel jitter hit-stale'd
    // the marquee state and lost the underlying selection.
    if mouse.just_pressed(MouseButton::Left) {
        box_select.state = MeshEditBoxSelectState::Pressing {
            anchor_vp: cursor_vp,
            mode: sel.mode,
            additive,
            target,
        };
        return;
    }

    // ── WHILE PRESS HELD: promote Pressing → Marqueeing when dragging ─────
    if let MeshEditBoxSelectState::Pressing {
        anchor_vp,
        mode,
        additive,
        target,
    } = box_select.state
    {
        if (cursor_vp - anchor_vp).length() > DRAG_THRESHOLD_PX {
            box_select.state = MeshEditBoxSelectState::Marqueeing {
                anchor_vp,
                current_vp: cursor_vp,
                mode,
                additive,
                target,
            };
        }
        // Fall through. The release handler below still needs to run when
        // state is `Pressing`: a quick click (no drag) is supposed to
        // commit the single-pick at the press anchor. An unconditional
        // `return` here (as in the previous build) made the picker
        // silently lose every no-drag click.
    }

    // ── WHILE DRAGGING: refresh rect (drawing is handled by overlay_ui::update_marquee)
    if let MeshEditBoxSelectState::Marqueeing {
        anchor_vp,
        current_vp,
        mode,
        additive,
        target,
    } = box_select.state
    {
        // Refresh the current corner every frame so the rubber-band tracks
        // the cursor smoothly. Drop the existing value via replace.
        // Rendering happens in `overlay_ui::update_marquee` — gizmo
        // `line_2d` calls go through the 3D pipeline and vanish at the
        // world's `z = 0` plane.
        box_select.state = MeshEditBoxSelectState::Marqueeing {
            anchor_vp,
            current_vp: cursor_vp,
            mode,
            additive,
            target,
        };
    }

    // ── LMB JUST RELEASED ───────────────────────────────────────────────────
    //
    // Releases the marquee commit (if the press became a drag), or the
    // historical "click outside the mesh releases the edit target" path
    // (if the press stayed a click).
    if mouse.just_released(MouseButton::Left) {
        match box_select.state {
            MeshEditBoxSelectState::Idle => {}
            MeshEditBoxSelectState::Pressing {
                mode,
                additive,
                anchor_vp,
                target: tgt,
            } => {
                // Cursor stayed inside `DRAG_THRESHOLD_PX` of the press.
                // Defer-from-press: run the hit-test now, against the press
                // anchor so a tiny release-time wiggle doesn't miss the
                // element the user was targeting.
let hit_any = match mode {
                    SelectMode::Vertex => {
                        if viewport_settings.mesh_edit_xray_select {
                            // X-ray: closest vertex in *screen space* within
                            // the pick radius — back-side vertices can be
                            // selected when their projected position is
                            // closer to the cursor than the near-side ones.
                            let mut best: Option<(f32, VertexId)> = None;
                            for (i, v) in edit.vertices.iter().enumerate() {
                                if let Some(sp) = project(v.position) {
                                    let d = (sp - anchor_vp).length();
                                    if d <= PICK_RADIUS_PX_VERTEX
                                        && best.is_none_or(|(bd, _)| d < bd)
                                    {
                                        best = Some((d, VertexId(i as u32)));
                                    }
                                }
                            }
                            let hit = best.is_some();
                            apply_pick(
                                &mut sel.verts,
                                best.map(|(_, id)| id),
                                additive,
                            );
                            hit
                        } else {
                            // Depth-tested (default): find the vertex
                            // closest to the camera, then check it's within
                            // the pick radius. Stops clicks on a sphere's
                            // silhouette from accidentally selecting the
                            // back-side vertex that happens to project
                            // onto the same screen point.
                            //
                            // Camera view-space depth = `-view_pos.z`
                            // (camera looks down -Z; larger Z = further
                            // back).
                            let mut best: Option<(f32, f32, VertexId)> = None;
                            for (i, v) in edit.vertices.iter().enumerate() {
                                let Some(sp) = project(v.position) else {
                                    continue;
                                };
                                let world_pos = gt.transform_point(v.position);
                                let view_pos = cam_gt
                                    .affine()
                                    .inverse()
                                    .transform_point3(world_pos);
                                let depth = -view_pos.z;
                                let d_screen = (sp - anchor_vp).length();
                                if best.is_none_or(|(_, bd, _)| depth < bd) {
                                    best = Some((
                                        d_screen,
                                        depth,
                                        VertexId(i as u32),
                                    ));
                                }
                            }
                            let picked = best.and_then(
                                |(d_screen, depth, id)| {
                                    if d_screen <= PICK_RADIUS_PX_VERTEX
                                        && depth > 0.0
                                    {
                                        Some(id)
                                    } else {
                                        None
                                    }
                                },
                            );
                            let hit = picked.is_some();
                            apply_pick(&mut sel.verts, picked, additive);
                            hit
                        }
                    }
                    SelectMode::Edge => {
                        let mut best: Option<(f32, crate::edit_mesh::EdgeId)> = None;
                        for (i, e) in edit.edges.iter().enumerate() {
                            let Some(a) = edit
                                .vertices
                                .get(e.verts[0].0 as usize)
                                .and_then(|v| project(v.position))
                            else {
                                continue;
                            };
                            let Some(b) = edit
                                .vertices
                                .get(e.verts[1].0 as usize)
                                .and_then(|v| project(v.position))
                            else {
                                continue;
                            };
                            let d = point_to_segment(anchor_vp, a, b);
                            if d <= PICK_RADIUS_PX_EDGE
                                && best.is_none_or(|(bd, _)| d < bd)
                            {
                                best = Some((d, crate::edit_mesh::EdgeId(i as u32)));
                            }
                        }
                        let hit = best.is_some();
                        apply_pick(&mut sel.edges, best.map(|(_, id)| id), additive);
                        hit
                    }
                    SelectMode::Face => {
                        let Some((ray_origin, ray_dir)) =
                            build_world_ray(camera, cam_gt, anchor_vp, &viewport)
                        else {
                            // Without a ray we can't do a face hit-test at
                            // all; drop out without committing anything.
                            box_select.state = MeshEditBoxSelectState::Idle;
                            return;
                        };
                        let inv = gt.to_matrix().inverse();
                        let local_origin = inv.transform_point3(ray_origin);
                        let local_dir = inv.transform_vector3(ray_dir).normalize_or_zero();
                        let mut best: Option<(f32, crate::edit_mesh::FaceId)> = None;
                        for (i, f) in edit.faces.iter().enumerate() {
                            if f.verts.len() < 3 {
                                continue;
                            }
                            let p0 = edit.vertices[f.verts[0].0 as usize].position;
                            for w in f.verts.windows(2).skip(1) {
                                let p1 = edit.vertices[w[0].0 as usize].position;
                                let p2 = edit.vertices[w[1].0 as usize].position;
                                if let Some(t) =
                                    ray_triangle(local_origin, local_dir, p0, p1, p2)
                                {
                                    if best.is_none_or(|(bt, _)| t < bt) {
                                        best =
                                            Some((t, crate::edit_mesh::FaceId(i as u32)));
                                    }
                                }
                            }
                        }
                        // Select exactly the hit `FaceId`. `EditMesh` already represents
                        // logical faces — imported triangle pairs are
                        // merged into quads by
                        // `merge_coplanar_triangle_pairs`, and extrusion
                        // / loop-cut produce separate bounded faces. An
                        // edge between two coplanar faces is a real
                        // topological boundary, so we don't
                        // flood-fill through it. Blender-style.
                        let hit = best.map(|(_, face_id)| face_id);
                        commit_face_pick(&mut sel.faces, hit, additive)
                    }
                };

                if !hit_any && !additive {
                    // Click landed outside any element and the user wasn't
                    // holding Shift → release the edit target so they can
                    // pick a different mesh to edit. Entity picking takes
                    // over next frame once enter_edit_mode sees no
                    // selection. Shift-click in empty space keeps the
                    // current target (additive is for additive selection
                    // mod, not for 'release mesh' mod).
                    sel.target = None;
                    sel.clear();
                    commands.entity(tgt).remove::<EditMesh>();
                    editor_selection.set(None);
                    if *active_tool == ActiveTool::None {
                        *active_tool = ActiveTool::Select;
                    }
                }
                box_select.state = MeshEditBoxSelectState::Idle;
            }
            MeshEditBoxSelectState::Marqueeing {
                anchor_vp,
                current_vp,
                mode,
                additive,
                target: _,
            } => {
                let min = Vec2::new(anchor_vp.x.min(current_vp.x), anchor_vp.y.min(current_vp.y));
                let max = Vec2::new(anchor_vp.x.max(current_vp.x), anchor_vp.y.max(current_vp.y));
                let inside = |p: Vec2| p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y;

                match mode {
                    SelectMode::Vertex => {
                        if !additive {
                            sel.verts.clear();
                        }
                        for (i, v) in edit.vertices.iter().enumerate() {
                            if let Some(sp) = project(v.position) {
                                if inside(sp) {
                                    let id = VertexId(i as u32);
                                    if !sel.verts.insert(id) && additive {
                                        sel.verts.remove(&id);
                                    }
                                }
                            }
                        }
                    }
                    SelectMode::Edge => {
                        if !additive {
                            sel.edges.clear();
                        }
                        // Edge inside the rect when its midpoint is inside
                        // (Blender's "fully enclosed" rule). This avoids
                        // surface-area arguments for short edges and gives
                        // the same visual result users expect from a 3D
                        // viewport marquee.
                        for (i, e) in edit.edges.iter().enumerate() {
                            let Some(a) = edit
                                .vertices
                                .get(e.verts[0].0 as usize)
                                .and_then(|v| project(v.position))
                            else {
                                continue;
                            };
                            let Some(b) = edit
                                .vertices
                                .get(e.verts[1].0 as usize)
                                .and_then(|v| project(v.position))
                            else {
                                continue;
                            };
                            let mid = (a + b) * 0.5;
                            if inside(mid) {
                                let id = crate::edit_mesh::EdgeId(i as u32);
                                if !sel.edges.insert(id) && additive {
                                    sel.edges.remove(&id);
                                }
                            }
                        }
                    }
                    SelectMode::Face => {
                        if !additive {
                            sel.faces.clear();
                        }
                        // Face inside when its centroid (vertex average) is
                        // inside the rect. Cheap and stable for convex
                        // faces; long thin faces occasionally slip through,
                        // matching Blender's behaviour closely enough.
                        for (i, f) in edit.faces.iter().enumerate() {
                            if f.verts.len() < 3 {
                                continue;
                            }
                            let mut centroid_local = Vec3::ZERO;
                            for vid in &f.verts {
                                if let Some(v) = edit.vertices.get(vid.0 as usize) {
                                    centroid_local += v.position;
                                }
                            }
                            centroid_local /= f.verts.len() as f32;
                            if let Some(sp) = project(centroid_local) {
                                if inside(sp) {
                                    let id = crate::edit_mesh::FaceId(i as u32);
                                    if !sel.faces.insert(id) && additive {
                                        sel.faces.remove(&id);
                                    }
                                }
                            }
                        }
                    }
                }
                box_select.state = MeshEditBoxSelectState::Idle;
            }
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

/// Commit a single FaceId hit to the face-selection set. Extracted
/// from `pick_element`'s Face branch so it can be unit-tested without
/// spinning up a Bevy `App`.
///
/// Behaviour:
/// - `additive == false`, `hit == Some(f)` → clear the set, insert `f`
/// - `additive == true`,  `hit == Some(f)` → toggle `f`
/// - `additive == false`, `hit == None`    → clear the set
/// - `additive == true`,  `hit == None`    → no-op (preserve)
///
/// Critically, this only operates on the single `FaceId` passed in. It
/// does **not** flood-fill through coplanar neighbours — that was a
/// 2026-08-25 mistake (`coplanar_group`) that bypassed the data
/// model's bounded-face semantics. Imported triangle pairs are merged
/// into quads at bake; extruded quads are separate. An edge between
/// two coplanar faces is a real topological boundary and is treated as
/// one.
///
/// Returns `true` if the hit was non-empty (something was actually
/// committed), `false` otherwise — used by the picker to set the
/// `hit_any` flag for the empty-space-clears-target path.
pub(crate) fn commit_face_pick(
    selection: &mut std::collections::HashSet<crate::edit_mesh::FaceId>,
    hit: Option<crate::edit_mesh::FaceId>,
    additive: bool,
) -> bool {
    if !additive {
        selection.clear();
    }
    if let Some(face_id) = hit {
        if additive && !selection.insert(face_id) {
            selection.remove(&face_id);
        } else {
            selection.insert(face_id);
        }
    }
    hit.is_some()
}

// ── Extrude (E) ────────────────────────────────────────────────────────────

pub fn extrude_system(
    keys: Res<ButtonInput<KeyCode>>,
    input_focus: Res<InputFocusState>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut edit_q: Query<(&mut EditMesh, &GlobalTransform)>,
    mut sel: ResMut<MeshSelection>,
    mut grab: ResMut<GrabState>,
    mut commands: Commands,
) {
    if input_focus.ui_wants_keyboard {
        return;
    }
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    if !matches!(*grab, GrabState::Idle) {
        return;
    }
    let Some(target) = sel.target else { return };
    let Ok((mut edit, gt)) = edit_q.get_mut(target) else {
        return;
    };

    let before = edit.clone();
    let before_sel = SelectionSnapshot::from_selection(&sel);

    let Some(result) = operators::extrude(&mut edit, &sel) else {
        return;
    };

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
        renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
    });

    // Seed a grab so the user can immediately drag the new geometry.
    // Use face normal as the locked axis when available; otherwise
    // fall back to view-plane translation.
    let Some(cursor_vp) = viewport_cursor(&viewport, &window_q) else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };
    let Some((ray_origin, ray_dir)) = build_world_ray(camera, cam_gt, cursor_vp, &viewport) else {
        return;
    };

    let starts: Vec<(u32, Vec3)> = result
        .new_verts
        .iter()
        .map(|&id| (id, edit.vertices[id as usize].position))
        .collect();
    if starts.is_empty() {
        return;
    }

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
        let anchor =
            ray_plane(ray_origin, ray_dir, centroid_world, normal).unwrap_or(centroid_world);
        (None, anchor, normal)
    };

    *grab = GrabState::Active {
        anchor_world,
        plane_normal,
        plane_point: centroid_world,
        axis,
        starts,
        // Symmetry doesn't apply to extrude-seeded grabs: the mirrored side
        // would need its own extrusion, which the op didn't create.
        mirror_starts: Vec::new(),
        seeded_by_op: true,
    };
}

// ── Grab (G) — translate selected verts on the view plane ──────────────────

#[derive(Resource, Default)]
pub enum GrabState {
    #[default]
    Idle,
    /// A grab committed or cancelled this frame. Blocks `pick_element`
    /// (which runs later in the chain) from also consuming the same click;
    /// `grab_update` converts it back to `Idle` next frame.
    JustFinished,
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
        /// X-symmetry partners: verts mirroring the grabbed ones across the
        /// local X plane. They receive the delta with `x` negated.
        mirror_starts: Vec<(u32, Vec3)>,
        /// True when this grab was seeded by a topology op (extrude, inset,
        /// etc.) that already pushed a snapshot undo command. Cancelling
        /// must roll that op back by popping it off the undo stack.
        seeded_by_op: bool,
    },
}

/// Find the X-symmetry partner verts for a set of grabbed verts: the vert
/// nearest each grabbed vert's mirrored position (within tolerance) that
/// isn't itself grabbed. Verts sitting on the mirror plane pair with nothing.
pub(crate) fn mirror_partners(edit: &EditMesh, grabbed: &[(u32, Vec3)]) -> Vec<(u32, Vec3)> {
    const EPS: f32 = 1e-4;
    let grabbed_set: std::collections::HashSet<u32> = grabbed.iter().map(|(i, _)| *i).collect();
    let mut out: Vec<(u32, Vec3)> = Vec::new();
    let mut taken: std::collections::HashSet<u32> = Default::default();
    for (_, pos) in grabbed {
        if pos.x.abs() <= EPS {
            continue;
        }
        let mirrored = Vec3::new(-pos.x, pos.y, pos.z);
        let found = edit
            .vertices
            .iter()
            .enumerate()
            .filter(|(i, _)| !grabbed_set.contains(&(*i as u32)) && !taken.contains(&(*i as u32)))
            .find(|(_, v)| v.position.distance_squared(mirrored) < EPS * EPS);
        if let Some((i, v)) = found {
            taken.insert(i as u32);
            out.push((i as u32, v.position));
        }
    }
    out
}

pub fn grab_start(
    keys: Res<ButtonInput<KeyCode>>,
    input_focus: Res<InputFocusState>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    edit_q: Query<(&EditMesh, &GlobalTransform)>,
    sel: Res<MeshSelection>,
    modeling: Res<crate::tools::ModelingSettings>,
    mut grab: ResMut<GrabState>,
) {
    if input_focus.ui_wants_keyboard {
        return;
    }
    if !keys.just_pressed(KeyCode::KeyG) {
        return;
    }
    if !matches!(*grab, GrabState::Idle) {
        return;
    }
    let Some(target) = sel.target else { return };
    let Ok((edit, gt)) = edit_q.get(target) else {
        return;
    };
    let vert_ids = selected_vert_ids(edit, &sel);
    if vert_ids.is_empty() {
        return;
    }

    let Some(cursor_vp) = viewport_cursor(&viewport, &window_q) else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };
    let Some((ray_origin, ray_dir)) = build_world_ray(camera, cam_gt, cursor_vp, &viewport) else {
        return;
    };

    // Plane through the selection centroid, facing the camera.
    let centroid_local: Vec3 = vert_ids
        .iter()
        .map(|&id| edit.vertices[id as usize].position)
        .sum::<Vec3>()
        / vert_ids.len() as f32;
    let centroid_world = gt.transform_point(centroid_local);
    let normal = -cam_gt.forward().as_vec3();
    let Some(hit) = ray_plane(ray_origin, ray_dir, centroid_world, normal) else {
        return;
    };

    let starts: Vec<(u32, Vec3)> = vert_ids
        .iter()
        .map(|&id| (id, edit.vertices[id as usize].position))
        .collect();
    let mirror_starts = if modeling.symmetry_x {
        mirror_partners(edit, &starts)
    } else {
        Vec::new()
    };

    *grab = GrabState::Active {
        anchor_world: hit,
        plane_normal: normal,
        plane_point: centroid_world,
        axis: None,
        starts,
        mirror_starts,
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
    // One-frame cooldown after a commit/cancel so the click that ended the
    // grab isn't reinterpreted by later systems this frame.
    if matches!(*grab, GrabState::JustFinished) {
        *grab = GrabState::Idle;
        return;
    }
    let (
        mut anchor_world,
        plane_normal,
        plane_point,
        mut axis,
        starts,
        mirror_starts,
        seeded_by_op,
    ) = match &*grab {
        GrabState::Active {
            anchor_world,
            plane_normal,
            plane_point,
            axis,
            starts,
            mirror_starts,
            seeded_by_op,
        } => (
            *anchor_world,
            *plane_normal,
            *plane_point,
            *axis,
            starts.clone(),
            mirror_starts.clone(),
            *seeded_by_op,
        ),
        _ => return,
    };
    let Some(target) = sel.target else {
        *grab = GrabState::Idle;
        return;
    };
    let Ok((mut edit, gt)) = edit_q.get_mut(target) else {
        *grab = GrabState::Idle;
        return;
    };

    // Cancel (RMB or Esc): restore and exit. If the grab was seeded by a
    // topology op (extrude, inset, …) we also need to roll that op back,
    // since simply restoring vertex positions would leave zero-length
    // duplicated geometry behind.
    if mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) {
        if seeded_by_op {
            commands.queue(|world: &mut World| {
                renzora_undo::undo_once(world);
            });
        } else {
            for (id, start) in starts.iter().chain(mirror_starts.iter()) {
                edit.vertices[*id as usize].position = *start;
            }
            edit.dirty = true;
        }
        *grab = GrabState::JustFinished;
        return;
    }

    // Commit (LMB). Record an undo command with the net per-vertex deltas.
    if mouse.just_pressed(MouseButton::Left) {
        let deltas: Vec<(u32, Vec3, Vec3)> = starts
            .iter()
            .chain(mirror_starts.iter())
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
            let cmd = crate::undo::VertexMoveCmd {
                entity: target,
                deltas,
            };
            commands.queue(move |world: &mut World| {
                renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
            });
        }
        edit.dirty = true;
        *grab = GrabState::JustFinished;
        return;
    }

    // Drag.
    let Some(cursor_vp) = viewport_cursor(&viewport, &window_q) else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };
    let Some((ray_origin, ray_dir)) = build_world_ray(camera, cam_gt, cursor_vp, &viewport) else {
        return;
    };

    // Axis-constraint keys (tap to lock, tap same key again to release).
    let axis_toggle = if keys.just_pressed(KeyCode::KeyX) {
        Some(Vec3::X)
    } else if keys.just_pressed(KeyCode::KeyY) {
        Some(Vec3::Y)
    } else if keys.just_pressed(KeyCode::KeyZ) {
        Some(Vec3::Z)
    } else {
        None
    };
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
            anchor_world,
            plane_normal,
            plane_point,
            axis,
            starts: starts.clone(),
            mirror_starts: mirror_starts.clone(),
            seeded_by_op,
        };
        // Snap verts back to their start — subsequent frames will move
        // along the new constraint from zero.
        for (id, start) in starts.iter().chain(mirror_starts.iter()) {
            edit.vertices[*id as usize].position = *start;
        }
        edit.dirty = true;
        return;
    }

    let delta_world = if let Some(a) = axis {
        let Some(hit) = closest_point_on_line(plane_point, a, ray_origin, ray_dir) else {
            return;
        };
        hit - anchor_world
    } else {
        let Some(hit) = ray_plane(ray_origin, ray_dir, plane_point, plane_normal) else {
            return;
        };
        hit - anchor_world
    };

    // Convert world delta into the edit mesh's local space.
    let inv_rot = gt.affine().matrix3.inverse();
    let delta_local = inv_rot * delta_world;
    for (id, start) in &starts {
        edit.vertices[*id as usize].position = *start + delta_local;
    }
    // Symmetry partners take the delta with X flipped.
    let delta_mirror = Vec3::new(-delta_local.x, delta_local.y, delta_local.z);
    for (id, start) in &mirror_starts {
        edit.vertices[*id as usize].position = *start + delta_mirror;
    }
    edit.dirty = true;
}

// ── Bake on dirty ──────────────────────────────────────────────────────────

pub fn bake_if_dirty(
    mut meshes: ResMut<Assets<Mesh>>,
    mut edit_q: Query<(Entity, &mut EditMesh, &Mesh3d)>,
    mut commands: Commands,
) {
    for (entity, mut edit, mesh3d) in &mut edit_q {
        if !edit.dirty {
            continue;
        }
        if let Some(mut mesh) = meshes.get_mut(&mesh3d.0) {
            edit.bake_to_mesh(&mut mesh);
            // Persist the edit: scene saves carry the geometry, and the
            // rehydrator rebuilds it on load instead of the pristine
            // primitive / glTF source. The Applied marker keeps the
            // rehydrator's hands off while we're live-editing.
            if let Some(snapshot) = renzora::core::EditedMesh::from_mesh(&mesh) {
                commands
                    .entity(entity)
                    .try_insert((snapshot, renzora::core::EditedMeshApplied));
            }
        }
        edit.dirty = false;
    }
}

// ── Overlay ────────────────────────────────────────────────────────────────

pub fn draw_overlay(
    mesh_selection: Res<MeshSelection>,
    edit_q: Query<(&EditMesh, &GlobalTransform)>,
    camera_q: Query<(&Camera, &GlobalTransform, &Projection), With<EditorCamera>>,
    mut gizmos: Gizmos,
) {
    let Some(target) = mesh_selection.target else {
        return;
    };
    let Ok((edit, gt)) = edit_q.get(target) else {
        return;
    };
    let to_world = |v: Vec3| gt.transform_point(v);

    // Camera + viewport pixel size for the screen-space vertex dots. Either
    // missing → no dots (Vertex mode is 3D-only anyway, but stay defensive if
    // the editor camera hasn't spawned).
    let Ok((_camera, cam_gt, projection)) = camera_q.single() else {
        return;
    };

    // Edges: faint white unless selected.
    for (i, edge) in edit.edges.iter().enumerate() {
        let a = edit
            .vertices
            .get(edge.verts[0].0 as usize)
            .map(|v| v.position);
        let b = edit
            .vertices
            .get(edge.verts[1].0 as usize)
            .map(|v| v.position);
        let (Some(a), Some(b)) = (a, b) else { continue };
        let selected = mesh_selection.mode == SelectMode::Edge
            && mesh_selection
                .edges
                .contains(&crate::edit_mesh::EdgeId(i as u32));
        let color = if selected {
            Color::srgb(1.0, 0.55, 0.1)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.35)
        };
        gizmos.line(to_world(a), to_world(b), color);
    }

    // Vertex dots are rendered via `overlay_ui::update_vertex_dots` —
    // Gizmos `rect_2d`/`line_2d` go through the 3D pipeline at world
    // `(x, y, 0)` (see `bevy_gizmos-0.19.1/src/gizmos.rs:804`), so they
    // vanish inside a perspective viewport. The overlay owns a UI Node
    // tree parented under the viewport panel and positions each dot in
    // true screen pixels.

    // Face perimeter outline — drawn at full alpha on top of the
    // translucent fill that `overlay_ui::update_face_overlays` spawns.
    // The fill is a real 3D mesh (good for the tinted look); the outline
    // is a gizmo line-list (cheaper, and reads sharper than a mesh edge).
    if mesh_selection.mode == SelectMode::Face {
        let outline_color = Color::srgba(1.0, 0.55, 0.1, 0.9);
        for (i, face) in edit.faces.iter().enumerate() {
            if !mesh_selection
                .faces
                .contains(&crate::edit_mesh::FaceId(i as u32))
            {
                continue;
            }
            let n = face.verts.len();
            if n < 3 {
                continue;
            }
            // Perimeter (last → first) at full alpha. The mesh-fill handles
            // the interior, so we only need the boundary.
            for w in face.verts.windows(2) {
                let (Some(a), Some(b)) = (
                    edit.vertices.get(w[0].0 as usize).map(|v| v.position),
                    edit.vertices.get(w[1].0 as usize).map(|v| v.position),
                ) else {
                    continue;
                };
                gizmos.line(to_world(a), to_world(b), outline_color);
            }
            // Close the loop.
            if let (Some(&first), Some(&last)) = (face.verts.first(), face.verts.last()) {
                if let (Some(a), Some(b)) = (
                    edit.vertices.get(last.0 as usize).map(|v| v.position),
                    edit.vertices.get(first.0 as usize).map(|v| v.position),
                ) {
                    gizmos.line(to_world(a), to_world(b), outline_color);
                }
            }
        }
    }
}

/// World units per on-screen panel pixel at the vertex's distance from the
/// camera. Perspective: `(2·d·tan(fov/2)) / vp_px_height`. Orthographic:
/// `ortho.scale`. We only need this for vertex dots (small markers) so a
/// flat Euclidean distance is good enough — for a strictly on-axis vertex it
/// matches the camera-plane depth exactly.
fn world_per_pixel(dist_to_camera: f32, projection: &Projection, vp_px_height: f32) -> f32 {
    let h = vp_px_height.max(1.0);
    match projection {
        Projection::Perspective(p) => {
            let half = (p.fov * 0.5).tan();
            2.0 * dist_to_camera.max(0.001) * half / h
        }
        Projection::Orthographic(o) => o.scale,
        _ => dist_to_camera.max(0.001) * 0.05,
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

pub(crate) fn viewport_cursor(
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

pub(crate) fn ray_triangle(origin: Vec3, dir: Vec3, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<f32> {
    // Möller–Trumbore.
    let e1 = v1 - v0;
    let e2 = v2 - v0;
    let h = dir.cross(e2);
    let a = e1.dot(h);
    if a.abs() < 1e-6 {
        return None;
    }
    let f = 1.0 / a;
    let s = origin - v0;
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = f * dir.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = f * e2.dot(q);
    if t < 0.0 {
        return None;
    }
    Some(t)
}

pub(crate) fn point_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

#[cfg(test)]
mod tests {
    use super::commit_face_pick;
    use crate::edit_mesh::FaceId;
    use std::collections::HashSet;

    fn empty_selection() -> HashSet<FaceId> {
        HashSet::new()
    }

    /// The Face picker must commit **exactly** the hit `FaceId`. Earlier
    /// code (`coplanar_group`) flood-filled to every coplanar triangle
    /// reachable through shared edges — that bypassed the data model's
    /// bounded-face semantics (imported quads are already merged; an
    /// edge between two coplanar extruded quads is a real topological
    /// boundary). Blender-style face selection treats each bounded
    /// face independently.

    #[test]
    fn coplanar_faces_remain_separately_selectable() {
        // Two coplanar quads sharing an edge (e.g. a subdivided plane,
        // or a face that was extruded and re-adjacent to a neighbour
        // on the same plane). After committing face A, the selection
        // must contain exactly A — never B.
        let mut s = empty_selection();
        let face_a = FaceId(0);
        let face_b = FaceId(1);
        assert!(commit_face_pick(&mut s, Some(face_a), false));
        assert_eq!(s.len(), 1, "selection must hold exactly the hit face");
        assert!(s.contains(&face_a));
        assert!(
            !s.contains(&face_b),
            "committing face A must NOT flood-fill to coplanar face B"
        );

        // Re-commit B non-additively: selection becomes just B.
        assert!(commit_face_pick(&mut s, Some(face_b), false));
        assert_eq!(s.len(), 1);
        assert!(s.contains(&face_b));
        assert!(!s.contains(&face_a));
    }

    #[test]
    fn extruded_cube_top_does_not_select_bottom() {
        // After extruding a cube, the top face and bottom face are two
        // separate bounded quads that happen to be coplanar to their
        // respective inner parallels and share a vertical axis. Clicking
        // one must not include the other.
        let mut s = empty_selection();
        let top_face = FaceId(5); // first face of the extruded top
        let bottom_face = FaceId(0); // first face of the original bottom
        assert!(commit_face_pick(&mut s, Some(top_face), false));
        assert_eq!(s.len(), 1);
        assert!(s.contains(&top_face));
        assert!(
            !s.contains(&bottom_face),
            "clicking the top face must NOT include the bottom face"
        );
    }

    #[test]
    fn shift_click_toggles_only_the_clicked_face() {
        let mut s = empty_selection();
        let a = FaceId(0);
        let b = FaceId(1);

        // Normal click on A.
        assert!(commit_face_pick(&mut s, Some(a), false));
        assert_eq!(s.len(), 1);
        assert!(s.contains(&a));

        // Shift-click on A: toggles off.
        assert!(commit_face_pick(&mut s, Some(a), true));
        assert!(s.is_empty(), "shift-click on selected face must toggle off");

        // Shift-click on B: adds B.
        assert!(commit_face_pick(&mut s, Some(b), true));
        assert!(s.contains(&b));
        assert!(
            !s.contains(&a),
            "shift-click on B must NOT touch the unrelated face A"
        );

        // Normal click on A: clears the set, adds only A.
        assert!(commit_face_pick(&mut s, Some(a), false));
        assert_eq!(s.len(), 1);
        assert!(s.contains(&a));
        assert!(
            !s.contains(&b),
            "normal click must clear the set, not add to it"
        );
    }

    #[test]
    fn imported_cube_merged_quad_selects_as_one_whole_face() {
        // Imported cube has 8 verts and 6 quads after
        // `merge_coplanar_triangle_pairs` runs at bake. Clicking any
        // single quad must select only that quad — the two triangle
        // halves of the quad have already been merged into one
        // `Face` by bake, so there's nothing to "expand" here.
        let mut s = empty_selection();
        let quad = FaceId(2); // pick one of the 6 cube quads
        assert!(commit_face_pick(&mut s, Some(quad), false));
        assert_eq!(s.len(), 1);
        assert!(s.contains(&quad));
    }

    #[test]
    fn empty_hit_clears_when_not_additive() {
        let mut s = empty_selection();
        s.insert(FaceId(5));
        assert!(!commit_face_pick(&mut s, None, false));
        assert!(s.is_empty());
    }

    #[test]
    fn empty_hit_preserves_when_additive() {
        let mut s = empty_selection();
        s.insert(FaceId(5));
        assert!(!commit_face_pick(&mut s, None, true));
        assert_eq!(s.len(), 1);
        assert!(s.contains(&FaceId(5)));
    }
}

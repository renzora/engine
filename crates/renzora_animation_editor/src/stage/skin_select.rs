//! Skin mode's Select tool: picking vertices, and giving them exactly the
//! weights wanted.
//!
//! Automatic weights are a guess, and some guesses are wrong for a model no
//! rule foresaw: a tooth caught between the jaw and the head, a buckle that
//! should not bend, a hat that should follow only the head. Painting fixes a
//! region roughly; this fixes it exactly.
//!
//! - **Click** a part of the model to select the whole piece under the
//!   cursor (every vertex joined to it by triangles): one tooth, an eyeball,
//!   a button. **Drag** to select every vertex inside the box, through the
//!   model. Shift adds to the selection, Ctrl takes away.
//! - **Bind Rigidly** gives the selection wholly to the paint bone, so it
//!   moves with that bone as one solid thing. This is how a part is attached
//!   to a bone.
//! - **Set Weight** gives the selection the chosen share of the paint bone,
//!   the rest staying with the bones it already had.
//! - **Auto Weights** on the selection only, leaving the rest as it is.
//!
//! Each is one undo step, and Save writes it into the model like painting.

use std::collections::HashSet;

use bevy::prelude::*;

use renzora_rig::weights::{self, Influences};

use super::skin::{ray_triangle, skinned_positions, SkinEdit, WeightsCmd};
use super::view::{StageCursor, StageNav};
use super::StageMode;
use crate::rig::tell;
use crate::studio_preview::StudioPreviewCamera;

/// A press of the Select tool, until it is released.
#[derive(Resource, Default)]
struct Press {
    start: Option<Vec2>,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<Press>().add_systems(
        Update,
        (select, draw_box)
            .after(super::view::StageInputSet)
            .run_if(in_state(renzora_editor_framework::SplashState::Editor))
            .run_if(super::stage_mounted)
            .run_if(renzora::not_in_play_mode),
    );
}

/// Pixels a press must move before it is a box rather than a click.
const DRAG: f32 = 4.0;

fn select(world: &mut World) {
    if super::mode(world) != StageMode::Skin || !world.resource::<SkinEdit>().active() || !world.resource::<SkinEdit>().select_tool {
        world.resource_mut::<Press>().start = None;
        return;
    }
    let (over, px, ray) = {
        let c = world.resource::<StageCursor>();
        (c.over, c.px, c.ray)
    };
    let (pressed, released) = {
        let m = world.resource::<ButtonInput<MouseButton>>();
        (m.just_pressed(MouseButton::Left), m.just_released(MouseButton::Left))
    };
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if pressed && over && world.resource::<StageNav>().dragging.is_none() {
        world.resource_mut::<Press>().start = Some(px);
        return;
    }
    if !released {
        return;
    }
    let Some(start) = world.resource_mut::<Press>().start.take() else { return };

    // What the press picked, per mesh.
    let picked: Vec<Vec<usize>> = if start.distance(px) > DRAG {
        let Some((cam, cam_tf)) = world
            .try_query_filtered::<(&Camera, &GlobalTransform), With<StudioPreviewCamera>>()
            .and_then(|mut q| q.iter(world).next().map(|(c, t)| (c.clone(), *t)))
        else {
            return;
        };
        let (lo, hi) = (start.min(px), start.max(px));
        let edit = world.resource::<SkinEdit>();
        edit.meshes
            .iter()
            .map(|m| {
                skinned_positions(world, m)
                    .iter()
                    .enumerate()
                    .filter_map(|(i, p)| cam.world_to_viewport(&cam_tf, *p).ok().filter(|s| s.cmpge(lo).all() && s.cmple(hi).all()).map(|_| i))
                    .collect()
            })
            .collect()
    } else {
        let Some(ray) = ray else { return };
        piece_under(world, ray)
    };
    let mut edit = world.resource_mut::<SkinEdit>();
    for (m, picks) in edit.meshes.iter_mut().zip(&picked) {
        if !shift && !ctrl {
            m.selected.iter_mut().for_each(|s| *s = false);
        }
        for &i in picks {
            if let Some(s) = m.selected.get_mut(i) {
                *s = !ctrl;
            }
        }
    }
    edit.mark_dirty();
}

/// Every vertex of the piece under the ray: the triangle hit first, and
/// everything joined to it by triangles.
fn piece_under(world: &World, ray: Ray3d) -> Vec<Vec<usize>> {
    let edit = world.resource::<SkinEdit>();
    let posed: Vec<Vec<Vec3>> = edit.meshes.iter().map(|m| skinned_positions(world, m)).collect();
    let mut best: Option<(f32, usize, u32)> = None;
    for (mi, (m, pos)) in edit.meshes.iter().zip(&posed).enumerate() {
        for tri in m.indices.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (pos.get(tri[0] as usize), pos.get(tri[1] as usize), pos.get(tri[2] as usize)) else { continue };
            if let Some(t) = ray_triangle(ray, *a, *b, *c) {
                if best.is_none_or(|(bt, ..)| t < bt) {
                    best = Some((t, mi, tri[0]));
                }
            }
        }
    }
    let mut out = vec![Vec::new(); edit.meshes.len()];
    if let Some((_, mi, start)) = best {
        out[mi] = connected(&edit.meshes[mi].indices, edit.meshes[mi].positions.len(), start as usize);
    }
    out
}

/// The vertices joined to `start` by triangles.
fn connected(indices: &[u32], count: usize, start: usize) -> Vec<usize> {
    let adjacency = weights::adjacency(indices, count);
    let mut seen: HashSet<usize> = HashSet::from([start]);
    let mut stack = vec![start];
    while let Some(v) = stack.pop() {
        for &n in adjacency.get(v).into_iter().flatten() {
            if seen.insert(n as usize) {
                stack.push(n as usize);
            }
        }
    }
    let mut out: Vec<usize> = seen.into_iter().collect();
    out.sort_unstable();
    out
}

/// The box being dragged, drawn on the stage.
fn draw_box(press: Res<Press>, cursor: Res<StageCursor>, edit: Res<SkinEdit>, cams: Query<(&Camera, &GlobalTransform), With<StudioPreviewCamera>>, mut gizmos: Gizmos<crate::studio_preview::StudioPreviewGizmoGroup>) {
    let (Some(start), true) = (press.start, edit.select_tool) else { return };
    if start.distance(cursor.px) <= DRAG {
        return;
    }
    let Ok((cam, tf)) = cams.single() else { return };
    // The box's corners, a little in front of the camera.
    let at = |p: Vec2| cam.viewport_to_world(tf, p).ok().map(|r| r.origin + *r.direction * 0.5);
    let corners = [start, Vec2::new(cursor.px.x, start.y), cursor.px, Vec2::new(start.x, cursor.px.y)];
    let pts: Vec<Vec3> = corners.iter().filter_map(|c| at(*c)).collect();
    if pts.len() == 4 {
        gizmos.linestrip(pts.iter().copied().chain(std::iter::once(pts[0])), Color::srgb(1.0, 0.85, 0.3));
    }
}

// ── Actions, from the panel ──────────────────────────────────────────────────

/// Change the selected vertices' weights with `f(influences, joint)`, as one
/// undo step. Nothing is done without a selection or a paint bone.
fn apply_to_selection(world: &mut World, label: &'static str, f: impl Fn(&mut Influences, u16)) {
    let (count, bone) = {
        let edit = world.resource::<SkinEdit>();
        (edit.meshes.iter().map(|m| m.selected.iter().filter(|s| **s).count()).sum::<usize>(), edit.paint_bone.clone())
    };
    if count == 0 {
        tell(world, false, "Select some of the model first: click a part, or drag a box");
        return;
    }
    let Some(bone) = bone else {
        tell(world, false, "Pick the bone first, in the Paint Bone list");
        return;
    };
    let before: Vec<Vec<Influences>> = world.resource::<SkinEdit>().meshes.iter().map(|m| m.weights.clone()).collect();
    let mut missing = false;
    let mut edit = world.resource_mut::<SkinEdit>();
    for m in edit.meshes.iter_mut() {
        if !m.selected.iter().any(|s| *s) {
            continue;
        }
        let Some(joint) = m.joint(&bone) else {
            missing = true;
            continue;
        };
        for (inf, sel) in m.weights.iter_mut().zip(&m.selected) {
            if *sel {
                f(inf, joint);
            }
        }
    }
    edit.mark_dirty();
    edit.unsaved = true;
    let root = edit.root();
    let after: Vec<Vec<Influences>> = edit.meshes.iter().map(|m| m.weights.clone()).collect();
    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(WeightsCmd { root, before, after, label }));
    if missing {
        tell(world, false, format!("Some selected meshes are not skinned to {bone}, and were left as they were"));
    }
}

/// The selection moves with the paint bone alone, as one solid piece.
pub(crate) fn bind_rigidly(world: &mut World) {
    apply_to_selection(world, "Bind Rigidly", |inf, joint| *inf = [(joint, 1.0), (joint, 0.0), (joint, 0.0), (joint, 0.0)]);
}

/// The selection takes the chosen share of the paint bone.
pub(crate) fn set_weight(world: &mut World) {
    let value = world.resource::<SkinEdit>().set_value;
    apply_to_selection(world, "Set Weight", move |inf, joint| weights::set_weight(inf, joint, value));
}

/// Auto weights for the selected vertices only.
pub(crate) fn auto_weights_selection(world: &mut World) {
    let keep: Vec<Vec<Influences>> = world.resource::<SkinEdit>().meshes.iter().map(|m| m.weights.clone()).collect();
    let selected: Vec<Vec<bool>> = world.resource::<SkinEdit>().meshes.iter().map(|m| m.selected.clone()).collect();
    if !selected.iter().flatten().any(|s| *s) {
        tell(world, false, "Select some of the model first: click a part, or drag a box");
        return;
    }
    // The whole model weighted (one undo step), then the unselected put back
    // and the step replaced with one that covers only the change.
    super::skin::auto_weights(world);
    let fresh: Vec<Vec<Influences>> = world.resource::<SkinEdit>().meshes.iter().map(|m| m.weights.clone()).collect();
    renzora_undo::undo_once(world);
    let mut edit = world.resource_mut::<SkinEdit>();
    for ((m, old), (new, sel)) in edit.meshes.iter_mut().zip(&keep).zip(fresh.iter().zip(&selected)) {
        m.weights = old.iter().zip(new).zip(sel).map(|((o, n), s)| if *s { *n } else { *o }).collect();
    }
    edit.mark_dirty();
    edit.unsaved = true;
    let root = edit.root();
    let after: Vec<Vec<Influences>> = edit.meshes.iter().map(|m| m.weights.clone()).collect();
    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(WeightsCmd { root, before: keep, after, label: "Auto Weights (Selection)" }));
}

pub(crate) fn select_all(world: &mut World, on: bool) {
    let mut edit = world.resource_mut::<SkinEdit>();
    for m in edit.meshes.iter_mut() {
        m.selected.iter_mut().for_each(|s| *s = on);
    }
    edit.mark_dirty();
}

pub(crate) fn invert(world: &mut World) {
    let mut edit = world.resource_mut::<SkinEdit>();
    for m in edit.meshes.iter_mut() {
        m.selected.iter_mut().for_each(|s| *s = !*s);
    }
    edit.mark_dirty();
}

/// Add every vertex sharing an edge with the selection.
pub(crate) fn grow(world: &mut World) {
    let mut edit = world.resource_mut::<SkinEdit>();
    for m in edit.meshes.iter_mut() {
        if m.adjacency.is_none() {
            m.adjacency = Some(weights::adjacency(&m.indices, m.positions.len()));
        }
        let Some(adj) = &m.adjacency else { continue };
        let was = m.selected.clone();
        for (v, on) in was.iter().enumerate() {
            if *on {
                for &n in &adj[v] {
                    if let Some(s) = m.selected.get_mut(n as usize) {
                        *s = true;
                    }
                }
            }
        }
    }
    edit.mark_dirty();
}

/// How many vertices are selected, for the panel.
pub(crate) fn count(edit: &SkinEdit) -> usize {
    edit.meshes.iter().map(|m| m.selected.iter().filter(|s| **s).count()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two triangles sharing an edge, and a third on its own: a click on the
    /// pair selects the pair only.
    #[test]
    fn a_piece_is_what_its_triangles_join() {
        let indices = [0, 1, 2, 1, 3, 2, 4, 5, 6];
        assert_eq!(connected(&indices, 7, 0), vec![0, 1, 2, 3]);
        assert_eq!(connected(&indices, 7, 5), vec![4, 5, 6]);
    }
}

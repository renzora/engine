//! Sculpt mode — brush-based vertex deformation on the promoted [`EditMesh`].
//!
//! Active in [`ViewportMode::Sculpt`]. Shares the Edit-mode lifecycle: the
//! selected entity's mesh is promoted to an `EditMesh`, brushes move its
//! vertices, and `bake_if_dirty` streams the result back into the `Mesh`
//! asset every frame.
//!
//! Brush math follows Blender's sculpt core (simplified): a per-vertex
//! multiplicative "factor" (smoothstep falloff × strength), one shared "area
//! normal" per dab for Draw/Flatten, per-vertex normals for Inflate, and
//! full-brush mirroring per symmetry pass.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use renzora::core::viewport_types::ViewportState;
use renzora::core::{EditorCamera, InputFocusState};
use std::collections::HashMap;

use crate::edit_mesh::EditMesh;
use crate::selection::MeshSelection;
use crate::tools::ModelingSettings;
use crate::undo::VertexMoveCmd;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BrushKind {
    #[default]
    Draw,
    Smooth,
    Grab,
    Inflate,
    Flatten,
    Pinch,
}

impl BrushKind {
    pub const ALL: &'static [BrushKind] = &[
        Self::Draw,
        Self::Smooth,
        Self::Grab,
        Self::Inflate,
        Self::Flatten,
        Self::Pinch,
    ];
    pub fn label(&self) -> &'static str {
        match self {
            Self::Draw => "Draw",
            Self::Smooth => "Smooth",
            Self::Grab => "Grab",
            Self::Inflate => "Inflate",
            Self::Flatten => "Flatten",
            Self::Pinch => "Pinch",
        }
    }
}

#[derive(Resource)]
pub struct SculptBrush {
    pub kind: BrushKind,
    /// World-space brush radius.
    pub radius: f32,
    /// 0..1 strength multiplier.
    pub strength: f32,
}

impl Default for SculptBrush {
    fn default() -> Self {
        Self {
            kind: BrushKind::Draw,
            radius: 0.35,
            strength: 0.5,
        }
    }
}

/// Where the brush ray currently meets the mesh, refreshed every frame.
#[derive(Resource, Default)]
pub struct SculptHover(pub Option<SculptHit>);

#[derive(Clone, Copy)]
pub struct SculptHit {
    pub world_pos: Vec3,
    pub world_normal: Vec3,
    pub local_pos: Vec3,
}

/// Per-stroke state. `before` collects original positions of every vertex a
/// dab touched so releasing the button records one undo command.
#[derive(Resource, Default)]
pub struct SculptStroke {
    pub active: bool,
    last_dab: Option<Vec3>,
    grab: Option<SculptGrabData>,
    before: HashMap<u32, Vec3>,
}

/// Grab brush: verts captured at stroke start with their falloff factors,
/// dragged rigidly with the cursor on a camera-facing plane.
struct SculptGrabData {
    /// (vert index, factor, original local position)
    verts: Vec<(u32, f32, Vec3)>,
    /// Mirrored capture when X-symmetry is on.
    mirror_verts: Vec<(u32, f32, Vec3)>,
    anchor_world: Vec3,
    plane_point: Vec3,
    plane_normal: Vec3,
}

/// Blender's default falloff: smoothstep over normalized distance.
fn falloff(dist: f32, radius: f32) -> f32 {
    if dist >= radius {
        return 0.0;
    }
    let p = 1.0 - dist / radius;
    3.0 * p * p - 2.0 * p * p * p
}

// ── Stroke system ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn sculpt_stroke(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    input_focus: Res<InputFocusState>,
    viewport: Option<Res<ViewportState>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut edit_q: Query<(&mut EditMesh, &GlobalTransform)>,
    sel: Res<MeshSelection>,
    brush: Res<SculptBrush>,
    modeling: Res<ModelingSettings>,
    mut stroke: ResMut<SculptStroke>,
    mut hover: ResMut<SculptHover>,
    mut commands: Commands,
) {
    hover.0 = None;
    let Some(target) = sel.target else { return };
    let Ok((mut edit, gt)) = edit_q.get_mut(target) else {
        return;
    };

    // Cursor → local-space ray → nearest face hit.
    let cursor = crate::systems::viewport_cursor(&viewport, &window_q);
    let cam = camera_q.single().ok();
    let inv = gt.to_matrix().inverse();
    let mut view_normal_local = Vec3::Z;
    if let (Some(cursor_vp), Some((camera, cam_gt))) = (cursor, cam) {
        view_normal_local = (inv.transform_vector3(-cam_gt.forward().as_vec3()))
            .normalize_or_zero();
        if let Ok(ray) = camera.viewport_to_world(cam_gt, cursor_vp) {
            let local_origin = inv.transform_point3(ray.origin);
            let local_dir = inv
                .transform_vector3(ray.direction.as_vec3())
                .normalize_or_zero();
            let mut best: Option<(f32, Vec3)> = None; // (t, local face normal)
            for face in &edit.faces {
                if face.verts.len() < 3 {
                    continue;
                }
                let p0 = edit.vertices[face.verts[0].0 as usize].position;
                for w in face.verts.windows(2).skip(1) {
                    let p1 = edit.vertices[w[0].0 as usize].position;
                    let p2 = edit.vertices[w[1].0 as usize].position;
                    if let Some(t) = crate::systems::ray_triangle(local_origin, local_dir, p0, p1, p2)
                    {
                        if best.is_none_or(|(bt, _)| t < bt) {
                            best = Some((t, (p1 - p0).cross(p2 - p0).normalize_or_zero()));
                        }
                    }
                }
            }
            if let Some((t, n_local)) = best {
                let local_pos = local_origin + local_dir * t;
                hover.0 = Some(SculptHit {
                    world_pos: gt.transform_point(local_pos),
                    world_normal: (gt.affine().matrix3 * n_local).normalize_or_zero(),
                    local_pos,
                });
            }
        }
    }

    // Brush radius in local units — assumes roughly uniform scale.
    let scale = gt.affine().matrix3.x_axis.length().max(1e-6);
    let radius_local = brush.radius / scale;

    // Stroke end?
    if !mouse.pressed(MouseButton::Left) {
        if stroke.active {
            let deltas: Vec<(u32, Vec3, Vec3)> = stroke
                .before
                .iter()
                .filter_map(|(&id, &old)| {
                    let new = edit.vertices.get(id as usize)?.position;
                    ((new - old).length_squared() > 1e-12).then_some((id, old, new))
                })
                .collect();
            if !deltas.is_empty() {
                let cmd = VertexMoveCmd {
                    entity: target,
                    deltas,
                };
                commands.queue(move |world: &mut World| {
                    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
                });
            }
            *stroke = SculptStroke::default();
        }
        return;
    }

    // Don't start strokes from clicks outside the viewport / while typing.
    if !stroke.active && (input_focus.ui_wants_keyboard || hover.0.is_none()) {
        return;
    }

    let invert = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    // Shift = temporary Smooth, Blender-style.
    let kind = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        BrushKind::Smooth
    } else {
        brush.kind
    };

    // ── Grab: capture once, drag every frame ──
    if kind == BrushKind::Grab {
        if !stroke.active {
            let Some(hit) = hover.0.as_ref() else { return };
            let Some((_, cam_gt)) = cam else { return };
            let capture = |center: Vec3| -> Vec<(u32, f32, Vec3)> {
                edit.vertices
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| {
                        let f = falloff(v.position.distance(center), radius_local);
                        (f > 0.0).then_some((i as u32, f, v.position))
                    })
                    .collect()
            };
            let verts = capture(hit.local_pos);
            let mirror_verts = if modeling.symmetry_x {
                let m = Vec3::new(-hit.local_pos.x, hit.local_pos.y, hit.local_pos.z);
                capture(m)
            } else {
                Vec::new()
            };
            for (i, _, p) in verts.iter().chain(mirror_verts.iter()) {
                stroke.before.entry(*i).or_insert(*p);
            }
            stroke.grab = Some(SculptGrabData {
                verts,
                mirror_verts,
                anchor_world: hit.world_pos,
                plane_point: hit.world_pos,
                plane_normal: -cam_gt.forward().as_vec3(),
            });
            stroke.active = true;
            return;
        }
        let Some(grab) = stroke.grab.as_ref() else {
            return;
        };
        let (Some(cursor_vp), Some((camera, cam_gt))) = (cursor, cam) else {
            return;
        };
        let Ok(ray) = camera.viewport_to_world(cam_gt, cursor_vp) else {
            return;
        };
        let denom = grab.plane_normal.dot(ray.direction.as_vec3());
        if denom.abs() < 1e-6 {
            return;
        }
        let t = grab.plane_normal.dot(grab.plane_point - ray.origin) / denom;
        if t < 0.0 {
            return;
        }
        let hit_world = ray.origin + ray.direction.as_vec3() * t;
        let delta_local =
            inv.transform_vector3(hit_world - grab.anchor_world) * brush.strength.max(0.01);
        let mirror_delta = Vec3::new(-delta_local.x, delta_local.y, delta_local.z);
        // Split borrows: copy the movement lists, then mutate vertices.
        let moves: Vec<(u32, Vec3)> = grab
            .verts
            .iter()
            .map(|(i, f, orig)| (*i, *orig + delta_local * *f))
            .chain(
                grab.mirror_verts
                    .iter()
                    .map(|(i, f, orig)| (*i, *orig + mirror_delta * *f)),
            )
            .collect();
        for (i, pos) in moves {
            if let Some(v) = edit.vertices.get_mut(i as usize) {
                v.position = pos;
            }
        }
        edit.recompute_normals();
        edit.dirty = true;
        return;
    }

    // ── Dab-based brushes ──
    let Some(hit) = hover.0.as_ref().copied() else {
        return;
    };
    let spacing = radius_local * 0.25;
    let should_dab = if !stroke.active {
        stroke.active = true;
        true
    } else {
        stroke
            .last_dab
            .map(|last| last.distance(hit.local_pos) >= spacing)
            .unwrap_or(true)
    };
    if !should_dab {
        return;
    }
    stroke.last_dab = Some(hit.local_pos);

    // Blender runs the full brush once per symmetry pass with the center
    // (and directions) mirrored.
    let mut centers = vec![(hit.local_pos, view_normal_local)];
    if modeling.symmetry_x {
        centers.push((
            Vec3::new(-hit.local_pos.x, hit.local_pos.y, hit.local_pos.z),
            Vec3::new(-view_normal_local.x, view_normal_local.y, view_normal_local.z),
        ));
    }
    let mut before = std::mem::take(&mut stroke.before);
    for (center, _view_n) in centers {
        apply_dab(
            &mut edit,
            kind,
            center,
            radius_local,
            brush.strength,
            invert,
            &mut before,
        );
    }
    stroke.before = before;
    edit.recompute_normals();
    edit.dirty = true;
}

/// One brush application at `center` (local space).
fn apply_dab(
    edit: &mut EditMesh,
    kind: BrushKind,
    center: Vec3,
    radius: f32,
    strength: f32,
    invert: bool,
    before: &mut HashMap<u32, Vec3>,
) {
    // Gather affected verts + factors.
    let affected: Vec<(u32, f32)> = edit
        .vertices
        .iter()
        .enumerate()
        .filter_map(|(i, v)| {
            let f = falloff(v.position.distance(center), radius);
            (f > 0.0).then_some((i as u32, f * strength))
        })
        .collect();
    if affected.is_empty() {
        return;
    }
    for (i, _) in &affected {
        let p = edit.vertices[*i as usize].position;
        before.entry(*i).or_insert(p);
    }
    let flip = if invert { -1.0 } else { 1.0 };

    // Shared "area normal" — factor-weighted average of the affected verts'
    // normals (Blender's AREA sculpt-plane).
    let area_normal = affected
        .iter()
        .map(|(i, f)| edit.vertices[*i as usize].normal * *f)
        .sum::<Vec3>()
        .normalize_or_zero();

    match kind {
        BrushKind::Draw => {
            let offset = area_normal * radius * 0.25 * flip;
            for (i, f) in &affected {
                edit.vertices[*i as usize].position += offset * *f;
            }
        }
        BrushKind::Inflate => {
            for (i, f) in &affected {
                let n = edit.vertices[*i as usize].normal;
                edit.vertices[*i as usize].position += n * radius * 0.2 * flip * *f;
            }
        }
        BrushKind::Pinch => {
            for (i, f) in &affected {
                let p = edit.vertices[*i as usize].position;
                edit.vertices[*i as usize].position += (center - p) * 0.5 * flip * *f;
            }
        }
        BrushKind::Flatten => {
            // Plane through the factor-weighted centroid, facing the area
            // normal; verts move onto it proportionally to their factor.
            let total: f32 = affected.iter().map(|(_, f)| *f).sum();
            let centroid = affected
                .iter()
                .map(|(i, f)| edit.vertices[*i as usize].position * *f)
                .sum::<Vec3>()
                / total.max(1e-6);
            for (i, f) in &affected {
                let p = edit.vertices[*i as usize].position;
                let d = area_normal.dot(centroid - p);
                edit.vertices[*i as usize].position += area_normal * d * *f;
            }
        }
        BrushKind::Smooth => {
            let neighbors = edit.vertex_neighbors();
            let updates: Vec<(u32, Vec3)> = affected
                .iter()
                .filter_map(|(i, f)| {
                    let ns = &neighbors[*i as usize];
                    if ns.is_empty() {
                        return None;
                    }
                    let avg = ns
                        .iter()
                        .map(|&n| edit.vertices[n as usize].position)
                        .sum::<Vec3>()
                        / ns.len() as f32;
                    let p = edit.vertices[*i as usize].position;
                    Some((*i, p + (avg - p) * 0.5 * *f))
                })
                .collect();
            for (i, pos) in updates {
                edit.vertices[i as usize].position = pos;
            }
        }
        BrushKind::Grab => {} // handled by the stroke system
    }
}

// ── Brush controls ─────────────────────────────────────────────────────────

/// `[` / `]` shrink / grow the brush.
pub fn brush_size_keys(
    keys: Res<ButtonInput<KeyCode>>,
    input_focus: Res<InputFocusState>,
    mut brush: ResMut<SculptBrush>,
) {
    if input_focus.ui_wants_keyboard {
        return;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        brush.radius = (brush.radius * 0.8).max(0.01);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        brush.radius = (brush.radius * 1.25).min(50.0);
    }
}

/// Brush cursor: a circle on the surface under the pointer.
pub fn draw_brush_cursor(hover: Res<SculptHover>, brush: Res<SculptBrush>, mut gizmos: Gizmos) {
    let Some(hit) = hover.0.as_ref() else { return };
    let n = hit.world_normal;
    let rot = Quat::from_rotation_arc(Vec3::Z, n);
    let iso = Isometry3d::new(hit.world_pos + n * 0.005, rot);
    gizmos.circle(iso, brush.radius, Color::srgba(1.0, 1.0, 1.0, 0.8));
    gizmos.circle(iso, brush.radius * 0.05, Color::srgba(1.0, 1.0, 1.0, 0.5));
}

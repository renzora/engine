//! Shapes mode: making blend shapes (morph targets) by sculpting the model on
//! the stage, for any model, rigged or not.
//!
//! On entering Shapes, every mesh of the stage's copy gets a mesh of its own,
//! with its blend shapes taken out of it, so nothing sculpted shows on the
//! scene's model before it is saved and so the GPU does not add the file's
//! shapes on top of the ones being edited. The copy stands at rest (the bind
//! pose, like Rig), and its shapes are held here as displacements per vertex:
//! what the stage draws is the rest shape plus each shape times its preview
//! weight, written into the copy's own vertex positions and normals whenever
//! anything changes. Leaving puts the copy's meshes and blend shape weights
//! back, saving first if anything was sculpted.
//!
//! - **Shapes** are a model's blend shapes by name. A model made of several
//!   meshes (a head, its lashes, its teeth) has a shape wherever the file has
//!   it, and a new one wherever it is sculpted: "Blink" can move both the lids
//!   and the lashes, and is one row here.
//! - **The brush** (Draw, Smooth, Grab, Inflate, Flatten, Pinch, from
//!   `renzora_rig::sculpt`) sculpts the chosen shape on the surface as it is
//!   drawn, so a shape sculpted with another one dialled in is sculpted on top
//!   of it. A stroke keeps to the mesh it started on unless "Every mesh" is
//!   on, so working the lips does not drag the teeth. Ctrl inverts, Shift
//!   smooths, `[` and `]` size the brush. Each stroke is one undo step.
//! - **Save** writes the shapes into the model file as glTF morph targets
//!   (`renzora_rig::write_shapes`), position and normal displacements, and
//!   reloads the model everywhere, after which the shapes are blend shape
//!   sliders in Animate and key like any imported shape.
//!
//! Vertices that share a place (a UV seam) are sculpted as one point
//! (`renzora_rig::sculpt::weld`), and every displacement is made in the
//! stage's space and turned into each vertex's own: a skinned mesh's vertices
//! are each placed by their bones, so that turn differs vertex to vertex.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::morph::MeshMorphWeights;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use renzora_rig::morphs::{self, TargetData, TargetEdit};
use renzora_rig::sculpt::{self, Brush, Dab};
use renzora_rig::weights::Falloff;

use super::clone::StageRig;
use super::view::{StageCursor, StageNav};
use super::StageMode;
use crate::rig::tell;
use crate::studio_preview::{StudioPreviewCamera, StudioPreviewGizmoGroup};

/// A copy's mesh entity being sculpted: one glTF primitive.
struct Part {
    entity: Entity,
    original: Handle<Mesh>,
    own: Handle<Mesh>,
    /// The entity's blend shape weights, taken off while its mesh has none.
    original_weights: Option<MeshMorphWeights>,
    /// `(mesh, primitive)` in the model file, from the asset's label.
    label: Option<(usize, usize)>,
    /// At rest, in the mesh's own space.
    rest: Vec<Vec3>,
    /// The file's normals, own space; empty when the mesh had none.
    normals: Vec<Vec3>,
    /// Each vertex's turn from its own space into the stage's, and back.
    lin: Vec<Mat3>,
    inv: Vec<Mat3>,
    /// Each vertex's point ([`Points`]).
    point: Vec<u32>,
}

/// One blend shape of the model.
#[derive(Clone)]
pub(crate) struct Shape {
    pub(crate) name: String,
    /// Where it was in the file: `(glTF mesh, target index)`, once per mesh
    /// that has it.
    source: Vec<(usize, usize)>,
    /// The weight it is shown at on the stage. Nothing is keyed.
    pub(crate) weight: f32,
    /// Per part, per vertex, in the vertex's own space.
    deltas: Vec<Vec<Vec3>>,
    /// Sculpted since it was read from the file (or new): only these get new
    /// data on save, everything else keeps the file's bytes.
    edited: bool,
}

impl Shape {
    fn moves_part(&self, part: usize) -> bool {
        self.deltas.get(part).is_some_and(|d| d.iter().any(|v| v.length_squared() > 1e-14))
    }
}

/// The welded surface every part's vertices fold into.
#[derive(Default)]
struct Points {
    rest: Vec<Vec3>,
    /// Which mesh each point is on: the glTF mesh, or a stand-in for a mesh
    /// with no file of its own.
    group: Vec<usize>,
    /// Every vertex on each point, `(part, vertex)`.
    members: Vec<Vec<(u32, u32)>>,
    triangles: Vec<[u32; 3]>,
    neighbours: Option<Vec<Vec<u32>>>,
    rest_normals: Vec<Vec3>,
}

struct Grab {
    caught: Vec<(u32, f32)>,
    mirrored: Vec<(u32, f32)>,
    anchor: Vec3,
    plane: Vec3,
}

struct Stroke {
    shape: usize,
    before: Vec<Vec<Vec3>>,
    group: Option<usize>,
    last_dab: Option<Vec3>,
    grab: Option<Grab>,
}

#[derive(Resource)]
pub(crate) struct ShapeEdit {
    parts: Vec<Part>,
    points: Points,
    pub(crate) shapes: Vec<Shape>,
    pub(crate) active: Option<usize>,
    /// The copy these parts belong to.
    root: Option<Entity>,
    /// Frames spent in Shapes waiting for the copy to stand at rest.
    settle: u8,
    pub(crate) brush: Brush,
    /// A fraction of the model's size.
    pub(crate) radius: f32,
    pub(crate) strength: f32,
    pub(crate) falloff: Falloff,
    pub(crate) mirror: bool,
    pub(crate) every_mesh: bool,
    pub(crate) unsaved: bool,
    /// The name field, for New Shape and Rename.
    pub(crate) name: String,
    size: f32,
    stroke: Option<Stroke>,
    dirty: bool,
    display: Vec<Vec3>,
    display_normals: Vec<Vec3>,
    /// Where the brush is on the model, and the surface's facing there.
    pub(crate) hit: Option<(Vec3, Vec3)>,
    /// What to put back after the copy is made again from a saved file.
    keep: Option<(Option<String>, HashMap<String, f32>)>,
}

impl Default for ShapeEdit {
    fn default() -> Self {
        Self {
            parts: Vec::new(),
            points: Points::default(),
            shapes: Vec::new(),
            active: None,
            root: None,
            settle: 0,
            brush: Brush::Draw,
            radius: 0.05,
            strength: 0.5,
            falloff: Falloff::Smooth,
            mirror: true,
            every_mesh: false,
            unsaved: false,
            name: String::new(),
            size: 1.0,
            stroke: None,
            dirty: false,
            display: Vec::new(),
            display_normals: Vec::new(),
            hit: None,
            keep: None,
        }
    }
}

impl ShapeEdit {
    pub(crate) fn active(&self) -> bool {
        self.root.is_some()
    }

    pub(crate) fn has_meshes(&self) -> bool {
        !self.parts.is_empty()
    }

    fn brush_radius(&self) -> f32 {
        self.radius * self.size
    }

    pub(crate) fn active_shape(&self) -> Option<&Shape> {
        self.active.and_then(|i| self.shapes.get(i))
    }

    /// Show the stage again after a weight changed.
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn unique_name(&self, wanted: &str, except: Option<usize>) -> String {
        let base = if wanted.trim().is_empty() { "Shape" } else { wanted.trim() };
        let taken = |n: &str| self.shapes.iter().enumerate().any(|(i, s)| Some(i) != except && s.name == n);
        if !taken(base) {
            return base.to_string();
        }
        (2..).map(|k| format!("{base} {k}")).find(|n| !taken(n)).unwrap_or_else(|| base.to_string())
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<ShapeEdit>().add_systems(
        Update,
        (sync_shapes, sculpt_input, refresh, draw_brush)
            .chain()
            .after(super::view::StageInputSet)
            .run_if(in_state(renzora_editor_framework::SplashState::Editor))
            .run_if(super::stage_mounted)
            .run_if(renzora::not_in_play_mode),
    );
}

// ── Entering and leaving ─────────────────────────────────────────────────────

fn sync_shapes(world: &mut World) {
    let in_mode = super::mode(world) == StageMode::Shapes;
    let root = world.resource::<StageRig>().root;
    let current = world.resource::<ShapeEdit>().root;
    if current.is_some() && (!in_mode || current != root) {
        if world.resource::<ShapeEdit>().unsaved && current == root {
            save(world);
        }
        restore(world);
    }
    let Some(r) = root.filter(|_| in_mode) else {
        let mut edit = world.resource_mut::<ShapeEdit>();
        edit.settle = 0;
        // What a save leaves to put back is for the copy it remade, not for
        // whatever model is sculpted next.
        if !in_mode {
            edit.keep = None;
        }
        return;
    };
    if world.resource::<ShapeEdit>().root.is_none() {
        // The copy is posed at rest in PostUpdate; reading its transforms the
        // frame the mode changed would read the pose it had before.
        let settle = {
            let mut edit = world.resource_mut::<ShapeEdit>();
            edit.settle = edit.settle.saturating_add(1);
            edit.settle
        };
        if settle >= 3 {
            enter(world, r);
        }
    }
}

/// `Mesh3/Primitive1` → `(3, 1)`.
fn parse_label(label: &str) -> Option<(usize, usize)> {
    let (m, p) = label.split_once('/')?;
    Some((m.strip_prefix("Mesh")?.parse().ok()?, p.strip_prefix("Primitive")?.parse().ok()?))
}

/// A mesh with the same vertices and triangles and no blend shapes: Bevy has
/// no way to take a mesh's morph targets off, so the mesh is copied without
/// them.
fn without_shapes(mesh: &Mesh) -> Mesh {
    let mut out = Mesh::new(mesh.primitive_topology(), RenderAssetUsages::all());
    for (attr, values) in mesh.attributes() {
        out.insert_attribute(*attr, values.clone());
    }
    if let Some(i) = mesh.indices() {
        out.insert_indices(i.clone());
    }
    out
}

fn enter(world: &mut World, root: Entity) {
    let entities: Vec<Entity> = crate::rig::descendants(world, root).into_iter().filter(|e| world.get::<Mesh3d>(*e).is_some()).collect();
    let loading = entities.iter().any(|e| world.get::<Mesh3d>(*e).is_some_and(|m| world.resource::<Assets<Mesh>>().get(&m.0).is_none()));
    if loading {
        return;
    }
    let mut parts = Vec::new();
    // Per part: the file's shapes, `(name, displacements)`.
    let mut imported: Vec<Vec<(String, Vec<Vec3>)>> = Vec::new();
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for e in entities {
        let (Some(mesh3d), Some(gt)) = (world.get::<Mesh3d>(e), world.get::<GlobalTransform>(e)) else { continue };
        let original = mesh3d.0.clone();
        let mesh_world = gt.to_matrix();
        let Some(mesh) = world.resource::<Assets<Mesh>>().get(&original) else { continue };
        if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
            continue;
        }
        let rest: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(p)) => p.iter().map(|v| Vec3::from_array(*v)).collect(),
            _ => continue,
        };
        let normals: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(VertexAttributeValues::Float32x3(n)) if n.len() == rest.len() => n.iter().map(|v| Vec3::from_array(*v)).collect(),
            _ => Vec::new(),
        };
        let indices: Vec<u32> = match mesh.indices() {
            Some(Indices::U16(i)) => i.iter().map(|v| *v as u32).collect(),
            Some(Indices::U32(i)) => i.clone(),
            None => (0..rest.len() as u32).collect(),
        };
        let n = rest.len();
        let shapes: Vec<(String, Vec<Vec3>)> = match mesh.try_morph_targets() {
            Ok(t) if n > 0 && t.len().is_multiple_of(n) => {
                let names = mesh.try_morph_target_names().ok().flatten().map(<[String]>::to_vec).unwrap_or_default();
                t.chunks_exact(n)
                    .enumerate()
                    .map(|(i, c)| (names.get(i).cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("Shape {i}")), c.iter().map(|a| a.position).collect()))
                    .collect()
            }
            _ => Vec::new(),
        };
        // The turn each vertex is drawn with: its bones' at rest for a
        // skinned mesh (whose own transform the GPU ignores), else the mesh's.
        let per_vertex: Option<Vec<Mat4>> = world.get::<SkinnedMesh>(e).and_then(|skin| {
            let binds = world.resource::<Assets<SkinnedMeshInverseBindposes>>().get(&skin.inverse_bindposes)?;
            let mats: Vec<Mat4> = skin
                .joints
                .iter()
                .zip(binds.iter())
                .map(|(j, ibm)| world.get::<GlobalTransform>(*j).map(|g| g.to_matrix()).unwrap_or(Mat4::IDENTITY) * *ibm)
                .collect();
            let joints = match mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX) {
                Some(VertexAttributeValues::Uint16x4(j)) => j.clone(),
                _ => return None,
            };
            let weights = match mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT) {
                Some(VertexAttributeValues::Float32x4(w)) => w.clone(),
                _ => return None,
            };
            Some(
                joints
                    .iter()
                    .zip(&weights)
                    .map(|(j, w)| {
                        let mut m = Mat4::ZERO;
                        let mut total = 0.0;
                        for k in 0..4 {
                            if let Some(mat) = mats.get(j[k] as usize).filter(|_| w[k] > 0.0) {
                                m += *mat * w[k];
                                total += w[k];
                            }
                        }
                        if total > 1e-6 { m * (1.0 / total) } else { mesh_world }
                    })
                    .collect(),
            )
        });
        let matrix = |v: usize| per_vertex.as_ref().and_then(|m| m.get(v).copied()).unwrap_or(mesh_world);
        let lin: Vec<Mat3> = (0..n).map(|v| Mat3::from_mat4(matrix(v))).collect();
        let inv: Vec<Mat3> = lin.iter().map(|m| if m.determinant().abs() > 1e-12 { m.inverse() } else { Mat3::IDENTITY }).collect();
        let world_rest: Vec<Vec3> = (0..n).map(|v| matrix(v).transform_point3(rest[v])).collect();
        for p in &world_rest {
            min = min.min(*p);
            max = max.max(*p);
        }
        let own_mesh = without_shapes(mesh);
        let label = world
            .resource::<AssetServer>()
            .get_path(original.id())
            .and_then(|p| p.label().map(str::to_string))
            .and_then(|l| parse_label(&l));
        let own = world.resource_mut::<Assets<Mesh>>().add(own_mesh);
        let original_weights = world.get::<MeshMorphWeights>(e).cloned();
        world.entity_mut(e).insert(Mesh3d(own.clone())).remove::<MeshMorphWeights>();
        imported.push(shapes);
        parts.push((Part { entity: e, original, own, original_weights, label, rest, normals, lin, inv, point: Vec::new() }, world_rest, indices));
    }
    let size = if min.x <= max.x { (max - min).length().max(1e-3) } else { 1.0 };

    // Weld every part's vertices into points, each mesh on its own.
    let mut all = Vec::new();
    let mut groups = Vec::new();
    for (pi, (part, world_rest, _)) in parts.iter().enumerate() {
        let group = part.label.map(|l| l.0 as u32).unwrap_or(1_000_000 + pi as u32);
        all.extend_from_slice(world_rest);
        groups.extend(std::iter::repeat_n(group, world_rest.len()));
    }
    let (map, rest_points) = sculpt::weld(&all, &groups, size * 1e-5);
    let mut points = Points { group: vec![0; rest_points.len()], members: vec![Vec::new(); rest_points.len()], rest: rest_points, ..default() };
    let mut at = 0;
    let mut out_parts = Vec::with_capacity(parts.len());
    for (pi, (mut part, world_rest, indices)) in parts.into_iter().enumerate() {
        part.point = map[at..at + world_rest.len()].to_vec();
        at += world_rest.len();
        for (v, &p) in part.point.iter().enumerate() {
            points.members[p as usize].push((pi as u32, v as u32));
            points.group[p as usize] = groups.get(at - world_rest.len() + v).copied().unwrap_or(0) as usize;
        }
        for t in indices.chunks_exact(3) {
            let (Some(&a), Some(&b), Some(&c)) = (part.point.get(t[0] as usize), part.point.get(t[1] as usize), part.point.get(t[2] as usize)) else { continue };
            if a != b && b != c && a != c {
                points.triangles.push([a, b, c]);
            }
        }
        out_parts.push(part);
    }
    points.rest_normals = sculpt::normals(&points.rest, &points.triangles);

    // The file's shapes, by name across every part.
    let mut shapes: Vec<Shape> = Vec::new();
    for (pi, list) in imported.into_iter().enumerate() {
        for (ti, (name, deltas)) in list.into_iter().enumerate() {
            let si = match shapes.iter().position(|s| s.name == name) {
                Some(i) => i,
                None => {
                    shapes.push(Shape { name, source: Vec::new(), weight: 0.0, deltas: out_parts.iter().map(|p| vec![Vec3::ZERO; p.rest.len()]).collect(), edited: false });
                    shapes.len() - 1
                }
            };
            if deltas.len() == out_parts[pi].rest.len() {
                shapes[si].deltas[pi] = deltas;
            }
            if let Some((m, _)) = out_parts[pi].label {
                if !shapes[si].source.iter().any(|(sm, _)| *sm == m) {
                    shapes[si].source.push((m, ti));
                }
            }
        }
    }

    let mut edit = world.resource_mut::<ShapeEdit>();
    edit.parts = out_parts;
    edit.points = points;
    edit.shapes = shapes;
    edit.active = None;
    edit.root = Some(root);
    edit.size = size;
    edit.unsaved = false;
    edit.stroke = None;
    edit.dirty = true;
    if let Some((active, weights)) = edit.keep.take() {
        for s in edit.shapes.iter_mut() {
            if let Some(w) = weights.get(&s.name) {
                s.weight = *w;
            }
        }
        edit.active = active.and_then(|n| edit.shapes.iter().position(|s| s.name == n));
    }
    if edit.active.is_none() && !edit.shapes.is_empty() {
        edit.active = Some(0);
    }
}

fn restore(world: &mut World) {
    let parts = std::mem::take(&mut world.resource_mut::<ShapeEdit>().parts);
    for p in parts {
        if let Ok(mut e) = world.get_entity_mut(p.entity) {
            e.insert(Mesh3d(p.original.clone()));
            if let Some(w) = p.original_weights {
                e.insert(w);
            }
        }
        world.resource_mut::<Assets<Mesh>>().remove(&p.own);
    }
    let mut edit = world.resource_mut::<ShapeEdit>();
    edit.points = Points::default();
    edit.shapes.clear();
    edit.root = None;
    edit.settle = 0;
    edit.unsaved = false;
    edit.stroke = None;
    edit.hit = None;
    edit.display.clear();
    edit.display_normals.clear();
}

// ── Showing ──────────────────────────────────────────────────────────────────

/// Each part's vertices displaced by every shape at its preview weight, in
/// their own space.
fn mixed(edit: &ShapeEdit) -> Vec<Vec<Vec3>> {
    edit.parts
        .iter()
        .enumerate()
        .map(|(pi, part)| {
            let mut sum = vec![Vec3::ZERO; part.rest.len()];
            for s in edit.shapes.iter().filter(|s| s.weight != 0.0) {
                if let Some(d) = s.deltas.get(pi) {
                    for (o, v) in sum.iter_mut().zip(d) {
                        *o += *v * s.weight;
                    }
                }
            }
            sum
        })
        .collect()
}

/// Points displaced by per-part, per-vertex `offsets`, in the stage's space.
fn displaced(edit: &ShapeEdit, offsets: &[Vec<Vec3>]) -> Vec<Vec3> {
    edit.points
        .rest
        .iter()
        .zip(&edit.points.members)
        .map(|(r, m)| match m.first() {
            Some(&(pi, v)) => {
                let (pi, v) = (pi as usize, v as usize);
                *r + edit.parts[pi].lin[v] * offsets.get(pi).and_then(|o| o.get(v)).copied().unwrap_or(Vec3::ZERO)
            }
            None => *r,
        })
        .collect()
}

/// A vertex's normal on a displaced surface: its own normal turned by as much
/// as the surface turned under it, so hard edges and authored normals stay
/// what they were and only the change is the sculpt's.
fn vertex_normal(part: &Part, v: usize, rest_n: Vec3, new_n: Vec3) -> Vec3 {
    let a = (part.inv[v] * rest_n).normalize_or_zero();
    let b = (part.inv[v] * new_n).normalize_or_zero();
    let base = part.normals.get(v).copied().unwrap_or(a);
    (base + (b - a)).normalize_or(base)
}

/// A part's own mesh with its new positions and normals.
type MeshUpdate = (Handle<Mesh>, Vec<[f32; 3]>, Vec<[f32; 3]>);

fn refresh(world: &mut World) {
    if !world.resource::<ShapeEdit>().dirty || !world.resource::<ShapeEdit>().active() {
        return;
    }
    let (updates, display, normals) = {
        let edit = world.resource::<ShapeEdit>();
        let mix = mixed(edit);
        let display = displaced(edit, &mix);
        let normals = sculpt::normals(&display, &edit.points.triangles);
        let updates: Vec<MeshUpdate> = edit
            .parts
            .iter()
            .zip(&mix)
            .map(|(part, m)| {
                let pos = part.rest.iter().zip(m).map(|(r, d)| (*r + *d).to_array()).collect();
                let nor = (0..part.rest.len())
                    .map(|v| {
                        let p = part.point[v] as usize;
                        vertex_normal(part, v, edit.points.rest_normals[p], normals[p]).to_array()
                    })
                    .collect();
                (part.own.clone(), pos, nor)
            })
            .collect();
        (updates, display, normals)
    };
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    for (h, p, n) in updates {
        if let Some(mut mesh) = meshes.get_mut(&h) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n);
        }
    }
    let mut edit = world.resource_mut::<ShapeEdit>();
    edit.display = display;
    edit.display_normals = normals;
    edit.dirty = false;
}

// ── Sculpting ────────────────────────────────────────────────────────────────

fn ray_triangle(ray: Ray3d, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = ray.direction.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = ray.origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = ray.direction.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}

/// Where the ray meets the surface as drawn: the place, the surface's facing
/// toward the ray, and the mesh hit.
fn cast(edit: &ShapeEdit, ray: Ray3d) -> Option<(Vec3, Vec3, usize)> {
    let pos = &edit.display;
    let mut best: Option<(f32, [u32; 3])> = None;
    for t in &edit.points.triangles {
        let (Some(a), Some(b), Some(c)) = (pos.get(t[0] as usize), pos.get(t[1] as usize), pos.get(t[2] as usize)) else { continue };
        if let Some(d) = ray_triangle(ray, *a, *b, *c) {
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, *t));
            }
        }
    }
    let (d, t) = best?;
    let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
    let mut n = (b - a).cross(c - a).normalize_or_zero();
    if n.dot(*ray.direction) > 0.0 {
        n = -n;
    }
    Some((ray.origin + *ray.direction * d, n, edit.points.group[t[0] as usize]))
}

/// Move points of the active shape by `moves` (stage space), on top of
/// `base` when given (Grab measures from the stroke's start), else on top of
/// what the shape has now.
fn displace(edit: &mut ShapeEdit, shape: usize, moves: &[(u32, Vec3)], base: Option<&[Vec<Vec3>]>) {
    let ShapeEdit { parts, points, shapes, .. } = edit;
    let Some(s) = shapes.get_mut(shape) else { return };
    if let Some(b) = base {
        for (d, b) in s.deltas.iter_mut().zip(b) {
            d.clone_from(b);
        }
    }
    for (p, m) in moves {
        for &(pi, v) in points.members.get(*p as usize).into_iter().flatten() {
            let (pi, v) = (pi as usize, v as usize);
            if let Some(d) = s.deltas.get_mut(pi).and_then(|d| d.get_mut(v)) {
                *d += parts[pi].inv[v] * *m;
            }
        }
    }
    s.edited = true;
}

fn camera_position(world: &mut World) -> Option<Vec3> {
    let mut q = world.query_filtered::<&GlobalTransform, With<StudioPreviewCamera>>();
    q.iter(world).next().map(GlobalTransform::translation)
}

fn sculpt_input(world: &mut World) {
    if super::mode(world) != StageMode::Shapes || !world.resource::<ShapeEdit>().active() {
        return;
    }
    let (over, ray) = {
        let c = world.resource::<StageCursor>();
        (c.over, c.ray)
    };
    let (pressed, just_pressed) = {
        let m = world.resource::<ButtonInput<MouseButton>>();
        (m.pressed(MouseButton::Left), m.just_pressed(MouseButton::Left))
    };
    let (ctrl, shift, smaller, bigger) = {
        let k = world.resource::<ButtonInput<KeyCode>>();
        (
            k.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
            k.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            k.just_pressed(KeyCode::BracketLeft),
            k.just_pressed(KeyCode::BracketRight),
        )
    };
    let typing = world.get_resource::<renzora::InputFocusState>().is_some_and(|f| f.ui_wants_keyboard);
    let navigating = world.resource::<StageNav>().dragging.is_some();
    if over && !typing && (smaller || bigger) {
        let mut edit = world.resource_mut::<ShapeEdit>();
        edit.radius = if smaller { (edit.radius * 0.8).max(0.003) } else { (edit.radius * 1.25).min(0.5) };
    }

    if !pressed {
        finish_stroke(world);
    }
    let hit = ray.filter(|_| over && !navigating).and_then(|r| cast(world.resource::<ShapeEdit>(), r));
    world.resource_mut::<ShapeEdit>().hit = hit.map(|(p, n, _)| (p, n));
    if !pressed || navigating {
        return;
    }

    // Start a stroke only on a press on the model, never by dragging onto it
    // with the button already down.
    if world.resource::<ShapeEdit>().stroke.is_none() {
        let (Some((at, _, group)), true) = (hit, just_pressed) else { return };
        let Some(shape) = world.resource::<ShapeEdit>().active.filter(|i| *i < world.resource::<ShapeEdit>().shapes.len()) else {
            tell(world, false, "Make a shape first: New Shape in the panel on the left");
            return;
        };
        let cam = camera_position(world).unwrap_or(at + Vec3::Z);
        let mut edit = world.resource_mut::<ShapeEdit>();
        // Sculpting shows the shape at full strength, so what is moved is
        // what the shape will do at 1.
        if edit.shapes[shape].weight != 1.0 {
            edit.shapes[shape].weight = 1.0;
            edit.dirty = true;
        }
        let before = edit.shapes[shape].deltas.clone();
        let group = (!edit.every_mesh).then_some(group);
        let grab = (edit.brush == Brush::Grab && !shift).then(|| {
            let d = Dab { centre: at, radius: edit.brush_radius(), strength: edit.strength.max(0.05), falloff: edit.falloff, invert: false };
            let allow = |i: usize| group.is_none_or(|g| edit.points.group[i] == g);
            let caught = sculpt::reach(&edit.display, &d, allow);
            let mirrored = if edit.mirror { sculpt::reach(&edit.display, &Dab { centre: sculpt::mirror(at), ..d }, allow) } else { Vec::new() };
            Grab { caught, mirrored, anchor: at, plane: (cam - at).normalize_or(Vec3::Z) }
        });
        edit.stroke = Some(Stroke { shape, before, group, last_dab: None, grab });
    }

    let mut edit = world.resource_mut::<ShapeEdit>();
    let Some(stroke) = edit.stroke.as_ref() else { return };
    let (shape, group) = (stroke.shape, stroke.group);
    if let Some(g) = &stroke.grab {
        let Some(ray) = ray else { return };
        let denom = g.plane.dot(*ray.direction);
        if denom.abs() < 1e-6 {
            return;
        }
        let t = g.plane.dot(g.anchor - ray.origin) / denom;
        if t < 0.0 {
            return;
        }
        let offset = ray.origin + *ray.direction * t - g.anchor;
        let moves = sculpt::merge_mirrored(sculpt::grab(&g.caught, offset), &g.caught, sculpt::grab(&g.mirrored, sculpt::mirror(offset)), &g.mirrored);
        let before = stroke.before.clone();
        displace(&mut edit, shape, &moves, Some(&before));
    } else {
        let Some((at, _, _)) = hit else { return };
        let r = edit.brush_radius();
        // Dabs a quarter of the brush apart, so a stroke builds evenly however
        // fast the cursor moves and a held brush does not dig a hole.
        if stroke.last_dab.is_some_and(|l| l.distance(at) < r * 0.25) {
            return;
        }
        let brush = if shift { Brush::Smooth } else { edit.brush };
        if brush == Brush::Smooth && edit.points.neighbours.is_none() {
            edit.points.neighbours = Some(sculpt::neighbours(&edit.points.triangles, edit.points.rest.len()));
        }
        let d = Dab { centre: at, radius: r, strength: edit.strength.max(0.02), falloff: edit.falloff, invert: ctrl };
        let moves = {
            let e = &*edit;
            let allow = |i: usize| group.is_none_or(|g| e.points.group[i] == g);
            let nb = e.points.neighbours.as_deref().unwrap_or(&[]);
            let one = |d: &Dab| {
                let reached = sculpt::reach(&e.display, d, allow);
                (sculpt::dab(brush, &e.display, &e.display_normals, nb, d, &reached), reached)
            };
            let (first, first_reach) = one(&d);
            if e.mirror {
                let (second, second_reach) = one(&Dab { centre: sculpt::mirror(at), ..d });
                sculpt::merge_mirrored(first, &first_reach, second, &second_reach)
            } else {
                first
            }
        };
        displace(&mut edit, shape, &moves, None);
        if let Some(s) = edit.stroke.as_mut() {
            s.last_dab = Some(at);
        }
    }
    edit.dirty = true;
    edit.unsaved = true;
}

fn finish_stroke(world: &mut World) {
    let Some(stroke) = world.resource_mut::<ShapeEdit>().stroke.take() else { return };
    let edit = world.resource::<ShapeEdit>();
    let Some(s) = edit.shapes.get(stroke.shape) else { return };
    if s.deltas == stroke.before {
        return;
    }
    let cmd = DeltasCmd { root: edit.root, shape: s.name.clone(), before: stroke.before, after: s.deltas.clone(), label: "Sculpt Shape" };
    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
}

/// The brush on the model: a ring the size of the brush, lying on the
/// surface, and its mirror.
fn draw_brush(mut gizmos: Gizmos<StudioPreviewGizmoGroup>, edit: Res<ShapeEdit>, ui: Res<super::StageUi>) {
    if ui.mode != StageMode::Shapes {
        return;
    }
    let Some((p, n)) = edit.hit else { return };
    let r = edit.brush_radius();
    let colour = Color::srgba(1.0, 1.0, 1.0, 0.85);
    gizmos.circle(Isometry3d::new(p + n * r * 0.02, Quat::from_rotation_arc(Vec3::Z, n)), r, colour);
    gizmos.circle(Isometry3d::new(p + n * r * 0.02, Quat::from_rotation_arc(Vec3::Z, n)), r * 0.06, colour);
    if edit.mirror && p.x.abs() > r * 0.5 {
        let (mp, mn) = (sculpt::mirror(p), sculpt::mirror(n));
        gizmos.circle(Isometry3d::new(mp + mn * r * 0.02, Quat::from_rotation_arc(Vec3::Z, mn)), r, colour.with_alpha(0.4));
    }
}

// ── Undo ─────────────────────────────────────────────────────────────────────

/// One stroke (or Clear): a shape's displacements before and after.
struct DeltasCmd {
    root: Option<Entity>,
    shape: String,
    before: Vec<Vec<Vec3>>,
    after: Vec<Vec<Vec3>>,
    label: &'static str,
}

impl DeltasCmd {
    fn put(&self, world: &mut World, which: &[Vec<Vec3>]) {
        let mut edit = world.resource_mut::<ShapeEdit>();
        if edit.root != self.root {
            return;
        }
        let Some(s) = edit.shapes.iter_mut().find(|s| s.name == self.shape) else { return };
        if s.deltas.len() != which.len() || s.deltas.iter().zip(which).any(|(a, b)| a.len() != b.len()) {
            return;
        }
        s.deltas = which.to_vec();
        s.edited = true;
        edit.dirty = true;
        edit.unsaved = true;
    }
}

impl renzora_undo::UndoCommand for DeltasCmd {
    fn label(&self) -> &str {
        self.label
    }
    fn execute(&mut self, world: &mut World) {
        let after = std::mem::take(&mut self.after);
        self.put(world, &after);
        self.after = after;
    }
    fn undo(&mut self, world: &mut World) {
        let before = std::mem::take(&mut self.before);
        self.put(world, &before);
        self.before = before;
    }
}

/// A new, renamed or deleted shape: the whole list before and after.
struct ListCmd {
    root: Option<Entity>,
    before: (Vec<Shape>, Option<usize>),
    after: (Vec<Shape>, Option<usize>),
    label: &'static str,
}

impl ListCmd {
    fn put(&self, world: &mut World, which: &(Vec<Shape>, Option<usize>)) {
        let mut edit = world.resource_mut::<ShapeEdit>();
        if edit.root != self.root {
            return;
        }
        edit.shapes = which.0.clone();
        edit.active = which.1;
        edit.dirty = true;
        edit.unsaved = true;
    }
}

impl renzora_undo::UndoCommand for ListCmd {
    fn label(&self) -> &str {
        self.label
    }
    fn execute(&mut self, world: &mut World) {
        let after = std::mem::take(&mut self.after);
        self.put(world, &after);
        self.after = after;
    }
    fn undo(&mut self, world: &mut World) {
        let before = std::mem::take(&mut self.before);
        self.put(world, &before);
        self.before = before;
    }
}

/// Change the shape list through `f`, as one undo step.
fn change_list(world: &mut World, label: &'static str, f: impl FnOnce(&mut ShapeEdit)) {
    let mut edit = world.resource_mut::<ShapeEdit>();
    let before = (edit.shapes.clone(), edit.active);
    f(&mut edit);
    edit.dirty = true;
    edit.unsaved = true;
    let after = (edit.shapes.clone(), edit.active);
    let root = edit.root;
    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(ListCmd { root, before, after, label }));
}

// ── Actions ──────────────────────────────────────────────────────────────────

fn ready(world: &mut World) -> bool {
    let edit = world.resource::<ShapeEdit>();
    if !edit.active() || !edit.has_meshes() {
        tell(world, false, "Switch to Shapes with a model on the stage first");
        return false;
    }
    true
}

/// A new, flat shape named from the name field, ready to sculpt.
pub(crate) fn new_shape(world: &mut World) {
    if !ready(world) {
        return;
    }
    if world.resource::<ShapeEdit>().shapes.len() >= morphs::MAX_TARGETS {
        tell(world, false, format!("A model can have at most {} blend shapes", morphs::MAX_TARGETS));
        return;
    }
    change_list(world, "New Shape", |edit| {
        let name = edit.unique_name(&edit.name.clone(), None);
        let deltas = edit.parts.iter().map(|p| vec![Vec3::ZERO; p.rest.len()]).collect();
        edit.shapes.push(Shape { name, source: Vec::new(), weight: 1.0, deltas, edited: true });
        edit.active = Some(edit.shapes.len() - 1);
        edit.name.clear();
    });
}

/// Give the shape being sculpted the name in the name field.
pub(crate) fn rename(world: &mut World) {
    if !ready(world) {
        return;
    }
    let (Some(i), wanted) = ({
        let e = world.resource::<ShapeEdit>();
        (e.active, e.name.trim().to_string())
    }) else {
        return;
    };
    if wanted.is_empty() {
        tell(world, false, "Type the new name into the name field first");
        return;
    }
    change_list(world, "Rename Shape", |edit| {
        let name = edit.unique_name(&wanted, Some(i));
        if let Some(s) = edit.shapes.get_mut(i) {
            s.name = name;
        }
        edit.name.clear();
    });
}

/// Remove the shape being sculpted.
pub(crate) fn delete(world: &mut World) {
    if !ready(world) {
        return;
    }
    let Some(i) = world.resource::<ShapeEdit>().active else { return };
    change_list(world, "Delete Shape", |edit| {
        if i < edit.shapes.len() {
            edit.shapes.remove(i);
        }
        edit.active = if edit.shapes.is_empty() { None } else { Some(i.min(edit.shapes.len() - 1)) };
    });
}

/// Flatten the shape being sculpted back to the rest shape.
pub(crate) fn clear(world: &mut World) {
    if !ready(world) {
        return;
    }
    let edit = world.resource::<ShapeEdit>();
    let Some(s) = edit.active_shape() else { return };
    let before = s.deltas.clone();
    let after: Vec<Vec<Vec3>> = before.iter().map(|d| vec![Vec3::ZERO; d.len()]).collect();
    let mut cmd = DeltasCmd { root: edit.root, shape: s.name.clone(), before, after, label: "Clear Shape" };
    renzora_undo::UndoCommand::execute(&mut cmd, world);
    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
}

/// Put the preview weights back to the rest shape (the shape being sculpted
/// stays at full).
pub(crate) fn rest_weights(world: &mut World) {
    let mut edit = world.resource_mut::<ShapeEdit>();
    let active = edit.active;
    for (i, s) in edit.shapes.iter_mut().enumerate() {
        s.weight = if Some(i) == active { 1.0 } else { 0.0 };
    }
    edit.dirty = true;
}

/// One shape's new displacements and normals on one part, in the file's
/// space (`axes`, see `renzora_rig::morphs::file_axes`).
fn target_data(edit: &ShapeEdit, shape: &Shape, pi: usize, axes: Vec3, shape_normals: &[Vec3]) -> TargetData {
    let part = &edit.parts[pi];
    let deltas = &shape.deltas[pi];
    let positions = deltas.iter().map(|d| *d * axes).collect();
    let normals = (0..part.rest.len())
        .map(|v| {
            let p = part.point[v] as usize;
            let base = part.normals.get(v).copied().unwrap_or_else(|| (part.inv[v] * edit.points.rest_normals[p]).normalize_or_zero());
            (vertex_normal(part, v, edit.points.rest_normals[p], shape_normals[p]) - base) * axes
        })
        .collect();
    TargetData { positions, normals: Some(normals) }
}

/// Write the shapes into the model file and reload it everywhere.
pub(crate) fn save(world: &mut World) {
    let Some(subject) = crate::rig::subject(world) else { return };
    let Some((abs, rel)) = crate::rig::model_file(world, subject) else {
        tell(world, false, "This entity has no model file to save the shapes into");
        return;
    };
    let lower = rel.to_ascii_lowercase();
    if !lower.ends_with(".glb") && !lower.ends_with(".rvg") {
        tell(world, false, format!("Blend shapes can only be saved into a .rvg or .glb model, and {rel} is neither"));
        return;
    }
    let result = {
        let edit = world.resource::<ShapeEdit>();
        // Each edited shape's surface at full strength, for its normals.
        let shape_normals: Vec<Option<Vec<Vec3>>> = edit
            .shapes
            .iter()
            .map(|s| s.edited.then(|| sculpt::normals(&displaced(edit, &s.deltas), &edit.points.triangles)))
            .collect();
        let mut meshes: Vec<usize> = edit.parts.iter().filter_map(|p| p.label.map(|l| l.0)).collect();
        meshes.sort_unstable();
        meshes.dedup();
        renzora_rig::write_shapes(&abs, |glb| {
            let mut out = Vec::new();
            for &m in &meshes {
                let parts: Vec<usize> = (0..edit.parts.len()).filter(|&pi| edit.parts[pi].label.is_some_and(|l| l.0 == m)).collect();
                let mut axes: HashMap<usize, Vec3> = HashMap::new();
                for &pi in &parts {
                    let prim = edit.parts[pi].label.map(|l| l.1).unwrap_or(0);
                    let file = glb.positions(m, prim)?;
                    let a = morphs::file_axes(&file, &edit.parts[pi].rest).ok_or_else(|| {
                        renzora_rig::GlbError::Unsupported(format!(
                            "mesh \"{}\" is not the one in the file (the engine rebuilt it on load), so its shapes cannot be saved",
                            glb.mesh_name(m)
                        ))
                    })?;
                    axes.insert(pi, a);
                }
                let count = morphs::target_count(glb, m);
                let file_names = morphs::target_names(glb, m);
                // The shapes the file had, in the file's order, then new ones.
                let mut kept: Vec<(usize, &Shape, usize)> = edit
                    .shapes
                    .iter()
                    .enumerate()
                    .filter_map(|(si, s)| s.source.iter().find(|(sm, ti)| *sm == m && *ti < count).map(|(_, ti)| (*ti, s, si)))
                    .collect();
                kept.sort_by_key(|(ti, ..)| *ti);
                let fresh = edit.shapes.iter().enumerate().filter(|(_, s)| !s.source.iter().any(|(sm, _)| *sm == m) && parts.iter().any(|&pi| s.moves_part(pi)));
                let mut targets: Vec<TargetEdit> = Vec::new();
                let data = |s: &Shape, si: usize| {
                    shape_normals[si].as_ref().map(|n| parts.iter().map(|&pi| (edit.parts[pi].label.map(|l| l.1).unwrap_or(0), target_data(edit, s, pi, axes[&pi], n))).collect())
                };
                for &(ti, s, si) in &kept {
                    targets.push(TargetEdit { name: s.name.clone(), source: Some(ti), data: data(s, si) });
                }
                for (si, s) in fresh {
                    targets.push(TargetEdit { name: s.name.clone(), source: None, data: data(s, si) });
                }
                // A mesh nothing changed on is left exactly as it is.
                let same = targets.len() == count
                    && targets.iter().enumerate().all(|(i, t)| t.source == Some(i) && t.data.is_none() && file_names.get(i) == Some(&t.name));
                if !same {
                    out.push((m, targets));
                }
            }
            Ok(out)
        })
    };
    match result {
        Ok(()) => {
            let keep = {
                let edit = world.resource::<ShapeEdit>();
                (edit.active_shape().map(|s| s.name.clone()), edit.shapes.iter().map(|s| (s.name.clone(), s.weight)).collect())
            };
            let mut edit = world.resource_mut::<ShapeEdit>();
            edit.unsaved = false;
            edit.keep = Some(keep);
            tell(world, true, format!("Saved the blend shapes into {rel}"));
            crate::rig::respawn::request(world, &rel);
        }
        Err(e) => tell(world, false, format!("Could not save the blend shapes into {rel}: {e}")),
    }
}

/// Throw the sculpting away: back to the shapes the file has.
pub(crate) fn revert(world: &mut World) {
    world.resource_mut::<ShapeEdit>().unsaved = false;
    restore(world);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(n: usize, turn: Mat3) -> Part {
        Part {
            entity: Entity::PLACEHOLDER,
            original: Handle::default(),
            own: Handle::default(),
            original_weights: None,
            label: Some((0, 0)),
            rest: vec![Vec3::ZERO; n],
            normals: vec![Vec3::Z; n],
            lin: vec![turn; n],
            inv: vec![turn.inverse(); n],
            point: (0..n as u32).collect(),
        }
    }

    /// A displacement made on the stage lands in each vertex's own space, so a
    /// mesh turned on the stage gets it turned back.
    #[test]
    fn a_stage_move_is_stored_in_the_vertex_space() {
        let turn = Mat3::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mut edit = ShapeEdit { parts: vec![part(2, turn)], ..default() };
        edit.points = Points { rest: vec![Vec3::ZERO, Vec3::X], group: vec![0, 0], members: vec![vec![(0, 0)], vec![(0, 1)]], ..default() };
        edit.shapes.push(Shape { name: "S".into(), source: Vec::new(), weight: 1.0, deltas: vec![vec![Vec3::ZERO; 2]], edited: false });
        displace(&mut edit, 0, &[(1, Vec3::X)], None);
        let d = edit.shapes[0].deltas[0][1];
        assert!((turn * d - Vec3::X).length() < 1e-5, "turned back to the stage it is X, got {d}");
        assert!(edit.shapes[0].edited);
        // And the stage sees it there again.
        let shown = displaced(&edit, &edit.shapes[0].deltas);
        assert!((shown[1] - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-5);
    }

    /// A skinned strip whose halves follow different bones (each vertex its
    /// own turn and scale): a smooth move made on the stage shows as exactly
    /// that move everywhere, the two vertices welded on the boundary included,
    /// so the bone boundary cannot open a seam.
    #[test]
    fn a_move_across_a_bone_boundary_stays_continuous() {
        let a = Mat3::from_rotation_z(0.4) * 1.3;
        let b = Mat3::from_rotation_x(-0.7) * 0.6;
        let n = 6;
        let mut p = part(n, a);
        for v in 3..n {
            p.lin[v] = b;
            p.inv[v] = b.inverse();
        }
        let mut edit = ShapeEdit { parts: vec![p], ..default() };
        // Vertices 2 and 3 are one point on the boundary.
        let members = vec![vec![(0, 0)], vec![(0, 1)], vec![(0, 2), (0, 3)], vec![(0, 4)], vec![(0, 5)]];
        edit.points = Points { rest: (0..5).map(|i| Vec3::X * i as f32).collect(), group: vec![0; 5], members, ..default() };
        edit.shapes.push(Shape { name: "S".into(), source: Vec::new(), weight: 1.0, deltas: vec![vec![Vec3::ZERO; n]], edited: false });
        let field = |i: u32| Vec3::new(0.1, 0.5, 0.2) * (1.0 + i as f32 * 0.1);
        let moves: Vec<(u32, Vec3)> = (0..5).map(|i| (i, field(i))).collect();
        displace(&mut edit, 0, &moves, None);
        let shown = displaced(&edit, &edit.shapes[0].deltas);
        for i in 0..5u32 {
            assert!((shown[i as usize] - (Vec3::X * i as f32 + field(i))).length() < 1e-5);
        }
        // Both vertices on the boundary point, drawn by their own bones, land
        // in the same place.
        let d = &edit.shapes[0].deltas[0];
        assert!((a * d[2] - b * d[3]).length() < 1e-5);
    }

    #[test]
    fn a_flat_surface_keeps_the_authored_normal() {
        let p = part(1, Mat3::IDENTITY);
        let n = vertex_normal(&p, 0, Vec3::Z, Vec3::Z);
        assert!((n - Vec3::Z).length() < 1e-6);
        let tilted = vertex_normal(&p, 0, Vec3::Z, Vec3::new(1.0, 0.0, 1.0).normalize());
        assert!(tilted.x > 0.5);
    }

    #[test]
    fn names_stay_unique() {
        let mut edit = ShapeEdit::default();
        edit.shapes.push(Shape { name: "Smile".into(), source: Vec::new(), weight: 0.0, deltas: Vec::new(), edited: false });
        assert_eq!(edit.unique_name("Smile", None), "Smile 2");
        assert_eq!(edit.unique_name("Smile", Some(0)), "Smile");
        assert_eq!(edit.unique_name("  ", None), "Shape");
    }

    #[test]
    fn labels_parse() {
        assert_eq!(parse_label("Mesh3/Primitive1"), Some((3, 1)));
        assert_eq!(parse_label("Scene0"), None);
    }
}

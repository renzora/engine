//! Skin mode: seeing, generating and painting the weights that tie the mesh
//! to the bones.
//!
//! On entering Skin, each skinned mesh of the stage's copy gets a mesh of its
//! own (so nothing painted shows on the scene's model before it is saved) and
//! an unlit material showing the painted bone's weight as heat, dark blue for
//! none through red for all. Leaving puts the copy's own meshes and materials
//! back, saving first if anything was painted.
//!
//! - **The brush** paints the chosen bone (the panel's list, or Ctrl+click a
//!   bone on the model): Add, Remove, Smooth, Set, with a radius, a strength,
//!   a falloff and optional mirroring. It paints the mesh as it is posed, so a
//!   bent elbow is painted in the shape it actually makes: the stroke is cast
//!   against the skinned triangles, computed here, not the mesh at rest. Each
//!   stroke is one undo step.
//! - **Auto weights** weights every vertex by closeness to every bone
//!   (`renzora_rig::weights::envelope`), optionally smoothed across the mesh.
//! - **Test** bends the chosen bone, or plays a range of motion through every
//!   joint, on the copy only.
//! - **Save** writes the weights into the model file
//!   (`renzora_rig::write_weights`) and reloads it everywhere.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

use renzora_rig::weights::{self, Brush, Falloff, Health, Influences};

use super::clone::StageRig;
use super::view::{StageCursor, StageNav};
use super::StageMode;
use crate::rig::tell;

/// One skinned mesh of the copy, being painted.
pub(crate) struct PaintMesh {
    entity: Entity,
    original: Handle<Mesh>,
    original_material: Option<Handle<StandardMaterial>>,
    mesh: Handle<Mesh>,
    /// The skin's joints (copy entities) and their names, in skin order: a
    /// weight's joint index points into these.
    joints: Vec<Entity>,
    joint_names: Vec<String>,
    inverse_binds: Vec<Mat4>,
    /// At rest, in the mesh's own space.
    positions: Vec<Vec3>,
    indices: Vec<u32>,
    adjacency: Option<Vec<Vec<u32>>>,
    pub(crate) weights: Vec<Influences>,
    /// `(mesh, primitive)` in the model file, from the asset's label.
    label: Option<(usize, usize)>,
    /// The mesh entity's world transform, the space its bind pose is in.
    mesh_world: Mat4,
}

impl PaintMesh {
    fn joint(&self, name: &str) -> Option<u16> {
        self.joint_names.iter().position(|n| n == name).map(|i| i as u16)
    }
}

#[derive(Resource)]
pub(crate) struct SkinEdit {
    pub(crate) meshes: Vec<PaintMesh>,
    /// The copy these meshes belong to.
    root: Option<Entity>,
    pub(crate) paint_bone: Option<String>,
    pub(crate) brush: Brush,
    /// A fraction of the model's size.
    pub(crate) radius: f32,
    pub(crate) strength: f32,
    pub(crate) falloff: Falloff,
    pub(crate) mirror: bool,
    /// Auto weights: 0 smooth, 1 sharp.
    pub(crate) method: usize,
    pub(crate) unsaved: bool,
    /// The test bend, degrees about the bone's own X, Y, Z, by bone name.
    pub(crate) test: HashMap<String, Vec3>,
    pub(crate) range_of_motion: bool,
    rom_time: f32,
    /// The model's size, which the brush radius is a fraction of.
    size: f32,
    stroke: Option<Vec<Vec<Influences>>>,
    dirty: bool,
    pub(crate) health: Health,
    /// Where the brush is on the model, for drawing it.
    pub(crate) hit: Option<Vec3>,
    selection_seen: u64,
}

impl Default for SkinEdit {
    fn default() -> Self {
        Self {
            meshes: Vec::new(),
            root: None,
            paint_bone: None,
            brush: Brush::Add,
            radius: 0.06,
            strength: 0.5,
            falloff: Falloff::Smooth,
            mirror: true,
            method: 0,
            unsaved: false,
            test: HashMap::new(),
            range_of_motion: false,
            rom_time: 0.0,
            size: 1.0,
            stroke: None,
            dirty: false,
            health: Health::default(),
            hit: None,
            selection_seen: 0,
        }
    }
}

impl SkinEdit {
    pub(crate) fn brush_radius(&self) -> f32 {
        self.radius * self.size
    }

    pub(crate) fn active(&self) -> bool {
        self.root.is_some()
    }

    /// Recolour the heat, after the painted bone changed.
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<SkinEdit>().add_systems(
        Update,
        (sync_skin, paint, refresh_meshes)
            .chain()
            .after(super::view::StageInputSet)
            .run_if(in_state(renzora_editor_framework::SplashState::Editor))
            .run_if(super::stage_mounted)
            .run_if(renzora::not_in_play_mode),
    );
}

// ── Entering and leaving ─────────────────────────────────────────────────────

fn sync_skin(world: &mut World) {
    let in_skin = super::mode(world) == StageMode::Skin;
    let root = world.resource::<StageRig>().root;
    let current = world.resource::<SkinEdit>().root;
    if current.is_some() && (!in_skin || current != root) {
        if world.resource::<SkinEdit>().unsaved && current == root {
            save(world);
        }
        restore(world);
    }
    // Only once the copy has its skeleton: right after a save the copy is
    // made again, and its meshes arrive a few frames after its root does.
    if in_skin && world.resource::<StageRig>().rigged() && world.resource::<SkinEdit>().root != root {
        if let Some(r) = root {
            enter(world, r);
        }
    }
    if in_skin {
        follow_selection(world);
        let dt = world.resource::<Time>().delta_secs();
        let mut edit = world.resource_mut::<SkinEdit>();
        if edit.range_of_motion {
            edit.rom_time += dt;
        }
    }
}

/// Selecting a bone (Ctrl+click on the model, or anywhere else) makes it the
/// one painted.
fn follow_selection(world: &mut World) {
    let Some(sel) = world.get_resource::<renzora_editor_framework::EditorSelection>() else { return };
    let version = sel.version();
    if world.resource::<SkinEdit>().selection_seen == version {
        return;
    }
    let name = sel.get().and_then(|real| world.resource::<StageRig>().real_names.get(&real).cloned());
    let mut edit = world.resource_mut::<SkinEdit>();
    edit.selection_seen = version;
    if let Some(name) = name {
        if edit.paint_bone.as_deref() != Some(&name) {
            edit.paint_bone = Some(name);
            edit.dirty = true;
        }
    }
}

fn heat_material(world: &mut World) -> Handle<StandardMaterial> {
    world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        cull_mode: None,
        ..default()
    })
}

fn enter(world: &mut World, root: Entity) {
    // Every skinned mesh must have loaded first, or the ones still loading
    // would never be painted.
    let loading = crate::rig::descendants(world, root).into_iter().any(|e| {
        world.get::<SkinnedMesh>(e).is_some() && world.get::<Mesh3d>(e).is_some_and(|m| world.resource::<Assets<Mesh>>().get(&m.0).is_none())
    });
    if loading {
        return;
    }
    let heat = heat_material(world);
    let rig_names = world.resource::<StageRig>().names.clone();
    let mut out = Vec::new();
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for e in crate::rig::descendants(world, root) {
        let (Some(mesh3d), Some(skin), Some(gt)) = (world.get::<Mesh3d>(e), world.get::<SkinnedMesh>(e), world.get::<GlobalTransform>(e)) else {
            continue;
        };
        let original = mesh3d.0.clone();
        let joints = skin.joints.clone();
        let binds_handle = skin.inverse_bindposes.clone();
        let mesh_world = gt.to_matrix();
        let Some(mesh) = world.resource::<Assets<Mesh>>().get(&original).cloned() else { continue };
        let inverse_binds: Vec<Mat4> = world
            .resource::<Assets<SkinnedMeshInverseBindposes>>()
            .get(&binds_handle)
            .map(|b| b.iter().copied().collect())
            .unwrap_or_default();
        let positions: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(p)) => p.iter().map(|v| Vec3::from_array(*v)).collect(),
            _ => continue,
        };
        let indices: Vec<u32> = match mesh.indices() {
            Some(Indices::U16(i)) => i.iter().map(|v| *v as u32).collect(),
            Some(Indices::U32(i)) => i.clone(),
            None => (0..positions.len() as u32).collect(),
        };
        let joint_index: Vec<[u16; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX) {
            Some(VertexAttributeValues::Uint16x4(j)) => j.clone(),
            _ => vec![[0; 4]; positions.len()],
        };
        let joint_weight: Vec<[f32; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT) {
            Some(VertexAttributeValues::Float32x4(w)) => w.clone(),
            _ => vec![[1.0, 0.0, 0.0, 0.0]; positions.len()],
        };
        let weights: Vec<Influences> = joint_index
            .iter()
            .zip(&joint_weight)
            .map(|(j, w)| [(j[0], w[0]), (j[1], w[1]), (j[2], w[2]), (j[3], w[3])])
            .collect();
        let label = world
            .resource::<AssetServer>()
            .get_path(original.id())
            .and_then(|p| p.label().map(str::to_string))
            .and_then(|l| parse_label(&l));
        for p in &positions {
            let w = mesh_world.transform_point3(*p);
            min = min.min(w);
            max = max.max(w);
        }
        let mut own = mesh;
        own.asset_usage = RenderAssetUsages::all();
        let handle = world.resource_mut::<Assets<Mesh>>().add(own);
        let original_material = world.get::<MeshMaterial3d<StandardMaterial>>(e).map(|m| m.0.clone());
        world.entity_mut(e).insert((Mesh3d(handle.clone()), MeshMaterial3d(heat.clone())));
        let joint_names = joints.iter().map(|j| rig_names.get(j).cloned().unwrap_or_default()).collect();
        out.push(PaintMesh {
            entity: e,
            original,
            original_material,
            mesh: handle,
            joints,
            joint_names,
            inverse_binds,
            positions,
            indices,
            adjacency: None,
            weights,
            label,
            mesh_world,
        });
    }
    let size = if min.x <= max.x { (max - min).length().max(1e-3) } else { 1.0 };
    let first_joint = out.first().and_then(|m| m.joint_names.first().cloned());
    let mut edit = world.resource_mut::<SkinEdit>();
    edit.meshes = out;
    edit.root = Some(root);
    edit.size = size;
    edit.unsaved = false;
    edit.stroke = None;
    edit.dirty = true;
    edit.selection_seen = 0;
    if edit.paint_bone.is_none() {
        edit.paint_bone = first_joint;
    }
}

/// `Mesh3/Primitive1` → `(3, 1)`.
fn parse_label(label: &str) -> Option<(usize, usize)> {
    let (m, p) = label.split_once('/')?;
    Some((m.strip_prefix("Mesh")?.parse().ok()?, p.strip_prefix("Primitive")?.parse().ok()?))
}

fn restore(world: &mut World) {
    let meshes = std::mem::take(&mut world.resource_mut::<SkinEdit>().meshes);
    for m in meshes {
        if let Ok(mut e) = world.get_entity_mut(m.entity) {
            e.insert(Mesh3d(m.original.clone()));
            match m.original_material {
                Some(mat) => {
                    e.insert(MeshMaterial3d(mat));
                }
                None => {
                    e.remove::<MeshMaterial3d<StandardMaterial>>();
                }
            }
        }
        world.resource_mut::<Assets<Mesh>>().remove(&m.mesh);
    }
    let mut edit = world.resource_mut::<SkinEdit>();
    edit.root = None;
    edit.unsaved = false;
    edit.hit = None;
}

// ── Painting ─────────────────────────────────────────────────────────────────

/// Each mesh's vertices as posed now, in world space: what the GPU draws.
fn skinned_positions(world: &World, m: &PaintMesh) -> Vec<Vec3> {
    let mats: Vec<Mat4> = m
        .joints
        .iter()
        .zip(&m.inverse_binds)
        .map(|(j, ibm)| world.get::<GlobalTransform>(*j).map(|g| g.to_matrix()).unwrap_or(Mat4::IDENTITY) * *ibm)
        .collect();
    m.positions
        .iter()
        .zip(&m.weights)
        .map(|(p, inf)| {
            let mut out = Vec3::ZERO;
            let mut total = 0.0;
            for (j, w) in inf {
                if *w <= 0.0 {
                    continue;
                }
                if let Some(mat) = mats.get(*j as usize) {
                    out += mat.transform_point3(*p) * *w;
                    total += *w;
                }
            }
            if total > 1e-6 { out / total } else { m.mesh_world.transform_point3(*p) }
        })
        .collect()
}

fn ray_triangle(ray: Ray3d, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = ray.direction.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
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

fn paint(world: &mut World) {
    if super::mode(world) != StageMode::Skin || !world.resource::<SkinEdit>().active() {
        return;
    }
    let (over, ray) = {
        let c = world.resource::<StageCursor>();
        (c.over, c.ray)
    };
    let (pressed, released) = {
        let m = world.resource::<ButtonInput<MouseButton>>();
        (m.pressed(MouseButton::Left), m.just_released(MouseButton::Left))
    };
    let ctrl = world.resource::<ButtonInput<KeyCode>>().any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let navigating = world.resource::<StageNav>().dragging.is_some();

    if released || !pressed {
        if let Some(before) = world.resource_mut::<SkinEdit>().stroke.take() {
            let root = world.resource::<SkinEdit>().root;
            let after: Vec<Vec<Influences>> = world.resource::<SkinEdit>().meshes.iter().map(|m| m.weights.clone()).collect();
            renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(WeightsCmd { root, before, after, label: "Paint Weights" }));
        }
    }

    let Some(ray) = ray.filter(|_| over && !navigating) else {
        world.resource_mut::<SkinEdit>().hit = None;
        return;
    };
    // Where the brush is on the posed mesh.
    let posed: Vec<Vec<Vec3>> = {
        let edit = world.resource::<SkinEdit>();
        edit.meshes.iter().map(|m| skinned_positions(world, m)).collect()
    };
    let hit = {
        let edit = world.resource::<SkinEdit>();
        let mut best: Option<f32> = None;
        for (m, pos) in edit.meshes.iter().zip(&posed) {
            for tri in m.indices.chunks_exact(3) {
                let (Some(a), Some(b), Some(c)) = (pos.get(tri[0] as usize), pos.get(tri[1] as usize), pos.get(tri[2] as usize)) else { continue };
                if let Some(t) = ray_triangle(ray, *a, *b, *c) {
                    if best.is_none_or(|b| t < b) {
                        best = Some(t);
                    }
                }
            }
        }
        best.map(|t| ray.origin + *ray.direction * t)
    };
    world.resource_mut::<SkinEdit>().hit = hit;
    let painting = pressed && !ctrl && world.get_resource::<crate::rig::markers::Markers>().is_none_or(|m| m.placing.is_none());
    let (Some(h), true) = (hit, painting) else { return };
    let dt = world.resource::<Time>().delta_secs().min(0.05);

    let mut edit = world.resource_mut::<SkinEdit>();
    let Some(bone) = edit.paint_bone.clone() else { return };
    if edit.stroke.is_none() {
        edit.stroke = Some(edit.meshes.iter().map(|m| m.weights.clone()).collect());
    }
    let r = edit.brush_radius();
    let (brush, strength, falloff) = (edit.brush, edit.strength, edit.falloff);
    let mut dabs = vec![(h, bone.clone())];
    if edit.mirror {
        if let Some(other) = renzora_rig::mirror::mirror_name(&bone) {
            dabs.push((Vec3::new(-h.x, h.y, h.z), other));
        }
    }
    // A held brush builds up at a steady rate whatever the frame rate; Set is
    // exact and does not need to.
    let rate = if brush == Brush::Set { 1.0 } else { (dt * 6.0).min(1.0) };
    for (mi, pos) in posed.iter().enumerate() {
        for (centre, name) in &dabs {
            let m = &mut edit.meshes[mi];
            let Some(joint) = m.joint(name) else { continue };
            if brush == Brush::Smooth && m.adjacency.is_none() {
                m.adjacency = Some(weights::adjacency(&m.indices, m.positions.len()));
            }
            let before = m.weights.clone();
            for (v, p) in pos.iter().enumerate() {
                let d = p.distance(*centre);
                if d > r {
                    continue;
                }
                let amount = strength.max(0.05) * weights::falloff(falloff, d / r) * rate;
                let current = weights::weight_of(&before[v], joint);
                let around = match (&m.adjacency, brush) {
                    (Some(adj), Brush::Smooth) if !adj[v].is_empty() => {
                        adj[v].iter().map(|&n| weights::weight_of(&before[n as usize], joint)).sum::<f32>() / adj[v].len() as f32
                    }
                    _ => current,
                };
                let target = weights::dab(brush, current, if brush == Brush::Set { weights::falloff(falloff, d / r) } else { amount }, strength, around);
                weights::set_weight(&mut m.weights[v], joint, target);
            }
        }
    }
    edit.dirty = true;
    edit.unsaved = true;
}

/// One stroke, or one Auto Weights: the weights before and after.
struct WeightsCmd {
    root: Option<Entity>,
    before: Vec<Vec<Influences>>,
    after: Vec<Vec<Influences>>,
    label: &'static str,
}

impl WeightsCmd {
    fn put(&self, world: &mut World, which: &[Vec<Influences>]) {
        let mut edit = world.resource_mut::<SkinEdit>();
        if edit.root != self.root || edit.meshes.len() != which.len() {
            return;
        }
        for (m, w) in edit.meshes.iter_mut().zip(which) {
            if m.weights.len() == w.len() {
                m.weights.clone_from(w);
            }
        }
        edit.dirty = true;
        edit.unsaved = true;
    }
}

impl renzora_undo::UndoCommand for WeightsCmd {
    fn label(&self) -> &str {
        self.label
    }
    fn execute(&mut self, world: &mut World) {
        let after = self.after.clone();
        self.put(world, &after);
    }
    fn undo(&mut self, world: &mut World) {
        let before = self.before.clone();
        self.put(world, &before);
    }
}

/// A mesh's new joints, weights and heat colours.
type MeshUpdate = (Handle<Mesh>, Vec<[u16; 4]>, Vec<[f32; 4]>, Vec<[f32; 4]>);

/// Push changed weights into the meshes the GPU draws, and recolour the heat.
fn refresh_meshes(world: &mut World) {
    if !world.resource::<SkinEdit>().dirty {
        return;
    }
    let (updates, health) = {
        let edit = world.resource::<SkinEdit>();
        let bone = edit.paint_bone.clone().unwrap_or_default();
        let updates: Vec<MeshUpdate> = edit
            .meshes
            .iter()
            .map(|m| {
                let joint = m.joint(&bone);
                let j = m.weights.iter().map(|i| [i[0].0, i[1].0, i[2].0, i[3].0]).collect();
                let w = m.weights.iter().map(|i| [i[0].1, i[1].1, i[2].1, i[3].1]).collect();
                let c = m.weights.iter().map(|i| heat(joint.map(|j| weights::weight_of(i, j)).unwrap_or(0.0))).collect();
                (m.mesh.clone(), j, w, c)
            })
            .collect();
        let all: Vec<Influences> = edit.meshes.iter().flat_map(|m| m.weights.iter().copied()).collect();
        (updates, weights::health(&all))
    };
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    for (h, j, w, c) in updates {
        if let Some(mut mesh) = meshes.get_mut(&h) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(j));
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, w);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, c);
        }
    }
    let mut edit = world.resource_mut::<SkinEdit>();
    edit.dirty = false;
    edit.health = health;
}

/// A weight as heat: dark blue for none, through cyan, green and yellow, to
/// red for all.
fn heat(w: f32) -> [f32; 4] {
    const STOPS: [(u8, u8, u8); 6] = [(38, 52, 107), (47, 154, 176), (127, 209, 107), (242, 209, 74), (240, 130, 62), (224, 71, 62)];
    let t = w.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let i = (t.floor() as usize).min(STOPS.len() - 2);
    let f = t - i as f32;
    let (a, b) = (STOPS[i], STOPS[i + 1]);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f) / 255.0;
    let c = Color::srgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2)).to_linear();
    [c.red, c.green, c.blue, 1.0]
}

// ── Actions ──────────────────────────────────────────────────────────────────

/// Weight every vertex by closeness to every bone of its skin, then, for the
/// smooth method, blur the result across the mesh.
pub(crate) fn auto_weights(world: &mut World) {
    if !world.resource::<SkinEdit>().active() {
        tell(world, false, "Switch to Skin with a rigged model on the stage first");
        return;
    }
    let parents = world.resource::<StageRig>().parent.clone();
    let before: Vec<Vec<Influences>> = world.resource::<SkinEdit>().meshes.iter().map(|m| m.weights.clone()).collect();
    let smooth = world.resource::<SkinEdit>().method == 0;
    let mut edit = world.resource_mut::<SkinEdit>();
    for m in edit.meshes.iter_mut() {
        let bind: Vec<Vec3> = m.inverse_binds.iter().map(|ibm| (m.mesh_world * ibm.inverse()).transform_point3(Vec3::ZERO)).collect();
        let mut segments = Vec::new();
        for (i, j) in m.joints.iter().enumerate() {
            let children: Vec<usize> = m.joints.iter().enumerate().filter(|(_, c)| parents.get(c) == Some(j)).map(|(k, _)| k).collect();
            if children.is_empty() {
                segments.push(weights::Segment { joint: i as u16, a: bind[i], b: bind[i] });
            }
            for k in children {
                segments.push(weights::Segment { joint: i as u16, a: bind[i], b: bind[k] });
            }
        }
        let world_positions: Vec<Vec3> = m.positions.iter().map(|p| m.mesh_world.transform_point3(*p)).collect();
        m.weights = weights::envelope(&world_positions, &segments, 6.0);
        if smooth {
            if m.adjacency.is_none() {
                m.adjacency = Some(weights::adjacency(&m.indices, m.positions.len()));
            }
            if let Some(adj) = &m.adjacency {
                weights::smooth(&mut m.weights, adj, 3, 0.5);
            }
        }
    }
    edit.dirty = true;
    edit.unsaved = true;
    let root = edit.root;
    let after: Vec<Vec<Influences>> = edit.meshes.iter().map(|m| m.weights.clone()).collect();
    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(WeightsCmd { root, before, after, label: "Auto Weights" }));
    tell(world, true, "Weighted the model to its bones. Paint to adjust, then Save.");
}

pub(crate) fn normalise(world: &mut World) {
    let before: Vec<Vec<Influences>> = world.resource::<SkinEdit>().meshes.iter().map(|m| m.weights.clone()).collect();
    let mut edit = world.resource_mut::<SkinEdit>();
    for m in edit.meshes.iter_mut() {
        weights::normalise(&mut m.weights);
    }
    edit.dirty = true;
    edit.unsaved = true;
    let root = edit.root;
    let after: Vec<Vec<Influences>> = edit.meshes.iter().map(|m| m.weights.clone()).collect();
    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(WeightsCmd { root, before, after, label: "Normalise Weights" }));
}

/// Write the weights into the model file and reload it everywhere.
pub(crate) fn save(world: &mut World) {
    let Some(subject) = crate::rig::subject(world) else { return };
    let Some((abs, rel)) = crate::rig::model_file(world, subject) else {
        tell(world, false, "This entity has no model file to save the weights into");
        return;
    };
    let edits: Vec<(usize, usize, Vec<Influences>)> = world
        .resource::<SkinEdit>()
        .meshes
        .iter()
        .filter_map(|m| Some((m.label?.0, m.label?.1, m.weights.clone())))
        .collect();
    if edits.is_empty() {
        tell(world, false, "Nothing to save: the painted meshes could not be matched to the file");
        return;
    }
    match renzora_rig::write_weights(&abs, &edits) {
        Ok(()) => {
            world.resource_mut::<SkinEdit>().unsaved = false;
            tell(world, true, format!("Saved the weights into {rel}"));
            crate::rig::respawn::request(world, &rel);
        }
        Err(e) => tell(world, false, format!("Could not save the weights into {rel}: {e}")),
    }
}

/// Throw the painting away: back to the weights the file has.
pub(crate) fn revert(world: &mut World) {
    world.resource_mut::<SkinEdit>().unsaved = false;
    restore(world);
}

// ── Testing ──────────────────────────────────────────────────────────────────

/// The copy's pose in Skin: the bind pose, bent by the test sliders and the
/// range of motion.
pub(crate) fn test_pose(world: &World, rig: &StageRig) -> Vec<(Entity, Transform)> {
    let Some(edit) = world.get_resource::<SkinEdit>() else {
        return rig.rest.iter().map(|(e, t)| (*e, *t)).collect();
    };
    let depth = |mut e: Entity| {
        let mut d = 0;
        while let Some(p) = rig.parent.get(&e) {
            e = *p;
            d += 1;
        }
        d
    };
    rig.rest
        .iter()
        .map(|(e, rest)| {
            let mut t = *rest;
            if let Some(deg) = rig.names.get(e).and_then(|n| edit.test.get(n)) {
                let r = Quat::from_euler(EulerRot::XYZ, deg.x.to_radians(), deg.y.to_radians(), deg.z.to_radians());
                t.rotation = (rest.rotation * r).normalize();
            }
            if edit.range_of_motion && rig.parent.contains_key(e) {
                // Every joint swings, a little behind its parent, so the bend
                // travels down each limb the way a wave does.
                let phase = edit.rom_time * 1.7 - depth(*e) as f32 * 0.55;
                let r = Quat::from_euler(EulerRot::XYZ, phase.sin() * 0.55, (phase * 0.5).sin() * 0.25, (phase * 0.8).cos() * 0.35);
                t.rotation = (t.rotation * r).normalize();
            }
            (*e, t)
        })
        .collect()
}

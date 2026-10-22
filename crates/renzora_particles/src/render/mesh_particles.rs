//! Mesh particles: each particle drawn as a copy of a mesh instead of a quad.
//!
//! The source is a built-in shape or a model in the project
//! ([`ParticleMesh`]). Either way it is reduced once to a [`MeshTemplate`]
//! (positions, normals, UVs, tangents and indices on the CPU), cached by
//! shape in [`MeshTemplates`], because every renderer of that shape draws the
//! same copies.
//!
//! ## Two ways to feed them
//!
//! - **CPU.** The emitter's mesh holds one copy of the template per live
//!   particle. Every vertex carries its particle exactly as a quad corner
//!   does, plus its own position and normal in the template, and the vertex
//!   shader (under `MESH_PARTICLES`) turns, sizes and places the copy. The
//!   orientation is computed on the GPU from data the vertex already has, so
//!   nothing here needs the camera.
//! - **GPU.** The render child draws a pool mesh with one copy per pool slot
//!   ([`gpu_pool_mesh`]), built once per (shape, capacity), and the vertex
//!   shader fetches each copy's particle from the simulation's storage buffer
//!   by its slot. The same shader code as the CPU path from the fetch on.
//!
//! Either way the copies are drawn by the particle shader or by a project
//! `.material` (`super::surface`), whose pipeline runs the same vertex
//! expansion. A material used to need the CPU to build finished meshes,
//! because its own vertex shader could not read particles; it no longer does.
//! [`mesh_rotation`] is the Rust twin of the shader's `mesh_orientation`, held
//! to it by the tests.
//!
//! ## Loading
//!
//! A model loads through the asset server like any glTF sub-asset
//! (`rock.glb#Mesh0/Primitive0`), so the template appears some frames after
//! the renderer. Until then the renderer draws nothing; [`resolve_mesh_particles`]
//! fills it in, and a model that fails to load says so in the Console once.
//! A model that reloads (re-imported, or edited on disk) drops its cached
//! template and is read again.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::math::{Mat3, Vec2, Vec3, Vec4};
use bevy::mesh::{
    Capsule3dMeshBuilder, ConeMeshBuilder, CylinderMeshBuilder, Indices, MeshBuilder, PrimitiveTopology,
    TorusMeshBuilder, VertexAttributeValues,
};
use bevy::prelude::*;

use super::material::ATTRIBUTE_PARTICLE_LOCAL;
use crate::asset::{Facing, ParticleMesh};

/// The most vertices one mesh particle may have. Every particle is a full
/// copy, so a model past this is almost certainly not meant to be one, and
/// refusing it says so instead of building a mesh of tens of millions of
/// vertices.
pub const MAX_TEMPLATE_VERTICES: usize = 65_536;

/// The most vertices a GPU pool mesh holds (one copy per slot). Past it the
/// pool draws only as many slots as fit, with a Console line saying so: 4 M
/// vertices is about 150 MB of vertex data for a pool nobody can see the end
/// of anyway.
pub const MAX_GPU_POOL_VERTICES: usize = 4 << 20;

/// A mesh reduced to what a particle copy needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshTemplate {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    /// For a project material with a normal map; generated when the source
    /// has none (MikkTSpace where the mesh allows it, else any perpendicular).
    pub tangents: Vec<Vec4>,
    pub indices: Vec<u32>,
    /// The farthest vertex from the origin, for bounds.
    pub radius: f32,
}

impl MeshTemplate {
    /// The mesh of a built-in shape, unit sized and centred, low poly because
    /// every particle is a full copy. `None` for [`ParticleMesh::Asset`].
    pub fn built_in_mesh(shape: &ParticleMesh) -> Option<Mesh> {
        Some(match shape {
            ParticleMesh::Cube => Mesh::from(Cuboid::new(1.0, 1.0, 1.0)),
            ParticleMesh::Sphere => Sphere::new(0.5).mesh().uv(16, 10),
            ParticleMesh::Cylinder => CylinderMeshBuilder::new(0.5, 1.0, 12).build(),
            ParticleMesh::Capsule => Capsule3dMeshBuilder::new(0.25, 0.5, 12, 6).build(),
            ParticleMesh::Cone => ConeMeshBuilder::new(0.5, 1.0, 12).build(),
            ParticleMesh::Torus => TorusMeshBuilder::new(0.25, 0.5).minor_resolution(8).major_resolution(16).build(),
            ParticleMesh::Tetrahedron => Mesh::from(Tetrahedron::default()),
            ParticleMesh::Quad => Mesh::from(Rectangle::new(1.0, 1.0)),
            ParticleMesh::Asset(_) => return None,
        })
    }

    /// A built-in shape's template.
    pub fn built_in(shape: &ParticleMesh) -> Option<Self> {
        Self::built_in_mesh(shape).and_then(|m| Self::from_mesh(&m).ok())
    }

    /// Reduce `mesh` to a template, or say why it cannot be one.
    pub fn from_mesh(mesh: &Mesh) -> Result<Self, String> {
        if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
            return Err("it is not a triangle mesh".to_string());
        }
        let positions: Vec<Vec3> = match mesh.try_attribute(Mesh::ATTRIBUTE_POSITION) {
            Ok(VertexAttributeValues::Float32x3(p)) => p.iter().map(|p| Vec3::from_array(*p)).collect(),
            Ok(_) => return Err("its positions are not 3D floats".to_string()),
            Err(_) => return Err("its vertex data is not kept on the CPU".to_string()),
        };
        let n = positions.len();
        if n == 0 {
            return Err("it has no vertices".to_string());
        }
        if n > MAX_TEMPLATE_VERTICES {
            return Err(format!("it has {n} vertices; a particle mesh may have up to {MAX_TEMPLATE_VERTICES}"));
        }
        let indices: Vec<u32> = match mesh.indices() {
            Some(Indices::U16(i)) => i.iter().map(|&i| u32::from(i)).collect(),
            Some(Indices::U32(i)) => i.clone(),
            None => (0..n as u32).collect(),
        };
        if indices.iter().any(|&i| i as usize >= n) {
            return Err("its indices point past its vertices".to_string());
        }
        let float3 = |m: &Mesh, id| match m.try_attribute_option(id) {
            Ok(Some(VertexAttributeValues::Float32x3(v))) if v.len() == n => {
                Some(v.iter().map(|v| Vec3::from_array(*v)).collect::<Vec<_>>())
            }
            _ => None,
        };
        // A model without normals gets them computed, so it can still be lit.
        let mut with_normals = None;
        // Unit length whatever the source wrote (Bevy's own cone leaves its
        // tip normals unnormalized), since the shader scales them by size.
        let normals = match float3(mesh, Mesh::ATTRIBUTE_NORMAL) {
            Some(v) => v.into_iter().map(|n| n.normalize_or(Vec3::Y)).collect(),
            None => {
                let mut m = mesh.clone();
                m.compute_normals();
                let v = float3(&m, Mesh::ATTRIBUTE_NORMAL).unwrap_or_else(|| vec![Vec3::Y; n]);
                with_normals = Some(m);
                v
            }
        };
        let uvs: Vec<Vec2> = match mesh.try_attribute_option(Mesh::ATTRIBUTE_UV_0) {
            Ok(Some(VertexAttributeValues::Float32x2(v))) if v.len() == n => v.iter().map(|v| Vec2::from_array(*v)).collect(),
            _ => vec![Vec2::ZERO; n],
        };
        let float4 = |m: &Mesh| match m.try_attribute_option(Mesh::ATTRIBUTE_TANGENT) {
            Ok(Some(VertexAttributeValues::Float32x4(v))) if v.len() == n => {
                Some(v.iter().map(|v| Vec4::from_array(*v)).collect::<Vec<_>>())
            }
            _ => None,
        };
        let tangents = float4(mesh)
            .or_else(|| {
                let m = with_normals.unwrap_or_else(|| mesh.clone());
                m.with_generated_tangents().ok().and_then(|m| float4(&m))
            })
            .unwrap_or_else(|| normals.iter().map(|n| any_tangent(*n)).collect());
        let radius = positions.iter().map(|p| p.length()).fold(0.0, f32::max);
        Ok(Self { positions, normals, uvs, tangents, indices, radius })
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }
}

/// A unit tangent perpendicular to `n`, for a mesh MikkTSpace cannot handle.
fn any_tangent(n: Vec3) -> Vec4 {
    let helper = if n.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
    let t = helper.cross(n).normalize_or(Vec3::X);
    t.extend(1.0)
}

/// The asset path a model's mesh loads from: the path as given when it names
/// a sub-asset, else its first mesh's first primitive.
pub fn model_path(path: &str) -> String {
    let path = path.trim();
    if path.contains('#') {
        path.to_string()
    } else {
        format!("{path}#Mesh0/Primitive0")
    }
}

/// On a mesh-particle render child: which mesh it draws and, once known, its
/// template.
#[derive(Component)]
pub struct MeshParticles {
    pub shape: ParticleMesh,
    /// A model's mesh, held so it stays loaded while it is drawn.
    pub source: Option<Handle<Mesh>>,
    pub template: Option<Arc<MeshTemplate>>,
    /// On a GPU render child: the pool capacity its pool mesh is built for,
    /// swapped in for the placeholder once the template exists.
    pub gpu_capacity: Option<u32>,
    /// The model could not be used; said once, then left alone.
    pub failed: bool,
}

impl MeshParticles {
    pub fn new(shape: ParticleMesh, asset_server: &AssetServer, gpu_capacity: Option<u32>) -> Self {
        let source = match &shape {
            ParticleMesh::Asset(path) if !path.trim().is_empty() => Some(asset_server.load::<Mesh>(model_path(path))),
            _ => None,
        };
        Self { shape, source, template: None, gpu_capacity, failed: false }
    }
}

/// Templates by shape, and GPU pool meshes by (shape, capacity), so every
/// emitter of a shape shares one of each.
#[derive(Resource, Default)]
pub struct MeshTemplates {
    templates: HashMap<ParticleMesh, Arc<MeshTemplate>>,
    pools: HashMap<(ParticleMesh, u32), Handle<Mesh>>,
}

impl MeshTemplates {
    /// Forget a model's template and pools, so the next renderer of it reads
    /// the mesh again.
    pub fn forget(&mut self, shape: &ParticleMesh) {
        self.templates.remove(shape);
        self.pools.retain(|(s, _), _| s != shape);
    }
}

/// Give every mesh-particle renderer its template once the mesh is there, and
/// a GPU renderer its pool mesh.
pub fn resolve_mesh_particles(
    mut renderers: Query<(&mut MeshParticles, &mut Mesh3d)>,
    mut templates: ResMut<MeshTemplates>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut events: MessageReader<AssetEvent<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    // A model re-imported (or edited) on disk reloads its mesh: forget the
    // old template so every renderer of it reads the new one. The particle
    // meshes themselves are modified every frame, so only a model source is
    // looked at.
    let reloaded: Vec<AssetId<Mesh>> = events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    if !reloaded.is_empty() {
        for (mut particles, _) in &mut renderers {
            let Some(source) = particles.source.as_ref().map(Handle::id) else { continue };
            if reloaded.contains(&source) {
                templates.forget(&particles.shape);
                particles.template = None;
                particles.failed = false;
            }
        }
    }
    for (mut particles, mut mesh3d) in &mut renderers {
        if particles.template.is_some() || particles.failed {
            continue;
        }
        let shape = particles.shape.clone();
        let template = match templates.templates.get(&shape) {
            Some(t) => t.clone(),
            None => {
                let built = match &shape {
                    ParticleMesh::Asset(path) => {
                        let Some(handle) = particles.source.as_ref() else {
                            particles.failed = true;
                            continue;
                        };
                        match meshes.get(handle) {
                            Some(mesh) => MeshTemplate::from_mesh(mesh).map_err(|why| format!("`{path}` cannot be a particle mesh: {why}")),
                            None => {
                                if matches!(asset_server.get_load_state(handle.id()), Some(bevy::asset::LoadState::Failed(_))) {
                                    Err(format!("`{path}` did not load; particles using it draw nothing"))
                                } else {
                                    continue;
                                }
                            }
                        }
                    }
                    built_in => MeshTemplate::built_in(built_in).ok_or_else(|| "a built-in shape failed to build".to_string()),
                };
                match built {
                    Ok(t) => {
                        let t = Arc::new(t);
                        templates.templates.insert(shape.clone(), t.clone());
                        t
                    }
                    Err(why) => {
                        renzora::console_log::console_warn("Particles", why);
                        particles.failed = true;
                        continue;
                    }
                }
            }
        };
        if let Some(capacity) = particles.gpu_capacity {
            let pool = match templates.pools.get(&(shape.clone(), capacity)) {
                Some(h) => h.clone(),
                None => {
                    let slots = gpu_pool_slots(&template, capacity);
                    if slots < capacity {
                        renzora::console_log::console_warn(
                            "Particles",
                            format!(
                                "a {} vertex particle mesh fits {slots} of the {capacity} GPU particles in one pool; the rest are simulated but not drawn",
                                template.vertex_count()
                            ),
                        );
                    }
                    let h = meshes.add(gpu_pool_mesh(&template, slots));
                    templates.pools.insert((shape.clone(), capacity), h.clone());
                    h
                }
            };
            mesh3d.0 = pool;
        }
        particles.template = Some(template);
    }
}

// ── Orientation, the Rust twin of the shader's ──────────────────────────────

fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// `hash12` in the particle shaders.
fn hash12(p: Vec2) -> f32 {
    let mut p3 = Vec3::new(fract(p.x * 0.1031), fract(p.y * 0.1031), fract(p.x * 0.1031));
    let d = p3.dot(Vec3::new(p3.y, p3.z, p3.x) + Vec3::splat(33.33));
    p3 += Vec3::splat(d);
    fract((p3.x + p3.y) * p3.z)
}

fn seed_random(seed: f32, k: f32) -> f32 {
    hash12(Vec2::new(seed * 4096.0, k))
}

fn random_axis(seed: f32, k: f32) -> Vec3 {
    let z = seed_random(seed, k) * 2.0 - 1.0;
    let phi = seed_random(seed, k + 1.0) * std::f32::consts::TAU;
    let r = (1.0 - z * z).max(0.0).sqrt();
    Vec3::new(r * phi.cos(), r * phi.sin(), z)
}

fn align_y(dir: Vec3) -> Mat3 {
    let helper = if dir.x.abs() > 0.9 { Vec3::Z } else { Vec3::X };
    let z = helper.cross(dir).normalize();
    let x = dir.cross(z);
    Mat3::from_cols(x, dir, z)
}

/// A mesh particle's rotation in mesh space: `mesh_orientation` in
/// `particle3d.wesl`. `facing` is the effective facing
/// ([`RenderSettings::effective_facing`]) and `axis` what it delivers (the
/// velocity, the fixed axis, or nothing for a tumble).
pub fn mesh_rotation(facing: Facing, axis: Vec3, seed: f32, rotation: f32) -> Mat3 {
    if facing == Facing::CameraBillboard {
        let base = Mat3::from_axis_angle(random_axis(seed, 1.0), seed_random(seed, 3.0) * std::f32::consts::TAU);
        return Mat3::from_axis_angle(random_axis(seed, 4.0), rotation) * base;
    }
    let len = axis.length();
    let dir = if len > 1e-6 { axis / len } else { Vec3::Y };
    align_y(dir) * Mat3::from_axis_angle(Vec3::Y, rotation)
}

// ── The GPU pool mesh ───────────────────────────────────────────────────────

/// How many pool slots a pool mesh of `template` can hold.
pub fn gpu_pool_slots(template: &MeshTemplate, capacity: u32) -> u32 {
    let per = template.vertex_count().max(1);
    capacity.min((MAX_GPU_POOL_VERTICES / per).max(1) as u32)
}

/// One copy of `template` per pool slot: each vertex carries its UV as the
/// corner and its slot in z (location 0, as a pool quad's does), and its
/// position and normal in the template (locations 1 and 2).
pub fn gpu_pool_mesh(template: &MeshTemplate, slots: u32) -> Mesh {
    let n = template.vertex_count();
    let total = n * slots as usize;
    let mut position = Vec::with_capacity(total);
    let mut local = Vec::with_capacity(total);
    let mut normal = Vec::with_capacity(total);
    let mut indices = Vec::with_capacity(template.indices.len() * slots as usize);
    for slot in 0..slots {
        let base = slot * n as u32;
        for k in 0..n {
            let uv = template.uvs[k];
            position.push([uv.x - 0.5, 0.5 - uv.y, slot as f32]);
            local.push(template.positions[k].to_array());
            normal.push(template.normals[k].to_array());
        }
        indices.extend(template.indices.iter().map(|i| base + i));
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, position);
    mesh.insert_attribute(ATTRIBUTE_PARTICLE_LOCAL, local);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normal);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// What a GPU mesh-particle renderer draws until its pool mesh exists: one
/// degenerate triangle with the pool mesh's attributes, so the pipeline
/// specializes against the right layout from the first frame.
pub fn gpu_placeholder_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    mesh.insert_attribute(ATTRIBUTE_PARTICLE_LOCAL, vec![[0.0f32; 3]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3]);
    mesh.insert_indices(Indices::U32(vec![0, 1, 2]));
    mesh
}

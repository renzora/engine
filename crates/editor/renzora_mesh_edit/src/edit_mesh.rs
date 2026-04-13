//! Editable mesh data model.
//!
//! Bevy's `Mesh` is vertex-array-only — fine for rendering, painful for
//! topology ops. `EditMesh` is the mutable representation used while
//! Edit mode is active. It carries explicit vertices, edges, and faces so
//! operators (extrude / loop cut / bevel) can reason about connectivity.
//!
//! Phase 2: faces are triangles (one face per source triangle). N-gon
//! merging comes in Phase 3 when it actually buys us something.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EdgeId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FaceId(pub u32);

#[derive(Clone, Debug, Default)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub verts: [VertexId; 2],
    pub faces: Vec<FaceId>,
}

#[derive(Clone, Debug)]
pub struct Face {
    pub verts: Vec<VertexId>,
    pub edges: Vec<EdgeId>,
}

#[derive(Component, Default, Debug, Clone)]
pub struct EditMesh {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
    pub dirty: bool,
}

impl EditMesh {
    /// Build an EditMesh from a Bevy Mesh. Expects `TriangleList` topology.
    /// Unindexed meshes are rejected (callers can duplicate-weld first).
    pub fn from_mesh(mesh: &Mesh) -> Option<Self> {
        if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
            return None;
        }
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| match a {
                VertexAttributeValues::Float32x3(v) => Some(v.clone()),
                _ => None,
            })?;
        let normals: Vec<[f32; 3]> = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .and_then(|a| match a {
                VertexAttributeValues::Float32x3(v) => Some(v.clone()),
                _ => None,
            })
            .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
        let uvs: Vec<[f32; 2]> = mesh
            .attribute(Mesh::ATTRIBUTE_UV_0)
            .and_then(|a| match a {
                VertexAttributeValues::Float32x2(v) => Some(v.clone()),
                _ => None,
            })
            .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
        let indices: Vec<u32> = match mesh.indices()? {
            Indices::U16(v) => v.iter().map(|&i| i as u32).collect(),
            Indices::U32(v) => v.clone(),
        };

        // Weld coincident vertices. Bevy meshes store "split" vertices at
        // UV/normal seams — one logical corner becomes N independent entries
        // so each face can carry its own attributes. That's fine for
        // rendering but for editing we need shared verts, otherwise moving
        // one copy tears the mesh. Quantize position to a fine grid and
        // merge any that land in the same cell.
        const WELD_QUANT: f32 = 1.0e4; // ~0.0001 units granularity
        let mut weld_map: HashMap<(i32, i32, i32), u32> = HashMap::new();
        let mut remap: Vec<u32> = Vec::with_capacity(positions.len());
        let mut vertices: Vec<Vertex> = Vec::new();
        for (i, p) in positions.iter().enumerate() {
            let key = (
                (p[0] * WELD_QUANT).round() as i32,
                (p[1] * WELD_QUANT).round() as i32,
                (p[2] * WELD_QUANT).round() as i32,
            );
            let id = *weld_map.entry(key).or_insert_with(|| {
                let id = vertices.len() as u32;
                vertices.push(Vertex {
                    position: Vec3::from_array(*p),
                    normal: Vec3::from_array(normals[i]),
                    uv: Vec2::from_array(uvs[i]),
                });
                id
            });
            remap.push(id);
        }
        // Rewrite indices through the weld remap.
        let indices: Vec<u32> = indices.into_iter().map(|i| remap[i as usize]).collect();

        let mut edges: Vec<Edge> = Vec::new();
        let mut faces: Vec<Face> = Vec::with_capacity(indices.len() / 3);
        let mut edge_lookup: HashMap<(u32, u32), EdgeId> = HashMap::new();

        let canon = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
        let get_or_insert_edge =
            |edges: &mut Vec<Edge>,
             edge_lookup: &mut HashMap<(u32, u32), EdgeId>,
             a: u32,
             b: u32|
             -> EdgeId {
                let key = canon(a, b);
                if let Some(&id) = edge_lookup.get(&key) {
                    id
                } else {
                    let id = EdgeId(edges.len() as u32);
                    edges.push(Edge {
                        verts: [VertexId(key.0), VertexId(key.1)],
                        faces: Vec::new(),
                    });
                    edge_lookup.insert(key, id);
                    id
                }
            };

        for tri in indices.chunks_exact(3) {
            let (a, b, c) = (tri[0], tri[1], tri[2]);
            let face_id = FaceId(faces.len() as u32);
            let e_ab = get_or_insert_edge(&mut edges, &mut edge_lookup, a, b);
            let e_bc = get_or_insert_edge(&mut edges, &mut edge_lookup, b, c);
            let e_ca = get_or_insert_edge(&mut edges, &mut edge_lookup, c, a);
            edges[e_ab.0 as usize].faces.push(face_id);
            edges[e_bc.0 as usize].faces.push(face_id);
            edges[e_ca.0 as usize].faces.push(face_id);
            faces.push(Face {
                verts: vec![VertexId(a), VertexId(b), VertexId(c)],
                edges: vec![e_ab, e_bc, e_ca],
            });
        }

        Some(Self {
            vertices,
            edges,
            faces,
            dirty: false,
        })
    }

    /// Recompute `edges` and each face's edge list from face topology.
    /// Operators that add/remove faces should call this before baking.
    pub fn rebuild_edges(&mut self) {
        let canon = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
        self.edges.clear();
        let mut lookup: std::collections::HashMap<(u32, u32), EdgeId> = std::collections::HashMap::new();
        for face in &mut self.faces {
            face.edges.clear();
            let n = face.verts.len();
            for i in 0..n {
                let a = face.verts[i].0;
                let b = face.verts[(i + 1) % n].0;
                let key = canon(a, b);
                let eid = *lookup.entry(key).or_insert_with(|| {
                    let id = EdgeId(self.edges.len() as u32);
                    self.edges.push(Edge {
                        verts: [VertexId(key.0), VertexId(key.1)],
                        faces: Vec::new(),
                    });
                    id
                });
                face.edges.push(eid);
            }
        }
        // Populate edge.faces.
        for (fi, face) in self.faces.iter().enumerate() {
            for eid in &face.edges {
                self.edges[eid.0 as usize].faces.push(FaceId(fi as u32));
            }
        }
    }

    /// Average surface normal of a face. Uses Newell's method so n-gons don't
    /// go wrong when not perfectly planar. Returns `Vec3::Y` for degenerate faces.
    pub fn face_normal(&self, face: &Face) -> Vec3 {
        let n = face.verts.len();
        if n < 3 { return Vec3::Y; }
        let mut normal = Vec3::ZERO;
        for i in 0..n {
            let a = self.vertices[face.verts[i].0 as usize].position;
            let b = self.vertices[face.verts[(i + 1) % n].0 as usize].position;
            normal += (a - b).cross(a + b);
        }
        normal.normalize_or_zero()
    }

    /// Overwrite a Mesh asset from this EditMesh. Triangulates n-gons via
    /// a simple fan from the first vertex of each face.
    pub fn bake_to_mesh(&self, mesh: &mut Mesh) {
        let positions: Vec<[f32; 3]> =
            self.vertices.iter().map(|v| v.position.to_array()).collect();
        let normals: Vec<[f32; 3]> =
            self.vertices.iter().map(|v| v.normal.to_array()).collect();
        let uvs: Vec<[f32; 2]> = self.vertices.iter().map(|v| v.uv.to_array()).collect();
        let mut indices: Vec<u32> = Vec::with_capacity(self.faces.len() * 3);
        for face in &self.faces {
            if face.verts.len() < 3 {
                continue;
            }
            let anchor = face.verts[0].0;
            for w in face.verts.windows(2).skip(1) {
                indices.push(anchor);
                indices.push(w[0].0);
                indices.push(w[1].0);
            }
        }

        *mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(indices));
    }
}

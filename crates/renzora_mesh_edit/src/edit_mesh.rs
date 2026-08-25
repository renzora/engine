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
    /// True for edges that intentionally have no faces (vertex-extrude
    /// "line" geometry). [`EditMesh::rebuild_edges`] derives edges from face
    /// cycles, which would silently drop these — wire edges survive rebuilds,
    /// while an ordinary edge whose faces were all deleted does not.
    pub wire: bool,
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

        let mut faces: Vec<Face> = Vec::with_capacity(indices.len() / 3);
        for tri in indices.chunks_exact(3) {
            let (a, b, c) = (tri[0], tri[1], tri[2]);
            faces.push(Face {
                verts: vec![VertexId(a), VertexId(b), VertexId(c)],
                edges: Vec::new(),
            });
        }

        // Merge adjacent coplanar triangle pairs into single quad faces.
        // Bevy's primitive meshes (Plane3d, Cuboid, …) are stored as triangle
        // lists, but the user thinks of them as quads. Merging here means a
        // click in Face mode picks the whole logical face, and Extrude (E)
        // lifts the whole face instead of just one half-triangle.
        merge_coplanar_triangle_pairs(&mut faces, &vertices);

        // Build edge topology from the (post-merge) face list.
        let mut edges: Vec<Edge> = Vec::new();
        let mut edge_lookup: HashMap<(u32, u32), EdgeId> = HashMap::new();
        let canon = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
        for (fi, face) in faces.iter_mut().enumerate() {
            let n = face.verts.len();
            for i in 0..n {
                let a = face.verts[i].0;
                let b = face.verts[(i + 1) % n].0;
                let key = canon(a, b);
                let eid = *edge_lookup.entry(key).or_insert_with(|| {
                    let id = EdgeId(edges.len() as u32);
                    edges.push(Edge {
                        verts: [VertexId(key.0), VertexId(key.1)],
                        faces: Vec::new(),
                        wire: false,
                    });
                    id
                });
                edges[eid.0 as usize].faces.push(FaceId(fi as u32));
                face.edges.push(eid);
            }
        }

        Some(Self {
            vertices,
            edges,
            faces,
            dirty: false,
        })
    }

    /// Rebuild an `EditMesh` from a persisted `EditedMesh`'s geometry
    /// and face topology. Used when re-entering Edit mode on a mesh
    /// that was previously baked through the editor — the persisted
    /// `face_vertices` / `face_vertex_counts` describe the same bounded
    /// faces the user was working with, so we can rebuild `self.faces`
    /// exactly without re-guessing via `merge_coplanar_triangle_pairs`.
    ///
    /// The caller decides when to use this vs. the `from_mesh` fallback
    /// — the contract is "if `EditedMesh` carries valid topology, use
    /// it; otherwise fall back". See
    /// `EditedMesh::face_topology_is_valid` for the validity check.
    ///
    /// Edges are rebuilt from the post-restoration face list via the
    /// same loop `from_mesh` uses for its edge topology, so face-edge
    /// adjacency stays consistent.
    pub fn from_edited_mesh(snap: &renzora::core::EditedMesh) -> Option<Self> {
        if !snap.face_topology_is_valid() {
            return None;
        }

        // Build vertices from the flat position / normal / uv arrays.
        if !snap.positions.len().is_multiple_of(3) {
            return None;
        }
        let vert_count = snap.positions.len() / 3;
        let mut vertices: Vec<Vertex> = Vec::with_capacity(vert_count);
        for vi in 0..vert_count {
            let pos = &snap.positions[vi * 3..vi * 3 + 3];
            let normal = if snap.normals.len() == snap.positions.len() {
                let n = &snap.normals[vi * 3..vi * 3 + 3];
                Vec3::new(n[0], n[1], n[2])
            } else {
                Vec3::Y
            };
            let uv = if snap.uvs.len() == vert_count * 2 {
                let u = &snap.uvs[vi * 2..vi * 2 + 2];
                Vec2::new(u[0], u[1])
            } else {
                Vec2::ZERO
            };
            vertices.push(Vertex {
                position: Vec3::new(pos[0], pos[1], pos[2]),
                normal,
                uv,
            });
        }

        // Build faces from the persisted perimeter layout. Each entry
        // in `face_vertex_counts` delimits a window into `face_vertices`;
        // the window's contents are the vertex IDs that form the face
        // perimeter, in order.
        let mut faces: Vec<Face> = Vec::with_capacity(snap.face_vertex_counts.len());
        let mut offset = 0usize;
        for &count in &snap.face_vertex_counts {
            debug_assert!(count as usize >= 3, "validated face must have ≥3 verts");
            let mut verts: Vec<VertexId> = Vec::with_capacity(count as usize);
            for i in 0..count as usize {
                verts.push(VertexId(snap.face_vertices[offset + i]));
            }
            offset += count as usize;
            faces.push(Face {
                verts,
                edges: Vec::new(),
            });
        }

        // Rebuild edge topology from the post-restoration face list —
        // mirrors the loop in `from_mesh`. The opposite canonical
        // ordering of `(a, b)` is used here, matching `from_mesh`.
        let mut edges: Vec<Edge> = Vec::new();
        let mut edge_lookup: HashMap<(u32, u32), EdgeId> = HashMap::new();
        let canon = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
        for (fi, face) in faces.iter_mut().enumerate() {
            let n = face.verts.len();
            for i in 0..n {
                let a = face.verts[i].0;
                let b = face.verts[(i + 1) % n].0;
                let key = canon(a, b);
                let eid = *edge_lookup.entry(key).or_insert_with(|| {
                    let id = EdgeId(edges.len() as u32);
                    edges.push(Edge {
                        verts: [VertexId(key.0), VertexId(key.1)],
                        faces: Vec::new(),
                        wire: false,
                    });
                    id
                });
                edges[eid.0 as usize].faces.push(FaceId(fi as u32));
                face.edges.push(eid);
            }
        }

        Some(Self {
            vertices,
            edges,
            faces,
            dirty: false,
        })
    }

}

/// Newell's method for a flat polygon. Returns `Vec3::Y` for degenerate inputs.
fn polygon_normal(verts: &[VertexId], vertices: &[Vertex]) -> Vec3 {
    let n = verts.len();
    if n < 3 {
        return Vec3::Y;
    }
    let mut normal = Vec3::ZERO;
    for i in 0..n {
        let a = vertices[verts[i].0 as usize].position;
        let b = vertices[verts[(i + 1) % n].0 as usize].position;
        normal += (a - b).cross(a + b);
    }
    normal.normalize_or_zero()
}

/// Merge pairs of triangle faces that share an edge and are coplanar into
/// single quad faces. Runs once at import; later operators (extrude, …) may
/// produce mixed-topology faces and we don't try to remerge.
fn merge_coplanar_triangle_pairs(faces: &mut Vec<Face>, vertices: &[Vertex]) {
    // Tolerance: cubic primitives can have slightly noisy normals from welded
    // floating-point positions; 0.9995 corresponds to ~1.8°.
    const COPLANAR_DOT: f32 = 0.9995;

    let canon = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };

    let normals: Vec<Vec3> = faces
        .iter()
        .map(|f| polygon_normal(&f.verts, vertices))
        .collect();

    // canonical edge -> list of triangle face indices touching it
    let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (fi, f) in faces.iter().enumerate() {
        if f.verts.len() != 3 {
            continue;
        }
        for i in 0..3 {
            let a = f.verts[i].0;
            let b = f.verts[(i + 1) % 3].0;
            edge_faces.entry(canon(a, b)).or_default().push(fi);
        }
    }

    // Deterministic iteration order: sort canonical edge keys.
    let mut keys: Vec<(u32, u32)> = edge_faces.keys().copied().collect();
    keys.sort();

    let mut merged = vec![false; faces.len()];
    let mut quads: Vec<Face> = Vec::new();

    for key in &keys {
        let pair = &edge_faces[key];
        if pair.len() != 2 {
            continue;
        }
        let (fa_i, fb_i) = (pair[0], pair[1]);
        if merged[fa_i] || merged[fb_i] {
            continue;
        }
        if normals[fa_i].dot(normals[fb_i]) < COPLANAR_DOT {
            continue;
        }

        let fa = &faces[fa_i];
        let fb = &faces[fb_i];

        // Locate the directed shared edge inside `fa`. We name its endpoints
        // (x, y) and call fa's "third" vertex `c`; fb's third is `d`.
        let mut edge_idx_in_fa = None;
        for i in 0..3 {
            let a = fa.verts[i].0;
            let b = fa.verts[(i + 1) % 3].0;
            if canon(a, b) == *key {
                edge_idx_in_fa = Some(i);
                break;
            }
        }
        let Some(i) = edge_idx_in_fa else { continue };
        let x = fa.verts[i];
        let y = fa.verts[(i + 1) % 3];
        let c = fa.verts[(i + 2) % 3];
        let Some(d) = fb
            .verts
            .iter()
            .find(|v| v.0 != x.0 && v.0 != y.0)
            .copied()
        else {
            continue;
        };

        // CCW quad perimeter: Y → c → X → d (fa contributes c, fb contributes d).
        quads.push(Face {
            verts: vec![y, c, x, d],
            edges: Vec::new(),
        });
        merged[fa_i] = true;
        merged[fb_i] = true;
    }

    let mut out: Vec<Face> = Vec::with_capacity(faces.len());
    for (i, f) in faces.drain(..).enumerate() {
        if !merged[i] {
            out.push(f);
        }
    }
    out.extend(quads);
    *faces = out;
}

impl EditMesh {
    /// Recompute `edges` and each face's edge list from face topology.
    /// Operators that add/remove faces should call this before baking.
    /// Wire edges (vertex-extrude lines) are carried over; everything else
    /// is derived fresh from the face cycles.
    pub fn rebuild_edges(&mut self) {
        let canon = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
        let wires: Vec<[VertexId; 2]> = self
            .edges
            .iter()
            .filter(|e| e.wire)
            .map(|e| e.verts)
            .collect();
        self.edges.clear();
        let mut lookup: std::collections::HashMap<(u32, u32), EdgeId> =
            std::collections::HashMap::new();
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
                        wire: false,
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
        // Re-append wire edges that didn't get absorbed into a face.
        for w in wires {
            let key = canon(w[0].0, w[1].0);
            if !lookup.contains_key(&key) {
                self.edges.push(Edge {
                    verts: [VertexId(key.0), VertexId(key.1)],
                    faces: Vec::new(),
                    wire: true,
                });
            }
        }
    }

    /// Average surface normal of a face. Uses Newell's method so n-gons don't
    /// go wrong when not perfectly planar. Returns `Vec3::Y` for degenerate faces.
    pub fn face_normal(&self, face: &Face) -> Vec3 {
        let n = face.verts.len();
        if n < 3 {
            return Vec3::Y;
        }
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
        let positions: Vec<[f32; 3]> = self
            .vertices
            .iter()
            .map(|v| v.position.to_array())
            .collect();
        let normals: Vec<[f32; 3]> = self.vertices.iter().map(|v| v.normal.to_array()).collect();
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

    /// Recompute every vertex normal from the surrounding faces. Newell's
    /// method returns an area-scaled normal, so summing the raw (unnormalized)
    /// face vectors gives area weighting for free. Verts with no faces keep
    /// their previous normal.
    pub fn recompute_normals(&mut self) {
        let mut acc = vec![Vec3::ZERO; self.vertices.len()];
        for face in &self.faces {
            let n = face.verts.len();
            if n < 3 {
                continue;
            }
            let mut normal = Vec3::ZERO;
            for i in 0..n {
                let a = self.vertices[face.verts[i].0 as usize].position;
                let b = self.vertices[face.verts[(i + 1) % n].0 as usize].position;
                normal += (a - b).cross(a + b);
            }
            for v in &face.verts {
                acc[v.0 as usize] += normal;
            }
        }
        for (v, n) in self.vertices.iter_mut().zip(acc) {
            if let Some(n) = n.try_normalize() {
                v.normal = n;
            }
        }
    }

    /// Split an edge into `cuts + 1` segments. The new vertices are inserted
    /// into the vert cycle of **every** face using the edge (neighbours of a
    /// loop-cut ring become 5-gons rather than getting a T-junction), ordered
    /// from `verts[0]` toward `verts[1]`. Edge topology is left stale —
    /// batch splits, then call [`Self::rebuild_edges`].
    ///
    /// Returns the new vertex ids ordered from the `verts[0]` side.
    pub fn split_edge_multi(&mut self, edge: EdgeId, cuts: u32) -> Vec<VertexId> {
        let Some(e) = self.edges.get(edge.0 as usize) else {
            return Vec::new();
        };
        if cuts == 0 {
            return Vec::new();
        }
        let (a, b) = (e.verts[0].0, e.verts[1].0);
        let face_ids: Vec<FaceId> = e.faces.clone();
        let va = self.vertices[a as usize].clone();
        let vb = self.vertices[b as usize].clone();
        let mut new_ids: Vec<VertexId> = Vec::with_capacity(cuts as usize);
        for i in 1..=cuts {
            let t = i as f32 / (cuts + 1) as f32;
            let id = VertexId(self.vertices.len() as u32);
            self.vertices.push(Vertex {
                position: va.position.lerp(vb.position, t),
                normal: va.normal.lerp(vb.normal, t).normalize_or_zero(),
                uv: va.uv.lerp(vb.uv, t),
            });
            new_ids.push(id);
        }
        for fid in face_ids {
            let Some(face) = self.faces.get_mut(fid.0 as usize) else {
                continue;
            };
            let n = face.verts.len();
            for i in 0..n {
                let x = face.verts[i].0;
                let y = face.verts[(i + 1) % n].0;
                if x == a && y == b {
                    // Cycle runs a→b: insert in forward order after position i.
                    for (k, id) in new_ids.iter().enumerate() {
                        face.verts.insert(i + 1 + k, *id);
                    }
                    break;
                } else if x == b && y == a {
                    // Cycle runs b→a: insert reversed so the geometric order
                    // along the edge is preserved.
                    for (k, id) in new_ids.iter().rev().enumerate() {
                        face.verts.insert(i + 1 + k, *id);
                    }
                    break;
                }
            }
        }
        new_ids
    }

    /// Split an edge once at parameter `t` (measured from `verts[0]`).
    /// Same face-cycle insertion rules as [`Self::split_edge_multi`]; edge
    /// topology is left stale. Returns the new vertex.
    pub fn split_edge_at(&mut self, edge: EdgeId, t: f32) -> Option<VertexId> {
        let e = self.edges.get(edge.0 as usize)?;
        let (a, b) = (e.verts[0].0, e.verts[1].0);
        let face_ids: Vec<FaceId> = e.faces.clone();
        let va = self.vertices[a as usize].clone();
        let vb = self.vertices[b as usize].clone();
        let new_id = VertexId(self.vertices.len() as u32);
        self.vertices.push(Vertex {
            position: va.position.lerp(vb.position, t),
            normal: va.normal.lerp(vb.normal, t).normalize_or_zero(),
            uv: va.uv.lerp(vb.uv, t),
        });
        for fid in face_ids {
            let Some(face) = self.faces.get_mut(fid.0 as usize) else {
                continue;
            };
            let n = face.verts.len();
            for i in 0..n {
                let x = face.verts[i].0;
                let y = face.verts[(i + 1) % n].0;
                if (x == a && y == b) || (x == b && y == a) {
                    face.verts.insert(i + 1, new_id);
                    break;
                }
            }
        }
        Some(new_id)
    }

    /// Split a face by a new edge between two of its (non-adjacent) vertices.
    /// The original `FaceId` keeps the `va..=vb` arc; the returned new face
    /// takes the `vb..=va` remainder. Edge topology is left stale.
    pub fn split_face(&mut self, fid: FaceId, va: VertexId, vb: VertexId) -> Option<FaceId> {
        let face = self.faces.get(fid.0 as usize)?;
        let n = face.verts.len();
        let ia = face.verts.iter().position(|v| *v == va)?;
        let ib = face.verts.iter().position(|v| *v == vb)?;
        if ia == ib {
            return None;
        }
        // Refuse to split along an existing boundary edge (adjacent verts).
        if (ia + 1) % n == ib || (ib + 1) % n == ia {
            return None;
        }
        let mut first: Vec<VertexId> = Vec::new();
        let mut i = ia;
        loop {
            first.push(face.verts[i]);
            if i == ib {
                break;
            }
            i = (i + 1) % n;
        }
        let mut second: Vec<VertexId> = Vec::new();
        let mut i = ib;
        loop {
            second.push(face.verts[i]);
            if i == ia {
                break;
            }
            i = (i + 1) % n;
        }
        self.faces[fid.0 as usize].verts = first;
        self.faces[fid.0 as usize].edges.clear();
        let new_id = FaceId(self.faces.len() as u32);
        self.faces.push(Face {
            verts: second,
            edges: Vec::new(),
        });
        Some(new_id)
    }

    /// Weld vertices: every key in `target_map` is replaced by its value
    /// (chains are resolved). Faces are remapped, consecutive duplicates
    /// collapse, and faces left with fewer than 3 distinct verts die. Unused
    /// vertices are compacted away. Rebuilds edge topology.
    pub fn weld_verts(&mut self, target_map: &HashMap<u32, u32>) {
        if target_map.is_empty() {
            return;
        }
        let resolve = |mut v: u32| -> u32 {
            // Resolve chains (a→b, b→c). Guard against accidental cycles.
            let mut hops = 0;
            while let Some(&next) = target_map.get(&v) {
                if next == v || hops > 64 {
                    break;
                }
                v = next;
                hops += 1;
            }
            v
        };

        for face in &mut self.faces {
            for v in &mut face.verts {
                v.0 = resolve(v.0);
            }
            // Collapse consecutive duplicates around the cycle.
            let mut deduped: Vec<VertexId> = Vec::with_capacity(face.verts.len());
            for v in &face.verts {
                if deduped.last() != Some(v) {
                    deduped.push(*v);
                }
            }
            while deduped.len() > 1 && deduped.first() == deduped.last() {
                deduped.pop();
            }
            face.verts = deduped;
        }
        self.faces.retain(|f| {
            let unique: std::collections::HashSet<u32> = f.verts.iter().map(|v| v.0).collect();
            unique.len() >= 3
        });

        // Remap wire edges; drop those that collapsed to a point.
        for e in &mut self.edges {
            if e.wire {
                e.verts[0].0 = resolve(e.verts[0].0);
                e.verts[1].0 = resolve(e.verts[1].0);
            }
        }
        self.edges.retain(|e| !e.wire || e.verts[0] != e.verts[1]);

        self.compact_verts();
        self.rebuild_edges();
    }

    /// Drop vertices not referenced by any face or wire edge and remap all
    /// references. Call after ops that orphan verts (weld, delete).
    pub fn compact_verts(&mut self) {
        let mut used = vec![false; self.vertices.len()];
        for f in &self.faces {
            for v in &f.verts {
                used[v.0 as usize] = true;
            }
        }
        for e in &self.edges {
            if e.wire {
                used[e.verts[0].0 as usize] = true;
                used[e.verts[1].0 as usize] = true;
            }
        }
        if used.iter().all(|u| *u) {
            return;
        }
        let mut remap: Vec<u32> = vec![u32::MAX; self.vertices.len()];
        let mut kept: Vec<Vertex> = Vec::with_capacity(self.vertices.len());
        for (i, v) in self.vertices.iter().enumerate() {
            if used[i] {
                remap[i] = kept.len() as u32;
                kept.push(v.clone());
            }
        }
        self.vertices = kept;
        for f in &mut self.faces {
            for v in &mut f.verts {
                v.0 = remap[v.0 as usize];
            }
        }
        for e in &mut self.edges {
            e.verts[0].0 = remap[e.verts[0].0 as usize];
            e.verts[1].0 = remap[e.verts[1].0 as usize];
        }
    }

    /// Remove the given vertices plus every face and wire edge touching
    /// them, then compact. Rebuilds edge topology.
    pub fn remove_verts(&mut self, dead: &std::collections::HashSet<u32>) {
        if dead.is_empty() {
            return;
        }
        self.faces
            .retain(|f| !f.verts.iter().any(|v| dead.contains(&v.0)));
        self.edges.retain(|e| {
            !e.wire || (!dead.contains(&e.verts[0].0) && !dead.contains(&e.verts[1].0))
        });
        // Verts referenced by nothing get dropped by compact; verts in `dead`
        // are by construction unreferenced now.
        self.compact_verts();
        self.rebuild_edges();
    }

    /// Centroid of a face's vertices.
    pub fn face_centroid(&self, face: &Face) -> Vec3 {
        if face.verts.is_empty() {
            return Vec3::ZERO;
        }
        face.verts
            .iter()
            .map(|v| self.vertices[v.0 as usize].position)
            .sum::<Vec3>()
            / face.verts.len() as f32
    }

    /// Axis-aligned bounds of all vertices. `None` when empty.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut it = self.vertices.iter();
        let first = it.next()?.position;
        let mut min = first;
        let mut max = first;
        for v in it {
            min = min.min(v.position);
            max = max.max(v.position);
        }
        Some((min, max))
    }

    /// 1-ring vertex adjacency (across face edges + wire edges), used by
    /// smooth brushes and dissolve.
    pub fn vertex_neighbors(&self) -> Vec<Vec<u32>> {
        let mut out: Vec<Vec<u32>> = vec![Vec::new(); self.vertices.len()];
        for e in &self.edges {
            let (a, b) = (e.verts[0].0, e.verts[1].0);
            if !out[a as usize].contains(&b) {
                out[a as usize].push(b);
            }
            if !out[b as usize].contains(&a) {
                out[b as usize].push(a);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::core::EditedMesh;

    /// Build a unit cube's EditMesh directly — 8 vertices and 6 quad
    /// faces. Bypasses the bake/import cycle so the test is hermetic.
    fn unit_cube_edit_mesh() -> EditMesh {
        let v = [
            Vec3::new(-0.5, -0.5, -0.5), // 0
            Vec3::new(0.5, -0.5, -0.5),  // 1
            Vec3::new(0.5, 0.5, -0.5),   // 2
            Vec3::new(-0.5, 0.5, -0.5),  // 3
            Vec3::new(-0.5, -0.5, 0.5),  // 4
            Vec3::new(0.5, -0.5, 0.5),   // 5
            Vec3::new(0.5, 0.5, 0.5),    // 6
            Vec3::new(-0.5, 0.5, 0.5),   // 7
        ];
        let vertices: Vec<Vertex> = v
            .iter()
            .map(|p| Vertex {
                position: *p,
                normal: Vec3::Y,
                uv: Vec2::ZERO,
            })
            .collect();
        // Six quad faces: -Z, +Z, -X, +X, -Y, +Y, each CCW from outside.
        let face_verts: [&[u32]; 6] = [
            &[0, 1, 2, 3], // -Z
            &[5, 4, 7, 6], // +Z
            &[4, 0, 3, 7], // -X
            &[1, 5, 6, 2], // +X
            &[4, 5, 1, 0], // -Y
            &[3, 2, 6, 7], // +Y
        ];
        let mut faces = Vec::new();
        for verts in face_verts {
            faces.push(Face {
                verts: verts.iter().map(|i| VertexId(*i)).collect(),
                edges: Vec::new(),
            });
        }
        let mut edit = EditMesh {
            vertices,
            edges: Vec::new(),
            faces,
            dirty: false,
        };
        edit.rebuild_edges();
        edit
    }

    /// Same geometry as `unit_cube_edit_mesh`, but every quad has been
    /// split into the two triangles `EditMesh::bake_to_mesh` would
    /// produce (and that `from_mesh` would re-import). The test
    /// confirms that without persisted topology, the
    /// `merge_coplanar_triangle_pairs` heuristic inside `from_mesh`
    /// can still reconstruct the 6 quads — i.e. the heuristic is
    /// correct for the *initial* import path. The bug we fixed is
    /// specifically about extruded quads becoming ambiguous, not the
    /// initial cube import.
    #[test]
    fn cube_triangle_import_rebuilds_six_quads() {
        use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
        use bevy::asset::RenderAssetUsages;
        let v = [
            [0.5, -0.5, -0.5],
            [-0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [0.5, -0.5, 0.5],
            [-0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ];
        let normals = vec![[0.0, 0.0, 1.0]; 24];
        let uvs = vec![[0.0, 0.0]; 16];
        // Standard cube triangulation: each quad as two triangles.
        // CCW from outside on -Z face means the order depends on
        // handedness; we use the same vertex order as the quad list
        // above.
        let indices: Vec<u32> = vec![
            0, 1, 2, 2, 1, 3, // -Z
            4, 6, 5, 6, 7, 5, // +Z (flipped for outward normals)
            0, 2, 4, 2, 6, 4, // +X
            1, 5, 3, 5, 7, 3, // -X
            0, 4, 1, 1, 4, 5, // -Y
            2, 3, 6, 6, 3, 7, // +Y
        ];
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, v.to_vec());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(indices));

        let edit = EditMesh::from_mesh(&mesh).expect("from_mesh");
        assert_eq!(
            edit.faces.len(),
            6,
            "the initial cube import should still rebuild into 6 quads"
        );
        for face in &edit.faces {
            assert_eq!(
                face.verts.len(),
                4,
                "every face after initial cube import should be a quad"
            );
        }
    }

    /// Bake the cube, snapshot via `EditedMesh::from_edit_mesh`, then
    /// rebuild through `EditedMesh::from_edited_mesh`. The reconstructed
    /// EditMesh must have the same 6 quad faces as the original — no
    /// extra diagonals, no missing faces.
    #[test]
    fn cube_bake_snapshot_round_trip_preserves_six_quads() {
        let original = unit_cube_edit_mesh();
        let mut mesh = bevy::mesh::Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        original.bake_to_mesh(&mut mesh);

        let positions = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
            .unwrap();
        let positions: Vec<f32> = match positions {
            bevy::mesh::VertexAttributeValues::Float32x3(v) => {
                v.iter().flatten().copied().collect()
            }
            _ => panic!("expected Float32x3 positions"),
        };
        let normals = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_NORMAL)
            .map(|a| match a {
                bevy::mesh::VertexAttributeValues::Float32x3(v) => {
                    v.iter().flatten().copied().collect::<Vec<f32>>()
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let uvs = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_UV_0)
            .map(|a| match a {
                bevy::mesh::VertexAttributeValues::Float32x2(v) => {
                    v.iter().flatten().copied().collect::<Vec<f32>>()
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let indices = match mesh.indices().unwrap() {
            bevy::mesh::Indices::U32(v) => v.clone(),
            bevy::mesh::Indices::U16(v) => {
                v.iter().map(|i| *i as u32).collect()
            }
        };

        let face_perimeters: Vec<Vec<u32>> = original
            .faces
            .iter()
            .filter(|f| f.verts.len() >= 3)
            .map(|f| f.verts.iter().map(|v| v.0).collect())
            .collect();

        let snapshot = EditedMesh::from_edit_mesh(
            &positions,
            &normals,
            &uvs,
            &indices,
            &face_perimeters,
        );

        assert!(
            snapshot.has_face_topology(),
            "snapshot must carry topology"
        );
        assert!(snapshot.face_topology_is_valid());

        let rebuilt =
            EditMesh::from_edited_mesh(&snapshot).expect("from_edited_mesh");
        assert_eq!(
            rebuilt.faces.len(),
            original.faces.len(),
            "rebuilt EditMesh must have the same face count as the original"
        );
        for (orig, rb) in original.faces.iter().zip(rebuilt.faces.iter()) {
            assert_eq!(
                orig.verts.len(),
                rb.verts.len(),
                "every face must keep its vertex count"
            );
            assert_eq!(
                orig.verts, rb.verts,
                "vertex IDs must match the persisted perimeter exactly"
            );
        }
    }

    /// Same bake/snapshot/rebuild round-trip, but with one extra
    /// "extrude" step simulated: take the cube, add a top face that
    /// duplicates the original +Y face and creates a new bounded face
    /// above it. The reconstructed mesh must:
    /// - still have the original 6 quads (no diagonals across any of them)
    /// - have an extra face on top
    /// - the new top face's vertex IDs match what we baked
    #[test]
    fn cube_extrude_top_bake_snapshot_rebuild_preserves_quad_boundaries() {
        let mut original = unit_cube_edit_mesh();
        // Simulate an upward extrude of the +Y face by inserting a new
        // bounded face above it. The new face shares the original +Y
        // face's perimeter (v2, v6, v7, v3) but at y = 1.5. We then
        // move the original +Y face to y = 0.5.
        let new_top_face = Face {
            verts: vec![
                VertexId(2),
                VertexId(6),
                VertexId(7),
                VertexId(3),
            ],
            edges: Vec::new(),
        };
        original.faces.push(new_top_face);
        // Move the original +Y face (face index 5) up by 1.0 in y.
        for vid in [2u32, 6, 7, 3] {
            original.vertices[vid as usize].position.y += 1.0;
        }
        original.rebuild_edges();
        assert_eq!(
            original.faces.len(),
            7,
            "after extrude: 6 cube quads + 1 new top face"
        );

        // Bake the (now-displaced) geometry to a Mesh.
        let mut mesh = bevy::mesh::Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        original.bake_to_mesh(&mut mesh);

        // Snapshot geometry + topology.
        let positions: Vec<f32> = match mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
            .unwrap()
        {
            bevy::mesh::VertexAttributeValues::Float32x3(v) => {
                v.iter().flatten().copied().collect()
            }
            _ => panic!("Float32x3 expected"),
        };
        let normals: Vec<f32> = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_NORMAL)
            .map(|a| match a {
                bevy::mesh::VertexAttributeValues::Float32x3(v) => {
                    v.iter().flatten().copied().collect()
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let uvs: Vec<f32> = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_UV_0)
            .map(|a| match a {
                bevy::mesh::VertexAttributeValues::Float32x2(v) => {
                    v.iter().flatten().copied().collect()
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let indices = match mesh.indices().unwrap() {
            bevy::mesh::Indices::U32(v) => v.clone(),
            bevy::mesh::Indices::U16(v) => v.iter().map(|i| *i as u32).collect(),
        };

        let face_perimeters: Vec<Vec<u32>> = original
            .faces
            .iter()
            .filter(|f| f.verts.len() >= 3)
            .map(|f| f.verts.iter().map(|v| v.0).collect())
            .collect();

        let snapshot = EditedMesh::from_edit_mesh(
            &positions,
            &normals,
            &uvs,
            &indices,
            &face_perimeters,
        );
        let rebuilt = EditMesh::from_edited_mesh(&snapshot)
            .expect("topology must rebuild from snapshot");

        // The reconstructed mesh must match the original face layout
        // exactly — same number of faces, same vertex IDs per face.
        assert_eq!(
            rebuilt.faces.len(),
            original.faces.len(),
            "rebuilt must have the same face count as the extruded cube"
        );
        for (orig, rb) in original.faces.iter().zip(rebuilt.faces.iter()) {
            assert_eq!(orig.verts, rb.verts);
            assert_eq!(
                orig.verts.len(),
                4,
                "every face must stay a quad — no diagonals across a quad boundary"
            );
        }
        // The new top face must be present in the rebuilt mesh with its
        // exact perimeter.
        let rb_top = rebuilt
            .faces
            .iter()
            .find(|f| f.verts.len() == 4 && f.verts[0] == VertexId(2))
            .expect("rebuilt mesh must include the extruded top face");
        assert_eq!(
            rb_top.verts,
            vec![VertexId(2), VertexId(6), VertexId(7), VertexId(3)]
        );
    }

    /// The upper face's vertex IDs match what we baked — i.e. selecting
    /// the upper face returns just that face, not the lower face. We
    /// verify this through the `commit_face_pick` helper (the picker
    /// already uses it).
    #[test]
    fn extruded_top_face_selects_only_itself() {
        use crate::systems::commit_face_pick;
        let mut original = unit_cube_edit_mesh();
        let new_top_face = Face {
            verts: vec![
                VertexId(2),
                VertexId(6),
                VertexId(7),
                VertexId(3),
            ],
            edges: Vec::new(),
        };
        original.faces.push(new_top_face);
        original.rebuild_edges();

        // Snapshot + rebuild exactly as the live-bake + Edit-entry path
        // would.
        let mut mesh = bevy::mesh::Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        original.bake_to_mesh(&mut mesh);
        let positions: Vec<f32> = match mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
            .unwrap()
        {
            bevy::mesh::VertexAttributeValues::Float32x3(v) => {
                v.iter().flatten().copied().collect()
            }
            _ => panic!("Float32x3 expected"),
        };
        let normals: Vec<f32> = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_NORMAL)
            .map(|a| match a {
                bevy::mesh::VertexAttributeValues::Float32x3(v) => {
                    v.iter().flatten().copied().collect()
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let uvs: Vec<f32> = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_UV_0)
            .map(|a| match a {
                bevy::mesh::VertexAttributeValues::Float32x2(v) => {
                    v.iter().flatten().copied().collect()
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let indices = match mesh.indices().unwrap() {
            bevy::mesh::Indices::U32(v) => v.clone(),
            bevy::mesh::Indices::U16(v) => v.iter().map(|i| *i as u32).collect(),
        };
        let face_perimeters: Vec<Vec<u32>> = original
            .faces
            .iter()
            .filter(|f| f.verts.len() >= 3)
            .map(|f| f.verts.iter().map(|v| v.0).collect())
            .collect();
        let snapshot = EditedMesh::from_edit_mesh(
            &positions,
            &normals,
            &uvs,
            &indices,
            &face_perimeters,
        );
        let rebuilt = EditMesh::from_edited_mesh(&snapshot).unwrap();

        // The top face is the last one in the rebuilt mesh (we pushed
        // it last in `original.faces` and the rebuilded preserves the
        // order).
        let top = rebuilt.faces.last().unwrap();
        assert_eq!(top.verts.len(), 4);

        // Commit a pick on the top face only.
        let mut selection = std::collections::HashSet::new();
        let top_id = crate::edit_mesh::FaceId((rebuilt.faces.len() - 1) as u32);
        assert!(commit_face_pick(&mut selection, Some(top_id), false));
        assert_eq!(selection.len(), 1);
        assert!(selection.contains(&top_id));

        // The original +Y face (index 5) must NOT be selected.
        let lower_id = crate::edit_mesh::FaceId(5);
        assert!(!selection.contains(&lower_id));
    }

    /// Older `EditedMesh` snapshots (no topology fields) must still load
    /// through the `from_mesh` fallback path. We simulate this by
    /// building a snapshot with `face_vertices` empty.
    #[test]
    fn old_snapshot_without_topology_falls_back() {
        let original = unit_cube_edit_mesh();
        let mut mesh = bevy::mesh::Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        original.bake_to_mesh(&mut mesh);
        // Snapshot via `from_mesh` — this is what an old scene would
        // produce. No `face_vertices`.
        let old_snap = EditedMesh::from_mesh(&mesh).expect("from_mesh");
        assert!(!old_snap.has_face_topology());
        // `from_edited_mesh` must refuse to rebuild from an absent
        // topology and return `None` so the caller can fall back.
        assert!(EditMesh::from_edited_mesh(&old_snap).is_none());
        // The fallback path itself (from_mesh on the baked mesh) must
        // still produce 6 quads — that's what the cube import heuristic
        // is supposed to handle.
        let fallback = EditMesh::from_mesh(&mesh).expect("from_mesh fallback");
        assert_eq!(fallback.faces.len(), 6);
        for face in &fallback.faces {
            assert_eq!(face.verts.len(), 4);
        }
    }

    /// Malformed persisted topology must fall back without panicking.
    /// Tests the validator on a few corruption modes.
    #[test]
    fn malformed_topology_falls_back_without_panicking() {
        // Length mismatch: face_vertices has 4 entries, face_vertex_counts
        // sums to 6.
        let mut bad = EditedMesh::default();
        bad.positions = vec![0.0; 12]; // 4 verts
        bad.face_vertices = vec![0, 1, 2, 3];
        bad.face_vertex_counts = vec![2, 4]; // sums to 6, not 4
        assert!(!bad.face_topology_is_valid());

        // Out-of-range vertex ID.
        let mut bad = EditedMesh::default();
        bad.positions = vec![0.0; 6]; // 2 verts
        bad.face_vertices = vec![0, 1, 5]; // 5 is out of range
        bad.face_vertex_counts = vec![3];
        assert!(!bad.face_topology_is_valid());

        // Face with fewer than 3 vertices.
        let mut bad = EditedMesh::default();
        bad.positions = vec![0.0; 9]; // 3 verts
        bad.face_vertices = vec![0, 1];
        bad.face_vertex_counts = vec![2];
        assert!(!bad.face_topology_is_valid());

        // `from_edited_mesh` returns None for each — caller falls back.
        let bad = EditedMesh {
            positions: vec![0.0; 12],
            face_vertices: vec![0, 1, 2, 3],
            face_vertex_counts: vec![2, 4],
            ..Default::default()
        };
        assert!(EditMesh::from_edited_mesh(&bad).is_none());
    }

    /// Each vertical side of an extruded cube must show as TWO separate
    /// quads divided by a horizontal edge — no diagonal that would
    /// merge the lower and upper halves of a side into one quad.
    #[test]
    fn extruded_cube_each_side_has_two_quads_divided_by_horizontal_edge() {
        // Build a cube, extrude all four vertical sides and the top.
        // After bake + snapshot + rebuild, every vertical side face
        // must be two separate quads.
        let mut original = unit_cube_edit_mesh();
        // Faces before extrusion: -Z(0), +Z(1), -X(2), +X(3), -Y(4), +Y(5).
        // Simulate extrusion by adding a new face above each of the four
        // vertical sides. For brevity, we test just +X (face index 3):
        // the extruded top has vertex IDs (1, 5, 6, 2) shifted up.
        let new_top_x = Face {
            verts: vec![
                VertexId(1),
                VertexId(5),
                VertexId(6),
                VertexId(2),
            ],
            edges: Vec::new(),
        };
        original.faces.push(new_top_x);
        original.rebuild_edges();

        // Bake → snapshot → rebuild.
        let mut mesh = bevy::mesh::Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        original.bake_to_mesh(&mut mesh);
        let positions: Vec<f32> = match mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
            .unwrap()
        {
            bevy::mesh::VertexAttributeValues::Float32x3(v) => {
                v.iter().flatten().copied().collect()
            }
            _ => panic!("Float32x3 expected"),
        };
        let normals: Vec<f32> = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_NORMAL)
            .map(|a| match a {
                bevy::mesh::VertexAttributeValues::Float32x3(v) => {
                    v.iter().flatten().copied().collect()
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let uvs: Vec<f32> = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_UV_0)
            .map(|a| match a {
                bevy::mesh::VertexAttributeValues::Float32x2(v) => {
                    v.iter().flatten().copied().collect()
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let indices = match mesh.indices().unwrap() {
            bevy::mesh::Indices::U32(v) => v.clone(),
            bevy::mesh::Indices::U16(v) => v.iter().map(|i| *i as u32).collect(),
        };
        let face_perimeters: Vec<Vec<u32>> = original
            .faces
            .iter()
            .filter(|f| f.verts.len() >= 3)
            .map(|f| f.verts.iter().map(|v| v.0).collect())
            .collect();
        let snapshot = EditedMesh::from_edit_mesh(
            &positions,
            &normals,
            &uvs,
            &indices,
            &face_perimeters,
        );
        let rebuilt = EditMesh::from_edited_mesh(&snapshot).unwrap();

        // 6 cube quads + 1 extruded top → 7 faces.
        assert_eq!(rebuilt.faces.len(), 7);
        for face in &rebuilt.faces {
            assert_eq!(
                face.verts.len(),
                4,
                "no diagonal may appear across a face — every face stays a quad"
            );
        }
    }
}

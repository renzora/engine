//! Topological operators that mutate an [`EditMesh`] in place.
//!
//! Each operator returns the post-op selection sets and, where relevant, a
//! "grab axis" hint — the direction along which it's conventional to
//! immediately drag the newly-created geometry (e.g. face normal for face
//! extrude). Callers are expected to snapshot the `EditMesh` before
//! invocation so undo can restore the prior topology wholesale.

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::edit_mesh::{EditMesh, EdgeId, Face, FaceId, VertexId};
use crate::selection::{MeshSelection, SelectMode};

/// Output of an extrude op. The caller uses these to:
///  - update the selection so the new geometry is immediately selected,
///  - seed a grab operation whose "starts" are the duplicated vertex positions.
pub struct ExtrudeResult {
    pub new_verts: Vec<u32>,
    pub grab_axis: Option<Vec3>,
    /// Post-op element selection (replaces whatever the caller had).
    pub post_verts: HashSet<VertexId>,
    pub post_edges: HashSet<EdgeId>,
    pub post_faces: HashSet<FaceId>,
}

/// Dispatch to the right extrude flavor based on the current select mode.
pub fn extrude(edit: &mut EditMesh, sel: &MeshSelection) -> Option<ExtrudeResult> {
    match sel.mode {
        SelectMode::Vertex => extrude_verts(edit, &sel.verts),
        SelectMode::Edge => extrude_edges(edit, &sel.edges),
        SelectMode::Face => extrude_faces(edit, &sel.faces),
    }
}

// ── Face extrude ───────────────────────────────────────────────────────────

fn extrude_faces(edit: &mut EditMesh, selected: &HashSet<FaceId>) -> Option<ExtrudeResult> {
    if selected.is_empty() { return None; }

    // 1. Collect every vertex touched by any selected face.
    let mut ref_verts: HashSet<u32> = HashSet::new();
    for fid in selected {
        if let Some(f) = edit.faces.get(fid.0 as usize) {
            for v in &f.verts {
                ref_verts.insert(v.0);
            }
        }
    }

    // 2. Duplicate each referenced vertex once.
    let mut dup: HashMap<u32, u32> = HashMap::new();
    for &v in &ref_verts {
        let clone = edit.vertices[v as usize].clone();
        let new_id = edit.vertices.len() as u32;
        edit.vertices.push(clone);
        dup.insert(v, new_id);
    }

    // 3. Count how many selected faces touch each canonical edge. Edges that
    //    touch exactly one selected face are boundary edges and get bridged
    //    by a side quad. Track the original-orientation (a,b) pair of the
    //    single touching face so the side quad's winding is correct.
    let canon = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
    let mut edge_occurrences: HashMap<(u32, u32), Vec<(u32, u32)>> = HashMap::new();
    for fid in selected {
        let Some(f) = edit.faces.get(fid.0 as usize) else { continue };
        let n = f.verts.len();
        for i in 0..n {
            let a = f.verts[i].0;
            let b = f.verts[(i + 1) % n].0;
            edge_occurrences
                .entry(canon(a, b))
                .or_default()
                .push((a, b));
        }
    }

    // 4. Replace each selected face's verts with the duplicates (the face
    //    "moves" outward — the side walls fill in between).
    for fid in selected {
        if let Some(f) = edit.faces.get_mut(fid.0 as usize) {
            for v in &mut f.verts {
                v.0 = dup[&v.0];
            }
        }
    }

    // 5. Emit the side quads for boundary edges.
    for (_key, occurrences) in &edge_occurrences {
        if occurrences.len() != 1 { continue; }
        let (a, b) = occurrences[0];
        let new_a = dup[&a];
        let new_b = dup[&b];
        edit.faces.push(Face {
            verts: vec![VertexId(a), VertexId(b), VertexId(new_b), VertexId(new_a)],
            edges: Vec::new(),
        });
    }

    // 6. Rebuild topology so Edge.faces / Face.edges are consistent.
    edit.rebuild_edges();

    // 7. Compute average face normal of the now-moved selected faces for the
    //    grab axis hint.
    let mut normal_sum = Vec3::ZERO;
    for fid in selected {
        if let Some(f) = edit.faces.get(fid.0 as usize) {
            normal_sum += edit.face_normal(f);
        }
    }
    let grab_axis = normal_sum.try_normalize();

    // 8. The selected faces stay selected (same FaceIds); expose the new
    //    verts so the caller can seed grab.
    let new_verts: Vec<u32> = dup.values().copied().collect();
    let post_verts: HashSet<VertexId> = new_verts.iter().copied().map(VertexId).collect();

    Some(ExtrudeResult {
        new_verts,
        grab_axis,
        post_verts,
        post_edges: HashSet::new(),
        post_faces: selected.clone(),
    })
}

// ── Edge extrude ───────────────────────────────────────────────────────────

fn extrude_edges(edit: &mut EditMesh, selected: &HashSet<EdgeId>) -> Option<ExtrudeResult> {
    if selected.is_empty() { return None; }

    // Duplicate every vertex referenced by a selected edge.
    let mut ref_verts: HashSet<u32> = HashSet::new();
    for eid in selected {
        if let Some(e) = edit.edges.get(eid.0 as usize) {
            ref_verts.insert(e.verts[0].0);
            ref_verts.insert(e.verts[1].0);
        }
    }
    let mut dup: HashMap<u32, u32> = HashMap::new();
    for &v in &ref_verts {
        let clone = edit.vertices[v as usize].clone();
        let new_id = edit.vertices.len() as u32;
        edit.vertices.push(clone);
        dup.insert(v, new_id);
    }

    // Bridge each selected edge with a quad.
    let mut new_edge_keys: Vec<(u32, u32)> = Vec::new();
    for eid in selected {
        let Some(e) = edit.edges.get(eid.0 as usize) else { continue };
        let a = e.verts[0].0;
        let b = e.verts[1].0;
        let new_a = dup[&a];
        let new_b = dup[&b];
        edit.faces.push(Face {
            verts: vec![VertexId(a), VertexId(b), VertexId(new_b), VertexId(new_a)],
            edges: Vec::new(),
        });
        new_edge_keys.push(if new_a < new_b { (new_a, new_b) } else { (new_b, new_a) });
    }

    edit.rebuild_edges();

    // Map the (new_a, new_b) keys back to EdgeIds in the rebuilt edge list so
    // we can select the "far" edges.
    let mut post_edges: HashSet<EdgeId> = HashSet::new();
    for (i, e) in edit.edges.iter().enumerate() {
        let (a, b) = (e.verts[0].0, e.verts[1].0);
        let key = if a < b { (a, b) } else { (b, a) };
        if new_edge_keys.contains(&key) {
            post_edges.insert(EdgeId(i as u32));
        }
    }

    let new_verts: Vec<u32> = dup.values().copied().collect();
    let post_verts: HashSet<VertexId> = new_verts.iter().copied().map(VertexId).collect();

    Some(ExtrudeResult {
        new_verts,
        grab_axis: None,
        post_verts,
        post_edges,
        post_faces: HashSet::new(),
    })
}

// ── Vertex extrude ─────────────────────────────────────────────────────────

fn extrude_verts(edit: &mut EditMesh, selected: &HashSet<VertexId>) -> Option<ExtrudeResult> {
    if selected.is_empty() { return None; }

    // Duplicate each selected vertex and add an edge from original to dup.
    // Vertex extrude doesn't create faces — it creates a line strip geometry.
    let mut new_verts: Vec<u32> = Vec::with_capacity(selected.len());
    let mut post_verts: HashSet<VertexId> = HashSet::new();
    for vid in selected {
        let idx = vid.0 as usize;
        if idx >= edit.vertices.len() { continue; }
        let clone = edit.vertices[idx].clone();
        let new_id = edit.vertices.len() as u32;
        edit.vertices.push(clone);
        // Manually add the original→dup edge (not attached to any face).
        edit.edges.push(crate::edit_mesh::Edge {
            verts: [*vid, VertexId(new_id)],
            faces: Vec::new(),
        });
        new_verts.push(new_id);
        post_verts.insert(VertexId(new_id));
    }

    Some(ExtrudeResult {
        new_verts,
        grab_axis: None,
        post_verts,
        post_edges: HashSet::new(),
        post_faces: HashSet::new(),
    })
}

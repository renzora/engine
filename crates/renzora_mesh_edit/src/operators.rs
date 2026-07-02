//! Topological operators that mutate an [`EditMesh`] in place.
//!
//! Each operator returns the post-op selection sets and, where relevant, a
//! "grab axis" hint — the direction along which it's conventional to
//! immediately drag the newly-created geometry (e.g. face normal for face
//! extrude). Callers are expected to snapshot the `EditMesh` before
//! invocation so undo can restore the prior topology wholesale.

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::edit_mesh::{EdgeId, EditMesh, Face, FaceId, VertexId};
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
    if selected.is_empty() {
        return None;
    }

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
        let Some(f) = edit.faces.get(fid.0 as usize) else {
            continue;
        };
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
    for occurrences in edge_occurrences.values() {
        if occurrences.len() != 1 {
            continue;
        }
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
    if selected.is_empty() {
        return None;
    }

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
        let Some(e) = edit.edges.get(eid.0 as usize) else {
            continue;
        };
        let a = e.verts[0].0;
        let b = e.verts[1].0;
        let new_a = dup[&a];
        let new_b = dup[&b];
        edit.faces.push(Face {
            verts: vec![VertexId(a), VertexId(b), VertexId(new_b), VertexId(new_a)],
            edges: Vec::new(),
        });
        new_edge_keys.push(if new_a < new_b {
            (new_a, new_b)
        } else {
            (new_b, new_a)
        });
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
    if selected.is_empty() {
        return None;
    }

    // Duplicate each selected vertex and add an edge from original to dup.
    // Vertex extrude doesn't create faces — it creates a line strip geometry.
    let mut new_verts: Vec<u32> = Vec::with_capacity(selected.len());
    let mut post_verts: HashSet<VertexId> = HashSet::new();
    for vid in selected {
        let idx = vid.0 as usize;
        if idx >= edit.vertices.len() {
            continue;
        }
        let clone = edit.vertices[idx].clone();
        let new_id = edit.vertices.len() as u32;
        edit.vertices.push(clone);
        // Manually add the original→dup edge (not attached to any face).
        edit.edges.push(crate::edit_mesh::Edge {
            verts: [*vid, VertexId(new_id)],
            faces: Vec::new(),
            wire: true,
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

// ── Shared topology helpers ────────────────────────────────────────────────

fn canon(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Find the edge connecting two vertices in the (rebuilt) edge list.
fn find_edge(edit: &EditMesh, a: u32, b: u32) -> Option<EdgeId> {
    let key = canon(a, b);
    edit.edges
        .iter()
        .position(|e| canon(e.verts[0].0, e.verts[1].0) == key)
        .map(|i| EdgeId(i as u32))
}

/// The edge of a quad face opposite to `edge` — the one spanning the two
/// verts that `edge` doesn't touch, taken in cycle order. `None` for
/// non-quads.
fn opposite_edge_in_quad(edit: &EditMesh, fid: FaceId, edge: EdgeId) -> Option<EdgeId> {
    let face = edit.faces.get(fid.0 as usize)?;
    if face.verts.len() != 4 {
        return None;
    }
    let e = edit.edges.get(edge.0 as usize)?;
    let (a, b) = (e.verts[0].0, e.verts[1].0);
    let i = (0..4).find(|&i| {
        let x = face.verts[i].0;
        let y = face.verts[(i + 1) % 4].0;
        canon(x, y) == canon(a, b)
    })?;
    let c = face.verts[(i + 2) % 4].0;
    let d = face.verts[(i + 3) % 4].0;
    face.edges
        .iter()
        .copied()
        .find(|eid| {
            edit.edges
                .get(eid.0 as usize)
                .map(|e| canon(e.verts[0].0, e.verts[1].0) == canon(c, d))
                .unwrap_or(false)
        })
        .or_else(|| find_edge(edit, c, d))
}

// ── Edge-ring walk (loop cut) ──────────────────────────────────────────────

/// One traversal step of an edge ring: `(quad face crossed, edge on its far
/// side)`. The ring is the chain of opposite edges hopping quad → quad.
pub struct EdgeRing {
    /// Every edge in the ring (start edge included), deduplicated.
    pub edges: Vec<EdgeId>,
    /// Every quad the ring passes through, deduplicated.
    pub faces: Vec<FaceId>,
    /// True when the walk arrived back at the start edge.
    pub closed: bool,
}

/// Walk the edge ring through `start`, both directions, quads only —
/// Blender's Ctrl+R ring select. Stops at boundaries, non-quads, and
/// already-visited edges.
pub fn walk_edge_ring(edit: &EditMesh, start: EdgeId) -> EdgeRing {
    let mut edges: Vec<EdgeId> = vec![start];
    let mut faces: Vec<FaceId> = Vec::new();
    let mut seen_edges: HashSet<u32> = HashSet::from([start.0]);
    let mut seen_faces: HashSet<u32> = HashSet::new();
    let mut closed = false;

    let Some(start_edge) = edit.edges.get(start.0 as usize) else {
        return EdgeRing {
            edges,
            faces,
            closed,
        };
    };

    for &first_face in &start_edge.faces {
        let mut cur_edge = start;
        let mut cur_face = first_face;
        loop {
            if seen_faces.contains(&cur_face.0) {
                break;
            }
            let Some(next) = opposite_edge_in_quad(edit, cur_face, cur_edge) else {
                break; // non-quad face terminates the ring
            };
            seen_faces.insert(cur_face.0);
            faces.push(cur_face);
            if next == start {
                closed = true;
                break;
            }
            if !seen_edges.insert(next.0) {
                break;
            }
            edges.push(next);
            // Cross to the face on the other side of `next`.
            let Some(next_edge) = edit.edges.get(next.0 as usize) else {
                break;
            };
            let Some(&other) = next_edge.faces.iter().find(|f| f.0 != cur_face.0) else {
                break; // boundary
            };
            cur_edge = next;
            cur_face = other;
        }
        if closed {
            break;
        }
    }

    EdgeRing {
        edges,
        faces,
        closed,
    }
}

/// Cut `cuts` evenly-spaced loops across the edge ring through `start`.
/// Splits every ring edge, then splits each traversed quad between the
/// matching pairs of new verts so the cut is one continuous loop.
///
/// Returns the new loop edges (for post-op selection), or `None` when the
/// ring is degenerate (start edge has no quad).
pub fn loop_cut(edit: &mut EditMesh, start: EdgeId, cuts: u32) -> Option<HashSet<EdgeId>> {
    let cuts = cuts.max(1);
    let ring = walk_edge_ring(edit, start);
    if ring.faces.is_empty() {
        return None;
    }

    // Record each ring edge's endpoints before splitting invalidates them.
    let mut edge_ends: HashMap<u32, (u32, u32)> = HashMap::new();
    for eid in &ring.edges {
        let e = &edit.edges[eid.0 as usize];
        edge_ends.insert(eid.0, (e.verts[0].0, e.verts[1].0));
    }
    // Face → its (up to 2) ring edges, captured before splits.
    let mut face_ring_edges: HashMap<u32, Vec<u32>> = HashMap::new();
    for fid in &ring.faces {
        let face = &edit.faces[fid.0 as usize];
        for eid in &face.edges {
            if edge_ends.contains_key(&eid.0) {
                face_ring_edges.entry(fid.0).or_default().push(eid.0);
            }
        }
    }

    // Split every ring edge once per cut.
    let mut new_verts_per_edge: HashMap<u32, HashSet<u32>> = HashMap::new();
    for eid in &ring.edges {
        let ids = edit.split_edge_multi(*eid, cuts);
        new_verts_per_edge.insert(eid.0, ids.iter().map(|v| v.0).collect());
    }

    // Extract, per face, each ring edge's new verts in cycle order. In one
    // cycle the two ring edges are traversed in opposite spatial directions,
    // so pairing index i on the first with (cuts-1-i) on the second yields
    // geometrically parallel cut lines.
    let mut cut_pairs: Vec<(u32, u32)> = Vec::new();
    for fid in &ring.faces {
        let Some(ring_eids) = face_ring_edges.get(&fid.0) else {
            continue;
        };
        if ring_eids.len() != 2 {
            continue; // ring entered but didn't cross (shouldn't happen for quads)
        }
        let cycle: Vec<u32> = edit.faces[fid.0 as usize]
            .verts
            .iter()
            .map(|v| v.0)
            .collect();
        let in_cycle_order = |eid: u32| -> Option<Vec<u32>> {
            let set = new_verts_per_edge.get(&eid)?;
            let mut ordered: Vec<u32> = Vec::with_capacity(set.len());
            let n = cycle.len();
            // New verts were inserted contiguously; find the first run of
            // them in the cycle that belongs to this edge.
            let start = (0..n).find(|&i| {
                set.contains(&cycle[i]) && !set.contains(&cycle[(i + n - 1) % n])
            })?;
            let mut i = start;
            while set.contains(&cycle[i]) {
                ordered.push(cycle[i]);
                i = (i + 1) % n;
                if i == start {
                    break;
                }
            }
            Some(ordered)
        };
        let (Some(m1), Some(m2)) = (in_cycle_order(ring_eids[0]), in_cycle_order(ring_eids[1]))
        else {
            continue;
        };
        if m1.len() != cuts as usize || m2.len() != cuts as usize {
            continue;
        }
        for i in 0..cuts as usize {
            let va = VertexId(m1[i]);
            let vb = VertexId(m2[cuts as usize - 1 - i]);
            cut_pairs.push(canon(va.0, vb.0));
            // Successive pairs always live inside the arc the original face
            // keeps after a split, so keep splitting the same face id.
            edit.split_face(*fid, va, vb);
        }
    }

    edit.rebuild_edges();
    edit.recompute_normals();

    let pair_set: HashSet<(u32, u32)> = cut_pairs.into_iter().collect();
    let mut selected: HashSet<EdgeId> = HashSet::new();
    for (i, e) in edit.edges.iter().enumerate() {
        if pair_set.contains(&canon(e.verts[0].0, e.verts[1].0)) {
            selected.insert(EdgeId(i as u32));
        }
    }
    Some(selected)
}

// ── Edge-loop walk (Alt+click select) ──────────────────────────────────────

/// Walk the edge *loop* through `start` — Blender's Alt+click. At each end
/// vertex the loop continues along the single edge that shares no face with
/// the current edge; poles (valence ≠ 4) and boundaries terminate it.
pub fn walk_edge_loop(edit: &EditMesh, start: EdgeId) -> HashSet<EdgeId> {
    let mut result: HashSet<EdgeId> = HashSet::from([start]);
    // vertex → incident edges
    let mut vert_edges: HashMap<u32, Vec<u32>> = HashMap::new();
    for (i, e) in edit.edges.iter().enumerate() {
        vert_edges.entry(e.verts[0].0).or_default().push(i as u32);
        vert_edges.entry(e.verts[1].0).or_default().push(i as u32);
    }
    let Some(start_e) = edit.edges.get(start.0 as usize) else {
        return result;
    };

    for dir in 0..2 {
        let mut cur = start;
        let mut vert = start_e.verts[dir].0;
        while let Some(incident) = vert_edges.get(&vert) {
            if incident.len() != 4 {
                break; // pole or boundary vert
            }
            let cur_faces: HashSet<u32> = edit.edges[cur.0 as usize]
                .faces
                .iter()
                .map(|f| f.0)
                .collect();
            let mut next: Option<u32> = None;
            for &cand in incident {
                if cand == cur.0 {
                    continue;
                }
                let shares_face = edit.edges[cand as usize]
                    .faces
                    .iter()
                    .any(|f| cur_faces.contains(&f.0));
                if !shares_face {
                    if next.is_some() {
                        next = None; // ambiguous — stop
                        break;
                    }
                    next = Some(cand);
                }
            }
            let Some(next) = next else { break };
            if !result.insert(EdgeId(next)) {
                break; // closed loop
            }
            let e = &edit.edges[next as usize];
            vert = if e.verts[0].0 == vert {
                e.verts[1].0
            } else {
                e.verts[0].0
            };
            cur = EdgeId(next);
        }
    }
    result
}

// ── Subdivide ──────────────────────────────────────────────────────────────

/// Subdivide the selected faces once: every edge gains a midpoint; tris
/// become 4 tris, quads and n-gons become a fan of quads around a new
/// center vertex. Unselected neighbours keep valid topology (they gain the
/// midpoint in their cycle, becoming n-gons).
///
/// Returns the post-op face selection (all faces produced by the subdivision).
pub fn subdivide_faces(edit: &mut EditMesh, selected: &HashSet<FaceId>) -> Option<HashSet<FaceId>> {
    if selected.is_empty() {
        return None;
    }
    // Snapshot the corner cycles before splitting inserts midpoints.
    let originals: Vec<(FaceId, Vec<u32>)> = selected
        .iter()
        .filter_map(|fid| {
            edit.faces
                .get(fid.0 as usize)
                .map(|f| (*fid, f.verts.iter().map(|v| v.0).collect()))
        })
        .collect();

    // Split every edge of every selected face exactly once.
    let mut edge_ids: HashSet<u32> = HashSet::new();
    for (fid, _) in &originals {
        for eid in &edit.faces[fid.0 as usize].edges {
            edge_ids.insert(eid.0);
        }
    }
    let mut mid_of_edge: HashMap<(u32, u32), u32> = HashMap::new();
    for eid in edge_ids {
        let key = {
            let e = &edit.edges[eid as usize];
            canon(e.verts[0].0, e.verts[1].0)
        };
        if let Some(mid) = edit.split_edge_at(EdgeId(eid), 0.5) {
            mid_of_edge.insert(key, mid.0);
        }
    }

    let mut post_faces_markers: Vec<Vec<u32>> = Vec::new(); // vert cycles of new faces
    for (fid, corners) in &originals {
        let n = corners.len();
        let mids: Vec<u32> = (0..n)
            .filter_map(|i| mid_of_edge.get(&canon(corners[i], corners[(i + 1) % n])).copied())
            .collect();
        if mids.len() != n {
            continue; // some edge failed to split — leave the face alone
        }
        let mut new_cycles: Vec<Vec<u32>> = Vec::new();
        if n == 3 {
            // Classic 1-to-4 triangle split via the three midpoints.
            new_cycles.push(vec![corners[0], mids[0], mids[2]]);
            new_cycles.push(vec![corners[1], mids[1], mids[0]]);
            new_cycles.push(vec![corners[2], mids[2], mids[1]]);
            new_cycles.push(vec![mids[0], mids[1], mids[2]]);
        } else {
            // Quad fan around a center vertex: (corner, next mid, center, prev mid).
            let centroid = corners
                .iter()
                .map(|&c| edit.vertices[c as usize].position)
                .sum::<Vec3>()
                / n as f32;
            let uv = corners
                .iter()
                .map(|&c| edit.vertices[c as usize].uv)
                .sum::<Vec2>()
                / n as f32;
            let center = edit.vertices.len() as u32;
            edit.vertices.push(crate::edit_mesh::Vertex {
                position: centroid,
                normal: Vec3::Y,
                uv,
            });
            for i in 0..n {
                new_cycles.push(vec![
                    corners[i],
                    mids[i],
                    center,
                    mids[(i + n - 1) % n],
                ]);
            }
        }
        // First cycle replaces the original face; the rest append.
        let mut cycles = new_cycles.into_iter();
        if let Some(first) = cycles.next() {
            post_faces_markers.push(first.clone());
            let face = &mut edit.faces[fid.0 as usize];
            face.verts = first.into_iter().map(VertexId).collect();
            face.edges.clear();
        }
        for cycle in cycles {
            post_faces_markers.push(cycle.clone());
            edit.faces.push(Face {
                verts: cycle.into_iter().map(VertexId).collect(),
                edges: Vec::new(),
            });
        }
    }

    edit.rebuild_edges();
    edit.recompute_normals();

    // Re-derive the selection: faces whose cycle matches a recorded new cycle.
    let marker_set: HashSet<Vec<u32>> = post_faces_markers.into_iter().collect();
    let mut post: HashSet<FaceId> = HashSet::new();
    for (i, f) in edit.faces.iter().enumerate() {
        let cycle: Vec<u32> = f.verts.iter().map(|v| v.0).collect();
        if marker_set.contains(&cycle) {
            post.insert(FaceId(i as u32));
        }
    }
    Some(post)
}

// ── Inset ──────────────────────────────────────────────────────────────────

/// Inset each selected face individually: shrink the face toward its
/// centroid by `amount` (0..1) and bridge the border with quads. The
/// selected `FaceId`s become the inner faces, so I → G "inset then adjust"
/// flows naturally.
pub fn inset_faces(
    edit: &mut EditMesh,
    selected: &HashSet<FaceId>,
    amount: f32,
) -> Option<HashSet<FaceId>> {
    if selected.is_empty() {
        return None;
    }
    let amount = amount.clamp(0.01, 0.95);
    for fid in selected {
        let Some(face) = edit.faces.get(fid.0 as usize) else {
            continue;
        };
        let outer: Vec<u32> = face.verts.iter().map(|v| v.0).collect();
        let n = outer.len();
        if n < 3 {
            continue;
        }
        let centroid = edit.face_centroid(face);
        let uv_centroid = outer
            .iter()
            .map(|&c| edit.vertices[c as usize].uv)
            .sum::<Vec2>()
            / n as f32;
        let mut inner: Vec<u32> = Vec::with_capacity(n);
        for &o in &outer {
            let v = &edit.vertices[o as usize];
            let id = edit.vertices.len() as u32;
            let position = v.position.lerp(centroid, amount);
            let normal = v.normal;
            let uv = v.uv.lerp(uv_centroid, amount);
            edit.vertices.push(crate::edit_mesh::Vertex {
                position,
                normal,
                uv,
            });
            inner.push(id);
        }
        // Rim quads between outer ring and inner ring.
        for i in 0..n {
            let j = (i + 1) % n;
            edit.faces.push(Face {
                verts: vec![
                    VertexId(outer[i]),
                    VertexId(outer[j]),
                    VertexId(inner[j]),
                    VertexId(inner[i]),
                ],
                edges: Vec::new(),
            });
        }
        // The original face becomes the inner face (same FaceId → stays selected).
        let face = &mut edit.faces[fid.0 as usize];
        face.verts = inner.into_iter().map(VertexId).collect();
        face.edges.clear();
    }
    edit.rebuild_edges();
    edit.recompute_normals();
    Some(selected.clone())
}

// ── Delete / dissolve ──────────────────────────────────────────────────────

/// Delete the current selection with Blender's cascade rules for the active
/// select mode: verts take their faces with them, edges take their faces,
/// faces go alone.
pub fn delete_selection(edit: &mut EditMesh, sel: &MeshSelection) -> bool {
    match sel.mode {
        SelectMode::Vertex => {
            if sel.verts.is_empty() {
                return false;
            }
            let dead: HashSet<u32> = sel.verts.iter().map(|v| v.0).collect();
            edit.remove_verts(&dead);
        }
        SelectMode::Edge => {
            if sel.edges.is_empty() {
                return false;
            }
            let mut dead_pairs: HashSet<(u32, u32)> = HashSet::new();
            for eid in &sel.edges {
                if let Some(e) = edit.edges.get(eid.0 as usize) {
                    dead_pairs.insert(canon(e.verts[0].0, e.verts[1].0));
                }
            }
            edit.faces.retain(|f| {
                let n = f.verts.len();
                !(0..n).any(|i| {
                    dead_pairs.contains(&canon(f.verts[i].0, f.verts[(i + 1) % n].0))
                })
            });
            // Selected wire edges die too.
            edit.edges.retain(|e| {
                !e.wire || !dead_pairs.contains(&canon(e.verts[0].0, e.verts[1].0))
            });
            edit.rebuild_edges();
        }
        SelectMode::Face => {
            if sel.faces.is_empty() {
                return false;
            }
            let dead: HashSet<u32> = sel.faces.iter().map(|f| f.0).collect();
            let mut i = 0u32;
            edit.faces.retain(|_| {
                let keep = !dead.contains(&i);
                i += 1;
                keep
            });
            edit.rebuild_edges();
        }
    }
    edit.recompute_normals();
    true
}

/// Dissolve edges: each selected edge with exactly two faces is removed and
/// its faces merged into one n-gon. Simplified vs Blender (no vert cleanup).
pub fn dissolve_edges(edit: &mut EditMesh, selected: &HashSet<EdgeId>) -> bool {
    if selected.is_empty() {
        return false;
    }
    let mut any = false;
    let mut dead_faces: HashSet<u32> = HashSet::new();
    let pairs: Vec<(u32, u32)> = selected
        .iter()
        .filter_map(|eid| edit.edges.get(eid.0 as usize))
        .filter(|e| !e.wire)
        .map(|e| (e.verts[0].0, e.verts[1].0))
        .collect();

    for (a, b) in pairs {
        // Re-find the two live faces currently sharing the pair — earlier
        // dissolves may have restructured things.
        let mut hosts: Vec<usize> = Vec::new();
        for (i, f) in edit.faces.iter().enumerate() {
            if dead_faces.contains(&(i as u32)) {
                continue;
            }
            let n = f.verts.len();
            if (0..n).any(|k| canon(f.verts[k].0, f.verts[(k + 1) % n].0) == canon(a, b)) {
                hosts.push(i);
            }
        }
        if hosts.len() != 2 {
            continue;
        }
        let (fa, fb) = (hosts[0], hosts[1]);
        // Walk face A from b around to a (inclusive), then face B's verts
        // strictly between a and b — a single merged cycle without the
        // dissolved edge.
        let cycle_a: Vec<u32> = edit.faces[fa].verts.iter().map(|v| v.0).collect();
        let cycle_b: Vec<u32> = edit.faces[fb].verts.iter().map(|v| v.0).collect();
        // Orient A so it contains a→b consecutively; if it contains b→a,
        // swap roles of a/b to keep the walk logic single-path.
        let na = cycle_a.len();
        let dir_ab = (0..na).find(|&i| cycle_a[i] == a && cycle_a[(i + 1) % na] == b);
        let (a, b) = if dir_ab.is_some() { (a, b) } else { (b, a) };
        let Some(ia_b) = cycle_a.iter().position(|&v| v == b) else {
            continue;
        };
        let mut merged: Vec<u32> = Vec::with_capacity(cycle_a.len() + cycle_b.len() - 2);
        let mut i = ia_b;
        loop {
            merged.push(cycle_a[i]);
            if cycle_a[i] == a {
                break;
            }
            i = (i + 1) % na;
        }
        // B contains b→a consecutively (opposite winding). Walk from a
        // forward, stopping before b.
        let nb = cycle_b.len();
        let Some(ib_a) = cycle_b.iter().position(|&v| v == a) else {
            continue;
        };
        let mut i = (ib_a + 1) % nb;
        while cycle_b[i] != b {
            merged.push(cycle_b[i]);
            i = (i + 1) % nb;
        }
        // Reject merges that would produce a degenerate or self-touching cycle.
        let unique: HashSet<u32> = merged.iter().copied().collect();
        if unique.len() < 3 || unique.len() != merged.len() {
            continue;
        }
        edit.faces[fa].verts = merged.into_iter().map(VertexId).collect();
        edit.faces[fa].edges.clear();
        dead_faces.insert(fb as u32);
        any = true;
    }

    if any {
        let mut i = 0u32;
        edit.faces.retain(|_| {
            let keep = !dead_faces.contains(&i);
            i += 1;
            keep
        });
        edit.rebuild_edges();
        edit.recompute_normals();
    }
    any
}

/// Dissolve vertices (simplified): each selected vert is removed from every
/// face cycle it appears in; faces reduced below 3 verts die, orphaned verts
/// are compacted away.
pub fn dissolve_verts(edit: &mut EditMesh, selected: &HashSet<VertexId>) -> bool {
    if selected.is_empty() {
        return false;
    }
    let dead: HashSet<u32> = selected.iter().map(|v| v.0).collect();
    let mut any = false;
    for face in &mut edit.faces {
        let before = face.verts.len();
        face.verts.retain(|v| !dead.contains(&v.0));
        if face.verts.len() != before {
            face.edges.clear();
            any = true;
        }
    }
    if !any {
        return false;
    }
    edit.faces.retain(|f| f.verts.len() >= 3);
    edit.edges
        .retain(|e| !e.wire || (!dead.contains(&e.verts[0].0) && !dead.contains(&e.verts[1].0)));
    edit.compact_verts();
    edit.rebuild_edges();
    edit.recompute_normals();
    true
}

// ── Merge ──────────────────────────────────────────────────────────────────

/// Merge a set of verts into one at their centroid. Returns the surviving
/// vert (post-compaction id) via a fresh lookup of the centroid position.
pub fn merge_at_center(edit: &mut EditMesh, verts: &HashSet<VertexId>) -> Option<VertexId> {
    if verts.len() < 2 {
        return None;
    }
    let ids: Vec<u32> = verts.iter().map(|v| v.0).collect();
    let centroid = ids
        .iter()
        .map(|&i| edit.vertices[i as usize].position)
        .sum::<Vec3>()
        / ids.len() as f32;
    let survivor = *ids.iter().min()?;
    edit.vertices[survivor as usize].position = centroid;
    let mut map: HashMap<u32, u32> = HashMap::new();
    for &i in &ids {
        if i != survivor {
            map.insert(i, survivor);
        }
    }
    edit.weld_verts(&map);
    edit.recompute_normals();
    // weld_verts compacted ids — find the survivor by position.
    edit.vertices
        .iter()
        .position(|v| v.position.distance_squared(centroid) < 1e-10)
        .map(|i| VertexId(i as u32))
}

/// Merge all verts closer than `dist` (Blender's "Merge by Distance" /
/// remove doubles). Returns how many verts were removed.
pub fn remove_doubles(edit: &mut EditMesh, dist: f32) -> usize {
    let dist = dist.max(1e-6);
    let inv_cell = 1.0 / dist;
    let before = edit.vertices.len();
    // Spatial hash: candidates in the 27 neighbouring cells.
    let mut grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
    let mut map: HashMap<u32, u32> = HashMap::new();
    for (i, v) in edit.vertices.iter().enumerate() {
        let p = v.position * inv_cell;
        let cell = (
            p.x.floor() as i32,
            p.y.floor() as i32,
            p.z.floor() as i32,
        );
        let mut target: Option<u32> = None;
        'search: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let key = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
                    if let Some(ids) = grid.get(&key) {
                        for &cand in ids {
                            if edit.vertices[cand as usize]
                                .position
                                .distance(v.position)
                                <= dist
                            {
                                target = Some(cand);
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        if let Some(t) = target {
            map.insert(i as u32, t);
        } else {
            grid.entry(cell).or_default().push(i as u32);
        }
    }
    if map.is_empty() {
        return 0;
    }
    edit.weld_verts(&map);
    edit.recompute_normals();
    before - edit.vertices.len()
}

// ── Bisect ─────────────────────────────────────────────────────────────────

/// Cut the whole mesh by a plane: spanning edges are split exactly on the
/// plane and every crossed face is split along its on-plane verts. With
/// `clear_neg`, geometry on the negative side is deleted (used by
/// symmetrize). Returns the edges of the cut line for post-op selection.
pub fn bisect(
    edit: &mut EditMesh,
    plane_point: Vec3,
    plane_normal: Vec3,
    clear_neg: bool,
) -> Option<HashSet<EdgeId>> {
    const EPS: f32 = 1e-5;
    let normal = plane_normal.try_normalize()?;
    let side =
        |p: Vec3| -> f32 { normal.dot(p - plane_point) };

    // Verts already on the plane count as cut verts.
    let mut on_plane: HashSet<u32> = edit
        .vertices
        .iter()
        .enumerate()
        .filter(|(_, v)| side(v.position).abs() <= EPS)
        .map(|(i, _)| i as u32)
        .collect();

    // Split every spanning edge at the plane crossing.
    let spanning: Vec<(EdgeId, f32)> = edit
        .edges
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let da = side(edit.vertices[e.verts[0].0 as usize].position);
            let db = side(edit.vertices[e.verts[1].0 as usize].position);
            if da.abs() > EPS && db.abs() > EPS && (da > 0.0) != (db > 0.0) {
                Some((EdgeId(i as u32), da / (da - db)))
            } else {
                None
            }
        })
        .collect();
    for (eid, t) in spanning {
        if let Some(v) = edit.split_edge_at(eid, t) {
            on_plane.insert(v.0);
        }
    }

    // Split every face that has two non-adjacent on-plane verts.
    let mut cut_pairs: HashSet<(u32, u32)> = HashSet::new();
    let face_count = edit.faces.len();
    for fi in 0..face_count {
        let cycle: Vec<u32> = edit.faces[fi].verts.iter().map(|v| v.0).collect();
        let hits: Vec<usize> = (0..cycle.len())
            .filter(|&i| on_plane.contains(&cycle[i]))
            .collect();
        if hits.len() < 2 {
            continue;
        }
        // Take the first non-adjacent pair — sufficient for convex faces,
        // which is what the primitive + quad pipeline produces.
        let n = cycle.len();
        let mut pair: Option<(u32, u32)> = None;
        'outer: for (ai, &i) in hits.iter().enumerate() {
            for &j in hits.iter().skip(ai + 1) {
                let adjacent = (i + 1) % n == j || (j + 1) % n == i;
                if !adjacent {
                    pair = Some((cycle[i], cycle[j]));
                    break 'outer;
                }
            }
        }
        let Some((a, b)) = pair else { continue };
        if edit.split_face(FaceId(fi as u32), VertexId(a), VertexId(b)).is_some() {
            cut_pairs.insert(canon(a, b));
        }
    }

    if clear_neg {
        // Snapshot positions — `retain` on faces can't also borrow vertices.
        let verts_snapshot: Vec<Vec3> = edit.vertices.iter().map(|v| v.position).collect();
        edit.faces.retain(|f| {
            let c = f
                .verts
                .iter()
                .map(|v| verts_snapshot[v.0 as usize])
                .sum::<Vec3>()
                / f.verts.len().max(1) as f32;
            side(c) >= -EPS
        });
        edit.edges.retain(|e| {
            !e.wire || {
                let mid = (verts_snapshot[e.verts[0].0 as usize]
                    + verts_snapshot[e.verts[1].0 as usize])
                    / 2.0;
                side(mid) >= -EPS
            }
        });
        edit.compact_verts();
    }

    edit.rebuild_edges();
    edit.recompute_normals();

    let mut selected: HashSet<EdgeId> = HashSet::new();
    for (i, e) in edit.edges.iter().enumerate() {
        if cut_pairs.contains(&canon(e.verts[0].0, e.verts[1].0)) {
            selected.insert(EdgeId(i as u32));
        }
    }
    Some(selected)
}

// ── Mirror (symmetrize) ────────────────────────────────────────────────────

/// Symmetrize the mesh across a local axis plane through the origin: the
/// positive side is kept, mirrored to the negative side, and the seam is
/// welded. Equivalent to Blender's Symmetrize (+X → −X etc.).
pub fn mirror_symmetrize(edit: &mut EditMesh, axis: usize) -> bool {
    if axis > 2 || edit.faces.is_empty() {
        return false;
    }
    const SEAM_EPS: f32 = 1e-4;
    let mut normal = Vec3::ZERO;
    normal[axis] = 1.0;

    // Cut on the plane and drop the negative half.
    bisect(edit, Vec3::ZERO, normal, true);

    // Duplicate every off-plane vert, negated on the axis; seam verts map to
    // themselves so the halves share the boundary loop.
    let vert_count = edit.vertices.len();
    let mut mirror_of: Vec<u32> = Vec::with_capacity(vert_count);
    for i in 0..vert_count {
        let v = edit.vertices[i].clone();
        if v.position[axis].abs() <= SEAM_EPS {
            mirror_of.push(i as u32);
        } else {
            let mut m = v;
            m.position[axis] = -m.position[axis];
            m.normal[axis] = -m.normal[axis];
            mirror_of.push(edit.vertices.len() as u32);
            edit.vertices.push(m);
        }
    }

    // Mirror faces with reversed winding so normals stay outward.
    let face_count = edit.faces.len();
    for fi in 0..face_count {
        let mut cycle: Vec<VertexId> = edit.faces[fi]
            .verts
            .iter()
            .map(|v| VertexId(mirror_of[v.0 as usize]))
            .collect();
        cycle.reverse();
        // Skip faces that live entirely on the seam (would duplicate).
        if cycle
            .iter()
            .zip(edit.faces[fi].verts.iter())
            .all(|(m, o)| m == o)
        {
            continue;
        }
        edit.faces.push(Face {
            verts: cycle,
            edges: Vec::new(),
        });
    }

    edit.compact_verts();
    edit.rebuild_edges();
    edit.recompute_normals();
    true
}

// ── Array ──────────────────────────────────────────────────────────────────

/// Duplicate the whole mesh `count - 1` times along a cumulative offset.
/// `relative` scales the offset by the mesh's bounding-box size per axis
/// (Blender's "Relative Offset"); otherwise it's a constant local-space
/// translation. `weld_dist > 0` merges verts between adjacent copies.
pub fn array_duplicate(
    edit: &mut EditMesh,
    count: u32,
    offset: Vec3,
    relative: bool,
    weld_dist: f32,
) -> bool {
    if count < 2 || edit.vertices.is_empty() {
        return false;
    }
    let step = if relative {
        let (min, max) = edit.bounds().unwrap_or((Vec3::ZERO, Vec3::ZERO));
        offset * (max - min)
    } else {
        offset
    };
    if step.length_squared() < 1e-12 {
        return false;
    }

    let base_verts = edit.vertices.clone();
    let base_faces: Vec<Vec<u32>> = edit
        .faces
        .iter()
        .map(|f| f.verts.iter().map(|v| v.0).collect())
        .collect();
    let base_wires: Vec<[u32; 2]> = edit
        .edges
        .iter()
        .filter(|e| e.wire)
        .map(|e| [e.verts[0].0, e.verts[1].0])
        .collect();

    for c in 1..count {
        let vert_base = edit.vertices.len() as u32;
        let shift = step * c as f32;
        for v in &base_verts {
            let mut nv = v.clone();
            nv.position += shift;
            edit.vertices.push(nv);
        }
        for cycle in &base_faces {
            edit.faces.push(Face {
                verts: cycle.iter().map(|&v| VertexId(v + vert_base)).collect(),
                edges: Vec::new(),
            });
        }
        for w in &base_wires {
            edit.edges.push(crate::edit_mesh::Edge {
                verts: [VertexId(w[0] + vert_base), VertexId(w[1] + vert_base)],
                faces: Vec::new(),
                wire: true,
            });
        }
    }

    edit.rebuild_edges();
    if weld_dist > 0.0 {
        remove_doubles(edit, weld_dist);
    }
    edit.recompute_normals();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit_mesh::Vertex;

    /// Unit cube: 8 verts, 6 quads, 12 edges — the canonical operator fixture.
    fn cube() -> EditMesh {
        let p = |x: f32, y: f32, z: f32| Vertex {
            position: Vec3::new(x, y, z),
            normal: Vec3::Y,
            uv: Vec2::ZERO,
        };
        let mut edit = EditMesh {
            vertices: vec![
                p(-0.5, -0.5, -0.5), // 0
                p(0.5, -0.5, -0.5),  // 1
                p(0.5, 0.5, -0.5),   // 2
                p(-0.5, 0.5, -0.5),  // 3
                p(-0.5, -0.5, 0.5),  // 4
                p(0.5, -0.5, 0.5),   // 5
                p(0.5, 0.5, 0.5),    // 6
                p(-0.5, 0.5, 0.5),   // 7
            ],
            edges: Vec::new(),
            faces: [
                [0u32, 3, 2, 1], // back  (−Z)
                [4, 5, 6, 7],    // front (+Z)
                [0, 1, 5, 4],    // bottom
                [2, 3, 7, 6],    // top
                [0, 4, 7, 3],    // left
                [1, 2, 6, 5],    // right
            ]
            .iter()
            .map(|q| Face {
                verts: q.iter().map(|&v| VertexId(v)).collect(),
                edges: Vec::new(),
            })
            .collect(),
            dirty: false,
        };
        edit.rebuild_edges();
        edit.recompute_normals();
        edit
    }

    fn face_set(ids: &[u32]) -> HashSet<FaceId> {
        ids.iter().map(|&i| FaceId(i)).collect()
    }

    #[test]
    fn cube_topology() {
        let c = cube();
        assert_eq!(c.vertices.len(), 8);
        assert_eq!(c.faces.len(), 6);
        assert_eq!(c.edges.len(), 12);
        // Every edge of a closed cube borders exactly 2 faces.
        assert!(c.edges.iter().all(|e| e.faces.len() == 2));
    }

    #[test]
    fn cube_normals_point_outward() {
        let c = cube();
        for f in &c.faces {
            let n = c.face_normal(f);
            let centroid = c.face_centroid(f);
            assert!(
                n.dot(centroid) > 0.0,
                "face normal {n} at {centroid} points inward"
            );
        }
    }

    #[test]
    fn loop_cut_rings_around_cube() {
        let mut c = cube();
        let start = find_edge(&c, 0, 1).unwrap();
        let ring = walk_edge_ring(&c, start);
        // The ring around a cube crosses 4 faces and holds 4 edges.
        assert_eq!(ring.edges.len(), 4);
        assert_eq!(ring.faces.len(), 4);
        assert!(ring.closed);

        let new_loop = loop_cut(&mut c, start, 1).unwrap();
        // One cut adds 4 verts and the loop is 4 edges long.
        assert_eq!(c.vertices.len(), 12);
        assert_eq!(new_loop.len(), 4);
        // 4 quads got split in two: 6 + 4 = 10 faces.
        assert_eq!(c.faces.len(), 10);
        // Closed manifold must survive the cut.
        assert!(c.edges.iter().all(|e| e.faces.len() == 2));
    }

    #[test]
    fn loop_cut_two_cuts() {
        let mut c = cube();
        let start = find_edge(&c, 0, 1).unwrap();
        let new_loop = loop_cut(&mut c, start, 2).unwrap();
        assert_eq!(c.vertices.len(), 16); // 8 + 4 ring edges × 2 cuts
        assert_eq!(new_loop.len(), 8); // two loops of 4
        assert_eq!(c.faces.len(), 14); // 4 ring quads → 3 strips each
        assert!(c.edges.iter().all(|e| e.faces.len() == 2));
    }

    #[test]
    fn edge_loop_walks_around() {
        let c = cube();
        // On a cube every vert has valence 3, so the loop can't extend —
        // just the start edge comes back.
        let start = find_edge(&c, 0, 1).unwrap();
        assert_eq!(walk_edge_loop(&c, start).len(), 1);

        // After one loop cut, the new loop's verts have valence 4 and the
        // loop walk finds the full ring of 4.
        let mut c = cube();
        let cut = loop_cut(&mut c, start, 1).unwrap();
        let one = *cut.iter().next().unwrap();
        assert_eq!(walk_edge_loop(&c, one).len(), 4);
    }

    #[test]
    fn subdivide_quad_fan() {
        let mut c = cube();
        let post = subdivide_faces(&mut c, &face_set(&[0])).unwrap();
        // One quad → 4 quads; 4 midpoint verts + 1 center vert.
        assert_eq!(post.len(), 4);
        assert_eq!(c.vertices.len(), 13);
        assert_eq!(c.faces.len(), 9); // 5 untouched (as n-gons) + 4 new
    }

    #[test]
    fn subdivide_all_faces() {
        let mut c = cube();
        let all = face_set(&[0, 1, 2, 3, 4, 5]);
        let post = subdivide_faces(&mut c, &all).unwrap();
        assert_eq!(post.len(), 24);
        assert_eq!(c.faces.len(), 24);
        // 8 corners + 12 edge midpoints + 6 centers.
        assert_eq!(c.vertices.len(), 26);
        assert!(c.edges.iter().all(|e| e.faces.len() == 2));
    }

    #[test]
    fn inset_keeps_selection_and_manifold() {
        let mut c = cube();
        let post = inset_faces(&mut c, &face_set(&[1]), 0.25).unwrap();
        assert_eq!(post, face_set(&[1]));
        assert_eq!(c.vertices.len(), 12);
        assert_eq!(c.faces.len(), 10); // 5 original + 4 rim + inner
        assert!(c.edges.iter().all(|e| e.faces.len() == 2));
    }

    #[test]
    fn extrude_face_is_manifold() {
        let mut c = cube();
        let sel = MeshSelection {
            target: None,
            mode: SelectMode::Face,
            verts: HashSet::new(),
            edges: HashSet::new(),
            faces: face_set(&[1]),
        };
        let result = extrude(&mut c, &sel).unwrap();
        assert_eq!(result.new_verts.len(), 4);
        assert_eq!(c.faces.len(), 10);
        assert!(c.edges.iter().all(|e| e.faces.len() == 2));
        assert!(result.grab_axis.is_some());
    }

    #[test]
    fn delete_face_leaves_boundary() {
        let mut c = cube();
        let sel = MeshSelection {
            target: None,
            mode: SelectMode::Face,
            verts: HashSet::new(),
            edges: HashSet::new(),
            faces: face_set(&[1]),
        };
        assert!(delete_selection(&mut c, &sel));
        assert_eq!(c.faces.len(), 5);
        // The 4 boundary edges now border exactly 1 face.
        assert_eq!(c.edges.iter().filter(|e| e.faces.len() == 1).count(), 4);
    }

    #[test]
    fn delete_vert_cascades() {
        let mut c = cube();
        let sel = MeshSelection {
            target: None,
            mode: SelectMode::Vertex,
            verts: HashSet::from([VertexId(0)]),
            edges: HashSet::new(),
            faces: HashSet::new(),
        };
        assert!(delete_selection(&mut c, &sel));
        // Vert 0 touches 3 faces; they die with it.
        assert_eq!(c.faces.len(), 3);
        assert_eq!(c.vertices.len(), 7);
    }

    #[test]
    fn dissolve_edge_merges_faces() {
        let mut c = cube();
        let e = find_edge(&c, 0, 1).unwrap();
        assert!(dissolve_edges(&mut c, &HashSet::from([e])));
        assert_eq!(c.faces.len(), 5);
        // The merged face is a 6-gon.
        assert!(c.faces.iter().any(|f| f.verts.len() == 6));
    }

    #[test]
    fn merge_at_center_welds() {
        let mut c = cube();
        let verts: HashSet<VertexId> = [VertexId(0), VertexId(1)].into();
        let survivor = merge_at_center(&mut c, &verts).unwrap();
        assert_eq!(c.vertices.len(), 7);
        assert_eq!(
            c.vertices[survivor.0 as usize].position,
            Vec3::new(0.0, -0.5, -0.5)
        );
        // The two quads that used both verts became tris.
        assert_eq!(c.faces.iter().filter(|f| f.verts.len() == 3).count(), 2);
    }

    #[test]
    fn remove_doubles_welds_seam() {
        let mut c = cube();
        // Nudge vert 1 to nearly coincide with vert 0.
        c.vertices[1].position = c.vertices[0].position + Vec3::splat(1e-4);
        let removed = remove_doubles(&mut c, 0.001);
        assert_eq!(removed, 1);
        assert_eq!(c.vertices.len(), 7);
    }

    #[test]
    fn bisect_splits_cube() {
        let mut c = cube();
        let cut = bisect(&mut c, Vec3::ZERO, Vec3::X, false).unwrap();
        // The YZ plane crosses the cube's 4 X-aligned edges — 4 new verts on
        // the plane, and the cut loop threading them is 4 edges.
        assert_eq!(cut.len(), 4);
        assert_eq!(c.vertices.len(), 12);
        assert_eq!(c.faces.len(), 10);
        assert!(c.edges.iter().all(|e| e.faces.len() == 2));
    }

    #[test]
    fn bisect_clear_keeps_half() {
        let mut c = cube();
        bisect(&mut c, Vec3::ZERO, Vec3::X, true).unwrap();
        assert!(c.vertices.iter().all(|v| v.position.x >= -1e-4));
        // Open box: 4 cut quads halved + right cap = 5 faces.
        assert_eq!(c.faces.len(), 5);
    }

    #[test]
    fn mirror_symmetrize_restores_cube() {
        let mut c = cube();
        // Skew the -X side, then symmetrize +X→−X: result must be symmetric.
        c.vertices[0].position += Vec3::new(-0.3, 0.1, 0.0);
        assert!(mirror_symmetrize(&mut c, 0));
        assert!(c.edges.iter().all(|e| e.faces.len() <= 2));
        for v in &c.vertices {
            let mirrored = Vec3::new(-v.position.x, v.position.y, v.position.z);
            assert!(
                c.vertices
                    .iter()
                    .any(|w| w.position.distance(mirrored) < 1e-3),
                "no mirror partner for {}",
                v.position
            );
        }
    }

    #[test]
    fn array_duplicates_with_weld() {
        let mut c = cube();
        // Two copies exactly one cube apart, welded → shared wall verts merge.
        assert!(array_duplicate(&mut c, 2, Vec3::new(1.0, 0.0, 0.0), false, 1e-3));
        assert_eq!(c.vertices.len(), 12); // 16 − 4 welded
        assert_eq!(c.faces.len(), 12);
    }

    #[test]
    fn wire_edges_survive_rebuild() {
        let mut c = cube();
        c.edges.push(crate::edit_mesh::Edge {
            verts: [VertexId(0), VertexId(6)],
            faces: Vec::new(),
            wire: true,
        });
        c.rebuild_edges();
        assert_eq!(c.edges.len(), 13);
        assert!(c.edges.iter().any(|e| e.wire));
    }
}

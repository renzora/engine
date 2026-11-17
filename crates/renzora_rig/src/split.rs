//! Separating part of a model into the pieces it is made of.
//!
//! Models often arrive with things that move on their own merged together:
//! both front wheels of a car as one "axle", every door of a building as one.
//! Nothing can turn one wheel of that, because there is one node for both and
//! its origin is neither wheel's hub. A wheel is also rarely one mesh: an
//! exporter writes one mesh per material (tyre, rim, brake disc, badge), each
//! on a node of its own under the axle.
//!
//! [`separate_parts`] takes the meshes to split, as the editor collects them
//! from what was right-clicked, and regroups their geometry into pieces. Each
//! piece gets one new node under the meshes' nearest common parent, placed at
//! the piece's centre, holding one primitive per material the piece uses, with
//! its vertices moved so that centre is their origin. A wheel then spins and
//! steers about its own hub by turning its node.
//!
//! What counts as a piece is geometry, not naming:
//!
//! 1. Each primitive is split into the triangles that connect to each other.
//!    Vertices at the same position are joined first, so a texture seam (two
//!    vertices in one place with different UVs) does not cut a tyre in two.
//! 2. Those bits are merged wherever their bounds overlap. A tyre, its rim and
//!    the bolts in it touch nowhere but sit inside one another, and they
//!    belong together; the two wheels of an axle are a metre apart and do not.
//!
//! The file is edited like rigging edits it ([`crate::glb`]): new meshes,
//! accessors and nodes are appended, the meshes split are taken off their
//! nodes, and everything else passes through, including every material.

use std::collections::HashMap;

use bevy::math::{Mat3, Mat4, Vec3};
use serde_json::{json, Value};

use crate::glb::{Glb, GlbError};

/// What [`separate_parts`] made.
#[derive(Debug, Clone, PartialEq)]
pub struct Separated {
    /// The node the pieces were put under.
    pub parent: String,
    /// The pieces, in order along the direction they are most spread out in.
    pub parts: Vec<Part>,
}

/// One piece: its node's name, where it sits in its parent's space, and how
/// many triangles it has.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    pub centre: Vec3,
    pub triangles: usize,
}

const FLOAT: u64 = 5126;
const UNSIGNED_INT: u64 = 5125;
const ARRAY_BUFFER: u64 = 34962;
const ELEMENT_ARRAY_BUFFER: u64 = 34963;

fn unsupported(message: impl Into<String>) -> GlbError {
    GlbError::Unsupported(message.into())
}

/// One mesh being split: the node drawing it, its primitives, and the
/// transform from that node into the common parent's space.
struct Source {
    node: usize,
    mesh: usize,
    primitives: Vec<Value>,
    to_parent: Mat4,
}

/// A connected bit of one primitive, with its bounds in the parent's space.
struct Bit {
    source: usize,
    prim: usize,
    triangles: Vec<[u32; 3]>,
    min: Vec3,
    max: Vec3,
}

/// Regroup the meshes named in `mesh_names` into separate pieces, each on a
/// new node `<parent>_part1`, `<parent>_part2`, ... under the nearest node all
/// of them sit under. The pieces are numbered along the direction they are
/// most spread out in, so the numbering is the same every time.
///
/// Refused, leaving the file exactly as it was, when the geometry is a single
/// piece, when a mesh is drawn by more than one node (splitting it would
/// change all of them), and for what this cannot copy faithfully: skinned
/// meshes, blend shapes, compressed or sparse data.
pub fn separate_parts(glb: &mut Glb, mesh_names: &[&str]) -> Result<Separated, GlbError> {
    if mesh_names.is_empty() {
        return Err(unsupported("there is nothing to separate"));
    }
    // The node drawing each mesh.
    let mut drawn = Vec::new();
    for name in mesh_names {
        let mesh = glb
            .meshes()
            .iter()
            .position(|m| m.get("name").and_then(Value::as_str) == Some(*name))
            .ok_or_else(|| unsupported(format!("the model has no mesh named `{name}`")))?;
        let users: Vec<usize> = glb
            .nodes()
            .iter()
            .enumerate()
            .filter(|(_, n)| n.get("mesh").and_then(Value::as_u64) == Some(mesh as u64))
            .map(|(i, _)| i)
            .collect();
        let node = match users.as_slice() {
            [node] => *node,
            [] => return Err(unsupported(format!("no node draws the mesh `{name}`"))),
            _ => {
                return Err(unsupported(format!(
                    "the mesh `{name}` is drawn in {} places, and separating it would change all of them",
                    users.len()
                )))
            }
        };
        if glb.nodes()[node].get("skin").is_some() {
            return Err(unsupported("a skinned mesh moves with its bones and cannot be separated"));
        }
        if !drawn.iter().any(|&(_, m)| m == mesh) {
            drawn.push((node, mesh));
        }
    }

    let parents = glb.parents();
    let parent = common_parent(&parents, &drawn.iter().map(|&(n, _)| n).collect::<Vec<_>>());
    let world = glb.world_matrices();
    let parent_inverse = world[parent].inverse();
    let sources: Vec<Source> = drawn
        .iter()
        .map(|&(node, mesh)| Source {
            node,
            mesh,
            primitives: glb.meshes()[mesh].get("primitives").and_then(Value::as_array).cloned().unwrap_or_default(),
            to_parent: parent_inverse * world[node],
        })
        .collect();
    if sources.iter().flat_map(|s| &s.primitives).any(|p| p.get("targets").is_some()) {
        return Err(unsupported("a mesh with blend shapes cannot be separated"));
    }

    let mut bits: Vec<Bit> = Vec::new();
    for (s, source) in sources.iter().enumerate() {
        for prim in 0..source.primitives.len() {
            let positions: Vec<Vec3> = glb
                .positions(source.mesh, prim)?
                .into_iter()
                .map(|p| source.to_parent.transform_point3(p))
                .collect();
            let indices = match glb.indices(source.mesh, prim)? {
                Some(i) => i,
                None if source.primitives[prim].get("mode").and_then(Value::as_u64).unwrap_or(4) == 4 => {
                    (0..positions.len() as u32).collect()
                }
                None => return Err(unsupported("only triangle meshes can be separated")),
            };
            for (triangles, (min, max)) in connected_bits(&positions, &indices) {
                bits.push(Bit { source: s, prim, triangles, min, max });
            }
        }
    }

    // Bits whose bounds overlap are one piece. The margin is a sliver of the
    // whole thing's size, so parts that only just touch (a tyre around its
    // rim, modelled with a hair of a gap) count as overlapping.
    let (all_min, all_max) = bounds(bits.iter().flat_map(|b| [b.min, b.max]));
    let margin = (all_max - all_min).length() * 1e-3;
    let mut groups = UnionFind::new(bits.len());
    for i in 0..bits.len() {
        for j in i + 1..bits.len() {
            let (a, b) = (&bits[i], &bits[j]);
            if (a.min - margin).cmple(b.max).all() && (b.min - margin).cmple(a.max).all() {
                groups.join(i, j);
            }
        }
    }
    let mut by_group: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..bits.len() {
        by_group.entry(groups.root(i)).or_default().push(i);
    }
    if by_group.len() < 2 {
        return Err(unsupported("it is already one piece"));
    }

    let mut pieces: Vec<(Vec<usize>, Vec3)> = by_group
        .into_values()
        .map(|members| {
            let (lo, hi) = bounds(members.iter().flat_map(|&i| [bits[i].min, bits[i].max]));
            (members, (lo + hi) * 0.5)
        })
        .collect();
    let (c_min, c_max) = bounds(pieces.iter().map(|(_, c)| *c));
    let spread = c_max - c_min;
    let axis = if spread.x >= spread.y && spread.x >= spread.z {
        0
    } else if spread.y >= spread.z {
        1
    } else {
        2
    };
    pieces.sort_by(|a, b| a.1[axis].total_cmp(&b.1[axis]));

    let parent_name = match glb.node_name(parent) {
        n if n.is_empty() => format!("node{parent}"),
        n => n,
    };
    let mut parts = Vec::new();
    for (number, (members, centre)) in pieces.iter().enumerate() {
        let mut primitives = Vec::new();
        let mut triangle_count = 0;
        for (s, source) in sources.iter().enumerate() {
            for prim in 0..source.primitives.len() {
                let triangles: Vec<[u32; 3]> = members
                    .iter()
                    .filter(|&&i| bits[i].source == s && bits[i].prim == prim)
                    .flat_map(|&i| bits[i].triangles.iter().copied())
                    .collect();
                if triangles.is_empty() {
                    continue;
                }
                triangle_count += triangles.len();
                let to_piece = Mat4::from_translation(-*centre) * source.to_parent;
                primitives.push(copy_primitive(glb, &source.primitives[prim], &triangles, to_piece)?);
            }
        }
        let name = format!("{parent_name}_part{}", number + 1);
        let mesh = {
            let meshes = glb.json["meshes"].as_array_mut().expect("the meshes were found in it");
            meshes.push(json!({ "name": name, "primitives": primitives }));
            meshes.len() - 1
        };
        glb.add_node(json!({ "name": name, "mesh": mesh, "translation": [centre.x, centre.y, centre.z] }), Some(parent));
        parts.push(Part { name, centre: *centre, triangles: triangle_count });
    }
    // The nodes that drew the meshes stay, now empty, so they keep their place
    // in the tree and anything that named them still finds them.
    for source in &sources {
        if let Some(n) = glb.json["nodes"][source.node].as_object_mut() {
            n.remove("mesh");
        }
    }
    Ok(Separated { parent: parent_name, parts })
}

/// The nearest node every one of `nodes` is, or sits under. A single node is
/// its own answer, so its pieces go under it.
fn common_parent(parents: &[Option<usize>], nodes: &[usize]) -> usize {
    let chain = |mut n: usize| {
        let mut out = vec![n];
        while let Some(p) = parents.get(n).copied().flatten() {
            if out.len() > parents.len() {
                break;
            }
            out.push(p);
            n = p;
        }
        out
    };
    let first = chain(nodes[0]);
    first
        .into_iter()
        .find(|candidate| nodes[1..].iter().all(|&n| chain(n).contains(candidate)))
        // Nodes in different trees: the first one's root holds them all as well
        // as anything can.
        .unwrap_or(nodes[0])
}

fn bounds(points: impl Iterator<Item = Vec3>) -> (Vec3, Vec3) {
    points.fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(p), hi.max(p)))
}

/// Connected triangles and their bounds, as (min, max).
type Connected = (Vec<[u32; 3]>, (Vec3, Vec3));

/// The triangles of one primitive, grouped by what connects to what, each
/// group with its bounds.
fn connected_bits(positions: &[Vec3], indices: &[u32]) -> Vec<Connected> {
    // Join vertices that share a position, so seams do not split a surface.
    // Positions are compared exactly: seam duplicates are copies of one value,
    // and the same transform applied to equal values gives equal values.
    let mut welded: Vec<usize> = Vec::with_capacity(positions.len());
    let mut first_at: HashMap<[u32; 3], usize> = HashMap::new();
    for (i, p) in positions.iter().enumerate() {
        let key = [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
        welded.push(*first_at.entry(key).or_insert(i));
    }
    let mut sets = UnionFind::new(positions.len());
    let triangles: Vec<[u32; 3]> = indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|t| t.iter().all(|&i| (i as usize) < positions.len()))
        .copied()
        .collect();
    for t in &triangles {
        let a = welded[t[0] as usize];
        sets.join(a, welded[t[1] as usize]);
        sets.join(a, welded[t[2] as usize]);
    }
    let mut bits: HashMap<usize, Connected> = HashMap::new();
    for t in triangles {
        let root = sets.root(welded[t[0] as usize]);
        let entry = bits.entry(root).or_insert_with(|| (Vec::new(), (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN))));
        for &i in &t {
            let p = positions[i as usize];
            entry.1 .0 = entry.1 .0.min(p);
            entry.1 .1 = entry.1 .1.max(p);
        }
        entry.0.push(t);
    }
    bits.into_values().collect()
}

/// A new primitive drawing `triangles` of `source`, with its own copy of every
/// vertex they use, moved into the piece's space by `to_piece`. Positions,
/// normals and tangents are transformed; every other attribute is copied byte
/// for byte. Everything else about the primitive (material, extras) is kept.
fn copy_primitive(glb: &mut Glb, source: &Value, triangles: &[[u32; 3]], to_piece: Mat4) -> Result<Value, GlbError> {
    // The vertices used, in first-use order, and each one's new index.
    let mut remap: HashMap<u32, u32> = HashMap::new();
    let mut used: Vec<u32> = Vec::new();
    let mut indices = Vec::with_capacity(triangles.len() * 3);
    for &i in triangles.iter().flatten() {
        let next = used.len() as u32;
        let new = *remap.entry(i).or_insert_with(|| {
            used.push(i);
            next
        });
        indices.push(new);
    }

    let linear = Mat3::from_mat4(to_piece);
    let turned = !linear.abs_diff_eq(Mat3::IDENTITY, 1e-6);
    let normal_matrix = linear.inverse().transpose();

    let mut attributes = serde_json::Map::new();
    let source_attributes = source.get("attributes").and_then(Value::as_object).cloned().unwrap_or_default();
    for (semantic, accessor) in source_attributes {
        let accessor = accessor.as_u64().ok_or_else(|| unsupported("an attribute without an accessor"))? as usize;
        let (elements, mut meta) = read_elements(glb, accessor)?;
        let is_float = meta.get("componentType").and_then(Value::as_u64) == Some(FLOAT);
        let shape = meta.get("type").and_then(Value::as_str).unwrap_or("").to_string();
        let transform: Option<Box<dyn Fn(&[u8]) -> Vec<u8>>> = match semantic.as_str() {
            "POSITION" => Some(Box::new(move |b| to_bytes(to_piece.transform_point3(vec3_at(b)).to_array()))),
            "NORMAL" if turned => {
                if !is_float {
                    return Err(unsupported("normals stored as integers cannot be turned"));
                }
                Some(Box::new(move |b| to_bytes((normal_matrix * vec3_at(b)).normalize_or_zero().to_array())))
            }
            "TANGENT" if turned => {
                if !is_float || shape != "VEC4" {
                    return Err(unsupported("tangents stored as integers cannot be turned"));
                }
                Some(Box::new(move |b| {
                    let t = (linear * vec3_at(b)).normalize_or_zero();
                    let w = f32::from_le_bytes(b[12..16].try_into().expect("4 bytes"));
                    to_bytes([t.x, t.y, t.z, w])
                }))
            }
            _ => None,
        };
        let mut data = Vec::new();
        let mut lo_hi = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for &v in &used {
            let bytes = elements.get(v as usize).ok_or_else(|| unsupported("a triangle uses a vertex the mesh does not have"))?;
            match &transform {
                Some(f) => {
                    let out = f(bytes);
                    if semantic == "POSITION" {
                        let p = vec3_at(&out);
                        lo_hi = (lo_hi.0.min(p), lo_hi.1.max(p));
                    }
                    data.extend(out);
                }
                None => data.extend_from_slice(bytes),
            }
        }
        let view = glb.push_view(&data, Some(ARRAY_BUFFER));
        meta["bufferView"] = json!(view);
        meta["count"] = json!(used.len());
        if let Some(m) = meta.as_object_mut() {
            m.remove("byteOffset");
            m.remove("min");
            m.remove("max");
        }
        // The spec requires bounds on positions; other attributes drop theirs,
        // which described the whole mesh and would now be wrong.
        if semantic == "POSITION" {
            meta["min"] = json!(lo_hi.0.to_array());
            meta["max"] = json!(lo_hi.1.to_array());
        }
        attributes.insert(semantic, json!(glb.push_accessor(meta)));
    }

    let index_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let view = glb.push_view(&index_bytes, Some(ELEMENT_ARRAY_BUFFER));
    let index_accessor = glb.push_accessor(json!({
        "bufferView": view,
        "componentType": UNSIGNED_INT,
        "count": indices.len(),
        "type": "SCALAR",
    }));

    let mut primitive = source.clone();
    primitive["attributes"] = Value::Object(attributes);
    primitive["indices"] = json!(index_accessor);
    primitive["mode"] = json!(4);
    Ok(primitive)
}

fn vec3_at(b: &[u8]) -> Vec3 {
    let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().expect("4 bytes"));
    Vec3::new(f(0), f(4), f(8))
}

fn to_bytes<const N: usize>(values: [f32; N]) -> Vec<u8> {
    values.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// An accessor's elements as raw bytes, one per element, with the accessor's
/// own description (component type, shape, normalised) to reuse for a copy.
fn read_elements(glb: &Glb, accessor: usize) -> Result<(Vec<Vec<u8>>, Value), GlbError> {
    let acc = glb.json["accessors"].get(accessor).cloned().ok_or_else(|| unsupported("missing accessor"))?;
    if acc.get("sparse").is_some() {
        return Err(unsupported("sparse vertex data cannot be separated"));
    }
    let component = match acc.get("componentType").and_then(Value::as_u64) {
        Some(5120 | 5121) => 1,
        Some(5122 | 5123) => 2,
        Some(5125 | FLOAT) => 4,
        _ => return Err(unsupported("vertex data of an unknown type")),
    };
    let width = match acc.get("type").and_then(Value::as_str) {
        Some("SCALAR") => 1,
        Some("VEC2") => 2,
        Some("VEC3") => 3,
        Some("VEC4") | Some("MAT2") => 4,
        Some("MAT3") => 9,
        Some("MAT4") => 16,
        _ => return Err(unsupported("vertex data of an unknown shape")),
    };
    let element = component * width;
    let count = acc.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
    let Some(view_i) = acc.get("bufferView").and_then(Value::as_u64) else {
        // No buffer view means all zeros, by the spec.
        return Ok((vec![vec![0; element]; count], acc));
    };
    let view = glb.json["bufferViews"].get(view_i as usize).ok_or_else(|| unsupported("missing buffer view"))?;
    if view.get("buffer").and_then(Value::as_u64).unwrap_or(0) != 0 {
        return Err(unsupported("geometry in an external buffer"));
    }
    let base = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let stride = view.get("byteStride").and_then(Value::as_u64).map(|s| s as usize).unwrap_or(element).max(element);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let at = base + i * stride;
        let bytes = glb.bin.get(at..at + element).ok_or_else(|| unsupported("vertex data runs past the end of the binary"))?;
        out.push(bytes.to_vec());
    }
    Ok((out, acc))
}

/// Disjoint sets with path halving.
struct UnionFind(Vec<usize>);

impl UnionFind {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }

    fn root(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.root(a), self.root(b));
        if a != b {
            self.0[a] = b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::Geometry;

    /// A cube's 24 vertices (four per face, so faces share no vertices, as an
    /// exported model's seams do not) and 12 triangles, centred on `at`.
    fn cube(at: Vec3, size: f32) -> (Vec<Vec3>, Vec<u32>) {
        let h = size * 0.5;
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                let mut n = Vec3::ZERO;
                n[axis] = sign;
                let (u, v) = match axis {
                    0 => (Vec3::Y, Vec3::Z),
                    1 => (Vec3::Z, Vec3::X),
                    _ => (Vec3::X, Vec3::Y),
                };
                let base = positions.len() as u32;
                for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    positions.push(at + (n + u * a + v * b) * h);
                }
                indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        (positions, indices)
    }

    /// Cubes merged into one mesh.
    fn cubes(name: &str, list: &[(Vec3, f32)]) -> Glb {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for &(at, size) in list {
            let (p, i) = cube(at, size);
            let base = positions.len() as u32;
            positions.extend(p);
            indices.extend(i.into_iter().map(|i| i + base));
        }
        Glb::from_geometry(&Geometry { name, positions: &positions, normals: &[], uvs: &[], indices: &indices, base_color: [1.0; 4] })
            .expect("geometry")
    }

    fn mesh_bounds(glb: &Glb, mesh: usize) -> (Vec3, Vec3) {
        let mut all = Vec::new();
        for prim in 0..glb.meshes()[mesh]["primitives"].as_array().unwrap().len() {
            all.extend(glb.positions(mesh, prim).unwrap());
        }
        bounds(all.into_iter())
    }

    /// Two wheels a car's width apart, each a tyre with a smaller hub inside it
    /// that touches it nowhere, all in one mesh.
    #[test]
    fn an_axle_separates_into_two_wheels_each_centred_on_its_hub() {
        let mut glb = cubes(
            "axle",
            &[
                (Vec3::new(-1.5, 0.35, 2.0), 0.7),
                (Vec3::new(-1.5, 0.35, 2.0), 0.3),
                (Vec3::new(1.5, 0.35, 2.0), 0.7),
                (Vec3::new(1.5, 0.35, 2.0), 0.3),
            ],
        );
        let done = separate_parts(&mut glb, &["axle"]).expect("separates");
        assert_eq!(done.parent, "axle");
        let parts = &done.parts;
        assert_eq!(parts.len(), 2, "{parts:?}");
        assert_eq!(parts[0].name, "axle_part1");
        assert!((parts[0].centre - Vec3::new(-1.5, 0.35, 2.0)).length() < 1e-4, "{parts:?}");
        assert!((parts[1].centre - Vec3::new(1.5, 0.35, 2.0)).length() < 1e-4, "{parts:?}");
        // Tyre and hub stayed together: both cubes' triangles in each wheel.
        assert!(parts.iter().all(|p| p.triangles == 24), "{parts:?}");

        // The file round-trips, the original node lost its mesh and gained
        // the wheels, and each wheel's vertices sit around its own origin.
        let bytes = glb.to_bytes();
        let glb = Glb::parse(&bytes).expect("still a glb");
        let original = glb.nodes().iter().position(|n| n["name"] == "axle").expect("original node");
        assert!(glb.nodes()[original].get("mesh").is_none());
        let children = glb.nodes()[original]["children"].as_array().expect("children").clone();
        assert_eq!(children.len(), 2);
        for child in children {
            let mesh = glb.nodes()[child.as_u64().unwrap() as usize]["mesh"].as_u64().unwrap() as usize;
            let (lo, hi) = mesh_bounds(&glb, mesh);
            assert!(((lo + hi) * 0.5).length() < 1e-4, "a wheel is not centred on its node");
            assert!((hi - lo - Vec3::splat(0.7)).length() < 1e-4, "a wheel lost its tyre");
        }
    }

    /// The way exporters write an axle: a node per material under it, each
    /// material's mesh holding both wheels' share. The wheels come back whole,
    /// one primitive per material, under the axle, and every source mesh's
    /// node offset is carried into the result.
    #[test]
    fn meshes_under_one_node_regroup_into_whole_wheels() {
        // Tyres as one mesh, hubs as another, merged into one file.
        let mut glb = cubes("tyres", &[(Vec3::new(-1.5, 0.0, 0.0), 0.7), (Vec3::new(1.5, 0.0, 0.0), 0.7)]);
        let hubs = cubes("hubs", &[(Vec3::new(-1.5, 0.0, 0.0), 0.3), (Vec3::new(1.5, 0.0, 0.0), 0.3)]);
        // Splice the hubs' mesh into the tyres' file by appending its data.
        let offset = glb.bin.len();
        glb.bin.extend_from_slice(&hubs.bin);
        let view_base = glb.json["bufferViews"].as_array().unwrap().len();
        for mut v in hubs.json["bufferViews"].as_array().unwrap().clone() {
            v["byteOffset"] = json!(v["byteOffset"].as_u64().unwrap_or(0) as usize + offset);
            glb.json["bufferViews"].as_array_mut().unwrap().push(v);
        }
        let acc_base = glb.json["accessors"].as_array().unwrap().len();
        for mut a in hubs.json["accessors"].as_array().unwrap().clone() {
            a["bufferView"] = json!(a["bufferView"].as_u64().unwrap() as usize + view_base);
            glb.json["accessors"].as_array_mut().unwrap().push(a);
        }
        let mut hub_mesh = hubs.json["meshes"][0].clone();
        hub_mesh["primitives"][0]["attributes"]["POSITION"] =
            json!(hub_mesh["primitives"][0]["attributes"]["POSITION"].as_u64().unwrap() as usize + acc_base);
        hub_mesh["primitives"][0]["indices"] = json!(hub_mesh["primitives"][0]["indices"].as_u64().unwrap() as usize + acc_base);
        glb.json["meshes"].as_array_mut().unwrap().push(hub_mesh);
        // An axle node 2 m forward, holding both meshes on nodes of their own.
        let tyres_node = glb.nodes().iter().position(|n| n["name"] == "tyres").unwrap();
        glb.json["scenes"][0]["nodes"] = json!([]);
        let axle = glb.add_node(json!({ "name": "axle", "translation": [0.0, 0.0, 2.0] }), None);
        glb.json["nodes"][axle]["children"] = json!([tyres_node]);
        glb.add_node(json!({ "name": "hubs", "mesh": 1 }), Some(axle));

        let done = separate_parts(&mut glb, &["tyres", "hubs"]).expect("separates");
        assert_eq!(done.parent, "axle");
        assert_eq!(done.parts.len(), 2, "{:?}", done.parts);
        assert!((done.parts[0].centre - Vec3::new(-1.5, 0.0, 0.0)).length() < 1e-4, "{:?}", done.parts);
        for part in &done.parts {
            let node = glb.nodes().iter().position(|n| n["name"] == part.name.as_str()).unwrap();
            let mesh = glb.nodes()[node]["mesh"].as_u64().unwrap() as usize;
            assert_eq!(glb.meshes()[mesh]["primitives"].as_array().unwrap().len(), 2, "tyre and hub are one wheel");
            let (lo, hi) = mesh_bounds(&glb, mesh);
            assert!(((lo + hi) * 0.5).length() < 1e-4);
            assert!(glb.parents()[node] == Some(axle));
        }
        assert!(glb.nodes()[tyres_node].get("mesh").is_none());
    }

    #[test]
    fn a_mesh_in_one_piece_is_refused_and_left_alone() {
        let mut glb = cubes("box", &[(Vec3::ZERO, 1.0)]);
        let before = glb.json.clone();
        assert!(separate_parts(&mut glb, &["box"]).is_err());
        assert_eq!(glb.json, before);
    }

    #[test]
    fn an_unknown_mesh_is_refused() {
        let mut glb = cubes("axle", &[(Vec3::new(-1.5, 0.0, 0.0), 0.7), (Vec3::new(1.5, 0.0, 0.0), 0.7)]);
        assert!(separate_parts(&mut glb, &["no_such_mesh"]).is_err());
    }
}

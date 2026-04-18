#![allow(dead_code, unused_variables)] // Legacy FBX parser kept for reference after ufbx swap.

//! FBX skin + skeleton extraction.
//!
//! Parses the raw FBX node tree to extract:
//! - Joint hierarchy (Model::LimbNode objects + parent/child connections)
//! - Per-joint local transforms (PreRotation, Lcl Translation/Rotation/Scaling)
//! - Skin clusters (vertex weights + inverse bind matrices via TransformLink)
//!
//! Output is a [`SkinData`] struct that the GLB builder consumes to emit a
//! skinned mesh with JOINTS_0 / WEIGHTS_0 attributes + GLTF skin + joint nodes.
//!
//! Mixamo-specific notes:
//! - Bones carry PreRotation that must be baked into the joint's local
//!   rotation so the bind pose matches the skin weights.
//! - TransformLink (bone bind-pose world matrix) and Transform (mesh bind-pose
//!   world matrix) together yield the inverse bind matrix:
//!     `IBM = inverse(TransformLink) * Transform`
//!   For Mixamo, Transform is usually identity.

use std::collections::HashMap;

use crate::fbx_legacy::{
    extract_f64_array, extract_i32_array, find_child, get_i64_prop,
    get_string_prop, FbxNode, FbxProp,
};

// ─── Public types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SkinData {
    pub joints: Vec<Joint>,
    /// geometry object_id → per-vertex influence list. Indexed by FBX vertex
    /// index (same index space as the Vertices/PolygonVertexIndex arrays).
    pub per_geometry: HashMap<i64, GeometrySkin>,
}

#[derive(Debug, Clone)]
pub struct Joint {
    pub id: i64,
    pub name: String,
    /// Index into [`SkinData::joints`] of this joint's parent, or `None` if root.
    pub parent: Option<usize>,
    /// Local TRS relative to parent. Rotation is pre-composed with PreRotation.
    pub translation: [f32; 3],
    pub rotation: [f32; 4], // quaternion (x,y,z,w)
    pub scale: [f32; 3],
    /// 4×4 column-major inverse bind matrix.
    pub inverse_bind_matrix: [f32; 16],
}

#[derive(Debug, Clone, Default)]
pub struct GeometrySkin {
    /// Per-vertex joint indices (up to 4). Indexed by FBX vertex index.
    pub joint_indices: Vec<[u16; 4]>,
    /// Per-vertex weights (parallel to joint_indices). Normalized to sum to 1.
    pub weights: Vec<[f32; 4]>,
    /// Number of vertices we expect — used to validate and size arrays.
    pub vertex_count: usize,
}

// ─── Connection model (local copy — fbx_anim.rs has private copy) ──────────

#[derive(Debug, Clone)]
struct Connection {
    src_id: i64,
    dst_id: i64,
    property: Option<String>,
}

fn parse_connections(nodes: &[FbxNode]) -> Vec<Connection> {
    let mut conns = Vec::new();
    for node in nodes {
        if node.name != "Connections" {
            continue;
        }
        for child in &node.children {
            if child.name != "C" {
                continue;
            }
            let conn_type = get_string_prop(child, 0).unwrap_or("");
            let src_id = get_i64_prop(child, 1);
            let dst_id = get_i64_prop(child, 2);
            if let (Some(src), Some(dst)) = (src_id, dst_id) {
                let property = if conn_type == "OP" {
                    get_string_prop(child, 3).map(|s| s.to_string())
                } else {
                    None
                };
                conns.push(Connection {
                    src_id: src,
                    dst_id: dst,
                    property,
                });
            }
        }
    }
    conns
}

fn build_child_to_parents(conns: &[Connection]) -> HashMap<i64, Vec<(i64, Option<String>)>> {
    let mut map: HashMap<i64, Vec<(i64, Option<String>)>> = HashMap::new();
    for c in conns {
        map.entry(c.src_id)
            .or_default()
            .push((c.dst_id, c.property.clone()));
    }
    map
}

fn build_parent_to_children(conns: &[Connection]) -> HashMap<i64, Vec<(i64, Option<String>)>> {
    let mut map: HashMap<i64, Vec<(i64, Option<String>)>> = HashMap::new();
    for c in conns {
        map.entry(c.dst_id)
            .or_default()
            .push((c.src_id, c.property.clone()));
    }
    map
}

// ─── Object index ──────────────────────────────────────────────────────────

struct ObjectIndex<'a> {
    /// object_id → (class, name, subclass, node)
    objects: HashMap<i64, (&'a str, String, String, &'a FbxNode)>,
}

impl<'a> ObjectIndex<'a> {
    fn build(nodes: &'a [FbxNode]) -> Self {
        let mut objects = HashMap::new();
        for node in nodes {
            if node.name != "Objects" {
                continue;
            }
            for child in &node.children {
                let id = match get_i64_prop(child, 0) {
                    Some(id) => id,
                    None => continue,
                };
                let raw_name = get_string_prop(child, 1).unwrap_or("");
                let name = raw_name
                    .find("::")
                    .map(|i| &raw_name[i + 2..])
                    .unwrap_or(raw_name)
                    .to_string();
                let subclass = get_string_prop(child, 2).unwrap_or("").to_string();
                objects.insert(id, (child.name.as_str(), name, subclass, child));
            }
        }
        ObjectIndex { objects }
    }
    fn get(&self, id: i64) -> Option<&(&'a str, String, String, &'a FbxNode)> {
        self.objects.get(&id)
    }
    fn iter_by_class<'b>(
        &'b self,
        class: &'b str,
    ) -> impl Iterator<Item = (i64, &'b str, &'b str, &'a FbxNode)> + 'b {
        self.objects
            .iter()
            .filter(move |(_, (c, _, _, _))| *c == class)
            .map(|(&id, (c, n, s, node))| (id, *c, s.as_str(), *node))
            .map(move |(id, _c, sub, node)| {
                let name = self.objects.get(&id).map(|v| v.1.as_str()).unwrap_or("");
                (id, name, sub, node)
            })
    }
}

// ─── Property70 parsing ────────────────────────────────────────────────────
//
// FBX stores per-object properties inside a `Properties70` sub-node containing
// many `P` entries: `P: "PropName", "Type", "Type2", "Flags", value...`
// For our purposes we need vector-valued properties like Lcl Translation,
// Lcl Rotation, PreRotation.

fn get_prop70_vec3(obj_node: &FbxNode, prop_name: &str) -> Option<[f64; 3]> {
    let props70 = find_child(obj_node, "Properties70")?;
    for p in &props70.children {
        if p.name != "P" {
            continue;
        }
        let name = match get_string_prop(p, 0) {
            Some(s) => s,
            None => continue,
        };
        if name != prop_name {
            continue;
        }
        // Value starts at property index 4 (after name, type, subtype, flags).
        let x = prop_as_f64(p.properties.get(4))?;
        let y = prop_as_f64(p.properties.get(5))?;
        let z = prop_as_f64(p.properties.get(6))?;
        return Some([x, y, z]);
    }
    None
}

fn prop_as_f64(p: Option<&FbxProp>) -> Option<f64> {
    match p? {
        FbxProp::F64(v) => Some(*v),
        FbxProp::F32(v) => Some(*v as f64),
        FbxProp::I32(v) => Some(*v as f64),
        FbxProp::I64(v) => Some(*v as f64),
        _ => None,
    }
}

// ─── Math helpers ──────────────────────────────────────────────────────────

/// Euler (degrees, intrinsic XYZ) → quaternion (x,y,z,w).
fn euler_xyz_to_quat(deg: [f64; 3]) -> [f32; 4] {
    let (hx, hy, hz) = (
        (deg[0].to_radians() * 0.5) as f32,
        (deg[1].to_radians() * 0.5) as f32,
        (deg[2].to_radians() * 0.5) as f32,
    );
    let (sx, cx) = hx.sin_cos();
    let (sy, cy) = hy.sin_cos();
    let (sz, cz) = hz.sin_cos();
    // XYZ intrinsic (standard FBX default)
    let w = cx * cy * cz + sx * sy * sz;
    let x = sx * cy * cz - cx * sy * sz;
    let y = cx * sy * cz + sx * cy * sz;
    let z = cx * cy * sz - sx * sy * cz;
    [x, y, z, w]
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn quat_identity() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

/// Invert a 4×4 affine matrix (column-major). Returns identity on failure.
fn mat4_inverse(m: [f64; 16]) -> [f32; 16] {
    // Full 4×4 inverse via cofactor expansion. Expects non-singular input.
    let mut inv = [0.0f64; 16];

    inv[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15]
        + m[9] * m[7] * m[14] + m[13] * m[6] * m[11] - m[13] * m[7] * m[10];
    inv[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15]
        - m[8] * m[7] * m[14] - m[12] * m[6] * m[11] + m[12] * m[7] * m[10];
    inv[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15]
        + m[8] * m[7] * m[13] + m[12] * m[5] * m[11] - m[12] * m[7] * m[9];
    inv[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14]
        - m[8] * m[6] * m[13] - m[12] * m[5] * m[10] + m[12] * m[6] * m[9];
    inv[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15]
        - m[9] * m[3] * m[14] - m[13] * m[2] * m[11] + m[13] * m[3] * m[10];
    inv[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15]
        + m[8] * m[3] * m[14] + m[12] * m[2] * m[11] - m[12] * m[3] * m[10];
    inv[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15]
        - m[8] * m[3] * m[13] - m[12] * m[1] * m[11] + m[12] * m[3] * m[9];
    inv[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14]
        + m[8] * m[2] * m[13] + m[12] * m[1] * m[10] - m[12] * m[2] * m[9];
    inv[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15]
        + m[5] * m[3] * m[14] + m[13] * m[2] * m[7] - m[13] * m[3] * m[6];
    inv[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15]
        - m[4] * m[3] * m[14] - m[12] * m[2] * m[7] + m[12] * m[3] * m[6];
    inv[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15]
        + m[4] * m[3] * m[13] + m[12] * m[1] * m[7] - m[12] * m[3] * m[5];
    inv[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14]
        - m[4] * m[2] * m[13] - m[12] * m[1] * m[6] + m[12] * m[2] * m[5];
    inv[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11]
        - m[5] * m[3] * m[10] - m[9] * m[2] * m[7] + m[9] * m[3] * m[6];
    inv[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11]
        + m[4] * m[3] * m[10] + m[8] * m[2] * m[7] - m[8] * m[3] * m[6];
    inv[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11]
        - m[4] * m[3] * m[9] - m[8] * m[1] * m[7] + m[8] * m[3] * m[5];
    inv[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10]
        + m[4] * m[2] * m[9] + m[8] * m[1] * m[6] - m[8] * m[2] * m[5];

    let det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
    if det.abs() < 1e-12 {
        // Singular — return identity.
        let mut id = [0.0f32; 16];
        id[0] = 1.0;
        id[5] = 1.0;
        id[10] = 1.0;
        id[15] = 1.0;
        return id;
    }
    let inv_det = 1.0 / det;
    let mut out = [0.0f32; 16];
    for i in 0..16 {
        out[i] = (inv[i] * inv_det) as f32;
    }
    out
}

fn identity_mat4() -> [f32; 16] {
    let mut m = [0.0f32; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

// ─── Main extraction ───────────────────────────────────────────────────────

pub fn extract(nodes: &[FbxNode], scale: f32) -> Result<SkinData, String> {
    let conns = parse_connections(nodes);
    let parent_to_children = build_parent_to_children(&conns);
    let child_to_parents = build_child_to_parents(&conns);
    let index = ObjectIndex::build(nodes);

    // Collect all LimbNode joints.
    let mut joint_ids: Vec<i64> = index
        .objects
        .iter()
        .filter(|(_, (class, _, sub, _))| *class == "Model" && sub == "LimbNode")
        .map(|(&id, _)| id)
        .collect();

    // Some files use subclass "Null" for the skeleton root — include a node if
    // it's a parent of any LimbNode and has connections to any cluster.
    // Conservative: just use LimbNode for now; the root will be whichever node
    // has no LimbNode parent.

    if joint_ids.is_empty() {
        return Ok(SkinData {
            joints: Vec::new(),
            per_geometry: HashMap::new(),
        });
    }

    // Deterministic ordering — sort by id so output is stable across runs.
    joint_ids.sort_unstable();

    // Map object_id → joint vec index.
    let joint_id_to_idx: HashMap<i64, usize> = joint_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();

    // Build joints with local transforms + parent link.
    let mut joints: Vec<Joint> = Vec::with_capacity(joint_ids.len());
    for &jid in &joint_ids {
        let (_, name, _, node) = index.get(jid).ok_or_else(|| {
            format!("joint id {} missing from object index", jid)
        })?;
        let name = name.clone();

        // Local transform from Properties70 — fall back to zero/identity.
        let t = get_prop70_vec3(node, "Lcl Translation").unwrap_or([0.0; 3]);
        let r_euler = get_prop70_vec3(node, "Lcl Rotation").unwrap_or([0.0; 3]);
        let pre_euler = get_prop70_vec3(node, "PreRotation").unwrap_or([0.0; 3]);
        let post_euler = get_prop70_vec3(node, "PostRotation").unwrap_or([0.0; 3]);
        let s = get_prop70_vec3(node, "Lcl Scaling").unwrap_or([1.0; 3]);

        // Compose PreRotation * Rotation * inv(PostRotation). For Mixamo the
        // PostRotation is usually zero so this simplifies to Pre * R.
        let q_pre = euler_xyz_to_quat(pre_euler);
        let q_r = euler_xyz_to_quat(r_euler);
        let q_post = euler_xyz_to_quat(post_euler);
        let q_post_inv = [-q_post[0], -q_post[1], -q_post[2], q_post[3]];
        let rotation = quat_mul(quat_mul(q_pre, q_r), q_post_inv);

        // Parent: find a LimbNode parent via OO connections (property == None).
        let parent_idx = child_to_parents.get(&jid).and_then(|parents| {
            parents.iter().find_map(|(pid, prop)| {
                if prop.is_some() {
                    return None;
                }
                joint_id_to_idx.get(pid).copied()
            })
        });

        // Bake the import scale into every joint's local translation so the
        // skeleton lives in the same units as the mesh vertices.
        let translation = [
            (t[0] as f32) * scale,
            (t[1] as f32) * scale,
            (t[2] as f32) * scale,
        ];

        joints.push(Joint {
            id: jid,
            name,
            parent: parent_idx,
            translation,
            rotation,
            scale: [s[0] as f32, s[1] as f32, s[2] as f32],
            inverse_bind_matrix: identity_mat4(),
        });
    }

    // Collect per-geometry skin weights by walking Deformer::Skin → Deformer::Cluster.
    let mut per_geometry: HashMap<i64, GeometrySkin> = HashMap::new();

    for (geom_id, _g_name, _g_sub, g_node) in index.iter_by_class("Geometry") {
        // Determine vertex count for this geometry.
        let vertex_count = match find_child(g_node, "Vertices") {
            Some(v) => extract_f64_array(v).len() / 3,
            None => continue,
        };
        if vertex_count == 0 {
            continue;
        }

        // Find Deformer::Skin nodes linked to this geometry.
        let mut skin_ids: Vec<i64> = Vec::new();
        if let Some(kids) = parent_to_children.get(&geom_id) {
            for (child_id, _) in kids {
                if let Some((class, _, sub, _)) = index.get(*child_id) {
                    if *class == "Deformer" && sub == "Skin" {
                        skin_ids.push(*child_id);
                    }
                }
            }
        }
        if skin_ids.is_empty() {
            continue;
        }

        // Accumulate (joint_idx, weight) per vertex from all clusters.
        let mut raw: Vec<Vec<(u16, f32)>> = vec![Vec::new(); vertex_count];

        for skin_id in skin_ids {
            // Clusters are children of the skin.
            let cluster_ids: Vec<i64> = parent_to_children
                .get(&skin_id)
                .map(|v| {
                    v.iter()
                        .filter_map(|(cid, _)| {
                            index.get(*cid).and_then(|(class, _, sub, _)| {
                                if *class == "Deformer" && sub == "Cluster" {
                                    Some(*cid)
                                } else {
                                    None
                                }
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            for cluster_id in cluster_ids {
                let (_, _, _, cluster_node) = match index.get(cluster_id) {
                    Some(v) => v,
                    None => continue,
                };

                // The bone is the child (src) of the cluster.
                let bone_id = parent_to_children
                    .get(&cluster_id)
                    .and_then(|kids| {
                        kids.iter().find_map(|(cid, _)| {
                            if joint_id_to_idx.contains_key(cid) {
                                Some(*cid)
                            } else {
                                None
                            }
                        })
                    });
                let Some(bone_id) = bone_id else { continue };
                let jidx = joint_id_to_idx[&bone_id];

                let indexes = find_child(cluster_node, "Indexes")
                    .map(extract_i32_array)
                    .unwrap_or_default();
                let weights = find_child(cluster_node, "Weights")
                    .map(extract_f64_array)
                    .unwrap_or_default();

                for (i, &vidx) in indexes.iter().enumerate() {
                    if (vidx as usize) < vertex_count {
                        let w = *weights.get(i).unwrap_or(&0.0) as f32;
                        if w > 0.0 {
                            raw[vidx as usize].push((jidx as u16, w));
                        }
                    }
                }

                // Inverse bind matrix: inverse of TransformLink (world bind of bone),
                // optionally combined with Transform (world bind of mesh). Mixamo's
                // mesh Transform is normally identity so inv(TransformLink) is fine.
                let transform_link = find_child(cluster_node, "TransformLink")
                    .map(extract_f64_array)
                    .unwrap_or_default();
                if transform_link.len() == 16 {
                    // Scale the translation column by import scale so the bind
                    // pose lives in meters alongside the mesh vertices.
                    let mut m = [0.0f64; 16];
                    m.copy_from_slice(&transform_link);
                    m[12] *= scale as f64;
                    m[13] *= scale as f64;
                    m[14] *= scale as f64;
                    let ibm = mat4_inverse(m);
                    joints[jidx].inverse_bind_matrix = ibm;
                }
            }
        }

        // Normalize to top-4 per vertex.
        let mut joint_indices = Vec::with_capacity(vertex_count);
        let mut weight_values = Vec::with_capacity(vertex_count);
        for infl in &mut raw {
            infl.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top = &infl[..infl.len().min(4)];
            let mut js = [0u16; 4];
            let mut ws = [0.0f32; 4];
            for (i, (j, w)) in top.iter().enumerate() {
                js[i] = *j;
                ws[i] = *w;
            }
            let sum: f32 = ws.iter().sum();
            if sum > 0.0 {
                for w in &mut ws {
                    *w /= sum;
                }
            }
            joint_indices.push(js);
            weight_values.push(ws);
        }

        per_geometry.insert(
            geom_id,
            GeometrySkin {
                joint_indices,
                weights: weight_values,
                vertex_count,
            },
        );
    }

    Ok(SkinData {
        joints,
        per_geometry,
    })
}

// ─── Public top-level entry ────────────────────────────────────────────────

pub fn extract_from_file(
    path: &std::path::Path,
    scale: f32,
) -> Result<SkinData, String> {
    let data =
        std::fs::read(path).map_err(|e| format!("failed to read FBX file: {}", e))?;
    let (_version, nodes) = crate::fbx_legacy::parse_document(&data)
        .map_err(|e| format!("FBX parse error: {}", e))?;
    extract(&nodes, scale)
}

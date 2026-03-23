//! FBX skeleton, skin-weight, and animation-curve parsing.
//!
//! Uses the fbxcel tree API to extract:
//! - Connection maps (OO and OP)
//! - Skeleton (LimbNode bones)
//! - Inverse bind matrices (from BindPose)
//! - Skin cluster weights
//! - Animation stacks/layers/curves

use std::collections::HashMap;
use fbxcel::low::v7400::AttributeValue;
use fbxcel::tree::v7400::NodeHandle;

// ============================================================================
// Connection Maps
// ============================================================================

/// src_id → [(dst_id, optional property name)]
pub type ConnMap = HashMap<i64, Vec<(i64, Option<String>)>>;

/// Parse the Connections section into child_of and prop_of maps.
///
/// Returns `(child_of, prop_of)` where:
/// - `child_of[child_id] = [(parent_id, None)]`  — OO connections
/// - `prop_of[src_id] = [(dst_id, Some("Lcl Translation"))]`  — OP connections
pub fn parse_connections(root: &NodeHandle) -> (ConnMap, ConnMap) {
    let mut child_of: ConnMap = HashMap::new();
    let mut prop_of: ConnMap = HashMap::new();

    let connections_node = root
        .children()
        .find(|n| n.name() == "Connections");

    let Some(conn_node) = connections_node else {
        return (child_of, prop_of);
    };

    for c in conn_node.children() {
        if c.name() != "C" {
            continue;
        }
        let attrs = c.attributes();
        let conn_type = attrs.get(0).and_then(get_string).unwrap_or_default();
        let src_id = attrs.get(1).and_then(get_i64).unwrap_or(0);
        let dst_id = attrs.get(2).and_then(get_i64).unwrap_or(0);

        match conn_type.as_str() {
            "OO" => {
                child_of.entry(src_id).or_default().push((dst_id, None));
            }
            "OP" => {
                let prop_name = attrs.get(3).and_then(get_string);
                prop_of.entry(src_id).or_default().push((dst_id, prop_name));
            }
            _ => {}
        }
    }

    (child_of, prop_of)
}

// ============================================================================
// Skeleton
// ============================================================================

#[derive(Debug, Clone)]
pub struct BoneData {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub local_translation: [f32; 3],
    pub local_rotation: [f32; 3],  // Euler degrees XYZ
    pub local_scale: [f32; 3],
}

/// Parse all LimbNode Model nodes from the Objects section.
pub fn parse_skeleton(objects: &NodeHandle, child_of: &ConnMap) -> Vec<BoneData> {
    let mut bones: Vec<BoneData> = Vec::new();

    // Build reverse map: parent_id → [child_id] for OO connections
    let mut parent_of: HashMap<i64, i64> = HashMap::new();
    for (child_id, parents) in child_of {
        for (parent_id, _) in parents {
            parent_of.insert(*child_id, *parent_id);
        }
    }

    // Collect all bone node IDs first
    let mut bone_ids: HashMap<i64, ()> = HashMap::new();

    for node in objects.children() {
        if node.name() != "Model" {
            continue;
        }
        let attrs = node.attributes();
        // Model nodes: [id: i64, name: String, "LimbNode" or "Mesh" or "Camera" ...]
        let node_type = attrs.get(2).and_then(get_string).unwrap_or_default();
        if node_type != "LimbNode" && node_type != "Root" {
            continue;
        }
        let id = attrs.get(0).and_then(get_i64).unwrap_or(0);
        bone_ids.insert(id, ());
    }

    for node in objects.children() {
        if node.name() != "Model" {
            continue;
        }
        let attrs = node.attributes();
        let node_type = attrs.get(2).and_then(get_string).unwrap_or_default();
        if node_type != "LimbNode" && node_type != "Root" {
            continue;
        }

        let id = attrs.get(0).and_then(get_i64).unwrap_or(0);
        let raw_name = attrs.get(1).and_then(get_string).unwrap_or_default();
        let name = clean_fbx_name(&raw_name);

        let mut translation = [0.0f32; 3];
        let mut rotation = [0.0f32; 3];
        let mut scale = [1.0f32; 3];

        // Parse Properties70
        for child in node.children() {
            if child.name() != "Properties70" {
                continue;
            }
            for p in child.children() {
                if p.name() != "P" {
                    continue;
                }
                let p_attrs = p.attributes();
                let prop_name = p_attrs.get(0).and_then(get_string).unwrap_or_default();
                match prop_name.as_str() {
                    "Lcl Translation" => {
                        translation[0] = p_attrs.get(4).and_then(get_f64).unwrap_or(0.0) as f32;
                        translation[1] = p_attrs.get(5).and_then(get_f64).unwrap_or(0.0) as f32;
                        translation[2] = p_attrs.get(6).and_then(get_f64).unwrap_or(0.0) as f32;
                    }
                    "Lcl Rotation" => {
                        rotation[0] = p_attrs.get(4).and_then(get_f64).unwrap_or(0.0) as f32;
                        rotation[1] = p_attrs.get(5).and_then(get_f64).unwrap_or(0.0) as f32;
                        rotation[2] = p_attrs.get(6).and_then(get_f64).unwrap_or(0.0) as f32;
                    }
                    "Lcl Scaling" => {
                        scale[0] = p_attrs.get(4).and_then(get_f64).unwrap_or(1.0) as f32;
                        scale[1] = p_attrs.get(5).and_then(get_f64).unwrap_or(1.0) as f32;
                        scale[2] = p_attrs.get(6).and_then(get_f64).unwrap_or(1.0) as f32;
                    }
                    _ => {}
                }
            }
        }

        // Parent: look up in child_of (this bone's parent must also be a bone)
        let parent_id = parent_of
            .get(&id)
            .copied()
            .filter(|pid| bone_ids.contains_key(pid));

        bones.push(BoneData {
            id,
            name,
            parent_id,
            local_translation: translation,
            local_rotation: rotation,
            local_scale: scale,
        });
    }

    bones
}

// ============================================================================
// Inverse Bind Matrices
// ============================================================================

/// Parse BindPose nodes to extract global bind-pose matrices per bone ID.
/// Returns a map from bone node ID → 4x4 column-major inverse bind matrix.
pub fn parse_inverse_bind_matrices(objects: &NodeHandle) -> HashMap<i64, [[f32; 4]; 4]> {
    let mut result: HashMap<i64, [[f32; 4]; 4]> = HashMap::new();

    for node in objects.children() {
        if node.name() != "Pose" {
            continue;
        }
        let attrs = node.attributes();
        let pose_type = node.children()
            .find(|c| c.name() == "Type")
            .and_then(|c| c.attributes().get(0).and_then(get_string))
            .unwrap_or_default();

        // Also check attribute[2]
        let pose_type2 = attrs.get(2).and_then(get_string).unwrap_or_default();
        if pose_type != "BindPose" && pose_type2 != "BindPose" {
            continue;
        }

        for pose_child in node.children() {
            if pose_child.name() != "PoseNode" {
                continue;
            }

            let mut node_id: i64 = 0;
            let mut matrix_data: Vec<f64> = Vec::new();

            for field in pose_child.children() {
                match field.name() {
                    "Node" => {
                        node_id = field.attributes().get(0).and_then(get_i64).unwrap_or(0);
                    }
                    "Matrix" => {
                        if let Some(data) = field.attributes().get(0).and_then(get_f64_array) {
                            matrix_data = data;
                        }
                    }
                    _ => {}
                }
            }

            if node_id == 0 || matrix_data.len() < 16 {
                continue;
            }

            // FBX stores matrices in row-major order; invert to get inverse bind matrix
            let mat = mat4_from_f64_row_major(&matrix_data);
            let inv = mat4_invert(mat);
            result.insert(node_id, inv);
        }
    }

    result
}

// ============================================================================
// Skin Weights
// ============================================================================

#[derive(Debug)]
pub struct VertexWeights {
    pub joint_indices: Vec<[u16; 4]>,
    pub joint_weights: Vec<[f32; 4]>,
}

/// Parse skin weights from Deformer/Cluster nodes.
pub fn parse_skin_weights(
    objects: &NodeHandle,
    child_of: &ConnMap,
    bone_id_to_joint_idx: &HashMap<i64, u16>,
    vertex_count: usize,
) -> Option<VertexWeights> {
    if vertex_count == 0 {
        return None;
    }

    // Collect per-vertex contributions: (vertex_idx, joint_idx, weight)
    let mut contributions: Vec<Vec<(u16, f32)>> = vec![Vec::new(); vertex_count];
    let mut found_any = false;

    for node in objects.children() {
        if node.name() != "Deformer" {
            continue;
        }
        let attrs = node.attributes();
        let def_type = attrs.get(2).and_then(get_string).unwrap_or_default();
        if def_type != "Cluster" {
            continue;
        }

        let cluster_id = attrs.get(0).and_then(get_i64).unwrap_or(0);

        // Find which bone this cluster connects to via child_of (OO connection)
        let bone_id = child_of
            .get(&cluster_id)
            .and_then(|parents| parents.first())
            .map(|(pid, _)| *pid)
            .unwrap_or(0);

        let joint_idx = match bone_id_to_joint_idx.get(&bone_id) {
            Some(&idx) => idx,
            None => continue,
        };

        let mut vert_indices: Vec<i32> = Vec::new();
        let mut vert_weights: Vec<f64> = Vec::new();

        for cluster_child in node.children() {
            match cluster_child.name() {
                "Indexes" => {
                    if let Some(data) = cluster_child.attributes().get(0).and_then(get_i32_array) {
                        vert_indices = data;
                    }
                }
                "Weights" => {
                    if let Some(data) = cluster_child.attributes().get(0).and_then(get_f64_array) {
                        vert_weights = data;
                    }
                }
                _ => {}
            }
        }

        for (vi_idx, &vi) in vert_indices.iter().enumerate() {
            let vi = vi as usize;
            if vi < vertex_count {
                let w = vert_weights.get(vi_idx).copied().unwrap_or(0.0) as f32;
                if w > 0.0 {
                    contributions[vi].push((joint_idx, w));
                    found_any = true;
                }
            }
        }
    }

    if !found_any {
        return None;
    }

    // Build final joint_indices and joint_weights arrays
    let mut joint_indices: Vec<[u16; 4]> = vec![[0; 4]; vertex_count];
    let mut joint_weights: Vec<[f32; 4]> = vec![[0.0; 4]; vertex_count];

    for (vi, contribs) in contributions.iter().enumerate() {
        // Sort by weight descending and keep top 4
        let mut sorted = contribs.clone();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(4);

        // Normalize weights
        let weight_sum: f32 = sorted.iter().map(|(_, w)| w).sum();
        for (slot, (ji, w)) in sorted.iter().enumerate() {
            joint_indices[vi][slot] = *ji;
            joint_weights[vi][slot] = if weight_sum > 0.0 { w / weight_sum } else { 0.0 };
        }
    }

    Some(VertexWeights { joint_indices, joint_weights })
}

// ============================================================================
// Animation Extraction
// ============================================================================

#[derive(Debug, Clone)]
pub struct AnimKeyframe {
    pub time: f32,   // seconds
    pub value: f32,  // raw channel value
}

#[derive(Debug, Clone)]
pub struct AnimCurveNode {
    pub bone_id: i64,
    pub property: String,  // "Lcl Translation", "Lcl Rotation", "Lcl Scaling"
    pub x_curve: Vec<AnimKeyframe>,
    pub y_curve: Vec<AnimKeyframe>,
    pub z_curve: Vec<AnimKeyframe>,
}

#[derive(Debug, Clone)]
pub struct AnimStackData {
    pub name: String,
    pub duration: f32,  // seconds
    pub curve_nodes: Vec<AnimCurveNode>,
}

const KTIME_TICKS_PER_SECOND: f64 = 46_186_158_000.0;

/// Parse all AnimationStack entries from the Objects section.
pub fn parse_animations(
    objects: &NodeHandle,
    child_of: &ConnMap,
    prop_of: &ConnMap,
) -> Vec<AnimStackData> {
    let mut result = Vec::new();

    // Build reverse child_of map: parent → [children]
    let mut children_of: HashMap<i64, Vec<i64>> = HashMap::new();
    for (child_id, parents) in child_of {
        for (parent_id, _) in parents {
            children_of.entry(*parent_id).or_default().push(*child_id);
        }
    }

    // Collect all nodes by id for fast lookup
    let mut nodes_by_id: HashMap<i64, (String, String)> = HashMap::new(); // id → (node_type, subtype)
    for node in objects.children() {
        let attrs = node.attributes();
        let id = attrs.get(0).and_then(get_i64).unwrap_or(0);
        let subtype = attrs.get(2).and_then(get_string).unwrap_or_default();
        nodes_by_id.insert(id, (node.name().to_string(), subtype));
    }

    // Build map: id → full node handle (for curve data access)
    // We'll need to re-iterate objects for curve data
    // First collect stacks
    let mut stack_ids: Vec<i64> = Vec::new();
    for node in objects.children() {
        if node.name() == "AnimationStack" {
            let id = node.attributes().get(0).and_then(get_i64).unwrap_or(0);
            stack_ids.push(id);
        }
    }

    // Build animation curve data from objects
    // curve_id → (x_keys, y_keys, z_keys) — we'll map them via prop_of
    let mut curve_keys: HashMap<i64, Vec<AnimKeyframe>> = HashMap::new();
    for node in objects.children() {
        if node.name() != "AnimationCurve" {
            continue;
        }
        let id = node.attributes().get(0).and_then(get_i64).unwrap_or(0);
        let mut key_times: Vec<i64> = Vec::new();
        let mut key_values: Vec<f32> = Vec::new();

        for child in node.children() {
            match child.name() {
                "KeyTime" => {
                    if let Some(data) = child.attributes().get(0).and_then(get_i64_array) {
                        key_times = data;
                    }
                }
                "KeyValueFloat" => {
                    if let Some(data) = child.attributes().get(0).and_then(get_f32_array) {
                        key_values = data;
                    }
                }
                _ => {}
            }
        }

        let keyframes: Vec<AnimKeyframe> = key_times
            .iter()
            .zip(key_values.iter())
            .map(|(&t, &v)| AnimKeyframe {
                time: (t as f64 / KTIME_TICKS_PER_SECOND) as f32,
                value: v,
            })
            .collect();

        curve_keys.insert(id, keyframes);
    }

    // Build map: curve_node_id → (bone_id, property)
    // prop_of[curve_node_id] = [(bone_id, Some("Lcl Translation"|"Lcl Rotation"|"Lcl Scaling"))]
    let mut curve_node_bone: HashMap<i64, (i64, String)> = HashMap::new();
    for (src_id, dsts) in prop_of {
        for (dst_id, prop_name) in dsts {
            if let Some(prop) = prop_name {
                if matches!(prop.as_str(), "Lcl Translation" | "Lcl Rotation" | "Lcl Scaling") {
                    curve_node_bone.insert(*src_id, (*dst_id, prop.clone()));
                }
            }
        }
    }

    // For each AnimationCurveNode, find its X/Y/Z curves via prop_of
    // prop_of[curve_id] = [(curve_node_id, Some("d|X"|"d|Y"|"d|Z"))]
    // We need the reverse: curve_node_id → [(curve_id, "d|X")]
    let mut curve_node_curves: HashMap<i64, Vec<(i64, String)>> = HashMap::new();
    for (curve_id, dsts) in prop_of {
        // Check if this is an AnimationCurve → AnimationCurveNode connection
        if let Some((_, ref subtype)) = nodes_by_id.get(curve_id) {
            if subtype.is_empty() && nodes_by_id.get(curve_id).map(|(n, _)| n.as_str()) != Some("AnimationCurve") {
                // Skip non-curves
            }
        }
        for (curve_node_id, prop_name) in dsts {
            if let Some(prop) = prop_name {
                if matches!(prop.as_str(), "d|X" | "d|Y" | "d|Z") {
                    curve_node_curves
                        .entry(*curve_node_id)
                        .or_default()
                        .push((*curve_id, prop.clone()));
                }
            }
        }
    }

    // Process each stack
    for stack_id in stack_ids {
        let stack_node = objects.children()
            .find(|n| n.name() == "AnimationStack" &&
                n.attributes().get(0).and_then(get_i64) == Some(stack_id));

        let Some(stack_node) = stack_node else { continue };

        let raw_name = stack_node.attributes().get(1).and_then(get_string).unwrap_or_default();
        let name = clean_anim_name(&raw_name);

        // Compute duration from LocalStop property
        let mut duration = 0.0f32;
        for child in stack_node.children() {
            if child.name() != "Properties70" {
                continue;
            }
            for p in child.children() {
                if p.name() != "P" {
                    continue;
                }
                let prop_name = p.attributes().get(0).and_then(get_string).unwrap_or_default();
                if prop_name == "LocalStop" {
                    let ktime = p.attributes().get(4).and_then(get_i64).unwrap_or(0);
                    duration = (ktime as f64 / KTIME_TICKS_PER_SECOND) as f32;
                }
            }
        }

        // Traverse: stack → layer → curve_nodes
        let layer_ids = children_of.get(&stack_id).cloned().unwrap_or_default();
        let mut curve_nodes: Vec<AnimCurveNode> = Vec::new();

        for layer_id in layer_ids {
            if nodes_by_id.get(&layer_id).map(|(n, _)| n.as_str()) != Some("AnimationLayer") {
                continue;
            }

            let cn_ids = children_of.get(&layer_id).cloned().unwrap_or_default();
            for cn_id in cn_ids {
                if nodes_by_id.get(&cn_id).map(|(n, _)| n.as_str()) != Some("AnimationCurveNode") {
                    continue;
                }

                let Some((bone_id, property)) = curve_node_bone.get(&cn_id).cloned() else {
                    continue;
                };

                // Find X, Y, Z curves
                let mut x_curve = Vec::new();
                let mut y_curve = Vec::new();
                let mut z_curve = Vec::new();

                if let Some(curve_refs) = curve_node_curves.get(&cn_id) {
                    for (curve_id, axis) in curve_refs {
                        let keys = curve_keys.get(curve_id).cloned().unwrap_or_default();
                        match axis.as_str() {
                            "d|X" => x_curve = keys,
                            "d|Y" => y_curve = keys,
                            "d|Z" => z_curve = keys,
                            _ => {}
                        }
                    }
                }

                if x_curve.is_empty() && y_curve.is_empty() && z_curve.is_empty() {
                    continue;
                }

                curve_nodes.push(AnimCurveNode {
                    bone_id,
                    property,
                    x_curve,
                    y_curve,
                    z_curve,
                });
            }
        }

        // Infer duration from keyframes if not set by property
        if duration <= 0.0 {
            for cn in &curve_nodes {
                for kf in cn.x_curve.iter().chain(cn.y_curve.iter()).chain(cn.z_curve.iter()) {
                    duration = duration.max(kf.time);
                }
            }
        }

        if !curve_nodes.is_empty() || duration > 0.0 {
            result.push(AnimStackData { name, duration, curve_nodes });
        }
    }

    result
}

// ============================================================================
// Euler → Quaternion (ZYX order for Mixamo)
// ============================================================================

/// Convert Euler angles (degrees) in ZYX order to quaternion [x, y, z, w].
/// ZYX = Rz * Ry * Rx (X applied first, then Y, then Z — FBX default for Mixamo).
pub fn euler_zyx_to_quat(x_deg: f32, y_deg: f32, z_deg: f32) -> [f32; 4] {
    let hx = x_deg.to_radians() * 0.5;
    let hy = y_deg.to_radians() * 0.5;
    let hz = z_deg.to_radians() * 0.5;

    let (sx, cx) = hx.sin_cos();
    let (sy, cy) = hy.sin_cos();
    let (sz, cz) = hz.sin_cos();

    // ZYX order: q = qZ * qY * qX
    [
        sx * cy * cz - cx * sy * sz,
        cx * sy * cz + sx * cy * sz,
        cx * cy * sz - sx * sy * cz,
        cx * cy * cz + sx * sy * sz,
    ]
}

// ============================================================================
// Matrix Math Helpers
// ============================================================================

fn mat4_from_f64_row_major(data: &[f64]) -> [[f32; 4]; 4] {
    // FBX stores in row-major; glTF expects column-major
    // We'll store as [[col0], [col1], [col2], [col3]] where each is a column
    // Actually store row-major and transpose when writing
    let mut mat = [[0.0f32; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            mat[col][row] = data[row * 4 + col] as f32; // transpose to column-major
        }
    }
    mat
}

/// Invert a 4x4 column-major matrix (for rigid-body transforms).
fn mat4_invert(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // Use cofactor expansion (works for any matrix but we expect rigid-body)
    let m = [
        m[0][0], m[0][1], m[0][2], m[0][3],
        m[1][0], m[1][1], m[1][2], m[1][3],
        m[2][0], m[2][1], m[2][2], m[2][3],
        m[3][0], m[3][1], m[3][2], m[3][3],
    ];

    let cofactors = [
        det3(m[5],m[6],m[7], m[9],m[10],m[11], m[13],m[14],m[15]),
       -det3(m[1],m[2],m[3], m[9],m[10],m[11], m[13],m[14],m[15]),
        det3(m[1],m[2],m[3], m[5],m[6],m[7], m[13],m[14],m[15]),
       -det3(m[1],m[2],m[3], m[5],m[6],m[7], m[9],m[10],m[11]),

       -det3(m[4],m[6],m[7], m[8],m[10],m[11], m[12],m[14],m[15]),
        det3(m[0],m[2],m[3], m[8],m[10],m[11], m[12],m[14],m[15]),
       -det3(m[0],m[2],m[3], m[4],m[6],m[7], m[12],m[14],m[15]),
        det3(m[0],m[2],m[3], m[4],m[6],m[7], m[8],m[10],m[11]),

        det3(m[4],m[5],m[7], m[8],m[9],m[11], m[12],m[13],m[15]),
       -det3(m[0],m[1],m[3], m[8],m[9],m[11], m[12],m[13],m[15]),
        det3(m[0],m[1],m[3], m[4],m[5],m[7], m[12],m[13],m[15]),
       -det3(m[0],m[1],m[3], m[4],m[5],m[7], m[8],m[9],m[11]),

       -det3(m[4],m[5],m[6], m[8],m[9],m[10], m[12],m[13],m[14]),
        det3(m[0],m[1],m[2], m[8],m[9],m[10], m[12],m[13],m[14]),
       -det3(m[0],m[1],m[2], m[4],m[5],m[6], m[12],m[13],m[14]),
        det3(m[0],m[1],m[2], m[4],m[5],m[6], m[8],m[9],m[10]),
    ];

    let det = m[0] * cofactors[0] + m[4] * cofactors[1] + m[8] * cofactors[2] + m[12] * cofactors[3];
    if det.abs() < 1e-8 {
        return [[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]];
    }
    let inv_det = 1.0 / det;

    // The adjugate is the transpose of the cofactor matrix
    [
        [cofactors[0]*inv_det, cofactors[1]*inv_det, cofactors[2]*inv_det, cofactors[3]*inv_det],
        [cofactors[4]*inv_det, cofactors[5]*inv_det, cofactors[6]*inv_det, cofactors[7]*inv_det],
        [cofactors[8]*inv_det, cofactors[9]*inv_det, cofactors[10]*inv_det, cofactors[11]*inv_det],
        [cofactors[12]*inv_det, cofactors[13]*inv_det, cofactors[14]*inv_det, cofactors[15]*inv_det],
    ]
}

fn det3(a0:f32, a1:f32, a2:f32, b0:f32, b1:f32, b2:f32, c0:f32, c1:f32, c2:f32) -> f32 {
    a0*(b1*c2 - b2*c1) - a1*(b0*c2 - b2*c0) + a2*(b0*c1 - b1*c0)
}

// ============================================================================
// Name Helpers
// ============================================================================

/// Strip FBX name prefixes and normalize bone names.
pub fn clean_fbx_name(s: &str) -> String {
    let s = s.split('\0').next().unwrap_or(s);
    let s = s.trim_start_matches("Model::")
        .trim_start_matches("Geometry::")
        .trim_start_matches("Material::")
        .trim_start_matches("AnimationStack::")
        .trim_start_matches("AnimationLayer::")
        .trim_start_matches("AnimationCurveNode::");
    normalize_bone_name(s)
}

/// Strip mixamorig: prefix and clean bone names for .anim files.
pub fn normalize_bone_name(name: &str) -> String {
    name.trim_start_matches("mixamorig:")
        .trim_start_matches("mixamorig_")
        .to_string()
}

/// Clean animation stack names: "Idle|Idle" → "Idle"
pub fn clean_anim_name(raw: &str) -> String {
    let s = raw.split('\0').next().unwrap_or(raw);
    let s = s.trim_start_matches("AnimationStack::");
    // Take the part after the last '|' if present
    s.rsplit('|').next().unwrap_or(s).to_string()
}

// ============================================================================
// Attribute helpers
// ============================================================================

pub fn get_string(attr: &AttributeValue) -> Option<String> {
    match attr {
        AttributeValue::String(s) => Some(s.to_string()),
        _ => None,
    }
}

pub fn get_i64(attr: &AttributeValue) -> Option<i64> {
    match attr {
        AttributeValue::I64(v) => Some(*v),
        AttributeValue::I32(v) => Some(*v as i64),
        AttributeValue::I16(v) => Some(*v as i64),
        AttributeValue::F64(v) => Some(*v as i64),
        AttributeValue::F32(v) => Some(*v as i64),
        _ => None,
    }
}

pub fn get_f64(attr: &AttributeValue) -> Option<f64> {
    match attr {
        AttributeValue::F64(v) => Some(*v),
        AttributeValue::F32(v) => Some(*v as f64),
        AttributeValue::I64(v) => Some(*v as f64),
        AttributeValue::I32(v) => Some(*v as f64),
        AttributeValue::I16(v) => Some(*v as f64),
        _ => None,
    }
}

pub fn get_f64_array(attr: &AttributeValue) -> Option<Vec<f64>> {
    match attr {
        AttributeValue::ArrF64(arr) => Some(arr.iter().copied().collect()),
        AttributeValue::ArrF32(arr) => Some(arr.iter().map(|&v| v as f64).collect()),
        _ => None,
    }
}

pub fn get_i32_array(attr: &AttributeValue) -> Option<Vec<i32>> {
    match attr {
        AttributeValue::ArrI32(arr) => Some(arr.iter().copied().collect()),
        AttributeValue::ArrI64(arr) => Some(arr.iter().map(|&v| v as i32).collect()),
        _ => None,
    }
}

pub fn get_i64_array(attr: &AttributeValue) -> Option<Vec<i64>> {
    match attr {
        AttributeValue::ArrI64(arr) => Some(arr.iter().copied().collect()),
        AttributeValue::ArrI32(arr) => Some(arr.iter().map(|&v| v as i64).collect()),
        _ => None,
    }
}

pub fn get_f32_array(attr: &AttributeValue) -> Option<Vec<f32>> {
    match attr {
        AttributeValue::ArrF32(arr) => Some(arr.iter().copied().collect()),
        AttributeValue::ArrF64(arr) => Some(arr.iter().map(|&v| v as f32).collect()),
        _ => None,
    }
}

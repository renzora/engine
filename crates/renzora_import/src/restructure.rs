//! Reshape a converted GLB's scene graph without touching its geometry.
//!
//! Importers disagree about what a "model" is. A DCC scene is a tree of named
//! objects; a renderer wants as few draw calls as it can get; an editor wants
//! things it can click. No single answer is right for all three, so this is an
//! import setting ([`crate::settings::SceneStructure`]) rather than a decision
//! baked into the converters.
//!
//! Everything here is a **JSON-only** transform. Splitting a mesh into one mesh
//! per primitive re-points existing accessors; flattening writes each node's
//! accumulated world transform into its own matrix. No vertex data is read,
//! rewritten or copied, so the cost is proportional to the node count rather
//! than the triangle count — reshaping a two-million-triangle city is
//! milliseconds.

use serde_json::{json, Map, Value};

/// A 4x4 matrix in glTF's column-major order.
type Mat4 = [f64; 16];

const IDENTITY: Mat4 = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// `a * b`, column-major — the same convention glTF stores matrices in, so a
/// parent's matrix times a child's gives the child's world transform.
fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0f64; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = sum;
        }
    }
    out
}

/// A node's local transform, from either an explicit `matrix` or its TRS parts.
fn local_matrix(node: &Value) -> Mat4 {
    if let Some(m) = node.get("matrix").and_then(|v| v.as_array()) {
        if m.len() == 16 {
            let mut out = IDENTITY;
            for (i, v) in m.iter().enumerate() {
                out[i] = v.as_f64().unwrap_or(0.0);
            }
            return out;
        }
    }
    let t = node
        .get("translation")
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0),
            ]
        })
        .unwrap_or([0.0; 3]);
    let r = node
        .get("rotation")
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0),
            ]
        })
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let s = node
        .get("scale")
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(1.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0),
                a.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0),
            ]
        })
        .unwrap_or([1.0; 3]);

    let (x, y, z, w) = (r[0], r[1], r[2], r[3]);
    // Quaternion → rotation matrix, then scale each basis column.
    let rot = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        0.0,
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        0.0,
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ];
    let mut out = rot;
    for col in 0..3 {
        for row in 0..3 {
            out[col * 4 + row] *= s[col];
        }
    }
    out
}

/// One mesh instance found while walking the graph.
struct Instance {
    mesh: usize,
    world: Mat4,
    name: String,
}

/// Rebuild the document so every mesh primitive is its own root-level node.
///
/// Group nodes disappear and their transforms are folded into the nodes that
/// actually draw something, so the result is a flat list of clickable,
/// independently cullable objects — which a merged import otherwise cannot
/// offer at all.
///
/// Refuses (returning the input unchanged) when the document has skins or
/// animations: both address nodes by index, and re-indexing the graph under
/// them would silently break a rig. That refusal is reported to the caller so
/// it can surface a warning rather than quietly doing nothing.
pub fn flatten_per_mesh(glb_bytes: &[u8]) -> Result<(Vec<u8>, Option<String>), String> {
    let (json_bytes, bin) = crate::glb_compat::split_glb(glb_bytes)
        .map_err(|_| "not a GLB".to_string())?;
    let mut root: Value =
        serde_json::from_slice(json_bytes).map_err(|e| format!("parse GLB JSON: {e}"))?;

    if root.get("skins").and_then(|v| v.as_array()).is_some_and(|a| !a.is_empty()) {
        return Ok((
            glb_bytes.to_vec(),
            Some("hierarchy left as-is: flattening a skinned model would break its skeleton".into()),
        ));
    }
    if root
        .get("animations")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty())
    {
        return Ok((
            glb_bytes.to_vec(),
            Some(
                "hierarchy left as-is: flattening would break animation channels, which target \
                 nodes by index"
                    .into(),
            ),
        ));
    }

    let nodes = root
        .get("nodes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if nodes.is_empty() {
        return Ok((glb_bytes.to_vec(), None));
    }
    let meshes = root
        .get("meshes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let material_names: Vec<String> = root
        .get("materials")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .enumerate()
                .map(|(i, m)| {
                    m.get("name")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("Material {i}"))
                })
                .collect()
        })
        .unwrap_or_default();

    // Roots of the default scene, falling back to nodes nothing claims.
    let scene_idx = root.get("scene").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let mut roots: Vec<usize> = root
        .get("scenes")
        .and_then(|v| v.as_array())
        .and_then(|a| a.get(scene_idx))
        .and_then(|s| s.get("nodes"))
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as usize).collect())
        .unwrap_or_default();
    if roots.is_empty() {
        let claimed: std::collections::HashSet<usize> = nodes
            .iter()
            .filter_map(|n| n.get("children").and_then(|v| v.as_array()))
            .flat_map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as usize))
            .collect();
        roots = (0..nodes.len()).filter(|i| !claimed.contains(i)).collect();
    }

    // Walk, accumulating world transforms. Iterative so a deep chain cannot
    // blow the stack on a pathological export.
    let mut instances: Vec<Instance> = Vec::new();
    let mut stack: Vec<(usize, Mat4)> = roots.iter().map(|&r| (r, IDENTITY)).collect();
    let mut guard = 0usize;
    while let Some((idx, parent)) = stack.pop() {
        guard += 1;
        if guard > 1_000_000 {
            return Err("node graph is cyclic or absurdly large".into());
        }
        let Some(node) = nodes.get(idx) else { continue };
        let world = mul(&parent, &local_matrix(node));
        if let Some(mesh) = node.get("mesh").and_then(|v| v.as_u64()) {
            instances.push(Instance {
                mesh: mesh as usize,
                world,
                name: node
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("Node {idx}")),
            });
        }
        if let Some(kids) = node.get("children").and_then(|v| v.as_array()) {
            for k in kids.iter().filter_map(|v| v.as_u64()) {
                stack.push((k as usize, world));
            }
        }
    }
    if instances.is_empty() {
        return Ok((glb_bytes.to_vec(), None));
    }

    // One mesh per primitive, one node per mesh. Splitting only re-points
    // existing primitive objects, so no accessor or buffer is touched.
    let mut new_meshes: Vec<Value> = Vec::new();
    let mut new_nodes: Vec<Value> = Vec::new();
    for inst in &instances {
        let Some(mesh) = meshes.get(inst.mesh) else { continue };
        let prims = mesh
            .get("primitives")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for prim in prims {
            // Name after the material where there is one: on a merged import
            // that is the only thing distinguishing one chunk from the next,
            // and "Node 0" repeated 132 times is useless in an outliner.
            let label = prim
                .get("material")
                .and_then(|v| v.as_u64())
                .and_then(|m| material_names.get(m as usize))
                .cloned()
                .unwrap_or_else(|| inst.name.clone());
            let mesh_index = new_meshes.len();
            new_meshes.push(json!({
                "name": label,
                "primitives": [prim],
            }));
            let mut node = Map::new();
            node.insert("name".into(), json!(label));
            node.insert("mesh".into(), json!(mesh_index));
            if inst.world != IDENTITY {
                node.insert("matrix".into(), json!(inst.world.to_vec()));
            }
            new_nodes.push(Value::Object(node));
        }
    }

    let node_indices: Vec<usize> = (0..new_nodes.len()).collect();
    root["meshes"] = Value::Array(new_meshes);
    root["nodes"] = Value::Array(new_nodes);
    root["scenes"] = json!([{ "nodes": node_indices }]);
    root["scene"] = json!(0);

    let out = serde_json::to_vec(&root).map_err(|e| format!("serialize GLB JSON: {e}"))?;
    Ok((crate::glb_compat::repack_glb(&out, bin), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glb(json: &str) -> Vec<u8> {
        let mut j = json.as_bytes().to_vec();
        while j.len() % 4 != 0 {
            j.push(b' ');
        }
        let mut out = Vec::new();
        out.extend_from_slice(&0x4654_6C67u32.to_le_bytes());
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&((12 + 8 + j.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        out.extend_from_slice(&j);
        out
    }

    fn parse(bytes: &[u8]) -> Value {
        let (j, _) = crate::glb_compat::split_glb(bytes).unwrap();
        serde_json::from_slice(j).unwrap()
    }

    #[test]
    fn splits_a_merged_mesh_into_one_node_per_primitive() {
        // The transcoded-FBX shape: one node, one mesh, a primitive per material.
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "scene":0,"scenes":[{"nodes":[0]}],
                "nodes":[{"mesh":0}],
                "meshes":[{"primitives":[
                    {"attributes":{"POSITION":0},"material":0},
                    {"attributes":{"POSITION":0},"material":1},
                    {"attributes":{"POSITION":0},"material":2}
                ]}],
                "materials":[{"name":"Brick"},{"name":"Glass"},{"name":"Metal"}]}"#,
        );
        let (out, warn) = flatten_per_mesh(&bytes).unwrap();
        assert!(warn.is_none());
        let j = parse(&out);
        assert_eq!(j["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(j["meshes"].as_array().unwrap().len(), 3);
        assert_eq!(j["scenes"][0]["nodes"].as_array().unwrap().len(), 3);
        // Named after their materials, so the outliner is readable.
        assert_eq!(j["nodes"][0]["name"], "Brick");
        assert_eq!(j["nodes"][1]["name"], "Glass");
        assert_eq!(j["nodes"][2]["name"], "Metal");
    }

    #[test]
    fn folds_parent_transforms_into_the_leaf() {
        // A group translated by 5 on X, holding a mesh translated by 2.
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "scene":0,"scenes":[{"nodes":[0]}],
                "nodes":[
                    {"children":[1],"translation":[5,0,0]},
                    {"mesh":0,"translation":[2,0,0]}
                ],
                "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}]}"#,
        );
        let (out, _) = flatten_per_mesh(&bytes).unwrap();
        let j = parse(&out);
        assert_eq!(j["nodes"].as_array().unwrap().len(), 1, "group node is gone");
        let m = j["nodes"][0]["matrix"].as_array().unwrap();
        // Column-major: translation is elements 12..15.
        assert!((m[12].as_f64().unwrap() - 7.0).abs() < 1e-9, "5 + 2 = 7");
    }

    #[test]
    fn drops_pass_through_group_nodes() {
        // The glTF-exporter shape: wrappers holding a single child.
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "scene":0,"scenes":[{"nodes":[0]}],
                "nodes":[
                    {"name":"wrapper_a","children":[1]},
                    {"name":"wrapper_b","children":[2]},
                    {"name":"leaf","mesh":0}
                ],
                "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}]}"#,
        );
        let (out, _) = flatten_per_mesh(&bytes).unwrap();
        let j = parse(&out);
        assert_eq!(j["nodes"].as_array().unwrap().len(), 1);
        assert_eq!(j["nodes"][0]["name"], "leaf");
    }

    #[test]
    fn refuses_to_flatten_a_skinned_model() {
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "nodes":[{"mesh":0}],
                "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],
                "skins":[{"joints":[0]}]}"#,
        );
        let (out, warn) = flatten_per_mesh(&bytes).unwrap();
        assert_eq!(out, bytes, "document is returned untouched");
        assert!(warn.unwrap().contains("skeleton"));
    }

    #[test]
    fn refuses_to_flatten_an_animated_model() {
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "nodes":[{"mesh":0}],
                "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],
                "animations":[{"channels":[],"samplers":[]}]}"#,
        );
        let (out, warn) = flatten_per_mesh(&bytes).unwrap();
        assert_eq!(out, bytes);
        assert!(warn.unwrap().contains("animation"));
    }

    #[test]
    fn an_empty_document_is_left_alone() {
        let bytes = glb(r#"{"asset":{"version":"2.0"}}"#);
        let (out, warn) = flatten_per_mesh(&bytes).unwrap();
        assert_eq!(out, bytes);
        assert!(warn.is_none());
    }

    #[test]
    fn matrix_multiply_matches_hand_computation() {
        // Translate by (1,2,3) then scale by 2 — column-major.
        let t = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0,
        ];
        let s = [
            2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let m = mul(&t, &s);
        // Scale applied first, then translation: basis scaled, offset intact.
        assert_eq!(m[0], 2.0);
        assert_eq!(m[12], 1.0);
        assert_eq!(m[13], 2.0);
        assert_eq!(m[14], 3.0);
    }
}

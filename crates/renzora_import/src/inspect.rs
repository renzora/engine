//! Describe a converted GLB without loading it into the engine.
//!
//! The import overlay needs to tell the user what an import actually produced —
//! how many nodes survived, whether the meshes carry UVs, which materials came
//! out masked or double-sided — *before* anything is written into the project.
//! All of that is already sitting in the GLB's JSON, so this reads it there
//! rather than round-tripping through Bevy's asset pipeline.
//!
//! It lives in `renzora_import` because this is the crate that already parses
//! glTF; the UI crate would otherwise need its own `gltf` + `serde_json`
//! dependency to ask a question about the importer's own output.

use serde_json::Value;

/// One node of the converted scene graph, flattened into an index-addressed
/// list. `children` holds indices into the same list, which is how glTF itself
/// stores it and what a tree view wants.
#[derive(Debug, Clone, Default)]
pub struct GlbNode {
    pub name: String,
    pub children: Vec<usize>,
    /// Index into [`GlbStats::mesh_list`], when this node draws something.
    pub mesh: Option<usize>,
    pub has_transform: bool,
}

/// One primitive of a mesh — a contiguous run of triangles sharing a material.
#[derive(Debug, Clone, Default)]
pub struct GlbPrimitive {
    /// Index into the material list.
    pub material: Option<usize>,
    pub vertices: usize,
    pub triangles: usize,
    pub attributes: Vec<String>,
}

/// A mesh and the primitives it is split into.
#[derive(Debug, Clone, Default)]
pub struct GlbMesh {
    pub name: String,
    pub primitives: Vec<GlbPrimitive>,
}

impl GlbMesh {
    pub fn vertices(&self) -> usize {
        self.primitives.iter().map(|p| p.vertices).sum()
    }
    pub fn triangles(&self) -> usize {
        self.primitives.iter().map(|p| p.triangles).sum()
    }
}

/// A structural summary of a GLB, read straight from its JSON chunk.
#[derive(Debug, Clone, Default)]
pub struct GlbStats {
    pub nodes: usize,
    pub meshes: usize,
    /// Total primitives across every mesh. A mesh with one primitive per
    /// material reads as `meshes: 1, primitives: N`, which is exactly the
    /// shape the converters produce and worth showing distinctly.
    pub primitives: usize,
    pub materials: usize,
    pub textures: usize,
    pub images: usize,
    pub animations: usize,
    pub skins: usize,
    pub cameras: usize,
    /// Summed `POSITION` accessor counts.
    pub vertices: usize,
    /// Summed index counts / 3, falling back to `POSITION / 3` for
    /// non-indexed primitives.
    pub triangles: usize,
    /// Every distinct vertex attribute name present, sorted. The absence of
    /// `TEXCOORD_0` here is the difference between a textured model and a
    /// flat-shaded one, so it is worth surfacing rather than assuming.
    pub attributes: Vec<String>,
    pub extensions_used: Vec<String>,
    pub extensions_required: Vec<String>,
    /// Size of the binary chunk in bytes.
    pub bin_bytes: usize,
    /// Per-primitive attribute coverage: how many primitives carry each
    /// attribute. A partial count (say `TEXCOORD_0` on 90 of 132) is a much
    /// more useful signal than a bare "present".
    pub attribute_coverage: Vec<(String, usize)>,
    /// The scene graph, for a tree view. Index-addressed, matching glTF.
    pub node_list: Vec<GlbNode>,
    /// Indices into [`Self::node_list`] that the default scene starts from.
    pub roots: Vec<usize>,
    /// Every mesh with its primitive breakdown.
    pub mesh_list: Vec<GlbMesh>,
    /// Material names, positionally matching glTF material indices so a
    /// primitive's `material` can be resolved to something readable.
    pub material_names: Vec<String>,
}

impl GlbStats {
    /// True when at least one primitive is missing UVs. Materials that sample
    /// a texture render flat on those primitives, so it is worth flagging.
    pub fn has_uv_gap(&self) -> bool {
        let uv = self
            .attribute_coverage
            .iter()
            .find(|(name, _)| name == "TEXCOORD_0")
            .map(|(_, n)| *n)
            .unwrap_or(0);
        uv < self.primitives
    }
}

/// Read a GLB's JSON chunk and summarize its structure.
///
/// Returns `None` when the bytes are not a parseable GLB — callers treat that
/// as "no summary available" rather than an import failure, since the caller
/// already has the conversion's own result to report.
pub fn inspect_glb(glb_bytes: &[u8]) -> Option<GlbStats> {
    let glb = gltf::Glb::from_slice(glb_bytes).ok()?;
    let root: Value = serde_json::from_slice(&glb.json).ok()?;

    let arr = |key: &str| -> &[Value] {
        root.get(key)
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[])
    };
    let len = |key: &str| arr(key).len();

    let accessors = arr("accessors");
    let accessor_count = |idx: Option<&Value>| -> usize {
        idx.and_then(|v| v.as_u64())
            .and_then(|i| accessors.get(i as usize))
            .and_then(|a| a.get("count"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as usize
    };

    let mut stats = GlbStats {
        nodes: len("nodes"),
        meshes: len("meshes"),
        materials: len("materials"),
        textures: len("textures"),
        images: len("images"),
        animations: len("animations"),
        skins: len("skins"),
        cameras: len("cameras"),
        bin_bytes: glb.bin.as_ref().map_or(0, |b| b.len()),
        ..Default::default()
    };

    let mut coverage: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for (mi, mesh) in arr("meshes").iter().enumerate() {
        let prims = mesh
            .get("primitives")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out_mesh = GlbMesh {
            // An unnamed mesh still needs something to show in a list, and its
            // index is what every other glTF tool falls back to.
            name: mesh
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("Mesh {mi}")),
            primitives: Vec::with_capacity(prims.len()),
        };
        for prim in &prims {
            stats.primitives += 1;
            let attrs = prim.get("attributes").and_then(|v| v.as_object());
            let mut names: Vec<String> = Vec::new();
            if let Some(attrs) = attrs {
                for name in attrs.keys() {
                    *coverage.entry(name.clone()).or_default() += 1;
                    names.push(name.clone());
                }
                names.sort();
            }
            let verts = accessor_count(attrs.and_then(|a| a.get("POSITION")));
            let indices = accessor_count(prim.get("indices"));
            let tris = if indices > 0 { indices / 3 } else { verts / 3 };
            stats.vertices += verts;
            stats.triangles += tris;
            out_mesh.primitives.push(GlbPrimitive {
                material: prim
                    .get("material")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize),
                vertices: verts,
                triangles: tris,
                attributes: names,
            });
        }
        stats.mesh_list.push(out_mesh);
    }
    stats.attributes = coverage.keys().cloned().collect();
    stats.attribute_coverage = coverage.into_iter().collect();

    // -- Scene graph ------------------------------------------------------
    stats.material_names = arr("materials")
        .iter()
        .enumerate()
        .map(|(i, m)| {
            m.get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("Material {i}"))
        })
        .collect();

    let index_list = |v: Option<&Value>| -> Vec<usize> {
        v.and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|i| i.as_u64())
                    .map(|i| i as usize)
                    .collect()
            })
            .unwrap_or_default()
    };

    stats.node_list = arr("nodes")
        .iter()
        .enumerate()
        .map(|(i, n)| GlbNode {
            name: n
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("Node {i}")),
            children: index_list(n.get("children")),
            mesh: n.get("mesh").and_then(|v| v.as_u64()).map(|v| v as usize),
            has_transform: n.get("matrix").is_some()
                || n.get("translation").is_some()
                || n.get("rotation").is_some()
                || n.get("scale").is_some(),
        })
        .collect();

    // Roots come from the default scene when there is one. Falling back to
    // every node that nothing else claims as a child keeps a scene-less or
    // malformed document from rendering as an empty tree.
    let scene_idx = root.get("scene").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    stats.roots = arr("scenes")
        .get(scene_idx)
        .map(|sc| index_list(sc.get("nodes")))
        .unwrap_or_default();
    if stats.roots.is_empty() && !stats.node_list.is_empty() {
        let claimed: std::collections::HashSet<usize> = stats
            .node_list
            .iter()
            .flat_map(|n| n.children.iter().copied())
            .collect();
        stats.roots = (0..stats.node_list.len())
            .filter(|i| !claimed.contains(i))
            .collect();
    }

    let str_list = |key: &str| -> Vec<String> {
        root.get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    stats.extensions_used = str_list("extensionsUsed");
    stats.extensions_required = str_list("extensionsRequired");

    Some(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal GLB with the given JSON and an empty BIN chunk.
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

    #[test]
    fn counts_primitives_and_attributes() {
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "accessors":[{"count":300},{"count":90}],
                "meshes":[{"primitives":[
                    {"attributes":{"POSITION":0,"TEXCOORD_0":0},"indices":1},
                    {"attributes":{"POSITION":0}}
                ]}],
                "nodes":[{}],"materials":[{},{}]}"#,
        );
        let s = inspect_glb(&bytes).expect("parses");
        assert_eq!(s.primitives, 2);
        assert_eq!(s.meshes, 1);
        assert_eq!(s.nodes, 1);
        assert_eq!(s.materials, 2);
        assert_eq!(s.vertices, 600);
        // 90 indices / 3 for the first, 300 positions / 3 for the non-indexed second.
        assert_eq!(s.triangles, 30 + 100);
        assert_eq!(s.attributes, vec!["POSITION", "TEXCOORD_0"]);
    }

    #[test]
    fn uv_gap_detected_when_some_primitives_lack_uvs() {
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "meshes":[{"primitives":[
                    {"attributes":{"POSITION":0,"TEXCOORD_0":0}},
                    {"attributes":{"POSITION":0}}
                ]}]}"#,
        );
        let s = inspect_glb(&bytes).expect("parses");
        assert!(s.has_uv_gap(), "one of two primitives has no UVs");
    }

    #[test]
    fn no_uv_gap_when_all_primitives_have_uvs() {
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "meshes":[{"primitives":[
                    {"attributes":{"POSITION":0,"TEXCOORD_0":0}}
                ]}]}"#,
        );
        let s = inspect_glb(&bytes).expect("parses");
        assert!(!s.has_uv_gap());
    }

    #[test]
    fn non_glb_returns_none() {
        assert!(inspect_glb(b"not a glb at all").is_none());
    }

    #[test]
    fn empty_document_counts_zero() {
        let bytes = glb(r#"{"asset":{"version":"2.0"}}"#);
        let s = inspect_glb(&bytes).expect("parses");
        assert_eq!(s.primitives, 0);
        assert_eq!(s.triangles, 0);
        assert!(s.attributes.is_empty());
    }

    #[test]
    fn builds_the_scene_graph_from_the_default_scene() {
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "scene":0,
                "scenes":[{"nodes":[0]}],
                "nodes":[
                    {"name":"Root","children":[1,2]},
                    {"name":"Body","mesh":0},
                    {"name":"Wheel","translation":[1,0,0]}
                ],
                "meshes":[{"name":"BodyMesh","primitives":[
                    {"attributes":{"POSITION":0},"material":0}
                ]}],
                "materials":[{"name":"Paint"}],
                "accessors":[{"count":30}]}"#,
        );
        let s = inspect_glb(&bytes).expect("parses");
        assert_eq!(s.roots, vec![0]);
        assert_eq!(s.node_list.len(), 3);
        assert_eq!(s.node_list[0].name, "Root");
        assert_eq!(s.node_list[0].children, vec![1, 2]);
        assert_eq!(s.node_list[1].mesh, Some(0));
        assert!(s.node_list[2].has_transform, "a translation is a transform");
        assert!(!s.node_list[0].has_transform);
        assert_eq!(s.mesh_list[0].name, "BodyMesh");
        assert_eq!(s.mesh_list[0].primitives[0].material, Some(0));
        assert_eq!(s.material_names, vec!["Paint"]);
    }

    #[test]
    fn unnamed_items_fall_back_to_indices() {
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "nodes":[{"mesh":0}],
                "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],
                "materials":[{}],
                "accessors":[{"count":3}]}"#,
        );
        let s = inspect_glb(&bytes).expect("parses");
        assert_eq!(s.node_list[0].name, "Node 0");
        assert_eq!(s.mesh_list[0].name, "Mesh 0");
        assert_eq!(s.material_names, vec!["Material 0"]);
    }

    #[test]
    fn roots_fall_back_to_unclaimed_nodes_without_a_scene() {
        let bytes = glb(r#"{"asset":{"version":"2.0"},"nodes":[{"children":[1]},{},{}]}"#);
        let s = inspect_glb(&bytes).expect("parses");
        // Node 1 is claimed as a child; 0 and 2 are not.
        assert_eq!(s.roots, vec![0, 2]);
    }

    #[test]
    fn mesh_totals_sum_their_primitives() {
        let bytes = glb(
            r#"{"asset":{"version":"2.0"},
                "meshes":[{"primitives":[
                    {"attributes":{"POSITION":0},"indices":1},
                    {"attributes":{"POSITION":0},"indices":1}
                ]}],
                "accessors":[{"count":90},{"count":30}]}"#,
        );
        let s = inspect_glb(&bytes).expect("parses");
        assert_eq!(s.mesh_list[0].vertices(), 180);
        assert_eq!(s.mesh_list[0].triangles(), 20);
    }
}

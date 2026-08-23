//! Drop the parts of a converted GLB the user unchecked in the import inspector.
//!
//! The inspector's scene tree is built from [`crate::inspect::GlbStats`], which
//! is index-addressed exactly like the glTF document it was read from. So a
//! deselected row is a node / mesh / primitive *index*, and honouring it is a
//! JSON-only edit of the staged GLB: unlink the node, drop the primitive, then
//! reindex every reference to what moved.
//!
//! Removal cascades the way a user expects "don't import this" to cascade:
//!
//! * A node takes its whole subtree with it.
//! * A mesh nothing points at any more is dropped, not left orphaned.
//! * A material no surviving primitive uses is dropped, and its name is
//!   reported so the caller can withhold the matching `.material` file.
//! * A texture/image no surviving material samples is dropped, and its URI is
//!   reported so the caller can delete the file from the staged tree.
//!
//! What this pass deliberately does **not** do is touch the binary chunk. Once
//! the accessors of a removed primitive have no primitive referencing them,
//! [`crate::compact::compact_glb`] already knows how to collect them and rebuild
//! the buffer — so the caller runs that afterwards and this stays a pure
//! structural edit that is easy to reason about.
//!
//! ## Skinning
//!
//! A skeleton is not a containment hierarchy: a skin's joints are ordinary
//! nodes that usually sit *beside* the mesh they deform, and a `joints` array
//! is a list of node indices that must all still resolve. Removing a subtree
//! that happens to contain a joint of a surviving skin would leave a dangling
//! index and an unloadable file.
//!
//! So joints of a surviving skin are **rescued**: the joint and the ancestor
//! chain that reaches it stay in the document, but any mesh they carried is
//! stripped. The user gets the geometry removal they asked for, and the
//! skeleton the remaining skinned meshes still need. A skin nothing references
//! after the edit is dropped outright, which is what frees its joints to go.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// Extensions that address geometry or index spaces this pass doesn't model.
/// Encountering one means an edit could silently corrupt data we don't
/// understand, so the GLB comes back untouched — the same bail-out
/// [`crate::compact`] takes, for the same reason.
const BAIL_EXTENSIONS: &[&str] = &[
    "EXT_meshopt_compression",
    "KHR_draco_mesh_compression",
    "EXT_mesh_gpu_instancing",
    // Variants remap `primitives[].material` through a side table this pass
    // would not reindex.
    "KHR_materials_variants",
];

/// What the user deselected, in glTF index space.
///
/// Empty means "import everything", which is the overwhelmingly common case and
/// is checked before any work happens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PruneSpec {
    /// Nodes to remove, each taking its subtree with it.
    pub nodes: HashSet<usize>,
    /// Meshes to detach everywhere they are used.
    pub meshes: HashSet<usize>,
    /// Individual `(mesh, primitive)` surfaces to drop.
    pub prims: HashSet<(usize, usize)>,
}

impl PruneSpec {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.meshes.is_empty() && self.prims.is_empty()
    }
}

/// The edited GLB plus what fell out of it, so the caller can clean up the
/// sibling files that were written next to it.
#[derive(Debug, Clone)]
pub struct Pruned {
    pub glb: Vec<u8>,
    /// Names of materials no surviving primitive uses.
    pub dropped_materials: Vec<String>,
    /// `uri` values of images no surviving material samples. Relative to the
    /// GLB, e.g. `textures/bark_basecolor.rmip`.
    pub dropped_texture_uris: Vec<String>,
}

/// Remove everything `spec` names from `glb_bytes`.
///
/// Returns the GLB unchanged (and nothing dropped) when the spec is empty, when
/// the document uses an extension from [`BAIL_EXTENSIONS`], or when there is no
/// node array to edit.
pub fn prune_glb(glb_bytes: &[u8], spec: &PruneSpec) -> Result<Pruned, String> {
    let unchanged = || Pruned {
        glb: glb_bytes.to_vec(),
        dropped_materials: Vec::new(),
        dropped_texture_uris: Vec::new(),
    };
    if spec.is_empty() {
        return Ok(unchanged());
    }

    let glb = gltf::Glb::from_slice(glb_bytes).map_err(|e| format!("GLB parse: {e}"))?;
    let mut json: Value =
        serde_json::from_slice(&glb.json).map_err(|e| format!("GLB JSON parse: {e}"))?;

    if let Some(used) = json.get("extensionsUsed").and_then(|v| v.as_array()) {
        if used
            .iter()
            .filter_map(|v| v.as_str())
            .any(|e| BAIL_EXTENSIONS.contains(&e))
        {
            return Ok(unchanged());
        }
    }

    let nodes = array_of(&json, "nodes");
    if nodes.is_empty() {
        return Ok(unchanged());
    }
    let meshes = array_of(&json, "meshes");
    let materials = array_of(&json, "materials");
    let textures = array_of(&json, "textures");
    let images = array_of(&json, "images");
    let skins = array_of(&json, "skins");

    // ── 1. Which nodes go? ───────────────────────────────────────────────
    let mut remove: HashSet<usize> = HashSet::new();
    for &n in &spec.nodes {
        collect_subtree(&nodes, n, &mut remove);
    }

    // ── 2. Which skins survive, and which joints must be rescued? ────────
    // A skin is live if a node that is staying still points at it. Its joints
    // then have to stay too, even if the user unchecked the branch they sit in
    // — see the module docs.
    let live_skins: HashSet<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| !remove.contains(i))
        .filter_map(|(_, n)| index_at(n, "skin"))
        .collect();
    let parents = parent_map(&nodes);
    let mut rescued: HashSet<usize> = HashSet::new();
    for (si, skin) in skins.iter().enumerate() {
        if !live_skins.contains(&si) {
            continue;
        }
        for joint in skin_nodes(skin) {
            if remove.contains(&joint) {
                rescue_path(&parents, joint, &remove, &mut rescued);
            }
        }
    }
    for r in &rescued {
        remove.remove(r);
    }

    let keep_node = |i: usize| !remove.contains(&i);

    // ── 3. Which meshes stay attached? ───────────────────────────────────
    // A rescued node is only in the document to hold the skeleton together, so
    // it does not bring its geometry back with it.
    let mut mesh_attached: HashSet<usize> = HashSet::new();
    for (i, n) in nodes.iter().enumerate() {
        if !keep_node(i) || rescued.contains(&i) {
            continue;
        }
        if let Some(m) = index_at(n, "mesh") {
            if !spec.meshes.contains(&m) {
                mesh_attached.insert(m);
            }
        }
    }

    // ── 4. Rebuild the mesh list, dropping deselected surfaces. ──────────
    let mut mesh_remap: HashMap<usize, usize> = HashMap::new();
    let mut new_meshes: Vec<Value> = Vec::new();
    for (mi, mesh) in meshes.iter().enumerate() {
        if !mesh_attached.contains(&mi) {
            continue;
        }
        let prims: Vec<Value> = mesh
            .get("primitives")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .enumerate()
                    .filter(|(k, _)| !spec.prims.contains(&(mi, *k)))
                    .map(|(_, p)| p.clone())
                    .collect()
            })
            .unwrap_or_default();
        // Every surface deselected is the same as deselecting the mesh: glTF
        // requires `primitives` to be non-empty, so an emptied mesh cannot stay.
        if prims.is_empty() {
            continue;
        }
        let mut m = mesh.clone();
        m["primitives"] = Value::Array(prims);
        mesh_remap.insert(mi, new_meshes.len());
        new_meshes.push(m);
    }

    // ── 5. Materials still used by a surviving primitive. ────────────────
    let mut keep_mat: HashSet<usize> = HashSet::new();
    for mesh in &new_meshes {
        for prim in mesh.get("primitives").and_then(|v| v.as_array()).into_iter().flatten() {
            if let Some(m) = index_at(prim, "material") {
                keep_mat.insert(m);
            }
        }
    }
    let mut mat_remap: HashMap<usize, usize> = HashMap::new();
    let mut new_materials: Vec<Value> = Vec::new();
    let mut dropped_materials: Vec<String> = Vec::new();
    for (i, mat) in materials.iter().enumerate() {
        if keep_mat.contains(&i) {
            mat_remap.insert(i, new_materials.len());
            new_materials.push(mat.clone());
        } else if let Some(name) = mat.get("name").and_then(|v| v.as_str()) {
            dropped_materials.push(name.to_string());
        }
    }

    // ── 6. Textures + images the surviving materials still sample. ───────
    let mut keep_tex: HashSet<usize> = HashSet::new();
    for mat in &new_materials {
        collect_texture_refs(mat, &mut keep_tex);
    }
    let mut tex_remap: HashMap<usize, usize> = HashMap::new();
    let mut new_textures: Vec<Value> = Vec::new();
    for (i, tex) in textures.iter().enumerate() {
        if keep_tex.contains(&i) {
            tex_remap.insert(i, new_textures.len());
            new_textures.push(tex.clone());
        }
    }
    let mut keep_img: HashSet<usize> = HashSet::new();
    for tex in &new_textures {
        collect_image_refs(tex, &mut keep_img);
    }
    let mut img_remap: HashMap<usize, usize> = HashMap::new();
    let mut new_images: Vec<Value> = Vec::new();
    let mut dropped_texture_uris: Vec<String> = Vec::new();
    for (i, img) in images.iter().enumerate() {
        if keep_img.contains(&i) {
            img_remap.insert(i, new_images.len());
            new_images.push(img.clone());
        } else if let Some(uri) = img.get("uri").and_then(|v| v.as_str()) {
            dropped_texture_uris.push(uri.to_string());
        }
    }

    // ── 7. Skins: keep the live ones, in order. ──────────────────────────
    let mut skin_remap: HashMap<usize, usize> = HashMap::new();
    let mut new_skins: Vec<Value> = Vec::new();
    for (i, skin) in skins.iter().enumerate() {
        if live_skins.contains(&i) {
            skin_remap.insert(i, new_skins.len());
            new_skins.push(skin.clone());
        }
    }

    // ── 8. Rebuild the node list and reindex every reference. ────────────
    let mut node_remap: HashMap<usize, usize> = HashMap::new();
    let mut kept_originals: Vec<usize> = Vec::new();
    for i in 0..nodes.len() {
        if keep_node(i) {
            node_remap.insert(i, kept_originals.len());
            kept_originals.push(i);
        }
    }
    let mut new_nodes: Vec<Value> = Vec::with_capacity(kept_originals.len());
    for &orig in &kept_originals {
        let mut n = nodes[orig].clone();
        if let Some(children) = n.get("children").and_then(|v| v.as_array()).cloned() {
            let kids: Vec<Value> = children
                .iter()
                .filter_map(|c| c.as_u64().map(|v| v as usize))
                .filter_map(|c| node_remap.get(&c))
                .map(|&c| Value::from(c as u64))
                .collect();
            if kids.is_empty() {
                n.as_object_mut().map(|o| o.remove("children"));
            } else {
                n["children"] = Value::Array(kids);
            }
        }
        let mesh_kept = index_at(&n, "mesh")
            .filter(|_| !rescued.contains(&orig))
            .and_then(|m| mesh_remap.get(&m).copied());
        match mesh_kept {
            Some(new_mi) => n["mesh"] = Value::from(new_mi as u64),
            None => {
                if let Some(o) = n.as_object_mut() {
                    o.remove("mesh");
                    // A skin without a mesh to deform has nothing to do, and
                    // leaving one behind would keep its whole joint list (and
                    // their inverse-bind matrices) alive for nothing.
                    o.remove("skin");
                }
            }
        }
        if let Some(s) = index_at(&n, "skin") {
            match skin_remap.get(&s) {
                Some(&new_si) => n["skin"] = Value::from(new_si as u64),
                None => {
                    n.as_object_mut().map(|o| o.remove("skin"));
                }
            }
        }
        new_nodes.push(n);
    }

    // Primitive material indices.
    for mesh in &mut new_meshes {
        let Some(prims) = mesh.get_mut("primitives").and_then(|v| v.as_array_mut()) else {
            continue;
        };
        for prim in prims {
            if let Some(m) = index_at(prim, "material") {
                match mat_remap.get(&m) {
                    Some(&new_mi) => prim["material"] = Value::from(new_mi as u64),
                    None => {
                        prim.as_object_mut().map(|o| o.remove("material"));
                    }
                }
            }
        }
    }
    // Material → texture indices, wherever they are nested.
    for mat in &mut new_materials {
        remap_texture_refs(mat, &tex_remap);
    }
    // Texture → image indices.
    for tex in &mut new_textures {
        remap_image_refs(tex, &img_remap);
    }
    // Skin joints + skeleton roots.
    for skin in &mut new_skins {
        if let Some(joints) = skin.get("joints").and_then(|v| v.as_array()).cloned() {
            let mapped: Vec<Value> = joints
                .iter()
                .filter_map(|j| j.as_u64().map(|v| v as usize))
                .filter_map(|j| node_remap.get(&j))
                .map(|&j| Value::from(j as u64))
                .collect();
            skin["joints"] = Value::Array(mapped);
        }
        if let Some(root) = index_at(skin, "skeleton") {
            match node_remap.get(&root) {
                Some(&new_r) => skin["skeleton"] = Value::from(new_r as u64),
                None => {
                    skin.as_object_mut().map(|o| o.remove("skeleton"));
                }
            }
        }
    }

    // Scene roots.
    if let Some(scenes) = json.get_mut("scenes").and_then(|v| v.as_array_mut()) {
        for scene in scenes {
            if let Some(roots) = scene.get("nodes").and_then(|v| v.as_array()).cloned() {
                let mapped: Vec<Value> = roots
                    .iter()
                    .filter_map(|r| r.as_u64().map(|v| v as usize))
                    .filter_map(|r| node_remap.get(&r))
                    .map(|&r| Value::from(r as u64))
                    .collect();
                scene["nodes"] = Value::Array(mapped);
            }
        }
    }

    // Animation channels that targeted a node which is now gone.
    if let Some(anims) = json.get_mut("animations").and_then(|v| v.as_array_mut()) {
        for anim in anims.iter_mut() {
            if let Some(channels) = anim.get("channels").and_then(|v| v.as_array()).cloned() {
                let kept: Vec<Value> = channels
                    .into_iter()
                    .filter_map(|mut ch| {
                        let target = ch.get("target")?.get("node")?.as_u64()? as usize;
                        let &new_t = node_remap.get(&target)?;
                        ch["target"]["node"] = Value::from(new_t as u64);
                        Some(ch)
                    })
                    .collect();
                anim["channels"] = Value::Array(kept);
            }
        }
        anims.retain(|a| {
            a.get("channels")
                .and_then(|v| v.as_array())
                .is_some_and(|c| !c.is_empty())
        });
    }
    if json
        .get("animations")
        .and_then(|v| v.as_array())
        .is_some_and(|a| a.is_empty())
    {
        json.as_object_mut().map(|o| o.remove("animations"));
    }

    set_or_remove(&mut json, "nodes", new_nodes);
    set_or_remove(&mut json, "meshes", new_meshes);
    set_or_remove(&mut json, "materials", new_materials);
    set_or_remove(&mut json, "textures", new_textures);
    set_or_remove(&mut json, "images", new_images);
    set_or_remove(&mut json, "skins", new_skins);

    let out = serde_json::to_vec(&json).map_err(|e| format!("serialize GLB JSON: {e}"))?;
    Ok(Pruned {
        glb: crate::glb_compat::repack_glb(&out, glb.bin.as_deref()),
        dropped_materials,
        dropped_texture_uris,
    })
}

/// A GLB's JSON chunk as text.
///
/// Exists for the "is this file still referenced?" check a caller wants before
/// deleting a texture the prune says it dropped — a substring test against the
/// document is a cheap, format-agnostic second opinion on a structural result.
/// `None` when the bytes don't parse as a GLB or the chunk isn't UTF-8.
pub fn glb_json_text(glb: &[u8]) -> Option<String> {
    let parsed = gltf::Glb::from_slice(glb).ok()?;
    String::from_utf8(parsed.json.into_owned()).ok()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn array_of(json: &Value, key: &str) -> Vec<Value> {
    json.get(key)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Write `values` back under `key`, removing the key entirely when nothing
/// survived — glTF forbids an empty array for these properties.
fn set_or_remove(json: &mut Value, key: &str, values: Vec<Value>) {
    if values.is_empty() {
        json.as_object_mut().map(|o| o.remove(key));
    } else {
        json[key] = Value::Array(values);
    }
}

fn index_at(value: &Value, key: &str) -> Option<usize> {
    value.get(key).and_then(|v| v.as_u64()).map(|v| v as usize)
}

/// `node` and everything under it, guarded against a cycle in a malformed file.
fn collect_subtree(nodes: &[Value], node: usize, out: &mut HashSet<usize>) {
    if node >= nodes.len() || !out.insert(node) {
        return;
    }
    for child in nodes[node]
        .get("children")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_u64())
    {
        collect_subtree(nodes, child as usize, out);
    }
}

fn parent_map(nodes: &[Value]) -> HashMap<usize, usize> {
    let mut parents = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        for child in n
            .get("children")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|c| c.as_u64())
        {
            parents.insert(child as usize, i);
        }
    }
    parents
}

fn skin_nodes(skin: &Value) -> Vec<usize> {
    let mut out: Vec<usize> = skin
        .get("joints")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|j| j.as_u64())
        .map(|j| j as usize)
        .collect();
    if let Some(root) = index_at(skin, "skeleton") {
        out.push(root);
    }
    out
}

/// Walk up from `joint` marking every removed node on the way to the root as
/// rescued, so the joint is still reachable from the scene.
fn rescue_path(
    parents: &HashMap<usize, usize>,
    joint: usize,
    remove: &HashSet<usize>,
    rescued: &mut HashSet<usize>,
) {
    let mut at = joint;
    loop {
        if remove.contains(&at) && !rescued.insert(at) {
            return; // already walked this chain
        }
        match parents.get(&at) {
            Some(&p) => at = p,
            None => return,
        }
    }
}

/// Whether a JSON object has the shape of a glTF `textureInfo` — an `index`
/// into `textures`, optionally with a UV set.
///
/// Matched structurally rather than by key name because texture references live
/// in extensions too (`KHR_materials_specular.specularTexture`, and every other
/// PBR extension), and a name list would silently miss whichever one a source
/// file happens to use — the failure mode being a live texture deleted off disk.
fn is_texture_info(value: &Value) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    obj.get("index").and_then(|v| v.as_u64()).is_some()
        && obj
            .keys()
            .all(|k| matches!(k.as_str(), "index" | "texCoord" | "scale" | "strength" | "extensions" | "extras"))
}

fn collect_texture_refs(value: &Value, out: &mut HashSet<usize>) {
    if is_texture_info(value) {
        if let Some(i) = index_at(value, "index") {
            out.insert(i);
        }
    }
    match value {
        Value::Object(map) => {
            for (_, v) in map {
                collect_texture_refs(v, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                collect_texture_refs(v, out);
            }
        }
        _ => {}
    }
}

fn remap_texture_refs(value: &mut Value, remap: &HashMap<usize, usize>) {
    if is_texture_info(value) {
        if let Some(i) = index_at(value, "index") {
            if let Some(&new) = remap.get(&i) {
                value["index"] = Value::from(new as u64);
            }
        }
    }
    match value {
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                remap_texture_refs(v, remap);
            }
        }
        Value::Array(items) => {
            for v in items.iter_mut() {
                remap_texture_refs(v, remap);
            }
        }
        _ => {}
    }
}

/// A texture's image: the core `source`, plus the `source` of any texture
/// extension (`KHR_texture_basisu` and friends all spell it the same way).
fn collect_image_refs(texture: &Value, out: &mut HashSet<usize>) {
    if let Some(i) = index_at(texture, "source") {
        out.insert(i);
    }
    for (_, ext) in texture
        .get("extensions")
        .and_then(|v| v.as_object())
        .into_iter()
        .flatten()
    {
        if let Some(i) = index_at(ext, "source") {
            out.insert(i);
        }
    }
}

fn remap_image_refs(texture: &mut Value, remap: &HashMap<usize, usize>) {
    if let Some(i) = index_at(texture, "source") {
        if let Some(&new) = remap.get(&i) {
            texture["source"] = Value::from(new as u64);
        }
    }
    if let Some(exts) = texture.get_mut("extensions").and_then(|v| v.as_object_mut()) {
        for (_, ext) in exts.iter_mut() {
            if let Some(i) = index_at(ext, "source") {
                if let Some(&new) = remap.get(&i) {
                    ext["source"] = Value::from(new as u64);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(json: &str) -> Vec<u8> {
        let mut j = json.as_bytes().to_vec();
        while !j.len().is_multiple_of(4) {
            j.push(b' ');
        }
        // Big enough for the two VEC3/count-3 accessors the fixtures declare,
        // so `gltf`'s validator has real ranges to check them against.
        let bin = [0u8; 72];
        let total = 12 + 8 + j.len() + 8 + bin.len();
        let mut out = Vec::new();
        out.extend_from_slice(&0x46546C67u32.to_le_bytes());
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
        out.extend_from_slice(&j);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x004E4942u32.to_le_bytes());
        out.extend_from_slice(&bin);
        out
    }

    fn read(glb: &[u8]) -> Value {
        let parsed = gltf::Glb::from_slice(glb).expect("parse");
        serde_json::from_slice(&parsed.json).expect("json")
    }

    /// Two root nodes, one mesh each, one material + texture + image each.
    fn two_models() -> Vec<u8> {
        pack(
            r#"{"asset":{"version":"2.0"},
            "scene":0,
            "scenes":[{"nodes":[0,2]}],
            "nodes":[
              {"name":"Chair","children":[1]},
              {"name":"ChairMesh","mesh":0},
              {"name":"Table","mesh":1}],
            "meshes":[
              {"name":"chair","primitives":[{"attributes":{"POSITION":0},"material":0}]},
              {"name":"table","primitives":[{"attributes":{"POSITION":1},"material":1}]}],
            "materials":[
              {"name":"Wood","pbrMetallicRoughness":{"baseColorTexture":{"index":0}}},
              {"name":"Marble","normalTexture":{"index":1,"scale":1.0}}],
            "textures":[{"source":0},{"source":1}],
            "images":[{"uri":"textures/wood.rmip"},{"uri":"textures/marble.rmip"}],
            "buffers":[{"byteLength":72}],
            "bufferViews":[
              {"buffer":0,"byteOffset":0,"byteLength":36},
              {"buffer":0,"byteOffset":36,"byteLength":36}],
            "accessors":[
              {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,1]},
              {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,1]}]}"#,
        )
    }

    #[test]
    fn an_empty_spec_returns_the_original_bytes() {
        let glb = two_models();
        let out = prune_glb(&glb, &PruneSpec::default()).expect("prune");
        assert_eq!(out.glb, glb);
    }

    #[test]
    fn dropping_a_node_takes_its_subtree_mesh_material_and_texture() {
        let glb = two_models();
        let spec = PruneSpec {
            nodes: HashSet::from([0]),
            ..Default::default()
        };
        let out = prune_glb(&glb, &spec).expect("prune");
        let json = read(&out.glb);

        // Only the Table root and its mesh survive.
        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["name"], "Table");
        // Reindexed: the sole scene root is now node 0, pointing at mesh 0.
        assert_eq!(json["scenes"][0]["nodes"], serde_json::json!([0]));
        assert_eq!(nodes[0]["mesh"], 0);

        let meshes = json["meshes"].as_array().unwrap();
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0]["name"], "table");
        assert_eq!(meshes[0]["primitives"][0]["material"], 0);

        // The chair's material, texture and image went with it.
        assert_eq!(json["materials"].as_array().unwrap().len(), 1);
        assert_eq!(json["materials"][0]["name"], "Marble");
        assert_eq!(json["materials"][0]["normalTexture"]["index"], 0);
        assert_eq!(json["images"].as_array().unwrap().len(), 1);
        assert_eq!(out.dropped_materials, vec!["Wood".to_string()]);
        assert_eq!(
            out.dropped_texture_uris,
            vec!["textures/wood.rmip".to_string()]
        );

        gltf::Gltf::from_slice(&out.glb).expect("still a valid GLB");
    }

    #[test]
    fn dropping_a_mesh_leaves_its_node_in_place() {
        let glb = two_models();
        let spec = PruneSpec {
            meshes: HashSet::from([1]),
            ..Default::default()
        };
        let out = prune_glb(&glb, &spec).expect("prune");
        let json = read(&out.glb);
        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 3, "the hierarchy is untouched");
        assert!(
            nodes[2].get("mesh").is_none(),
            "the Table node keeps its place but loses its geometry"
        );
        assert_eq!(json["meshes"].as_array().unwrap().len(), 1);
        assert_eq!(out.dropped_materials, vec!["Marble".to_string()]);
    }

    #[test]
    fn dropping_every_primitive_drops_the_mesh() {
        let glb = two_models();
        let spec = PruneSpec {
            prims: HashSet::from([(0, 0)]),
            ..Default::default()
        };
        let out = prune_glb(&glb, &spec).expect("prune");
        let json = read(&out.glb);
        assert_eq!(json["meshes"].as_array().unwrap().len(), 1);
        assert_eq!(json["meshes"][0]["name"], "table");
        // The node stays; glTF has no empty mesh, so it simply has none.
        assert!(json["nodes"][1].get("mesh").is_none());
    }

    #[test]
    fn a_joint_of_a_surviving_skin_is_rescued_without_its_geometry() {
        // Node 0 (removed) is the parent of joint 1, which skin 0 — used by the
        // surviving node 2 — deforms. The joint chain must stay resolvable.
        let glb = pack(
            r#"{"asset":{"version":"2.0"},
            "scene":0,
            "scenes":[{"nodes":[0,2]}],
            "nodes":[
              {"name":"Rig","children":[1],"mesh":0},
              {"name":"Joint"},
              {"name":"Skinned","mesh":1,"skin":0}],
            "skins":[{"joints":[1],"skeleton":1}],
            "meshes":[
              {"name":"prop","primitives":[{"attributes":{"POSITION":0}}]},
              {"name":"body","primitives":[{"attributes":{"POSITION":1}}]}],
            "buffers":[{"byteLength":72}],
            "bufferViews":[
              {"buffer":0,"byteOffset":0,"byteLength":36},
              {"buffer":0,"byteOffset":36,"byteLength":36}],
            "accessors":[
              {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,1]},
              {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,1]}]}"#,
        );
        let spec = PruneSpec {
            nodes: HashSet::from([0]),
            ..Default::default()
        };
        let out = prune_glb(&glb, &spec).expect("prune");
        let json = read(&out.glb);

        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 3, "Rig is kept purely to reach the joint");
        assert!(
            nodes[0].get("mesh").is_none(),
            "but the geometry the user unchecked is gone"
        );
        assert_eq!(json["meshes"].as_array().unwrap().len(), 1);
        assert_eq!(json["meshes"][0]["name"], "body");
        // The skin still resolves.
        assert_eq!(json["skins"][0]["joints"], serde_json::json!([1]));
        gltf::Gltf::from_slice(&out.glb).expect("still a valid GLB");
    }

    #[test]
    fn an_unused_skin_is_dropped_with_its_node() {
        let glb = pack(
            r#"{"asset":{"version":"2.0"},
            "scenes":[{"nodes":[0,2]}],
            "nodes":[
              {"name":"Joint"},
              {"name":"Unused"},
              {"name":"Skinned","mesh":0,"skin":0}],
            "skins":[{"joints":[0]}],
            "meshes":[{"name":"body","primitives":[{"attributes":{"POSITION":0}}]}],
            "accessors":[{"componentType":5126,"count":3,"type":"VEC3"}]}"#,
        );
        let spec = PruneSpec {
            nodes: HashSet::from([2]),
            ..Default::default()
        };
        let out = prune_glb(&glb, &spec).expect("prune");
        let json = read(&out.glb);
        assert!(
            json.get("skins").is_none(),
            "nothing references the skin once the skinned node goes"
        );
        assert!(json.get("meshes").is_none());
    }

    #[test]
    fn bails_unchanged_on_an_unsupported_extension() {
        let glb = pack(
            r#"{"asset":{"version":"2.0"},
            "extensionsUsed":["KHR_draco_mesh_compression"],
            "scenes":[{"nodes":[0]}],
            "nodes":[{"name":"A","mesh":0}],
            "meshes":[{"name":"a","primitives":[{"attributes":{"POSITION":0}}]}],
            "accessors":[{"componentType":5126,"count":3,"type":"VEC3"}]}"#,
        );
        let spec = PruneSpec {
            nodes: HashSet::from([0]),
            ..Default::default()
        };
        let out = prune_glb(&glb, &spec).expect("prune");
        assert_eq!(out.glb, glb);
    }

    #[test]
    fn a_texture_shared_with_a_surviving_material_is_kept() {
        let glb = pack(
            r#"{"asset":{"version":"2.0"},
            "scenes":[{"nodes":[0,1]}],
            "nodes":[{"name":"A","mesh":0},{"name":"B","mesh":1}],
            "meshes":[
              {"name":"a","primitives":[{"attributes":{"POSITION":0},"material":0}]},
              {"name":"b","primitives":[{"attributes":{"POSITION":0},"material":1}]}],
            "materials":[
              {"name":"One","pbrMetallicRoughness":{"baseColorTexture":{"index":0}}},
              {"name":"Two","pbrMetallicRoughness":{"baseColorTexture":{"index":0}}}],
            "textures":[{"source":0}],
            "images":[{"uri":"textures/shared.rmip"}],
            "accessors":[{"componentType":5126,"count":3,"type":"VEC3"}]}"#,
        );
        let spec = PruneSpec {
            nodes: HashSet::from([0]),
            ..Default::default()
        };
        let out = prune_glb(&glb, &spec).expect("prune");
        assert_eq!(out.dropped_materials, vec!["One".to_string()]);
        assert!(
            out.dropped_texture_uris.is_empty(),
            "the surviving material still samples it"
        );
        let json = read(&out.glb);
        assert_eq!(json["images"].as_array().unwrap().len(), 1);
    }
}

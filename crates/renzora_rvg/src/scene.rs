//! The scene a model `.rvg` carries: its source GLB with each converted
//! primitive's triangles swapped for the proxy.
//!
//! A model stored as `.rvg` keeps everything its GLB had (nodes, names, extras,
//! materials, skins, morph targets, cameras, lights) in this scene, which is
//! loaded by Bevy's own glTF loader, so every system that spawns or inspects a
//! model works on it unchanged. What it does not keep is the full-detail
//! triangles of the primitives the clusters cover: those are the clusters. In
//! their place each such primitive holds its proxy ([`crate::RvgMesh::proxy_indices`]),
//! compacted to the vertices the proxy uses, which is what shadows, physics,
//! picking and ray tracing read.
//!
//! The JSON is edited as plain values, never through typed glTF structs, so
//! extensions and extras the engine relies on (imported cameras, face-rig
//! settings) survive exactly as they were.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::{RvgFile, RvgMesh};

const JSON_CHUNK: u32 = 0x4E4F_534A;
const BIN_CHUNK: u32 = 0x004E_4942;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const UNSIGNED_INT: u32 = 5125;
const FLOAT: u32 = 5126;

/// `glb` with every primitive in `file` that has a proxy coarser than its full
/// detail rewritten to that proxy, and the binary chunk rebuilt without the data
/// nothing references any more.
///
/// A GLB this cannot safely rewrite (compressed geometry, buffers outside the
/// GLB) comes back unchanged: the model is then stored with its full geometry
/// twice, which costs space but never correctness.
pub(crate) fn proxy_scene(glb: &[u8], file: &RvgFile) -> Result<Vec<u8>, String> {
    let (mut doc, bin) = split(glb)?;
    if compressed(&doc) || external_buffers(&doc) {
        return Ok(glb.to_vec());
    }
    let mut bin = bin.to_vec();
    let refs = accessor_references(&doc);
    for mesh in &file.meshes {
        let proxy = mesh.proxy_indices();
        if proxy.len() >= mesh.original_triangle_count() * 3 {
            continue;
        }
        // A primitive this cannot read (sparse or bufferless accessors) keeps
        // its full geometry.
        let _ = rewrite(&mut doc, &mut bin, &refs, mesh, &proxy);
    }
    unshare(&mut doc, &mut bin);
    let bin = compact(&mut doc, &bin);
    Ok(join(&doc, &bin))
}

/// Give every accessor in use a tightly packed view of its own.
///
/// Compaction keeps or drops whole views, and an importer commonly packs many
/// primitives' vertices into one shared view. A single primitive left at full
/// detail (a small mesh, or one virtual geometry does not take) then kept the
/// entire view alive, and with it every other primitive's original vertices:
/// Bistro's scene carried 110 MB of full-detail geometry nothing read, beside
/// the clusters that hold it already. Copied out, a shared view survives only
/// if something other than an accessor (an image, an extension) still uses it.
///
/// Accessors that read the same bytes the same way share one copy, so this
/// never duplicates data that was deliberately shared.
fn unshare(doc: &mut Value, bin: &mut Vec<u8>) {
    let refs = accessor_references(doc);
    // Which accessors are index and vertex buffers, for their views' target.
    let mut indices = std::collections::HashSet::new();
    let mut vertex = std::collections::HashSet::new();
    for mesh in doc["meshes"].as_array().into_iter().flatten() {
        for prim in mesh["primitives"].as_array().into_iter().flatten() {
            if let Some(i) = prim["indices"].as_u64() {
                indices.insert(i);
            }
            let maps = std::iter::once(&prim["attributes"]).chain(prim["targets"].as_array().into_iter().flatten());
            for map in maps {
                vertex.extend(map.as_object().into_iter().flatten().filter_map(|(_, a)| a.as_u64()));
            }
        }
    }
    let mut copies: HashMap<String, u64> = HashMap::new();
    let mut used: Vec<u64> = refs.keys().copied().collect();
    used.sort_unstable();
    for index in used {
        let accessor = &doc["accessors"][index as usize];
        let Some(count) = accessor["count"].as_u64() else { continue };
        if accessor["bufferView"].as_u64().is_none() {
            continue;
        }
        // Same view, offset, layout and length: the same bytes.
        let key = format!(
            "{}:{}:{}:{}:{}:{}",
            accessor["bufferView"], accessor["byteOffset"], accessor["componentType"], accessor["type"], accessor["normalized"], count
        );
        let view = match copies.get(&key) {
            Some(&view) => view,
            None => {
                // Packed tightly, a vertex attribute of 3 or 6 bytes would
                // have a stride glTF does not allow (a multiple of 4).
                let element = accessor["componentType"].as_u64().and_then(component_size).zip(accessor["type"].as_str().and_then(components));
                if vertex.contains(&index) && element.is_none_or(|(size, n)| !(size * n).is_multiple_of(4)) {
                    continue;
                }
                let all: Vec<u32> = (0..count as u32).collect();
                // Sparse, or out of range: left reading the shared view.
                let Some(bytes) = gather(doc, bin, index, &all) else { continue };
                let target = if indices.contains(&index) { ELEMENT_ARRAY_BUFFER } else { ARRAY_BUFFER };
                let view = push_view(doc, bin, &bytes, target);
                if !vertex.contains(&index) && !indices.contains(&index) {
                    // Skin matrices and animation keys are not GPU buffers;
                    // a target on their view is wrong, not merely unneeded.
                    if let Some(v) = doc["bufferViews"].get_mut(view as usize).and_then(Value::as_object_mut) {
                        v.remove("target");
                    }
                }
                copies.insert(key, view);
                view
            }
        };
        if let Some(object) = doc["accessors"][index as usize].as_object_mut() {
            object.insert("bufferView".into(), json!(view));
            object.remove("byteOffset");
        }
    }
}

fn split(glb: &[u8]) -> Result<(Value, &[u8]), String> {
    let u32_at = |at: usize| -> Option<u32> { glb.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) };
    if glb.get(..4) != Some(b"glTF") {
        return Err("not a GLB".into());
    }
    let json_len = u32_at(12).ok_or("truncated GLB")? as usize;
    if u32_at(16) != Some(JSON_CHUNK) {
        return Err("GLB does not start with its JSON chunk".into());
    }
    let json = glb.get(20..20 + json_len).ok_or("truncated GLB JSON")?;
    let doc: Value = serde_json::from_slice(json).map_err(|e| format!("GLB JSON: {e}"))?;
    let bin_at = 20 + json_len;
    let bin = match (u32_at(bin_at), u32_at(bin_at + 4)) {
        (Some(len), Some(BIN_CHUNK)) => glb.get(bin_at + 8..bin_at + 8 + len as usize).ok_or("truncated GLB BIN")?,
        _ => &[],
    };
    Ok((doc, bin))
}

fn join(doc: &Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(doc).unwrap_or_default();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut bin = bin.to_vec();
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + if bin.is_empty() { 0 } else { 8 + bin.len() };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&JSON_CHUNK.to_le_bytes());
    out.extend_from_slice(&json);
    if !bin.is_empty() {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&BIN_CHUNK.to_le_bytes());
        out.extend_from_slice(&bin);
    }
    out
}

fn compressed(doc: &Value) -> bool {
    let listed = |key: &str| {
        doc[key].as_array().is_some_and(|a| {
            a.iter().any(|e| {
                matches!(
                    e.as_str(),
                    Some("KHR_draco_mesh_compression" | "EXT_meshopt_compression" | "KHR_meshopt_compression")
                )
            })
        })
    };
    listed("extensionsUsed") || listed("extensionsRequired")
}

fn external_buffers(doc: &Value) -> bool {
    doc["buffers"].as_array().is_some_and(|b| b.len() > 1 || b.iter().any(|b| b.get("uri").is_some()))
}

/// How many places use each accessor. One that only this primitive uses is
/// rewritten in place; a shared one is left for the others and a new one made.
fn accessor_references(doc: &Value) -> HashMap<u64, u32> {
    let mut refs: HashMap<u64, u32> = HashMap::new();
    let mut count = |v: &Value| {
        if let Some(i) = v.as_u64() {
            *refs.entry(i).or_default() += 1;
        }
    };
    for mesh in doc["meshes"].as_array().into_iter().flatten() {
        for prim in mesh["primitives"].as_array().into_iter().flatten() {
            prim["attributes"].as_object().into_iter().flatten().for_each(|(_, v)| count(v));
            count(&prim["indices"]);
            for target in prim["targets"].as_array().into_iter().flatten() {
                target.as_object().into_iter().flatten().for_each(|(_, v)| count(v));
            }
        }
    }
    for skin in doc["skins"].as_array().into_iter().flatten() {
        count(&skin["inverseBindMatrices"]);
    }
    for anim in doc["animations"].as_array().into_iter().flatten() {
        for sampler in anim["samplers"].as_array().into_iter().flatten() {
            count(&sampler["input"]);
            count(&sampler["output"]);
        }
    }
    for node in doc["nodes"].as_array().into_iter().flatten() {
        let instancing = &node["extensions"]["EXT_mesh_gpu_instancing"]["attributes"];
        instancing.as_object().into_iter().flatten().for_each(|(_, v)| count(v));
    }
    refs
}

fn component_size(component_type: u64) -> Option<usize> {
    match component_type {
        5120 | 5121 => Some(1),
        5122 | 5123 => Some(2),
        5125 | 5126 => Some(4),
        _ => None,
    }
}

fn components(kind: &str) -> Option<usize> {
    match kind {
        "SCALAR" => Some(1),
        "VEC2" => Some(2),
        "VEC3" => Some(3),
        "VEC4" | "MAT2" => Some(4),
        "MAT3" => Some(9),
        "MAT4" => Some(16),
        _ => None,
    }
}

/// The elements `vertices` of accessor `index`, tightly packed.
fn gather(doc: &Value, bin: &[u8], index: u64, vertices: &[u32]) -> Option<Vec<u8>> {
    let accessor = &doc["accessors"][index as usize];
    if accessor.get("sparse").is_some() {
        return None;
    }
    let element = component_size(accessor["componentType"].as_u64()?)? * components(accessor["type"].as_str()?)?;
    let view = &doc["bufferViews"][accessor["bufferView"].as_u64()? as usize];
    if view["buffer"].as_u64().unwrap_or(0) != 0 {
        return None;
    }
    let stride = view["byteStride"].as_u64().map_or(element, |s| s as usize);
    let base = view["byteOffset"].as_u64().unwrap_or(0) as usize + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
    let mut out = Vec::with_capacity(vertices.len() * element);
    for &v in vertices {
        let at = base + v as usize * stride;
        out.extend_from_slice(bin.get(at..at + element)?);
    }
    Some(out)
}

/// Append `data` as a new buffer view, 4-byte aligned. Returns its index.
fn push_view(doc: &mut Value, bin: &mut Vec<u8>, data: &[u8], target: u32) -> u64 {
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let offset = bin.len();
    bin.extend_from_slice(data);
    let views = doc["bufferViews"].as_array_mut().expect("checked by the caller");
    views.push(json!({ "buffer": 0, "byteOffset": offset, "byteLength": data.len(), "target": target }));
    views.len() as u64 - 1
}

/// Per-component min and max of tightly packed floats.
fn bounds(data: &[u8], comps: usize) -> (Vec<f32>, Vec<f32>) {
    let mut min = vec![f32::MAX; comps];
    let mut max = vec![f32::MIN; comps];
    for (i, chunk) in data.chunks_exact(4).enumerate() {
        let v = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let c = i % comps;
        min[c] = min[c].min(v);
        max[c] = max[c].max(v);
    }
    (min, max)
}

/// Point accessor `index` (or a new copy of it, when others share it) at
/// `data`, `count` elements long. Returns the accessor to use.
fn retarget(doc: &mut Value, refs: &HashMap<u64, u32>, index: u64, view: u64, count: usize, data: &[u8]) -> u64 {
    let mut accessor = doc["accessors"][index as usize].clone();
    let object = accessor.as_object_mut().expect("an accessor is an object");
    object.insert("bufferView".into(), json!(view));
    object.remove("byteOffset");
    object.insert("count".into(), json!(count));
    if object.contains_key("min") || object.contains_key("max") {
        let float = object.get("componentType").and_then(Value::as_u64) == Some(u64::from(FLOAT));
        let comps = object.get("type").and_then(Value::as_str).and_then(components).unwrap_or(1);
        if float {
            let (min, max) = bounds(data, comps);
            object.insert("min".into(), json!(min));
            object.insert("max".into(), json!(max));
        } else {
            object.remove("min");
            object.remove("max");
        }
    }
    let accessors = doc["accessors"].as_array_mut().expect("an accessor list");
    if refs.get(&index).copied().unwrap_or(0) <= 1 {
        accessors[index as usize] = accessor;
        index
    } else {
        accessors.push(accessor);
        accessors.len() as u64 - 1
    }
}

/// Swap one primitive's triangles for its proxy. `None` leaves it untouched.
fn rewrite(doc: &mut Value, bin: &mut Vec<u8>, refs: &HashMap<u64, u32>, mesh: &RvgMesh, proxy: &[u32]) -> Option<()> {
    let (mi, pi) = (mesh.mesh_index as usize, mesh.primitive_index as usize);
    let attributes: Vec<(String, u64)> = doc["meshes"][mi]["primitives"][pi]["attributes"]
        .as_object()?
        .iter()
        .map(|(k, v)| Some((k.clone(), v.as_u64()?)))
        .collect::<Option<_>>()?;
    if doc["meshes"][mi]["primitives"][pi].get("targets").is_some() {
        return None;
    }
    // The proxy's vertices, in first-use order, and its triangles renumbered.
    let mut local: HashMap<u32, u32> = HashMap::new();
    let mut order: Vec<u32> = Vec::new();
    let indices: Vec<u32> = proxy
        .iter()
        .map(|&v| {
            *local.entry(v).or_insert_with(|| {
                order.push(v);
                order.len() as u32 - 1
            })
        })
        .collect();
    // Read everything before changing anything, so a primitive that fails
    // halfway is not left half rewritten.
    let data: Vec<(String, u64, Vec<u8>)> = attributes
        .into_iter()
        .map(|(name, index)| Some((name, index, gather(doc, bin, index, &order)?)))
        .collect::<Option<_>>()?;
    doc["bufferViews"].as_array()?;

    for (name, index, bytes) in data {
        let view = push_view(doc, bin, &bytes, ARRAY_BUFFER);
        let accessor = retarget(doc, refs, index, view, order.len(), &bytes);
        doc["meshes"][mi]["primitives"][pi]["attributes"][name.as_str()] = json!(accessor);
    }
    let index_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let view = push_view(doc, bin, &index_bytes, ELEMENT_ARRAY_BUFFER);
    let accessor = json!({ "bufferView": view, "componentType": UNSIGNED_INT, "count": indices.len(), "type": "SCALAR" });
    let old = doc["meshes"][mi]["primitives"][pi]["indices"].as_u64();
    let index = match old {
        Some(old) if refs.get(&old).copied().unwrap_or(0) <= 1 => {
            doc["accessors"][old as usize] = accessor;
            old
        }
        _ => {
            let accessors = doc["accessors"].as_array_mut()?;
            accessors.push(accessor);
            accessors.len() as u64 - 1
        }
    };
    doc["meshes"][mi]["primitives"][pi]["indices"] = json!(index);
    Some(())
}

/// The model's scene at full detail: each converted primitive's proxy swapped
/// back for the full-detail triangles the clusters hold, with every attribute
/// they keep. What tools that edit a model's geometry (rigging, weight
/// painting, blend shapes) work on; [`crate::bake::build_model`] turns the
/// result back into an `.rvg`.
///
/// An attribute the clusters do not keep (a third UV set, a custom one) only
/// has the proxy's vertices, so it is dropped from the primitive.
pub(crate) fn full_detail_scene(file: &RvgFile) -> Result<Vec<u8>, String> {
    let scene = file.scene.as_deref().ok_or("this .rvg has no scene of its own")?;
    let (mut doc, bin) = split(scene)?;
    let mut bin = bin.to_vec();
    let refs = accessor_references(&doc);
    for mesh in &file.meshes {
        restore(&mut doc, &mut bin, &refs, mesh).ok_or_else(|| {
            format!("mesh {} primitive {} could not be restored", mesh.mesh_index, mesh.primitive_index)
        })?;
    }
    let bin = compact(&mut doc, &bin);
    Ok(join(&doc, &bin))
}

fn floats<const N: usize>(values: &[[f32; N]]) -> Vec<u8> {
    values.iter().flatten().flat_map(|f| f.to_le_bytes()).collect()
}

fn restore(doc: &mut Value, bin: &mut Vec<u8>, refs: &HashMap<u64, u32>, mesh: &RvgMesh) -> Option<()> {
    use crate::Attributes;
    let (mi, pi) = (mesh.mesh_index as usize, mesh.primitive_index as usize);
    doc["bufferViews"].as_array()?;
    let a = mesh.attributes;
    // (glTF name, bytes, component type, element type, present)
    let full: Vec<(&str, Vec<u8>, u32, &str)> = [
        ("POSITION", floats(&mesh.positions), FLOAT, "VEC3", true),
        ("NORMAL", floats(&mesh.normals), FLOAT, "VEC3", a.has(Attributes::NORMAL)),
        ("TEXCOORD_0", floats(&mesh.uv0), FLOAT, "VEC2", a.has(Attributes::UV0)),
        ("TANGENT", floats(&mesh.tangents), FLOAT, "VEC4", a.has(Attributes::TANGENT)),
        ("TEXCOORD_1", floats(&mesh.uv1), FLOAT, "VEC2", a.has(Attributes::UV1)),
        ("COLOR_0", floats(&mesh.colors), FLOAT, "VEC4", a.has(Attributes::COLOR)),
        (
            "JOINTS_0",
            mesh.joints.iter().flatten().flat_map(|j| j.to_le_bytes()).collect(),
            5123,
            "VEC4",
            a.has(Attributes::SKIN),
        ),
        ("WEIGHTS_0", floats(&mesh.weights), FLOAT, "VEC4", a.has(Attributes::SKIN)),
    ]
    .into_iter()
    .filter(|e| e.4)
    .map(|(name, bytes, component, kind, _)| (name, bytes, component, kind))
    .collect();
    let count = mesh.positions.len();
    let mut attributes = serde_json::Map::new();
    for (name, bytes, component, kind) in full {
        let view = push_view(doc, bin, &bytes, ARRAY_BUFFER);
        let mut accessor = json!({ "bufferView": view, "componentType": component, "count": count, "type": kind });
        if name == "POSITION" {
            let (min, max) = bounds(&bytes, 3);
            accessor["min"] = json!(min);
            accessor["max"] = json!(max);
        }
        let old = doc["meshes"][mi]["primitives"][pi]["attributes"][name].as_u64();
        let index = match old {
            Some(old) if refs.get(&old).copied().unwrap_or(0) <= 1 => {
                doc["accessors"][old as usize] = accessor;
                old
            }
            _ => {
                let accessors = doc["accessors"].as_array_mut()?;
                accessors.push(accessor);
                accessors.len() as u64 - 1
            }
        };
        attributes.insert(name.to_string(), json!(index));
    }
    doc["meshes"][mi]["primitives"][pi]["attributes"] = Value::Object(attributes);
    let indices: Vec<u8> = mesh.cut_indices(0.0).iter().flat_map(|i| i.to_le_bytes()).collect();
    let view = push_view(doc, bin, &indices, ELEMENT_ARRAY_BUFFER);
    let accessor = json!({ "bufferView": view, "componentType": UNSIGNED_INT, "count": indices.len() / 4, "type": "SCALAR" });
    let old = doc["meshes"][mi]["primitives"][pi]["indices"].as_u64();
    let index = match old {
        Some(old) if refs.get(&old).copied().unwrap_or(0) <= 1 => {
            doc["accessors"][old as usize] = accessor;
            old
        }
        _ => {
            let accessors = doc["accessors"].as_array_mut()?;
            accessors.push(accessor);
            accessors.len() as u64 - 1
        }
    };
    doc["meshes"][mi]["primitives"][pi]["indices"] = json!(index);
    Some(())
}

/// Every `"bufferView": n` in the document, wherever it is (accessors,
/// sparse accessors, images, extensions), except the view list itself.
fn view_references(value: &mut Value, visit: &mut dyn FnMut(&mut Value)) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if key == "bufferView" {
                    visit(child);
                } else if key != "bufferViews" {
                    view_references(child, visit);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| view_references(item, visit)),
        _ => {}
    }
}

/// Rebuild the binary chunk with only the buffer views something still points
/// at, renumbered in order.
fn compact(doc: &mut Value, bin: &[u8]) -> Vec<u8> {
    let Some(view_count) = doc["bufferViews"].as_array().map(Vec::len) else {
        return bin.to_vec();
    };
    let mut used = vec![false; view_count];
    view_references(doc, &mut |v| {
        if let Some(i) = v.as_u64().filter(|&i| (i as usize) < view_count) {
            used[i as usize] = true;
        }
    });
    let old_views = doc["bufferViews"].as_array().cloned().unwrap_or_default();
    let mut remap = vec![u64::MAX; view_count];
    let mut views = Vec::new();
    let mut out = Vec::new();
    for (i, view) in old_views.into_iter().enumerate() {
        if !used[i] {
            continue;
        }
        let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let length = view["byteLength"].as_u64().unwrap_or(0) as usize;
        let Some(data) = bin.get(offset..offset + length) else {
            // Out of range in the source already: keep it as it was.
            return bin.to_vec();
        };
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        let mut view = view;
        view["byteOffset"] = json!(out.len());
        out.extend_from_slice(data);
        remap[i] = views.len() as u64;
        views.push(view);
    }
    view_references(doc, &mut |v| {
        if let Some(i) = v.as_u64().filter(|&i| (i as usize) < view_count) {
            *v = json!(remap[i as usize]);
        }
    });
    doc["bufferViews"] = Value::Array(views);
    if let Some(buffer) = doc["buffers"].get_mut(0) {
        buffer["byteLength"] = json!(out.len());
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::bake::build_model;

    /// A GLB of one node drawing one primitive: a closed bumpy sphere with
    /// positions and normals, named, with a named material and an extra on
    /// the node, as an imported model would have.
    pub(crate) fn sphere_glb(rings: u32, segments: u32) -> Vec<u8> {
        let (positions, indices) = sphere(rings, segments);
        let mut bin: Vec<u8> = positions.iter().flatten().flat_map(|f| f.to_le_bytes()).collect();
        let normals_at = bin.len();
        bin.extend(positions.iter().flatten().flat_map(|f| f.to_le_bytes()));
        let indices_at = bin.len();
        bin.extend(indices.iter().flat_map(|i| i.to_le_bytes()));
        let n = positions.len();
        let doc = json!({
            "asset": { "version": "2.0" },
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "name": "Statue", "mesh": 0, "extras": { "renzora_camera": false } }],
            "materials": [{ "name": "Marble" }],
            "meshes": [{ "name": "Statue", "primitives": [{
                "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": 2, "material": 0
            }]}],
            "accessors": [
                { "bufferView": 0, "componentType": FLOAT, "count": n, "type": "VEC3",
                  "min": [-1.1, -1.1, -1.1], "max": [1.1, 1.1, 1.1] },
                { "bufferView": 1, "componentType": FLOAT, "count": n, "type": "VEC3" },
                { "bufferView": 2, "componentType": UNSIGNED_INT, "count": indices.len(), "type": "SCALAR" }
            ],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": normals_at },
                { "buffer": 0, "byteOffset": normals_at, "byteLength": indices_at - normals_at },
                { "buffer": 0, "byteOffset": indices_at, "byteLength": bin.len() - indices_at }
            ],
            "buffers": [{ "byteLength": bin.len() }]
        });
        join(&doc, &bin)
    }

    /// A closed bumpy sphere: positions (which double as normals) and
    /// triangles.
    fn sphere(rings: u32, segments: u32) -> (Vec<[f32; 3]>, Vec<u32>) {
        let mut positions: Vec<[f32; 3]> = vec![[0.0, 1.0, 0.0]];
        for r in 1..rings {
            let phi = std::f32::consts::PI * r as f32 / rings as f32;
            for s in 0..segments {
                let theta = std::f32::consts::TAU * s as f32 / segments as f32;
                let k = 1.0 + 0.05 * (theta * 5.0).sin() * (phi * 3.0).cos();
                positions.push([phi.sin() * theta.cos() * k, phi.cos() * k, phi.sin() * theta.sin() * k]);
            }
        }
        positions.push([0.0, -1.0, 0.0]);
        let ring = |r: u32, s: u32| 1 + (r - 1) * segments + s % segments;
        let bottom = positions.len() as u32 - 1;
        let mut indices: Vec<u32> = Vec::new();
        for s in 0..segments {
            indices.extend([0, ring(1, s + 1), ring(1, s)]);
            indices.extend([bottom, ring(rings - 1, s), ring(rings - 1, s + 1)]);
        }
        for r in 1..rings - 1 {
            for s in 0..segments {
                let (a, b, c, d) = (ring(r, s), ring(r, s + 1), ring(r + 1, s), ring(r + 1, s + 1));
                indices.extend([a, b, d, a, d, c]);
            }
        }
        (positions, indices)
    }

    /// Two meshes whose vertices and indices sit in shared views, as an
    /// importer packs them: a large one the proxy replaces and a small one
    /// kept at full detail. The small one must not keep the large one's
    /// original vertices alive (Bistro carried 110 MB of them).
    #[test]
    fn a_shared_view_does_not_keep_replaced_geometry() {
        let (big, big_indices) = sphere(96, 192);
        let (small, small_indices) = sphere(8, 16);
        let floats = |p: &[[f32; 3]]| -> Vec<u8> { p.iter().flatten().flat_map(|f| f.to_le_bytes()).collect() };
        let mut bin = floats(&big);
        bin.extend(floats(&small));
        let normals_at = bin.len();
        bin.extend(floats(&big));
        bin.extend(floats(&small));
        let indices_at = bin.len();
        bin.extend(big_indices.iter().chain(&small_indices).flat_map(|i| i.to_le_bytes()));
        let (nb, ns) = (big.len(), small.len());
        let doc = json!({
            "asset": { "version": "2.0" },
            "scene": 0,
            "scenes": [{ "nodes": [0, 1] }],
            "nodes": [{ "name": "Big", "mesh": 0 }, { "name": "Small", "mesh": 1 }],
            "meshes": [
                { "primitives": [{ "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": 2 }] },
                { "primitives": [{ "attributes": { "POSITION": 3, "NORMAL": 4 }, "indices": 5 }] }
            ],
            "accessors": [
                { "bufferView": 0, "componentType": FLOAT, "count": nb, "type": "VEC3", "min": [-1.1, -1.1, -1.1], "max": [1.1, 1.1, 1.1] },
                { "bufferView": 1, "componentType": FLOAT, "count": nb, "type": "VEC3" },
                { "bufferView": 2, "componentType": UNSIGNED_INT, "count": big_indices.len(), "type": "SCALAR" },
                { "bufferView": 0, "byteOffset": nb * 12, "componentType": FLOAT, "count": ns, "type": "VEC3", "min": [-1.1, -1.1, -1.1], "max": [1.1, 1.1, 1.1] },
                { "bufferView": 1, "byteOffset": nb * 12, "componentType": FLOAT, "count": ns, "type": "VEC3" },
                { "bufferView": 2, "byteOffset": big_indices.len() * 4, "componentType": UNSIGNED_INT, "count": small_indices.len(), "type": "SCALAR" }
            ],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": normals_at },
                { "buffer": 0, "byteOffset": normals_at, "byteLength": indices_at - normals_at },
                { "buffer": 0, "byteOffset": indices_at, "byteLength": bin.len() - indices_at }
            ],
            "buffers": [{ "byteLength": bin.len() }]
        });
        let glb = join(&doc, &bin);
        let file = build_model(&glb).unwrap();
        let scene = file.scene.as_deref().unwrap();

        let doc = gltf::Gltf::from_slice(scene).expect("the scene is a valid GLB");
        let blob = doc.blob.as_deref();
        let read = |mesh: usize| -> (Vec<[f32; 3]>, Vec<u32>) {
            let prim = doc.meshes().nth(mesh).unwrap().primitives().next().unwrap();
            let reader = prim.reader(|_| blob);
            (reader.read_positions().unwrap().collect(), reader.read_indices().unwrap().into_u32().collect())
        };
        let (proxy_positions, proxy_indices) = read(0);
        assert!(proxy_indices.len() < big_indices.len(), "the large mesh was not replaced by its proxy");
        assert_eq!(read(1), (small.clone(), small_indices.clone()), "the small mesh changed");
        // What the scene holds: the proxy, the small mesh, and nothing else.
        let needed = proxy_positions.len() * 24 + proxy_indices.len() * 4 + ns * 24 + small_indices.len() * 4;
        let views: usize = doc.views().map(|v| v.length()).sum();
        assert!(views <= needed + 64, "{views} bytes of views for {needed} bytes of geometry");
    }

    /// The scene of a model `.rvg` is a valid GLB that keeps the model's
    /// names, materials and extras, draws exactly the proxy (with every
    /// vertex it uses and no other), and is smaller than the GLB it came from.
    #[test]
    fn the_scene_holds_the_proxy() {
        let glb = sphere_glb(96, 192);
        let file = build_model(&glb).unwrap();
        let mesh = &file.meshes[0];
        let proxy = mesh.proxy_indices();
        assert!(proxy.len() < mesh.original_triangle_count() * 3, "the test mesh needs a coarser proxy");
        let scene = file.scene.as_deref().expect("a model carries its scene");
        assert!(scene.len() < glb.len() / 2, "{} bytes from {}", scene.len(), glb.len());

        let doc = gltf::Gltf::from_slice(scene).expect("the scene is a valid GLB");
        let blob = doc.blob.as_deref();
        let node = doc.nodes().next().unwrap();
        assert_eq!(node.name(), Some("Statue"));
        assert!(node.extras().is_some(), "the node's extras were dropped");
        assert_eq!(doc.materials().next().unwrap().name(), Some("Marble"));

        let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
        let reader = prim.reader(|_| blob);
        let positions: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
        let normals: Vec<[f32; 3]> = reader.read_normals().unwrap().collect();
        let indices: Vec<u32> = reader.read_indices().unwrap().into_u32().collect();
        assert_eq!(indices.len(), proxy.len());
        assert_eq!(normals.len(), positions.len());
        let mut used: Vec<u32> = proxy.clone();
        used.sort_unstable();
        used.dedup();
        assert_eq!(positions.len(), used.len(), "the proxy's vertices, and only those");
        for (&got, &want) in indices.iter().zip(&proxy) {
            assert_eq!(positions[got as usize], mesh.positions[want as usize]);
        }
        let bounds = prim.bounding_box();
        assert!(bounds.max[1] <= 1.06 && bounds.min[1] >= -1.06, "min/max were not recomputed: {bounds:?}");
    }

    /// The full-detail GLB rebuilt from a model `.rvg` draws exactly the
    /// triangles of the GLB it was built from, keeps its names, and builds
    /// back into the same model: what lets rigging and blend-shape tools edit
    /// a model stored as `.rvg`.
    #[test]
    fn the_full_detail_scene_is_the_original() {
        let glb = sphere_glb(96, 192);
        let file = build_model(&glb).unwrap();
        let full = crate::bake::full_detail_glb(&file).unwrap();

        let triangles = |bytes: &[u8]| -> Vec<[[u32; 3]; 3]> {
            let doc = gltf::Gltf::from_slice(bytes).unwrap();
            let blob = doc.blob.as_deref();
            let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
            let reader = prim.reader(|_| blob);
            let positions: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
            let indices: Vec<u32> = reader.read_indices().unwrap().into_u32().collect();
            let mut tris: Vec<[[u32; 3]; 3]> = indices
                .chunks_exact(3)
                .map(|t| {
                    // Rotated to start at the smallest corner, so the same
                    // triangle compares equal whichever corner came first.
                    let mut c = [0, 1, 2].map(|i| positions[t[i] as usize].map(f32::to_bits));
                    let lead = (0..3).min_by_key(|&i| c[i]).unwrap();
                    c.rotate_left(lead);
                    c
                })
                .collect();
            tris.sort_unstable();
            tris
        };
        assert_eq!(triangles(&full), triangles(&glb), "the rebuilt triangles differ from the original");
        let doc = gltf::Gltf::from_slice(&full).unwrap();
        assert_eq!(doc.nodes().next().unwrap().name(), Some("Statue"));
        assert_eq!(doc.materials().next().unwrap().name(), Some("Marble"));

        let again = build_model(&full).unwrap();
        assert_eq!(again.meshes[0].original_triangle_count(), file.meshes[0].original_triangle_count());
    }

    /// A small mesh's proxy is the whole mesh, so its scene keeps the GLB's
    /// geometry as it was.
    #[test]
    fn a_small_mesh_keeps_its_geometry() {
        let glb = sphere_glb(12, 24);
        let file = build_model(&glb).unwrap();
        let mesh = &file.meshes[0];
        assert_eq!(mesh.proxy_indices().len(), mesh.original_triangle_count() * 3);
        let doc = gltf::Gltf::from_slice(file.scene.as_deref().unwrap()).unwrap();
        let blob = doc.blob.as_deref();
        let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
        let count = prim.reader(|_| blob).read_indices().unwrap().into_u32().count();
        assert_eq!(count, mesh.original_triangle_count() * 3);
    }
}

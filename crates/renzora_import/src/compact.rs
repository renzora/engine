//! Garbage-collect a GLB's binary buffer.
//!
//! Import is *additive*: the converter pulls textures and animations out into
//! sibling files, but the original copies stay dead inside the GLB's BIN chunk:
//!
//!   - **Orphaned image bytes.** Texture extraction
//!     ([`crate::gltf_pass`]) rewrites each embedded image's JSON entry to
//!     point at the external `textures/*.png` file (clearing its `bufferView`),
//!     but repacks with the *original* binary chunk — so the encoded PNG/JPEG
//!     bytes remain in the buffer, referenced by nothing. For texture-heavy
//!     scenes this is the bulk of the file (a Bistro import lands ~190 MB).
//!   - **Animation keyframe buffers.** When animations are split out to `.anim`
//!     files (which the runtime plays — nothing reads the GLB's embedded
//!     clips), the sampler input/output accessors and their buffer data are
//!     likewise dead weight.
//!
//! [`compact_glb`] reclaims both: it (optionally) drops the `animations` array,
//! then rebuilds the buffer keeping only the bufferViews still referenced by a
//! surviving accessor or image, reindexing accessors/bufferViews and rewriting
//! every reference. Geometry, skins, morph targets, and materials are
//! untouched.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// Extensions whose accessors/bufferViews this pass doesn't fully model.
/// Encountering one means GC could drop or misindex data we don't understand,
/// so we bail and return the GLB unchanged.
const BAIL_EXTENSIONS: &[&str] = &[
    "EXT_meshopt_compression",
    "KHR_draco_mesh_compression",
    "EXT_mesh_gpu_instancing",
];

/// Garbage-collect dead data out of a GLB's binary buffer.
///
/// When `drop_animations` is true the `animations` array is removed first, so
/// the accessors/bufferViews it exclusively referenced become collectable.
/// Returns the rebuilt GLB, or the original bytes unchanged when there's
/// nothing to do (no BIN chunk, no accessors) or when an unsupported extension
/// is present.
pub fn compact_glb(glb_bytes: &[u8], drop_animations: bool) -> Result<Vec<u8>, String> {
    let glb = gltf::Glb::from_slice(glb_bytes).map_err(|e| format!("GLB parse: {e}"))?;
    let Some(bin_cow) = &glb.bin else {
        // No BIN chunk → all data is external already; nothing to reclaim.
        return Ok(glb_bytes.to_vec());
    };
    let bin = bin_cow.as_ref();

    let mut json: Value =
        serde_json::from_slice(&glb.json).map_err(|e| format!("GLB JSON parse: {e}"))?;

    // Escape hatch for extensions that reference buffer data in ways we don't
    // track — leave those assets byte-for-byte intact.
    if let Some(used) = json.get("extensionsUsed").and_then(|v| v.as_array()) {
        if used
            .iter()
            .filter_map(|v| v.as_str())
            .any(|e| BAIL_EXTENSIONS.contains(&e))
        {
            return Ok(glb_bytes.to_vec());
        }
    }

    let has_animations = json
        .get("animations")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let drop_anims = drop_animations && has_animations;

    let Some(accessors) = json.get("accessors").and_then(|v| v.as_array()).cloned() else {
        return Ok(glb_bytes.to_vec());
    };
    let buffer_views = json
        .get("bufferViews")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // ── 1. Which accessors survive? ──────────────────────────────────────
    // An accessor is kept if it's referenced by a mesh primitive, a skin, or
    // (when we're keeping them) an animation sampler. Everything else — i.e.
    // accessors only animations used, when dropping animations — is dead.
    let mut keep_acc: HashSet<usize> = HashSet::new();
    if let Some(meshes) = json.get("meshes").and_then(|v| v.as_array()) {
        for mesh in meshes {
            let Some(prims) = mesh.get("primitives").and_then(|v| v.as_array()) else {
                continue;
            };
            for prim in prims {
                if let Some(attrs) = prim.get("attributes").and_then(|v| v.as_object()) {
                    for (_, v) in attrs {
                        note_idx(&mut keep_acc, v);
                    }
                }
                if let Some(v) = prim.get("indices") {
                    note_idx(&mut keep_acc, v);
                }
                // Morph targets: array of { semantic: accessor } maps.
                if let Some(targets) = prim.get("targets").and_then(|v| v.as_array()) {
                    for target in targets {
                        if let Some(obj) = target.as_object() {
                            for (_, v) in obj {
                                note_idx(&mut keep_acc, v);
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(skins) = json.get("skins").and_then(|v| v.as_array()) {
        for skin in skins {
            if let Some(v) = skin.get("inverseBindMatrices") {
                note_idx(&mut keep_acc, v);
            }
        }
    }
    if !drop_anims {
        if let Some(anims) = json.get("animations").and_then(|v| v.as_array()) {
            for anim in anims {
                if let Some(samplers) = anim.get("samplers").and_then(|v| v.as_array()) {
                    for s in samplers {
                        if let Some(v) = s.get("input") {
                            note_idx(&mut keep_acc, v);
                        }
                        if let Some(v) = s.get("output") {
                            note_idx(&mut keep_acc, v);
                        }
                    }
                }
            }
        }
    }

    // ── 2. Which bufferViews survive? ────────────────────────────────────
    // Those referenced by a kept accessor (directly or via sparse), or by an
    // image that still stores its bytes in the buffer. Orphaned image views
    // (bytes left behind by texture extraction) are referenced by neither, so
    // they fall out here.
    let mut keep_bv: HashSet<usize> = HashSet::new();
    for (i, acc) in accessors.iter().enumerate() {
        if !keep_acc.contains(&i) {
            continue;
        }
        if let Some(v) = acc.get("bufferView") {
            note_idx(&mut keep_bv, v);
        }
        if let Some(sparse) = acc.get("sparse") {
            if let Some(v) = sparse.get("indices").and_then(|x| x.get("bufferView")) {
                note_idx(&mut keep_bv, v);
            }
            if let Some(v) = sparse.get("values").and_then(|x| x.get("bufferView")) {
                note_idx(&mut keep_bv, v);
            }
        }
    }
    if let Some(images) = json.get("images").and_then(|v| v.as_array()) {
        for img in images {
            if let Some(v) = img.get("bufferView") {
                note_idx(&mut keep_bv, v);
            }
        }
    }

    // ── 3. Compact the buffer: copy kept bufferViews into a fresh BIN. ────
    let mut new_bin: Vec<u8> = Vec::new();
    let mut bv_remap: HashMap<usize, usize> = HashMap::new();
    let mut new_bvs: Vec<Value> = Vec::with_capacity(keep_bv.len());
    for (j, bv) in buffer_views.iter().enumerate() {
        if !keep_bv.contains(&j) {
            continue;
        }
        // We only relocate data living in the GLB's own BIN chunk (buffer 0).
        // A bufferView into a secondary/external buffer can't be moved by
        // rewriting the BIN, so bail rather than corrupt offsets.
        let buffer = bv.get("buffer").and_then(|v| v.as_u64()).unwrap_or(0);
        if buffer != 0 {
            return Ok(glb_bytes.to_vec());
        }
        let off = bv.get("byteOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let len = bv
            .get("byteLength")
            .and_then(|v| v.as_u64())
            .ok_or("bufferView missing byteLength")? as usize;
        if off + len > bin.len() {
            return Err(format!(
                "bufferView {j} range {off}..{} exceeds BIN size {}",
                off + len,
                bin.len()
            ));
        }
        // Align each view to 4 bytes. Accessor component alignment is relative
        // to the buffer start; a 4-byte boundary satisfies every glTF
        // component type (1/2/4-byte) given accessors keep their own offsets.
        while new_bin.len() % 4 != 0 {
            new_bin.push(0);
        }
        let new_off = new_bin.len();
        new_bin.extend_from_slice(&bin[off..off + len]);

        let mut nbv = bv.clone();
        if new_off == 0 {
            // glTF: byteOffset defaults to 0 and must be omitted when 0 only if
            // it was; keeping it explicit is valid and simpler.
            nbv["byteOffset"] = Value::from(0u64);
        } else {
            nbv["byteOffset"] = Value::from(new_off as u64);
        }
        bv_remap.insert(j, new_bvs.len());
        new_bvs.push(nbv);
    }

    // ── 4. Rebuild accessors (drop dead ones, remap bufferView indices). ──
    let mut acc_remap: HashMap<usize, usize> = HashMap::new();
    let mut new_accs: Vec<Value> = Vec::with_capacity(keep_acc.len());
    for (i, acc) in accessors.iter().enumerate() {
        if !keep_acc.contains(&i) {
            continue;
        }
        let mut nacc = acc.clone();
        if let Some(old) = nacc.get("bufferView").and_then(|v| v.as_u64()) {
            if let Some(&new) = bv_remap.get(&(old as usize)) {
                nacc["bufferView"] = Value::from(new as u64);
            }
        }
        if let Some(sparse) = nacc.get_mut("sparse") {
            for key in ["indices", "values"] {
                if let Some(part) = sparse.get_mut(key) {
                    if let Some(old) = part.get("bufferView").and_then(|v| v.as_u64()) {
                        if let Some(&new) = bv_remap.get(&(old as usize)) {
                            part["bufferView"] = Value::from(new as u64);
                        }
                    }
                }
            }
        }
        acc_remap.insert(i, new_accs.len());
        new_accs.push(nacc);
    }

    // ── 5. Rewrite accessor references throughout the document. ──────────
    if let Some(meshes) = json.get_mut("meshes").and_then(|v| v.as_array_mut()) {
        for mesh in meshes {
            let Some(prims) = mesh.get_mut("primitives").and_then(|v| v.as_array_mut()) else {
                continue;
            };
            for prim in prims {
                if let Some(attrs) = prim.get_mut("attributes").and_then(|v| v.as_object_mut()) {
                    for (_, v) in attrs.iter_mut() {
                        remap_ref(v, &acc_remap);
                    }
                }
                if let Some(v) = prim.get_mut("indices") {
                    remap_ref(v, &acc_remap);
                }
                if let Some(targets) = prim.get_mut("targets").and_then(|v| v.as_array_mut()) {
                    for target in targets {
                        if let Some(obj) = target.as_object_mut() {
                            for (_, v) in obj.iter_mut() {
                                remap_ref(v, &acc_remap);
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(skins) = json.get_mut("skins").and_then(|v| v.as_array_mut()) {
        for skin in skins {
            if let Some(v) = skin.get_mut("inverseBindMatrices") {
                remap_ref(v, &acc_remap);
            }
        }
    }
    if let Some(images) = json.get_mut("images").and_then(|v| v.as_array_mut()) {
        for img in images {
            if let Some(v) = img.get_mut("bufferView") {
                remap_ref(v, &bv_remap);
            }
        }
    }

    // ── 6. Either drop animations, or remap their (kept) accessor refs. ──
    if drop_anims {
        if let Some(obj) = json.as_object_mut() {
            obj.remove("animations");
        }
    } else if let Some(anims) = json.get_mut("animations").and_then(|v| v.as_array_mut()) {
        for anim in anims {
            if let Some(samplers) = anim.get_mut("samplers").and_then(|v| v.as_array_mut()) {
                for s in samplers {
                    if let Some(v) = s.get_mut("input") {
                        remap_ref(v, &acc_remap);
                    }
                    if let Some(v) = s.get_mut("output") {
                        remap_ref(v, &acc_remap);
                    }
                }
            }
        }
    }

    // ── 7. Install the rebuilt arrays + buffer length. ───────────────────
    json["accessors"] = Value::Array(new_accs);
    json["bufferViews"] = Value::Array(new_bvs);
    if let Some(buffers) = json.get_mut("buffers").and_then(|v| v.as_array_mut()) {
        if let Some(buf0) = buffers.get_mut(0) {
            buf0["byteLength"] = Value::from(new_bin.len() as u64);
        }
    }

    let json_bytes = serde_json::to_vec(&json).map_err(|e| format!("serialize GLB JSON: {e}"))?;
    crate::optimize::rebuild_glb(&json_bytes, &new_bin)
}

/// Insert `v`'s integer value into `set` if it's a non-negative integer.
fn note_idx(set: &mut HashSet<usize>, v: &Value) {
    if let Some(i) = v.as_u64() {
        set.insert(i as usize);
    }
}

/// Rewrite an index value in place via `map` (old → new). Leaves it untouched
/// if it's not an integer or not in the map.
fn remap_ref(v: &mut Value, map: &HashMap<usize, usize>) {
    if let Some(old) = v.as_u64() {
        if let Some(&new) = map.get(&(old as usize)) {
            *v = Value::from(new as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pack a glTF JSON string + binary blob into a minimal GLB (mirrors the
    /// helper in `optimize.rs`).
    fn pack_test_glb(json: &str, bin: &[u8]) -> Vec<u8> {
        let json_bytes = json.as_bytes();
        let json_pad = (4 - json_bytes.len() % 4) % 4;
        let bin_pad = (4 - bin.len() % 4) % 4;
        let total = 12 + 8 + json_bytes.len() + json_pad + 8 + bin.len() + bin_pad;
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&((json_bytes.len() + json_pad) as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
        out.extend_from_slice(json_bytes);
        out.extend(std::iter::repeat_n(b' ', json_pad));
        out.extend_from_slice(&((bin.len() + bin_pad) as u32).to_le_bytes());
        out.extend_from_slice(&0x004E4942u32.to_le_bytes());
        out.extend_from_slice(bin);
        out.extend(std::iter::repeat_n(0u8, bin_pad));
        out
    }

    /// A GLB whose BIN holds: positions, indices, an orphaned image blob (an
    /// image entry rewritten to a URI, its bytes left dead in the buffer), and
    /// two animation accessors. Offsets are 4-byte aligned.
    fn synth_glb() -> Vec<u8> {
        let positions: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let mut bin = Vec::new();
        for p in positions {
            bin.extend_from_slice(&p.to_le_bytes()); // bv0 positions @0  len 36
        }
        for i in [0u16, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes()); // bv1 indices   @36 len 6
        }
        bin.extend_from_slice(&[0u8, 0]); // pad 42 → 44
        bin.extend_from_slice(&[0xABu8; 16]); // bv2 ORPHAN image  @44 len 16 → 60
        for t in [0.0f32, 1.0] {
            bin.extend_from_slice(&t.to_le_bytes()); // bv3 anim input @60 len 8 → 68
        }
        for _ in 0..8 {
            bin.extend_from_slice(&0.0f32.to_le_bytes()); // bv4 anim output @68 len 32 → 100
        }
        assert_eq!(bin.len(), 100);

        let json = r#"{"asset":{"version":"2.0"},
            "buffers":[{"byteLength":100}],
            "bufferViews":[
                {"buffer":0,"byteOffset":0,"byteLength":36},
                {"buffer":0,"byteOffset":36,"byteLength":6},
                {"buffer":0,"byteOffset":44,"byteLength":16},
                {"buffer":0,"byteOffset":60,"byteLength":8},
                {"buffer":0,"byteOffset":68,"byteLength":32}],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},
                {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"},
                {"bufferView":3,"componentType":5126,"count":2,"type":"SCALAR"},
                {"bufferView":4,"componentType":5126,"count":2,"type":"VEC4"}],
            "images":[{"uri":"textures/image_0.png"}],
            "meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}],
            "nodes":[{"mesh":0}],
            "animations":[{
                "channels":[{"sampler":0,"target":{"node":0,"path":"translation"}}],
                "samplers":[{"input":2,"output":3,"interpolation":"LINEAR"}]}]}"#;

        pack_test_glb(json, &bin)
    }

    fn read_json(glb: &[u8]) -> Value {
        let parsed = gltf::Glb::from_slice(glb).expect("parse out GLB");
        serde_json::from_slice(&parsed.json).expect("parse out JSON")
    }

    fn read_positions(glb: &[u8]) -> Vec<[f32; 3]> {
        let parsed = gltf::Glb::from_slice(glb).expect("parse");
        let bin = parsed.bin.expect("bin");
        let json = read_json(glb);
        // POSITION accessor is index 0 → its bufferView → offset.
        let acc = &json["accessors"][0];
        let bv_idx = acc["bufferView"].as_u64().unwrap() as usize;
        let base = json["bufferViews"][bv_idx]["byteOffset"]
            .as_u64()
            .unwrap_or(0) as usize;
        (0..3)
            .map(|i| {
                let o = base + i * 12;
                [
                    f32::from_le_bytes(bin[o..o + 4].try_into().unwrap()),
                    f32::from_le_bytes(bin[o + 4..o + 8].try_into().unwrap()),
                    f32::from_le_bytes(bin[o + 8..o + 12].try_into().unwrap()),
                ]
            })
            .collect()
    }

    #[test]
    fn drops_animations_and_orphaned_image_bytes() {
        let glb = synth_glb();
        let out = compact_glb(&glb, true).expect("compact");
        let json = read_json(&out);

        // Animations gone; only the two geometry accessors / bufferViews remain.
        assert!(json.get("animations").is_none(), "animations should be dropped");
        assert_eq!(json["accessors"].as_array().unwrap().len(), 2);
        assert_eq!(json["bufferViews"].as_array().unwrap().len(), 2);

        // Buffer shrank: kept 36 + 6 = 42 bytes (vs 100). Orphan + anim gone.
        assert_eq!(json["buffers"][0]["byteLength"].as_u64().unwrap(), 42);
        assert!(out.len() < glb.len());

        // Geometry survived intact and indices still resolve to the triangle.
        assert_eq!(
            read_positions(&out),
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(json["meshes"][0]["primitives"][0]["attributes"]["POSITION"], 0);
        assert_eq!(json["meshes"][0]["primitives"][0]["indices"], 1);

        // Still a valid GLB.
        gltf::Gltf::from_slice(&out).expect("re-parse");
    }

    #[test]
    fn keeps_animations_but_still_drops_orphan() {
        let glb = synth_glb();
        let out = compact_glb(&glb, false).expect("compact");
        let json = read_json(&out);

        // Animations kept (4 accessors: 2 geometry + 2 anim), but the orphaned
        // image bufferView is reclaimed (5 bufferViews → 4).
        assert!(json.get("animations").is_some(), "animations should survive");
        assert_eq!(json["accessors"].as_array().unwrap().len(), 4);
        assert_eq!(json["bufferViews"].as_array().unwrap().len(), 4);
        // 36 + 6 + 8 (aligned) + 32 = 84 (vs 100): just the 16-byte orphan gone.
        assert_eq!(json["buffers"][0]["byteLength"].as_u64().unwrap(), 84);

        // Animation sampler accessor refs were remapped and still valid.
        let input = json["animations"][0]["samplers"][0]["input"]
            .as_u64()
            .unwrap();
        assert!((input as usize) < 4, "remapped input accessor in range");

        assert_eq!(read_positions(&out), vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        gltf::Gltf::from_slice(&out).expect("re-parse");
    }

    #[test]
    fn bails_unchanged_on_unsupported_extension() {
        // A GLB advertising meshopt compression must be returned byte-for-byte.
        let json = r#"{"asset":{"version":"2.0"},
            "extensionsUsed":["EXT_meshopt_compression"],
            "buffers":[{"byteLength":4}],
            "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":4}],
            "accessors":[{"bufferView":0,"componentType":5126,"count":1,"type":"SCALAR"}]}"#;
        let glb = pack_test_glb(json, &[1, 2, 3, 4]);
        let out = compact_glb(&glb, true).expect("compact");
        assert_eq!(out, glb, "unsupported-extension GLB must be unchanged");
    }
}

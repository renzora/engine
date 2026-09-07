//! Make a GLB loadable by a consumer that only speaks core glTF 2.0: strip
//! `extensionsRequired` entries nothing here implements, and bake the legacy
//! spec-gloss workflow down into the metallic-roughness block.
//!
//! The `gltf` crate (used by both `optimize_glb` and Bevy's loader) refuses to
//! parse a document that lists an unknown extension as required. Several
//! third-party assets ship with `KHR_materials_pbrSpecularGlossiness` flagged
//! required even though they also carry `pbrMetallicRoughness` values — the
//! spec mandates that fallback. Removing the requirement lets the parser
//! consume the metallic-roughness path and load the file.
//!
//! Removing the requirement is only half the job, though, because plenty of
//! those files carry **no** metal-rough fallback: every texture and colour lives
//! inside the extension and `pbrMetallicRoughness` is either absent or written
//! out at its defaults. Bevy ignores the extension, so the model loads as
//! untextured white — which is what a Sketchfab scan looked like in the import
//! preview, and what it would have looked like in the project too. See
//! [`bake_spec_gloss`].
//!
//! This is the in-memory counterpart to `renzora_viewport::glb_compat`: the
//! viewport version patches files on disk; this one cleans the bytes during
//! the import pipeline so the file *written* to the project is already clean.

use serde_json::{Map, Value};

/// Extensions we silently drop from `extensionsRequired` because no consumer
/// in this engine implements them but the file has a usable PBR fallback.
const DROPPABLE_REQUIRED: &[&str] = &["KHR_materials_pbrSpecularGlossiness"];

/// The legacy spec-gloss extension, in full, since it is named in several
/// places here.
const SPEC_GLOSS: &str = "KHR_materials_pbrSpecularGlossiness";

/// Return GLB bytes a core-glTF consumer can render: unsupported
/// `extensionsRequired` entries removed, and spec-gloss materials baked into
/// `pbrMetallicRoughness`. If `bytes` is not a GLB or no patch is needed,
/// returns it unchanged.
pub fn strip_unsupported_extensions(bytes: &[u8]) -> Vec<u8> {
    let Ok((json_bytes, bin_chunk)) = split_glb(bytes) else {
        return bytes.to_vec();
    };
    let Ok(mut json) = serde_json::from_slice::<Value>(json_bytes) else {
        return bytes.to_vec();
    };
    // Both run: a file can need either, and `|` rather than `||` so the second
    // is not skipped when the first already reported a change.
    let changed = strip_unsupported_required(&mut json) | bake_spec_gloss(&mut json);
    if !changed {
        return bytes.to_vec();
    }
    let Ok(new_json) = serde_json::to_vec(&json) else {
        return bytes.to_vec();
    };
    repack_glb(&new_json, bin_chunk)
}

/// Returns `true` if the JSON was modified.
fn strip_unsupported_required(root: &mut Value) -> bool {
    let Some(obj) = root.as_object_mut() else {
        return false;
    };
    let mut changed = false;
    if let Some(Value::Array(required)) = obj.get_mut("extensionsRequired") {
        let before = required.len();
        required.retain(|v| match v.as_str() {
            Some(s) => !DROPPABLE_REQUIRED.contains(&s),
            None => true,
        });
        if required.len() != before {
            changed = true;
        }
        if required.is_empty() {
            obj.remove("extensionsRequired");
        }
    }
    changed
}

/// Write a spec-gloss material's diffuse and glossiness into the
/// `pbrMetallicRoughness` block Bevy actually reads.
///
/// The conversion is lossy and deliberately shallow — it maps the two things
/// that decide whether a model reads as *itself*:
///
/// * `diffuseTexture` / `diffuseFactor` → `baseColorTexture` / `baseColorFactor`
/// * `glossinessFactor` → `roughnessFactor` as `1 - g`, with `metallicFactor` 0
///
/// The per-pixel glossiness in `specularGlossinessTexture`'s alpha is *not*
/// converted: that needs a channel repack, and the import pipeline's material
/// extraction already routes it into the `.material` graph's roughness pin
/// (see `gltf_pass`). What is left here is the GLB, whose job is to look right
/// in the preview and in the viewport.
///
/// Only materials whose metal-rough block says nothing beyond the glTF defaults
/// are touched. A file that carries both workflows properly has an authored
/// fallback, and talking over it would be strictly worse than leaving it.
///
/// The extension object is left in place. Anything that implements spec-gloss
/// is entitled to prefer it, and the spec says the metal-rough block is the
/// fallback — which is now the case rather than a claim.
fn bake_spec_gloss(root: &mut Value) -> bool {
    let Some(materials) = root
        .as_object_mut()
        .and_then(|o| o.get_mut("materials"))
        .and_then(|m| m.as_array_mut())
    else {
        return false;
    };
    let mut changed = false;
    for mat in materials.iter_mut() {
        let Some(sg) = mat
            .get("extensions")
            .and_then(|e| e.get(SPEC_GLOSS))
            .cloned()
        else {
            continue;
        };
        // "Says nothing" is not the same as "is absent": most spec-gloss
        // exporters write the metal-rough defaults out in full.
        let has_authored_pbr = mat
            .get("pbrMetallicRoughness")
            .is_some_and(|p| !crate::gltf_pass::pbr_block_is_default(p));
        if has_authored_pbr {
            continue;
        }
        let Some(obj) = mat.as_object_mut() else { continue };
        let mut pbr = Map::new();
        if let Some(tex) = sg.get("diffuseTexture") {
            pbr.insert("baseColorTexture".into(), tex.clone());
        }
        if let Some(factor) = sg.get("diffuseFactor") {
            pbr.insert("baseColorFactor".into(), factor.clone());
        }
        // A spec-gloss surface with no specular is a pure dielectric, and the
        // ones that do have specular are still better served by a dielectric
        // than by the `metallicFactor: 1.0` default, which lights a stone wall
        // as a mirror.
        pbr.insert("metallicFactor".into(), Value::from(0.0));
        pbr.insert("roughnessFactor".into(), Value::from(roughness_from(&sg)));
        obj.insert("pbrMetallicRoughness".into(), Value::Object(pbr));
        changed = true;
    }
    changed
}

/// One roughness value for a spec-gloss material, being careful about the two
/// ways `glossinessFactor` alone lies.
///
/// The plain reading is `1 - glossinessFactor`, and for a material whose gloss
/// is only that scalar it is right. The traps:
///
/// * **`specularFactor` is black.** The surface has no specular lobe at all, so
///   there is no gloss to carry over however high the factor reads. A gravel
///   material in a scanned warehouse declares `glossinessFactor: 1.0` beside
///   `specularFactor: [0,0,0]`, and taking the factor at face value turned
///   gravel into a mirror.
/// * **A `specularGlossinessTexture` is present.** Then the factor is a
///   *multiplier* over per-pixel glossiness in that texture's alpha, and the
///   common value for a multiplier is 1.0 — which says nothing about the
///   surface. Since the texture is not converted, the factor is floored to a
///   middling roughness rather than believed.
fn roughness_from(sg: &Value) -> f64 {
    let specular_is_black = sg
        .get("specularFactor")
        .and_then(|v| v.as_array())
        .is_some_and(|a| {
            a.iter()
                .all(|c| c.as_f64().is_some_and(|f| f <= 1.0 / 255.0))
        });
    if specular_is_black {
        return 1.0;
    }
    // glTF's default glossiness is 1.0 (a mirror), so an absent factor has to
    // be read as such rather than as "unspecified".
    let gloss = sg
        .get("glossinessFactor")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let roughness = (1.0 - gloss).clamp(0.0, 1.0);
    if sg.get("specularGlossinessTexture").is_some() {
        roughness.max(0.5)
    } else {
        roughness
    }
}

pub(crate) fn split_glb(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>), ()> {
    if bytes.len() < 12 {
        return Err(());
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != 0x46546C67 {
        return Err(());
    }
    let json_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let json_kind = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    if json_kind != 0x4E4F534A {
        return Err(());
    }
    let json_start = 20;
    let json_end = json_start + json_len;
    if json_end > bytes.len() {
        return Err(());
    }
    let bin = if json_end + 8 <= bytes.len() {
        let bin_len = u32::from_le_bytes([
            bytes[json_end],
            bytes[json_end + 1],
            bytes[json_end + 2],
            bytes[json_end + 3],
        ]) as usize;
        let bin_kind = u32::from_le_bytes([
            bytes[json_end + 4],
            bytes[json_end + 5],
            bytes[json_end + 6],
            bytes[json_end + 7],
        ]);
        if bin_kind == 0x004E4942 {
            let bin_start = json_end + 8;
            let bin_end = bin_start + bin_len;
            if bin_end > bytes.len() {
                return Err(());
            }
            Some(&bytes[bin_start..bin_end])
        } else {
            None
        }
    } else {
        None
    };
    Ok((&bytes[json_start..json_end], bin))
}

pub(crate) fn repack_glb(json: &[u8], bin: Option<&[u8]>) -> Vec<u8> {
    let json_pad = (4 - (json.len() % 4)) % 4;
    let json_chunk_len = json.len() + json_pad;
    let bin_chunk_len = bin
        .map(|b| {
            let pad = (4 - (b.len() % 4)) % 4;
            b.len() + pad
        })
        .unwrap_or(0);
    let total_len = 12 + 8 + json_chunk_len + if bin.is_some() { 8 + bin_chunk_len } else { 0 };

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&0x46546C67u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());
    out.extend_from_slice(&(json_chunk_len as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
    out.extend_from_slice(json);
    out.extend(std::iter::repeat_n(b' ', json_pad));
    if let Some(b) = bin {
        let bin_pad = (4 - (b.len() % 4)) % 4;
        out.extend_from_slice(&(bin_chunk_len as u32).to_le_bytes());
        out.extend_from_slice(&0x004E4942u32.to_le_bytes());
        out.extend_from_slice(b);
        out.extend(std::iter::repeat_n(0, bin_pad));
    }
    out
}

#[cfg(test)]
mod spec_gloss_tests {
    use super::bake_spec_gloss;
    use serde_json::json;

    /// The shape a Sketchfab spec-gloss export actually has: everything in the
    /// extension, and a metal-rough block that is either absent or the defaults
    /// spelled out. Both render white in Bevy until this bake runs.
    #[test]
    fn bakes_diffuse_and_glossiness_into_metal_rough() {
        let mut doc = json!({
            "materials": [{
                "name": "wall",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                    "metallicFactor": 1.0,
                    "roughnessFactor": 1.0
                },
                "extensions": { "KHR_materials_pbrSpecularGlossiness": {
                    "diffuseTexture": { "index": 3 },
                    "diffuseFactor": [0.5, 0.4, 0.3, 1.0],
                    "glossinessFactor": 0.25
                }}
            }]
        });
        assert!(bake_spec_gloss(&mut doc));
        let pbr = &doc["materials"][0]["pbrMetallicRoughness"];
        assert_eq!(pbr["baseColorTexture"]["index"], 3);
        assert_eq!(pbr["baseColorFactor"][0], 0.5);
        assert_eq!(pbr["metallicFactor"], 0.0);
        assert_eq!(pbr["roughnessFactor"], 0.75);
    }

    /// An absent glossiness is glTF's 1.0, i.e. a mirror, not "unspecified".
    #[test]
    fn absent_glossiness_is_fully_glossy() {
        let mut doc = json!({
            "materials": [{
                "extensions": { "KHR_materials_pbrSpecularGlossiness": {} }
            }]
        });
        assert!(bake_spec_gloss(&mut doc));
        assert_eq!(doc["materials"][0]["pbrMetallicRoughness"]["roughnessFactor"], 0.0);
    }

    /// A surface with no specular reflectance has no glossy lobe to carry over,
    /// whatever its glossiness factor claims. The gravel in a scanned warehouse
    /// says `glossinessFactor: 1.0` beside `specularFactor: [0,0,0]`.
    #[test]
    fn black_specular_is_fully_rough() {
        let mut doc = json!({
            "materials": [{
                "extensions": { "KHR_materials_pbrSpecularGlossiness": {
                    "glossinessFactor": 1.0,
                    "specularFactor": [0.0, 0.0, 0.0]
                }}
            }]
        });
        assert!(bake_spec_gloss(&mut doc));
        assert_eq!(doc["materials"][0]["pbrMetallicRoughness"]["roughnessFactor"], 1.0);
    }

    /// With a spec-gloss texture the factor is a multiplier over per-pixel
    /// glossiness we are not converting, so a bare 1.0 must not become a mirror.
    #[test]
    fn textured_glossiness_does_not_become_a_mirror() {
        let mut doc = json!({
            "materials": [{
                "extensions": { "KHR_materials_pbrSpecularGlossiness": {
                    "glossinessFactor": 1.0,
                    "specularFactor": [1.0, 1.0, 1.0],
                    "specularGlossinessTexture": { "index": 4 }
                }}
            }]
        });
        assert!(bake_spec_gloss(&mut doc));
        assert_eq!(doc["materials"][0]["pbrMetallicRoughness"]["roughnessFactor"], 0.5);
    }

    /// A file carrying a real metal-rough fallback already renders correctly;
    /// overwriting it with a lossy conversion would be strictly worse.
    #[test]
    fn authored_metal_rough_is_left_alone() {
        let mut doc = json!({
            "materials": [{
                "pbrMetallicRoughness": { "baseColorTexture": { "index": 1 } },
                "extensions": { "KHR_materials_pbrSpecularGlossiness": {
                    "diffuseTexture": { "index": 9 }
                }}
            }]
        });
        assert!(!bake_spec_gloss(&mut doc));
        assert_eq!(
            doc["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["index"],
            1
        );
    }
}

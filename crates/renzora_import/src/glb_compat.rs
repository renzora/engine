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
/// converted here: that needs a channel repack, which
/// [`bake_spec_gloss_textures`] does earlier in the import, before textures are
/// extracted. A material it repacked arrives here with an authored metal-rough
/// block and is skipped. What is left is the material whose texture could not
/// be read, which gets the factors alone.
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
///   surface. A material that reaches this point has a texture that could not
///   be repacked (see [`bake_spec_gloss_textures`]), so the factor is floored
///   to a middling roughness rather than believed.
fn roughness_from(sg: &Value) -> f64 {
    if specular_is_black(sg) {
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

/// Whether a spec-gloss material declares no specular at all. Its glossiness,
/// factor and texture alike, then describes a lobe that does not exist, and the
/// surface is fully rough. See [`roughness_from`].
pub(crate) fn specular_is_black(sg: &Value) -> bool {
    sg.get("specularFactor")
        .and_then(|v| v.as_array())
        .is_some_and(|a| {
            a.iter()
                .all(|c| c.as_f64().is_some_and(|f| f <= 1.0 / 255.0))
        })
}

/// Bake each spec-gloss material's per-pixel glossiness into a real
/// metallic-roughness texture, embedded in the GLB.
///
/// [`bake_spec_gloss`] maps only the scalar factors, because the per-pixel
/// gloss lives in the alpha of `specularGlossinessTexture` and needs a channel
/// repack. Without one, the importer had to invert that alpha with a maths node
/// in the `.material` graph, and the maths node is what made every such
/// material a generated-shader `GraphMaterial`: slower, approximated in every
/// pass but the forward one, and only approximated by a ray tracer.
///
/// This does the repack. For each spec-gloss material whose metal-rough block
/// is still at the glTF defaults, it writes a new PNG with roughness in G
/// (`1 - glossinessFactor * alpha`) and metallic in B (0, a dielectric, as
/// [`bake_spec_gloss`] decides), and points `metallicRoughnessTexture` at it.
/// The rest of the import then treats the material like any metal-rough one:
/// the role scan marks the texture linear, it is baked to `.rmip` with the
/// others, and the `.material` comes out as a plain `StandardMaterial`.
///
/// A material with black `specularFactor` gets no texture: it has no specular
/// lobe, so it is fully rough whatever its gloss says, and [`bake_spec_gloss`]
/// writes exactly that.
///
/// Images referenced by an absolute `uri` (the format converters' output) are
/// read from disk; a texture that can't be read or decoded is left alone, and
/// that material keeps the factors-only conversion. Returns `bytes` unchanged
/// when nothing qualifies.
pub fn bake_spec_gloss_textures(bytes: &[u8]) -> Vec<u8> {
    let Ok((json_bytes, bin_chunk)) = split_glb(bytes) else {
        return bytes.to_vec();
    };
    let Ok(mut root) = serde_json::from_slice::<Value>(json_bytes) else {
        return bytes.to_vec();
    };
    let mut bin = bin_chunk.map(<[u8]>::to_vec).unwrap_or_default();

    // One baked texture per (source image, gloss factor), shared by every
    // material that names the same pair.
    let mut baked: std::collections::HashMap<(usize, u64), usize> =
        std::collections::HashMap::new();
    let material_count = root
        .get("materials")
        .and_then(|m| m.as_array())
        .map_or(0, Vec::len);
    let mut changed = false;
    for m in 0..material_count {
        let Some((info, gloss)) = repack_candidate(&root, m) else {
            continue;
        };
        let Some(texture) = info.get("index").and_then(Value::as_u64) else {
            continue;
        };
        let Some(image) = root
            .pointer(&format!("/textures/{texture}/source"))
            .and_then(Value::as_u64)
        else {
            continue;
        };
        let key = (image as usize, gloss.to_bits());
        let new_texture = match baked.get(&key) {
            Some(&t) => t,
            None => {
                let Some(png) = image_bytes(&root, image as usize, &bin)
                    .and_then(|src| roughness_png(&src, gloss))
                else {
                    continue;
                };
                let Some(t) = append_png(&mut root, &mut bin, texture as usize, image as usize, png)
                else {
                    continue;
                };
                baked.insert(key, t);
                t
            }
        };

        let mut mr_info = info.clone();
        if let Some(obj) = mr_info.as_object_mut() {
            obj.insert("index".into(), Value::from(new_texture));
        }
        let sg = root
            .pointer(&format!("/materials/{m}/extensions/{SPEC_GLOSS}"))
            .cloned()
            .unwrap_or(Value::Null);
        let mut pbr = Map::new();
        if let Some(tex) = sg.get("diffuseTexture") {
            pbr.insert("baseColorTexture".into(), tex.clone());
        }
        if let Some(factor) = sg.get("diffuseFactor") {
            pbr.insert("baseColorFactor".into(), factor.clone());
        }
        // The texture carries the whole value: gloss factor folded into G, and
        // B written as 0. Factors of 1 pass it through unscaled.
        pbr.insert("metallicFactor".into(), Value::from(1.0));
        pbr.insert("roughnessFactor".into(), Value::from(1.0));
        pbr.insert("metallicRoughnessTexture".into(), mr_info);
        if let Some(obj) = root
            .pointer_mut(&format!("/materials/{m}"))
            .and_then(Value::as_object_mut)
        {
            obj.insert("pbrMetallicRoughness".into(), Value::Object(pbr));
            changed = true;
        }
    }
    if !changed {
        return bytes.to_vec();
    }
    if let Some(buffer) = root
        .pointer_mut("/buffers/0")
        .and_then(Value::as_object_mut)
    {
        buffer.insert("byteLength".into(), Value::from(bin.len()));
    }
    let Ok(new_json) = serde_json::to_vec(&root) else {
        return bytes.to_vec();
    };
    repack_glb(&new_json, Some(&bin))
}

/// Material `m`'s `specularGlossinessTexture` info and glossiness factor, if it
/// is a spec-gloss material this pass should bake.
fn repack_candidate(root: &Value, m: usize) -> Option<(Value, f64)> {
    let mat = root.pointer(&format!("/materials/{m}"))?;
    let sg = mat.get("extensions")?.get(SPEC_GLOSS)?;
    let has_authored_pbr = mat
        .get("pbrMetallicRoughness")
        .is_some_and(|p| !crate::gltf_pass::pbr_block_is_default(p));
    if has_authored_pbr || specular_is_black(sg) {
        return None;
    }
    let info = sg.get("specularGlossinessTexture")?.clone();
    let gloss = sg
        .get("glossinessFactor")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    Some((info, gloss))
}

/// The encoded bytes of image `image`: from the BIN chunk for an embedded one,
/// from disk for one a converter pointed at an absolute path.
fn image_bytes(root: &Value, image: usize, bin: &[u8]) -> Option<Vec<u8>> {
    let img = root.pointer(&format!("/images/{image}"))?;
    if let Some(view) = img.get("bufferView").and_then(Value::as_u64) {
        let view = root.pointer(&format!("/bufferViews/{view}"))?;
        let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let length = view.get("byteLength")?.as_u64()? as usize;
        return bin.get(offset..offset.checked_add(length)?).map(<[u8]>::to_vec);
    }
    let path = std::path::Path::new(img.get("uri")?.as_str()?);
    if !path.is_absolute() {
        return None;
    }
    std::fs::read(path).ok()
}

/// A metal-rough PNG from a spec-gloss texture: G = `1 - gloss * alpha`, B = 0.
fn roughness_png(source: &[u8], gloss: f64) -> Option<Vec<u8>> {
    let rgba = image::load_from_memory(source).ok()?.to_rgba8();
    let out = image::RgbaImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let alpha = f64::from(rgba.get_pixel(x, y)[3]) / 255.0;
        let roughness = (1.0 - gloss * alpha).clamp(0.0, 1.0);
        image::Rgba([255, (roughness * 255.0).round() as u8, 0, 255])
    });
    let mut png = Vec::new();
    out.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}

/// Embed `png` in the BIN chunk as a new image and texture, sampled like the
/// texture it was derived from, and return the new texture's index. `None`
/// only for a document too malformed to extend, which is left untouched.
fn append_png(
    root: &mut Value,
    bin: &mut Vec<u8>,
    from_texture: usize,
    from_image: usize,
    png: Vec<u8>,
) -> Option<usize> {
    let name = root
        .pointer(&format!("/images/{from_image}/name"))
        .and_then(Value::as_str)
        .map_or_else(|| format!("image_{from_image}_roughness"), |n| format!("{n}_roughness"));
    let sampler = root
        .pointer(&format!("/textures/{from_texture}/sampler"))
        .cloned();
    let obj = root.as_object_mut()?;
    for key in ["bufferViews", "images", "textures"] {
        if obj.get(key).is_some_and(|v| !v.is_array()) {
            return None;
        }
    }

    // Every bufferView starts 4-aligned.
    bin.resize(bin.len().next_multiple_of(4), 0);
    let offset = bin.len();
    let length = png.len();
    bin.extend_from_slice(&png);

    if obj.get("buffers").and_then(Value::as_array).is_none_or(Vec::is_empty) {
        obj.insert("buffers".into(), serde_json::json!([{ "byteLength": 0 }]));
    }
    let view = push(obj, "bufferViews", serde_json::json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": length,
    }));
    let image = push(obj, "images", serde_json::json!({
        "bufferView": view,
        "mimeType": "image/png",
        "name": name,
    }));
    let mut texture = serde_json::json!({ "source": image });
    if let Some(sampler) = sampler {
        texture["sampler"] = sampler;
    }
    Some(push(obj, "textures", texture))
}

/// Append `value` to the array at `key`, creating it, and return its index.
/// The caller has checked that an existing `key` holds an array.
fn push(obj: &mut Map<String, Value>, key: &str, value: Value) -> usize {
    let entry = obj.entry(key).or_insert_with(|| Value::Array(Vec::new()));
    match entry.as_array_mut() {
        Some(array) => {
            array.push(value);
            array.len() - 1
        }
        None => 0,
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

#[cfg(test)]
mod spec_gloss_texture_tests {
    use super::{bake_spec_gloss_textures, repack_glb, split_glb};
    use serde_json::{json, Value};

    /// A 2x1 spec-gloss texture: one pixel fully glossy, one fully rough.
    fn gloss_png() -> Vec<u8> {
        let img = image::RgbaImage::from_raw(2, 1, vec![200, 200, 200, 255, 200, 200, 200, 0])
            .unwrap();
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png
    }

    /// A GLB with that texture embedded and two spec-gloss materials using it:
    /// `wet` with white specular, `brick` with black.
    fn spec_gloss_glb(gloss_factor: f64) -> Vec<u8> {
        let png = gloss_png();
        let doc = json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": png.len() }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": png.len() }],
            "images": [{ "bufferView": 0, "mimeType": "image/png", "name": "sg" }],
            "samplers": [{}],
            "textures": [{ "source": 0, "sampler": 0 }],
            "materials": [
                {
                    "name": "wet",
                    "extensions": { "KHR_materials_pbrSpecularGlossiness": {
                        "diffuseTexture": { "index": 0 },
                        "specularFactor": [1.0, 1.0, 1.0],
                        "glossinessFactor": gloss_factor,
                        "specularGlossinessTexture": { "index": 0, "texCoord": 0 }
                    }}
                },
                {
                    "name": "brick",
                    "extensions": { "KHR_materials_pbrSpecularGlossiness": {
                        "specularFactor": [0.0, 0.0, 0.0],
                        "glossinessFactor": 1.0,
                        "specularGlossinessTexture": { "index": 0 }
                    }}
                }
            ]
        });
        repack_glb(&serde_json::to_vec(&doc).unwrap(), Some(&png))
    }

    fn parse(glb: &[u8]) -> (Value, Vec<u8>) {
        let (json, bin) = split_glb(glb).unwrap();
        (serde_json::from_slice(json).unwrap(), bin.unwrap_or_default().to_vec())
    }

    /// The repacked texture `material` points at, decoded.
    fn metal_rough(doc: &Value, bin: &[u8], material: usize) -> image::RgbaImage {
        let tex = doc["materials"][material]["pbrMetallicRoughness"]["metallicRoughnessTexture"]
            ["index"]
            .as_u64()
            .unwrap() as usize;
        let img = doc["textures"][tex]["source"].as_u64().unwrap() as usize;
        let view = &doc["bufferViews"][doc["images"][img]["bufferView"].as_u64().unwrap() as usize];
        let start = view["byteOffset"].as_u64().unwrap() as usize;
        let len = view["byteLength"].as_u64().unwrap() as usize;
        image::load_from_memory(&bin[start..start + len]).unwrap().to_rgba8()
    }

    #[test]
    fn gloss_becomes_a_metal_rough_texture() {
        let (doc, bin) = parse(&bake_spec_gloss_textures(&spec_gloss_glb(1.0)));
        let pbr = &doc["materials"][0]["pbrMetallicRoughness"];
        assert_eq!(pbr["baseColorTexture"]["index"], 0);
        assert_eq!(pbr["roughnessFactor"], 1.0);
        assert_eq!(pbr["metallicFactor"], 1.0);
        // The texture info is carried over, only re-pointed.
        assert_eq!(pbr["metallicRoughnessTexture"]["texCoord"], 0);
        let mr = metal_rough(&doc, &bin, 0);
        // Fully glossy pixel → roughness 0; fully rough pixel → 1. Metallic 0.
        assert_eq!(mr.get_pixel(0, 0).0[1..3], [0, 0]);
        assert_eq!(mr.get_pixel(1, 0).0[1..3], [255, 0]);
        // The new texture samples like the one it came from.
        let tex = pbr["metallicRoughnessTexture"]["index"].as_u64().unwrap() as usize;
        assert_eq!(doc["textures"][tex]["sampler"], 0);
        // The buffer covers every view; the GLB chunk may pad past it by up to
        // three bytes, as the spec allows.
        let byte_length = doc["buffers"][0]["byteLength"].as_u64().unwrap() as usize;
        let view = &doc["bufferViews"][1];
        let end = (view["byteOffset"].as_u64().unwrap() + view["byteLength"].as_u64().unwrap())
            as usize;
        assert_eq!(byte_length, end);
        assert!(bin.len() >= byte_length && bin.len() - byte_length < 4);
    }

    /// The factor multiplies the texture's gloss, and is folded into the bake.
    #[test]
    fn the_gloss_factor_is_folded_in() {
        let (doc, bin) = parse(&bake_spec_gloss_textures(&spec_gloss_glb(0.5)));
        let mr = metal_rough(&doc, &bin, 0);
        assert_eq!(mr.get_pixel(0, 0)[1], 128);
    }

    /// No specular lobe, nothing to bake: the gloss texture describes nothing.
    #[test]
    fn black_specular_gets_no_texture() {
        let (doc, _) = parse(&bake_spec_gloss_textures(&spec_gloss_glb(1.0)));
        assert!(doc["materials"][1].get("pbrMetallicRoughness").is_none());
    }

    /// The result is still a GLB the importer's own parser accepts.
    #[test]
    fn the_baked_glb_still_parses() {
        let baked = bake_spec_gloss_textures(&spec_gloss_glb(1.0));
        let glb = gltf::Glb::from_slice(&baked).unwrap();
        let _: gltf_json::Root = serde_json::from_slice(&glb.json).unwrap();
    }
}

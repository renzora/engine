//! GLTF/GLB passthrough — reads and re-exports (or copies) the file.

use std::path::Path;

use renzora_rmip::bake::TextureRole;

use crate::convert::{
    ExtractedAlphaMode, ExtractedPbrMaterial, ExtractedTexture, ImportError, ImportResult,
    ProgressFn, TextureSource,
};
use crate::settings::ImportSettings;

/// Walk the GLB JSON's materials and classify each image's [`TextureRole`].
/// Default is [`TextureRole::Color`] (sRGB). Normal maps become
/// [`TextureRole::NormalMap`] (→ BC5, renormalized mips); metallic-roughness,
/// occlusion and spec-glossiness maps become [`TextureRole::LinearData`]
/// (linear, no gamma decode — a gamma-corrected data map is wrong everywhere).
///
/// Returns a vec indexed by glTF image index. If parsing fails the vec is
/// empty and the extractor falls back to the color default per image.
fn scan_image_roles(root: &serde_json::Value) -> Vec<TextureRole> {
    let images = root
        .get("images")
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);
    let textures = root
        .get("textures")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let materials = root
        .get("materials")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Resolve a texture index → image index.
    let image_of = |tex_idx: usize| -> Option<usize> {
        textures
            .get(tex_idx)
            .and_then(|t| t.get("source"))
            .and_then(|s| s.as_u64())
            .map(|s| s as usize)
    };
    let texture_info_image = |info: Option<&serde_json::Value>| -> Option<usize> {
        info.and_then(|t| t.get("index"))
            .and_then(|i| i.as_u64())
            .and_then(|i| image_of(i as usize))
    };

    let mut roles = vec![TextureRole::Color; images];
    let mut mark = |idx: Option<usize>, role: TextureRole| {
        if let Some(i) = idx {
            if let Some(slot) = roles.get_mut(i) {
                *slot = role;
            }
        }
    };

    for mat in &materials {
        let pbr = mat.get("pbrMetallicRoughness");
        mark(
            texture_info_image(mat.get("normalTexture")),
            TextureRole::NormalMap,
        );
        mark(
            texture_info_image(mat.get("occlusionTexture")),
            TextureRole::LinearData,
        );
        mark(
            texture_info_image(pbr.and_then(|p| p.get("metallicRoughnessTexture"))),
            TextureRole::LinearData,
        );
        // KHR_materials_pbrSpecularGlossiness specularGlossinessTexture
        // packs sRGB-encoded specular RGB plus linear glossiness in alpha.
        // We only sample the alpha (for roughness), so treat as linear —
        // gamma-decoding the alpha would be wrong.
        let sg = mat
            .get("extensions")
            .and_then(|e| e.get("KHR_materials_pbrSpecularGlossiness"));
        mark(
            texture_info_image(sg.and_then(|s| s.get("specularGlossinessTexture"))),
            TextureRole::LinearData,
        );

        // The channels glTF has no slot for, which the converters park in a
        // vendor extension. Every one of them is data rather than colour, so
        // they must not be gamma-decoded — an opacity mask or a roughness map
        // read as sRGB is wrong everywhere it's sampled.
        let legacy = mat
            .get("extensions")
            .and_then(|e| e.get(crate::glb_build::RENZORA_LEGACY_EXT));
        for key in [
            "opacityTexture",
            "specularTexture",
            "roughnessTexture",
            "metallicTexture",
        ] {
            mark(
                texture_info_image(legacy.and_then(|l| l.get(key))),
                TextureRole::LinearData,
            );
        }
    }

    roles
}

/// GLB files: read the binary directly, then extract any embedded images to
/// sit alongside the GLB in `<model_dir>/textures/`. Embedded image entries
/// are rewritten in the GLB's JSON to external URIs so the GLB and the
/// loose texture files agree on the layout.
pub fn convert_glb(
    path: &Path,
    settings: &ImportSettings,
    progress: &ProgressFn,
) -> Result<ImportResult, ImportError> {
    let bytes = std::fs::read(path)?;

    if bytes.len() < 12 {
        return Err(ImportError::ParseError("file too small for GLB".into()));
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != 0x46546C67 {
        return Err(ImportError::ParseError("invalid GLB magic number".into()));
    }

    // Drop embedded cameras up front so neither the passthrough nor the
    // texture-extraction path can carry an active renderer into the scene.
    let bytes = strip_cameras_from_glb(bytes);

    if !settings.extract_textures {
        // Passthrough — keep the GLB exactly as-is (embedded textures
        // included). The user can re-enable extraction later and re-import.
        let extracted_materials = if settings.extract_materials {
            extract_glb_materials(&bytes)
        } else {
            Vec::new()
        };
        let (glb_bytes, warning) = apply_structure(
            crate::glb_compat::strip_unsupported_extensions(&bytes),
            settings,
        );
        return Ok(ImportResult {
            glb_bytes,
            warnings: warning.into_iter().collect(),
            extracted_textures: Vec::new(),
            extracted_materials,
        });
    }

    // Pre-scan materials so the texture extractor knows each image's role
    // (color vs normal vs linear data) before baking. The role drives both
    // the sRGB/linear choice and the GPU compression format — getting it
    // wrong looks fine on color maps but breaks shading on data ones.
    let roles = gltf::Glb::from_slice(&bytes)
        .ok()
        .and_then(|glb| serde_json::from_slice::<serde_json::Value>(&glb.json).ok())
        .map(|root| scan_image_roles(&root))
        .unwrap_or_default();

    let (rewritten, extracted_textures, mut warnings) =
        extract_glb_textures(&bytes, &roles, settings, progress).unwrap_or_else(|e| {
            (
                bytes.clone(),
                Vec::new(),
                vec![format!("texture extraction: {}", e)],
            )
        });

    let extracted_materials = if settings.extract_materials {
        extract_glb_materials(&rewritten)
    } else {
        Vec::new()
    };

    let (glb_bytes, restructure_warning) = apply_structure(
        crate::glb_compat::strip_unsupported_extensions(&rewritten),
        settings,
    );
    warnings.extend(restructure_warning);

    Ok(ImportResult {
        glb_bytes,
        warnings,
        extracted_textures,
        extracted_materials,
    })
}

/// Run the shared texture + material pass over a GLB one of the format
/// converters just built.
///
/// This is the whole point of the converters emitting a GLB rather than their
/// own extraction results: role scanning, texture writing, the memory budget
/// and material extraction all live in one place, so FBX, OBJ, USD and Collada
/// get whatever the glTF importer gets, at the same time it gets it. They used
/// to each carry a partial copy of this, which is how FBX ended up as the only
/// format with no resolution clamp and no `.rmip` output.
///
/// A converter is responsible for exactly two things beyond geometry: writing
/// complete glTF materials (see `glb_build::material_json`), and pointing each image
/// at an **absolute path** on disk when the source referenced a file rather than
/// embedding it. Locating that file is format-specific; everything after it
/// isn't.
pub(crate) fn finish_converted_glb(
    glb_bytes: Vec<u8>,
    settings: &ImportSettings,
    progress: &ProgressFn,
    mut warnings: Vec<String>,
) -> ImportResult {
    if !settings.extract_textures {
        let extracted_materials = if settings.extract_materials {
            extract_glb_materials(&glb_bytes)
        } else {
            Vec::new()
        };
        return ImportResult {
            glb_bytes: crate::glb_compat::strip_unsupported_extensions(&glb_bytes),
            warnings,
            extracted_textures: Vec::new(),
            extracted_materials,
        };
    }

    let roles = gltf::Glb::from_slice(&glb_bytes)
        .ok()
        .and_then(|glb| serde_json::from_slice::<serde_json::Value>(&glb.json).ok())
        .map(|root| scan_image_roles(&root))
        .unwrap_or_default();

    let (rewritten, extracted_textures, texture_warnings) =
        match extract_glb_textures(&glb_bytes, &roles, settings, progress) {
            Ok(v) => v,
            Err(e) => {
                warnings.push(format!("texture extraction: {}", e));
                (glb_bytes, Vec::new(), Vec::new())
            }
        };
    warnings.extend(texture_warnings);

    let extracted_materials = if settings.extract_materials {
        extract_glb_materials(&rewritten)
    } else {
        Vec::new()
    };

    let (glb_bytes, restructure_warning) = apply_structure(
        crate::glb_compat::strip_unsupported_extensions(&rewritten),
        settings,
    );
    warnings.extend(restructure_warning);

    ImportResult {
        glb_bytes,
        warnings,
        extracted_textures,
        extracted_materials,
    }
}

/// Reshape the scene graph per [`crate::settings::SceneStructure`].
///
/// Runs at the very end of the shared tail so every format gets the same
/// treatment — the converters do not need to know the setting exists, and a
/// merged FBX and a deeply-nested glTF both arrive here as a GLB.
pub(crate) fn apply_structure(
    glb_bytes: Vec<u8>,
    settings: &ImportSettings,
) -> (Vec<u8>, Option<String>) {
    use crate::settings::SceneStructure;
    match settings.structure {
        // Combined is what the transcoders already produce, and un-merging a
        // glTF would mean rewriting its buffers rather than its JSON — so this
        // leaves the document alone either way.
        SceneStructure::Preserve | SceneStructure::Combined => (glb_bytes, None),
        SceneStructure::FlatPerMesh => match crate::restructure::flatten_per_mesh(&glb_bytes) {
            Ok((out, warning)) => (out, warning),
            Err(e) => (glb_bytes, Some(format!("hierarchy flatten: {e}"))),
        },
    }
}

/// Walk the GLB JSON's `materials` array and produce a flat
/// [`ExtractedPbrMaterial`] per entry. When called after
/// `extract_glb_textures` the texture URIs reference the now-external
/// `textures/...` files; when textures stay embedded the URI is `None` and
/// downstream consumers fall back to the PBR factors only.
fn extract_glb_materials(glb_bytes: &[u8]) -> Vec<ExtractedPbrMaterial> {
    let Ok(glb) = gltf::Glb::from_slice(glb_bytes) else {
        return Vec::new();
    };
    let Ok(root) = serde_json::from_slice::<serde_json::Value>(&glb.json) else {
        return Vec::new();
    };

    let materials = root
        .get("materials")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let textures = root
        .get("textures")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let images = root
        .get("images")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let texture_uri = |idx: usize| -> Option<String> {
        let tex = textures.get(idx)?;
        let img_idx = tex.get("source")?.as_u64()? as usize;
        let img = images.get(img_idx)?;
        let uri: &str = img.get("uri")?.as_str()?;
        // Materials reference the mipmapped `.rmip` file rather than the
        // original PNG/JPG/etc that Bevy's GLB loader uses. Both files
        // sit in the same `textures/` folder under the same stem; we just
        // swap the extension at the boundary.
        let stem = uri.rsplit_once('.').map(|(s, _)| s).unwrap_or(uri);
        Some(format!("{}.rmip", stem))
    };

    // Pull the texture index nested under any glTF "*Texture" entry — they
    // all share the shape `{ "index": N, "texCoord": M }`.
    let texture_info_uri = |info: Option<&serde_json::Value>| -> Option<String> {
        info.and_then(|t| t.get("index"))
            .and_then(|i| i.as_u64())
            .and_then(|i| texture_uri(i as usize))
    };

    let mut out = Vec::new();
    for (i, mat) in materials.iter().enumerate() {
        let name = mat
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("material_{}", i));

        let pbr = mat.get("pbrMetallicRoughness");

        let base_color = pbr
            .and_then(|p| p.get("baseColorFactor"))
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                let r = arr.first()?.as_f64()? as f32;
                let g = arr.get(1)?.as_f64()? as f32;
                let b = arr.get(2)?.as_f64()? as f32;
                let a = arr
                    .get(3)
                    .and_then(|v| v.as_f64())
                    .map(|x| x as f32)
                    .unwrap_or(1.0);
                Some([r, g, b, a])
            })
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);

        let metallic = pbr
            .and_then(|p| p.get("metallicFactor"))
            .and_then(|v| v.as_f64())
            .map(|x| x as f32)
            .unwrap_or(1.0);

        let roughness = pbr
            .and_then(|p| p.get("roughnessFactor"))
            .and_then(|v| v.as_f64())
            .map(|x| x as f32)
            .unwrap_or(1.0);

        // glTF default emissive is black [0, 0, 0]. Multiplied with
        // emissiveTexture per the spec; we surface both and let the graph
        // builder decide how to wire them.
        let emissive = mat
            .get("emissiveFactor")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                let r = arr.first()?.as_f64()? as f32;
                let g = arr.get(1)?.as_f64()? as f32;
                let b = arr.get(2)?.as_f64()? as f32;
                Some([r, g, b])
            })
            .unwrap_or([0.0, 0.0, 0.0]);

        let mut base_color_texture = texture_info_uri(pbr.and_then(|p| p.get("baseColorTexture")));
        let metallic_roughness_texture =
            texture_info_uri(pbr.and_then(|p| p.get("metallicRoughnessTexture")));
        let normal_texture = texture_info_uri(mat.get("normalTexture"));
        let emissive_texture = texture_info_uri(mat.get("emissiveTexture"));
        let occlusion_texture = texture_info_uri(mat.get("occlusionTexture"));

        // Fallback for the legacy `KHR_materials_pbrSpecularGlossiness` workflow.
        // Many third-party Sketchfab GLBs ship with all the actual texture and
        // color data inside this extension and an empty `pbrMetallicRoughness`
        // block. Spec-gloss → metal-rough is mathematically lossy, but pulling
        // diffuse + glossiness gives the user a recognizable starting point
        // they can refine in the material editor.
        //
        // Detection: presence of the extension AND a metal-rough block that says
        // nothing beyond the glTF defaults — the unambiguous "spec-gloss-only"
        // case where we should override those defaults rather than respect them.
        // See `pbr_block_is_default`: "says nothing" is not the same as "is
        // absent", and reading it as the latter is what left two scanned models
        // rendering as mirrors.
        let spec_gloss = mat
            .get("extensions")
            .and_then(|e| e.get("KHR_materials_pbrSpecularGlossiness"));
        let pbr_block_empty = pbr.map(pbr_block_is_default).unwrap_or(true);

        let mut roughness = roughness;
        let mut metallic = metallic;
        let mut base_color = base_color;
        // Always pull the spec-gloss texture path when the extension is
        // present so the graph builder can route per-pixel glossiness into
        // the roughness pin. Without this, every spec-gloss material gets
        // one uniform roughness and reflective surfaces (wet stone, glass)
        // render as flat matte.
        let specular_glossiness_texture =
            spec_gloss.and_then(|sg| texture_info_uri(sg.get("specularGlossinessTexture")));
        if let Some(sg) = spec_gloss {
            if base_color_texture.is_none() {
                base_color_texture = texture_info_uri(sg.get("diffuseTexture"));
            }
            // Diffuse factor only overrides if the metal-rough side didn't
            // declare its own (default white).
            if base_color == [1.0, 1.0, 1.0, 1.0] {
                if let Some(arr) = sg.get("diffuseFactor").and_then(|v| v.as_array()) {
                    let r = arr
                        .first()
                        .and_then(|v| v.as_f64())
                        .map(|x| x as f32)
                        .unwrap_or(1.0);
                    let g = arr
                        .get(1)
                        .and_then(|v| v.as_f64())
                        .map(|x| x as f32)
                        .unwrap_or(1.0);
                    let b = arr
                        .get(2)
                        .and_then(|v| v.as_f64())
                        .map(|x| x as f32)
                        .unwrap_or(1.0);
                    let a = arr
                        .get(3)
                        .and_then(|v| v.as_f64())
                        .map(|x| x as f32)
                        .unwrap_or(1.0);
                    base_color = [r, g, b, a];
                }
            }
            // Glossiness → roughness inversion when no metalRough roughness
            // was supplied. glTF default for both metallicFactor and
            // roughnessFactor is 1.0 — `pbr_block_empty` lets us tell apart
            // "explicitly default" from "missing entirely".
            if pbr_block_empty {
                if let Some(g) = sg.get("glossinessFactor").and_then(|v| v.as_f64()) {
                    roughness = 1.0 - (g as f32);
                }
                // Spec-gloss materials don't carry a metallic concept — almost
                // every surface authored this way is a dielectric. Force
                // metallic to 0 so we don't render every untextured wall as a
                // mirror under HDR lighting (which is what
                // `metallicFactor`'s default of 1.0 produces).
                metallic = 0.0;
            }
        }

        let alpha_mode = match mat
            .get("alphaMode")
            .and_then(|v| v.as_str())
            .unwrap_or("OPAQUE")
        {
            "BLEND" => ExtractedAlphaMode::Blend,
            "MASK" => ExtractedAlphaMode::Mask,
            _ => ExtractedAlphaMode::Opaque,
        };

        let alpha_cutoff = mat
            .get("alphaCutoff")
            .and_then(|v| v.as_f64())
            .map(|x| x as f32)
            .unwrap_or(0.5);

        let double_sided = mat
            .get("doubleSided")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // ── Extended PBR: KHR_materials_* extensions ──────────────────────
        // These carry the channels beyond base metal-rough that real-world
        // glTF/GLB exports rely on (car paint, glass, brushed metal, …). The
        // output node already has matching pins; we just have to read them.
        let exts = mat.get("extensions");
        let ext = |name: &str| exts.and_then(|e| e.get(name));
        let ext_f32 = |block: Option<&serde_json::Value>, key: &str, default: f32| -> f32 {
            block
                .and_then(|b| b.get(key))
                .and_then(|v| v.as_f64())
                .map(|x| x as f32)
                .unwrap_or(default)
        };

        let legacy_texture = |key: &str| -> Option<String> {
            texture_info_uri(ext(crate::glb_build::RENZORA_LEGACY_EXT).and_then(|l| l.get(key)))
        };

        let clearcoat_ext = ext("KHR_materials_clearcoat");
        let transmission_ext = ext("KHR_materials_transmission");
        let volume_ext = ext("KHR_materials_volume");
        let ior_ext = ext("KHR_materials_ior");
        let specular_ext = ext("KHR_materials_specular");
        let anisotropy_ext = ext("KHR_materials_anisotropy");
        let emissive_strength_ext = ext("KHR_materials_emissive_strength");

        let attenuation_color = volume_ext
            .and_then(|v| v.get("attenuationColor"))
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                let r = arr.first()?.as_f64()? as f32;
                let g = arr.get(1)?.as_f64()? as f32;
                let b = arr.get(2)?.as_f64()? as f32;
                Some([r, g, b])
            })
            .unwrap_or([1.0, 1.0, 1.0]);

        let advanced = renzora::core::PbrAdvanced {
            clearcoat: ext_f32(clearcoat_ext, "clearcoatFactor", 0.0),
            clearcoat_roughness: ext_f32(clearcoat_ext, "clearcoatRoughnessFactor", 0.0),
            clearcoat_texture: texture_info_uri(
                clearcoat_ext.and_then(|c| c.get("clearcoatTexture")),
            ),
            clearcoat_roughness_texture: texture_info_uri(
                clearcoat_ext.and_then(|c| c.get("clearcoatRoughnessTexture")),
            ),
            clearcoat_normal_texture: texture_info_uri(
                clearcoat_ext.and_then(|c| c.get("clearcoatNormalTexture")),
            ),
            specular_transmission: ext_f32(transmission_ext, "transmissionFactor", 0.0),
            transmission_texture: texture_info_uri(
                transmission_ext.and_then(|t| t.get("transmissionTexture")),
            ),
            diffuse_transmission: 0.0,
            thickness: ext_f32(volume_ext, "thicknessFactor", 0.0),
            thickness_texture: texture_info_uri(volume_ext.and_then(|v| v.get("thicknessTexture"))),
            ior: ext_f32(ior_ext, "ior", 1.5),
            attenuation_distance: volume_ext
                .and_then(|v| v.get("attenuationDistance"))
                .and_then(|v| v.as_f64())
                .map(|x| x as f32)
                .unwrap_or(1.0e37),
            attenuation_color,
            anisotropy_strength: ext_f32(anisotropy_ext, "anisotropyStrength", 0.0),
            anisotropy_rotation: ext_f32(anisotropy_ext, "anisotropyRotation", 0.0),
            anisotropy_texture: texture_info_uri(
                anisotropy_ext.and_then(|a| a.get("anisotropyTexture")),
            ),
            // KHR specularFactor is 0..1 scaling dielectric specular; Bevy's
            // reflectance default of 0.5 corresponds to specularFactor 1.0.
            reflectance: ext_f32(specular_ext, "specularFactor", 1.0) * 0.5,
            unlit: ext("KHR_materials_unlit").is_some(),
        };

        // KHR_materials_emissive_strength scales the emissive factor (HDR bloom
        // emitters author values > 1 here).
        let strength = ext_f32(emissive_strength_ext, "emissiveStrength", 1.0);
        let emissive = [
            emissive[0] * strength,
            emissive[1] * strength,
            emissive[2] * strength,
        ];

        out.push(ExtractedPbrMaterial {
            name,
            base_color,
            metallic,
            roughness,
            emissive,
            base_color_texture,
            normal_texture,
            metallic_roughness_texture,
            roughness_texture: legacy_texture("roughnessTexture"),
            metallic_texture: legacy_texture("metallicTexture"),
            emissive_texture,
            occlusion_texture,
            specular_glossiness_texture,
            // glTF has no separate opacity, specular, roughness or metallic
            // map — alpha lives in base colour, specular is a scalar, and
            // roughness/metallic share one packed texture. The converters park
            // the separate versions in a vendor extension so a material still
            // survives the round trip through the intermediate GLB.
            opacity_texture: legacy_texture("opacityTexture"),
            specular_texture: legacy_texture("specularTexture"),
            advanced,
            alpha_mode,
            alpha_cutoff,
            double_sided,
        });
    }
    out
}

/// Parse a GLB, pull every image out of it, and rewrite the image entries to
/// reference the files we write beside the model.
///
/// Images arrive two ways. A glTF/GLB source embeds them in the BIN chunk. A
/// converter-produced GLB (FBX, OBJ, USD, Collada, …) instead points each image
/// at the file it found on disk next to the source model, via an absolute
/// `uri` — those converters have format-specific rules for locating a texture,
/// so resolution stays with them and everything after it is shared.
///
/// Returns the rewritten GLB bytes, the extracted texture list, and any
/// non-fatal warnings. On fatal parse failure returns an error and the caller
/// falls back to passthrough.
fn extract_glb_textures(
    glb_bytes: &[u8],
    roles: &[TextureRole],
    settings: &ImportSettings,
    progress: &ProgressFn,
) -> Result<(Vec<u8>, Vec<ExtractedTexture>, Vec<String>), String> {
    let glb = gltf::Glb::from_slice(glb_bytes).map_err(|e| format!("parse GLB: {}", e))?;

    let json_slice = glb.json.as_ref();
    let bin_slice: Option<&[u8]> = glb.bin.as_deref();

    let mut root: gltf_json::Root =
        serde_json::from_slice(json_slice).map_err(|e| format!("parse GLB JSON: {}", e))?;

    if root.images.is_empty() {
        return Ok((glb_bytes.to_vec(), Vec::new(), Vec::new()));
    }

    let mut warnings = Vec::new();
    let mut extracted: Vec<ExtractedTexture> = Vec::new();
    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Decide one resolution cap for the whole model before writing anything.
    // A per-texture cap is no protection against a scene that stays under it
    // several hundred times over — see [`fit_texture_budget`].
    let max_size = fit_texture_budget(&root.images, settings.texture_max_size, &mut warnings);

    // ── Phase 1 (serial): pull each embedded image out of the BIN chunk,
    // rewrite its URI, emit the original bytes, and queue a bake job. The
    // GLB-JSON mutation and name dedup must stay single-threaded; only the
    // expensive bake is parallelized below.
    let mut jobs: Vec<BakeJob> = Vec::new();
    for (i, image) in root.images.iter_mut().enumerate() {
        let role = roles.get(i).copied().unwrap_or(TextureRole::Color);

        // An external reference: the converter already located the file, so
        // the work here is naming it, deciding its output format, and pointing
        // the GLB at where it will land.
        if let Some(path) = image.uri.as_deref().map(std::path::PathBuf::from) {
            if !path.is_absolute() {
                // Already a model-relative URI — a previous pass wrote it, or
                // the source authored it that way. Leave it alone.
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("texture");
            let name = unique_name(&sanitize_texture_name(stem), &mut used_names);

            match external_texture_outputs(&path, &name, role, max_size) {
                Ok((uri, outputs)) => {
                    image.uri = Some(uri);
                    image.mime_type = None;
                    extracted.extend(outputs);
                }
                Err(e) => warnings.push(format!("texture '{}': {}", path.display(), e)),
            }
            continue;
        }

        let Some(buffer_view_idx) = image.buffer_view.take() else {
            continue;
        };
        let Some(bin) = bin_slice else {
            warnings.push(format!(
                "image {}: bufferView {} but GLB has no BIN chunk",
                i,
                buffer_view_idx.value()
            ));
            continue;
        };

        let view = match root.buffer_views.get(buffer_view_idx.value()) {
            Some(v) => v,
            None => {
                warnings.push(format!(
                    "image {}: bufferView {} out of range",
                    i,
                    buffer_view_idx.value()
                ));
                continue;
            }
        };
        let byte_offset = view.byte_offset.map(|o| o.0 as usize).unwrap_or(0);
        let byte_length = view.byte_length.0 as usize;
        let end = byte_offset + byte_length;
        if end > bin.len() {
            warnings.push(format!(
                "image {}: bufferView range {}..{} exceeds BIN size {}",
                i,
                byte_offset,
                end,
                bin.len()
            ));
            continue;
        }
        let raw = &bin[byte_offset..end];

        let name = unique_name(&format!("image_{}", i), &mut used_names);

        // Point the GLB at the `.rmip` and write only that. This used to
        // externalize the original PNG/JPEG as well, on the belief that a
        // `.rmip` URI would trip a settings-type mismatch in Bevy's GLB
        // loader. It does not: `RmipAssetLoader` declares
        // `Settings = ImageLoaderSettings` precisely so the GLB loader can
        // route a `.rmip` through it. Writing both meant every texture landed
        // on disk twice and the heavy source image was decoded and uploaded at
        // load only to be thrown away.
        image.uri = Some(format!("textures/{name}.rmip"));
        image.mime_type = None;

        jobs.push(BakeJob {
            raw: raw.to_vec(),
            name,
            role,
        });
    }

    // ── Phase 2 (parallel): bake every queued texture across all cores.
    // BC compression is the import-time bottleneck, so this is where the
    // wall-clock win comes from. Progress is reported as each completes.
    extracted.extend(bake_jobs_parallel(jobs, settings, progress, &mut warnings));

    if extracted.is_empty() {
        return Ok((glb_bytes.to_vec(), Vec::new(), warnings));
    }

    let new_json = root
        .to_vec()
        .map_err(|e| format!("re-serialize GLB JSON: {}", e))?;
    let new_glb = pack_glb(&new_json, bin_slice);
    Ok((new_glb, extracted, warnings))
}

/// Reserve `base`, suffixing it until it doesn't collide.
fn unique_name(base: &str, used: &mut std::collections::HashSet<String>) -> String {
    let mut name = base.to_string();
    let mut n = 1;
    while used.contains(&name) {
        n += 1;
        name = format!("{}_{}", base, n);
    }
    used.insert(name.clone());
    name
}

/// How much GPU memory one model's textures may claim before the importer
/// reduces their resolution.
///
/// `ImportSettings::texture_max_size` caps a *single* texture, which is no
/// protection against a scene that stays under it several hundred times over: a
/// street exterior with 337 separate 2048² maps sits at 970 MB with every one of
/// them inside a 2048 cap. Nothing downstream saves it either — the distance
/// tier swap in `renzora_engine::texture_stream` only runs while world
/// streaming is active, which excludes the editor's edit mode, so in the editor
/// the whole set is resident at once. The result is
/// `Caught rendering error: Out of Memory`, followed by a cascade of invalid
/// buffers as every later allocation fails too.
///
/// 512 MB leaves headroom on an 8 GB card for the mesh, shadow maps, GI and
/// post-process targets, and is far above what an ordinary prop or character
/// needs — this only engages for scene-sized imports.
const TEXTURE_BUDGET_BYTES: usize = 512 * 1024 * 1024;

/// Smallest cap the budget is allowed to impose. Below this, textures are mush
/// and the import has bigger problems than memory.
const MIN_TEXTURE_SIZE: u32 = 256;

/// Pick the largest cap at or below `requested` whose total fits
/// [`TEXTURE_BUDGET_BYTES`].
///
/// Only externally-referenced DDS is measured: its header states the exact
/// on-GPU size at any cap, so this is arithmetic rather than a guess, and it's
/// where the gigabytes actually come from — an embedded PNG set large enough to
/// matter would have made the source file unopenable long before it got here.
///
/// Halving the cap quarters the data, so this converges in a step or two.
/// Returns `requested` unchanged when the set already fits.
fn fit_texture_budget(
    images: &[gltf_json::Image],
    requested: u32,
    warnings: &mut Vec<String>,
) -> u32 {
    let described: Vec<renzora_rmip::dds::Description> = images
        .iter()
        .filter_map(|image| {
            let path = std::path::PathBuf::from(image.uri.as_deref()?);
            if !path.is_absolute() {
                return None;
            }
            renzora_rmip::dds::probe(&read_file_header(&path)?, false).ok()
        })
        .collect();
    let cap = choose_texture_cap(&described, requested);
    if cap < requested {
        warnings.push(format!(
            "texture set is too large for the {} MB budget at {}px; imported at {}px instead",
            TEXTURE_BUDGET_BYTES / (1024 * 1024),
            requested,
            cap,
        ));
    }
    cap
}

/// The arithmetic half of [`fit_texture_budget`], split out so it can be tested
/// without a directory full of textures.
///
/// Never goes below [`MIN_TEXTURE_SIZE`] — a set that can't fit even there is
/// left oversized rather than ground down to nothing.
fn choose_texture_cap(described: &[renzora_rmip::dds::Description], requested: u32) -> u32 {
    if described.is_empty() {
        return requested;
    }
    let total_at = |cap: u32| -> usize { described.iter().map(|d| d.size_at(cap)).sum() };

    let mut cap = requested;
    while cap > MIN_TEXTURE_SIZE && total_at(cap) > TEXTURE_BUDGET_BYTES {
        cap = (cap / 2).max(MIN_TEXTURE_SIZE);
    }
    cap
}

/// Read the first 256 bytes of a file — enough for any image container header
/// we classify on, without pulling a multi-megabyte texture into memory.
fn read_file_header(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = vec![0u8; 256];
    let mut filled = 0;
    while filled < header.len() {
        match file.read(&mut header[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => return None,
        }
    }
    header.truncate(filled);
    Some(header)
}

/// The URI the GLB should reference, plus the files to write for it.
type TextureOutputs = (String, Vec<ExtractedTexture>);

/// Decide what an externally-referenced texture becomes on disk, and what URI
/// the GLB should point at.
///
/// One file comes out: the `.rmip`, which the GLB itself points at.
///
/// This used to write a second copy in the source's own format, on the belief
/// that the GLB's materials needed a format "Bevy's own image loader reads" to
/// resolve. That is not true for `.rmip` — [`renzora_rmip::RmipAssetLoader`]
/// declares `Settings = ImageLoaderSettings` precisely so Bevy's GLB loader can
/// route a `.rmip` URI through it, which is what `bake_external_images` below
/// has always relied on.
///
/// Keeping the companion was actively harmful. It doubled the texture footprint
/// exactly (231 MB of pure duplication on a scene like Bistro), and it made the
/// GLB resolve through Bevy's DDS loader — which has no mapping for `ATI2`, the
/// FourCC every DCC tool writes tangent-space normals as. Those images failed to
/// load and the model rendered untextured.
///
/// A block-compressed DDS takes a shortcut. `.rmip` stores exactly the same BC
/// block formats, so it's repacked rather than baked — the blocks are copied
/// across and clamping drops whole mip levels. Round-tripping through RGBA
/// would re-quantize every block for a worse result, take minutes on a large
/// set, and fail outright on `ATI2`, which is how DCC tools write tangent-space
/// normals and which the `image` crate cannot decode at all.
///
/// Neither output is buffered: both carry the source path and do their work as
/// they're written, one file at a time.
fn external_texture_outputs(
    path: &Path,
    name: &str,
    role: TextureRole,
    max_size: u32,
) -> Result<TextureOutputs, String> {
    let header = read_file_header(path).ok_or_else(|| "unreadable".to_string())?;
    let native = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_else(|| sniff_image_extension(&header).to_string());

    if renzora_rmip::dds::probe(&header, matches!(role, TextureRole::Color)).is_ok() {
        return Ok((
            format!("textures/{}.rmip", name),
            vec![ExtractedTexture {
                name: name.to_string(),
                extension: "rmip".into(),
                source: TextureSource::DdsToRmip {
                    path: path.to_path_buf(),
                    srgb: matches!(role, TextureRole::Color),
                    max_size,
                },
            }],
        ));
    }

    // Anything else is decoded and baked. `native` is unused now that the GLB
    // points at the `.rmip` rather than a copy of the source file.
    let _ = &native;
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let baked = renzora_rmip::bake::bake_image(
        &raw,
        renzora_rmip::bake::BakeParams {
            role,
            compress: true,
            high_quality: true,
            max_size,
        },
    )
    .map_err(|e| format!("bake .rmip failed: {e}"))?;

    Ok((
        format!("textures/{}.rmip", name),
        vec![ExtractedTexture {
            name: name.to_string(),
            extension: "rmip".to_string(),
            source: TextureSource::Embedded(baked),
        }],
    ))
}

/// Magic-byte sniff for embedded image bytes when the GLB doesn't carry a
/// MIME type. Mirrors the FBX-side helper so both extractors agree on
/// which extension to write.
fn sniff_image_extension(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if data.starts_with(b"DDS ") {
        "dds"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "gif"
    } else if data.starts_with(b"BM") {
        "bmp"
    } else if data.starts_with(&[0x52, 0x49, 0x46, 0x46]) && data.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        "bin"
    }
}

/// GLTF files: read the JSON and all referenced buffers/images, pack into GLB.
///
/// For now, we embed the JSON GLTF as a GLB by reading all external resources
/// and packing them into a single binary buffer.
pub fn convert_gltf(
    path: &Path,
    settings: &ImportSettings,
    progress: &ProgressFn,
) -> Result<ImportResult, ImportError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let json_str = std::fs::read_to_string(path)
        .map_err(|e| ImportError::ParseError(format!("failed to read GLTF: {}", e)))?;

    let mut root: gltf_json::Root = serde_json::from_str(&json_str)
        .map_err(|e| ImportError::ParseError(format!("invalid GLTF JSON: {}", e)))?;

    // Resolve every external/data-URI buffer the .gltf references (the sibling
    // `.bin` for Sponza) and inline them into a single GLB binary chunk. We
    // record each source buffer's start offset so buffer views can be remapped
    // onto the consolidated buffer; without this the output GLB would still
    // point at the now-missing external `.bin` and fail to load.
    let mut bin_data = Vec::new();
    let mut warnings = Vec::new();
    let mut buffer_offsets: Vec<usize> = Vec::with_capacity(root.buffers.len());

    for buffer in &root.buffers {
        // Keep each buffer 4-byte aligned so accessor component alignment that
        // held within the source buffer still holds after concatenation.
        while bin_data.len() % 4 != 0 {
            bin_data.push(0);
        }
        buffer_offsets.push(bin_data.len());

        match buffer.uri.as_deref() {
            Some(uri) if uri.starts_with("data:") => {
                if let Some(base64_start) = uri.find(";base64,") {
                    let decoded = base64_decode(&uri[base64_start + 8..]).map_err(|e| {
                        ImportError::ParseError(format!("invalid base64 in buffer URI: {}", e))
                    })?;
                    bin_data.extend_from_slice(&decoded);
                } else {
                    warnings.push("unsupported data URI scheme in buffer".to_string());
                }
            }
            Some(uri) => {
                // External file, resolved relative to the .gltf's folder.
                let buf_path = parent.join(uri);
                let data = std::fs::read(&buf_path).map_err(|e| {
                    ImportError::ParseError(format!(
                        "failed to read buffer '{}': {}",
                        buf_path.display(),
                        e
                    ))
                })?;
                bin_data.extend_from_slice(&data);
            }
            // A uri-less buffer in a .gltf would refer to a GLB BIN chunk that
            // doesn't exist here; nothing to inline.
            None => {}
        }
    }

    // Repoint every buffer view at the single consolidated buffer (index 0),
    // shifting its offset by where its original buffer landed, then collapse
    // the buffer list to one inline buffer with no URI.
    if !bin_data.is_empty() {
        for view in &mut root.buffer_views {
            let base = buffer_offsets.get(view.buffer.value()).copied().unwrap_or(0);
            let old = view.byte_offset.map(|o| o.0).unwrap_or(0);
            view.byte_offset = Some(gltf_json::validation::USize64(base as u64 + old));
            view.buffer = gltf_json::Index::new(0);
        }
        let buf0 = &mut root.buffers[0];
        buf0.uri = None;
        buf0.byte_length = gltf_json::validation::USize64(bin_data.len() as u64);
        root.buffers.truncate(1);
    }

    // Bake the external (or data-URI) images this glTF references into
    // mipmapped, block-compressed `.rmip` files and repoint the GLB at them.
    // Without this the loose 4K PNGs load raw — the exact bottleneck that
    // makes scenes like Sponza crawl.
    let mut extracted_textures = Vec::new();
    if settings.extract_textures && !root.images.is_empty() {
        let roles = serde_json::from_str::<serde_json::Value>(&json_str)
            .ok()
            .map(|v| scan_image_roles(&v))
            .unwrap_or_default();
        let (texs, warns) = bake_external_images(&mut root, parent, &roles, settings, progress);
        extracted_textures = texs;
        warnings.extend(warns);
    }

    // Imported cameras are authored viewpoints with no use in-engine; drop
    // them so no rogue active renderer spawns from the model.
    strip_cameras(&mut root);

    // Build GLB from JSON + binary chunk
    let json_bytes = root.to_vec().map_err(|e| {
        ImportError::ConversionError(format!("failed to serialize GLTF JSON: {}", e))
    })?;

    let glb_bytes = pack_glb(
        &json_bytes,
        if bin_data.is_empty() {
            None
        } else {
            Some(&bin_data)
        },
    );

    let extracted_materials = if settings.extract_materials {
        extract_glb_materials(&glb_bytes)
    } else {
        Vec::new()
    };

    Ok(ImportResult {
        glb_bytes: crate::glb_compat::strip_unsupported_extensions(&glb_bytes),
        warnings,
        extracted_textures,
        extracted_materials,
    })
}

/// Bake every external- or data-URI image referenced by a glTF document into
/// a `.rmip` (mipmapped + GPU-block-compressed) sitting under `textures/`,
/// and repoint each image's URI at the baked file.
///
/// Unlike the embedded-GLB path — which must externalize the original bytes
/// so Bevy's GLB loader can decode them — here we point the GLB straight at
/// the `.rmip`. Bevy's GLB loader routes those URIs through `RmipAssetLoader`
/// (its `Settings` type is `ImageLoaderSettings` precisely so this works), so
/// the heavy source PNGs are never decoded or uploaded at runtime: no load
/// stall, no transient uncompressed-VRAM spike. Materials are rewritten to
/// the same `.rmip` URIs by `extract_glb_materials`.
fn bake_external_images(
    root: &mut gltf_json::Root,
    parent: &Path,
    roles: &[TextureRole],
    settings: &ImportSettings,
    progress: &ProgressFn,
) -> (Vec<ExtractedTexture>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    // ── Phase 1 (serial): resolve each image's source bytes, dedup its name,
    // repoint the GLB URI at the (about-to-be-baked) `.rmip`, and queue a job.
    let mut jobs: Vec<BakeJob> = Vec::new();
    for (i, image) in root.images.iter_mut().enumerate() {
        let Some(uri) = image.uri.clone() else {
            // Already an embedded bufferView image — the .gltf path doesn't
            // inline those; the .glb path handles them separately.
            continue;
        };

        // Resolve the source bytes from a data URI or a sibling file.
        let raw = if uri.starts_with("data:") {
            match uri.find(";base64,") {
                Some(b) => match base64_decode(&uri[b + 8..]) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        warnings.push(format!("image {i}: bad data URI: {e}"));
                        continue;
                    }
                },
                None => {
                    warnings.push(format!("image {i}: unsupported data URI scheme"));
                    continue;
                }
            }
        } else {
            let p = parent.join(&uri);
            match std::fs::read(&p) {
                Ok(bytes) => bytes,
                Err(e) => {
                    warnings.push(format!("image {i}: read '{}': {e}", p.display()));
                    continue;
                }
            }
        };

        // Derive a stable, unique, filesystem-safe stem from the source name.
        let stem = Path::new(&uri)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("image");
        let mut name = sanitize_texture_name(stem);
        if name.is_empty() {
            name = format!("image_{i}");
        }
        let base = name.clone();
        let mut n = 1;
        while used_names.contains(&name) {
            n += 1;
            name = format!("{base}_{n}");
        }
        used_names.insert(name.clone());

        image.uri = Some(format!("textures/{name}.rmip"));
        image.mime_type = None;

        let role = roles.get(i).copied().unwrap_or(TextureRole::Color);
        jobs.push(BakeJob { raw, name, role });
    }

    // ── Phase 2 (parallel): bake every queued texture across all cores.
    let extracted = bake_jobs_parallel(jobs, settings, progress, &mut warnings);
    (extracted, warnings)
}

/// One texture queued for baking. Collected serially, baked in parallel.
struct BakeJob {
    /// Encoded source image bytes (PNG/JPG/etc).
    raw: Vec<u8>,
    /// Output stem (no extension); the `.rmip` is written as `<name>.rmip`.
    name: String,
    /// Semantic role driving sRGB/linear + GPU format selection.
    role: TextureRole,
}

/// Bake a batch of queued textures in parallel across the rayon pool,
/// reporting `(done, total, name)` progress as each finishes. Returns the
/// resulting `.rmip` textures; per-texture bake failures are pushed onto
/// `warnings` rather than aborting the whole import.
fn bake_jobs_parallel(
    jobs: Vec<BakeJob>,
    settings: &ImportSettings,
    progress: &ProgressFn,
    warnings: &mut Vec<String>,
) -> Vec<ExtractedTexture> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let total = jobs.len();
    if total == 0 {
        return Vec::new();
    }

    // Shared completion counter so progress reflects work finished, not the
    // (unordered) index rayon happens to schedule.
    let counter = AtomicUsize::new(0);
    let baked: Vec<(String, Result<Vec<u8>, String>)> = jobs
        .into_par_iter()
        .map(|job| {
            let res = renzora_rmip::bake::bake_image(&job.raw, settings.bake_params(job.role));
            let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
            progress(done, total, &job.name);
            (job.name, res)
        })
        .collect();

    let mut out = Vec::with_capacity(baked.len());
    for (name, res) in baked {
        match res {
            Ok(data) => out.push(ExtractedTexture {
                name,
                extension: "rmip".to_string(),
                source: TextureSource::Embedded(data),
            }),
            Err(e) => warnings.push(format!("texture '{name}': bake .rmip failed: {e}")),
        }
    }
    out
}

/// Sanitize a texture filename stem: keep alphanumerics, `_`, `-`, `.`;
/// replace anything else with `_`.
fn sanitize_texture_name(stem: &str) -> String {
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Simple base64 decoder (no external dep needed).
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 char: {}", c as char)),
        }
    }

    let input: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'\n' && b != b'\r' && b != b' ')
        .collect();
    let mut out = Vec::with_capacity(input.len() * 3 / 4);

    for chunk in input.chunks(4) {
        let len = chunk.iter().filter(|&&b| b != b'=').count();
        if len < 2 {
            break;
        }
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        out.push((a << 2) | (b >> 4));
        if len > 2 {
            let c = val(chunk[2])?;
            out.push((b << 4) | (c >> 2));
            if len > 3 {
                let d = val(chunk[3])?;
                out.push((c << 6) | d);
            }
        }
    }

    Ok(out)
}

/// Remove every camera from a glTF document.
///
/// Imported model cameras are authored viewpoints that have no use once the
/// asset is brought into the engine — the editor and scene cameras own the
/// view, and Bevy marks the first embedded camera active, so it silently
/// renders the whole scene a second time. We drop the `cameras` array and
/// clear every node's `camera` reference so imports never carry renderers;
/// the user adds cameras through the engine, which sets the proper markers.
fn strip_cameras(root: &mut gltf_json::Root) {
    root.cameras.clear();
    for node in &mut root.nodes {
        node.camera = None;
    }
}

/// Strip cameras from already-serialized GLB bytes. Re-parses, removes the
/// cameras, and repacks with the original BIN chunk; returns the input
/// unchanged if there's nothing to strip or parsing fails.
fn strip_cameras_from_glb(bytes: Vec<u8>) -> Vec<u8> {
    let Ok(glb) = gltf::Glb::from_slice(&bytes) else {
        return bytes;
    };
    let Ok(mut root) = serde_json::from_slice::<gltf_json::Root>(&glb.json) else {
        return bytes;
    };
    if root.cameras.is_empty() && root.nodes.iter().all(|n| n.camera.is_none()) {
        return bytes; // nothing to strip — avoid a needless repack
    }
    strip_cameras(&mut root);
    let Ok(json) = root.to_vec() else {
        return bytes;
    };
    pack_glb(&json, glb.bin.as_deref())
}

/// Pack JSON and optional binary data into a GLB container.
pub(crate) fn pack_glb(json: &[u8], bin: Option<&[u8]>) -> Vec<u8> {
    // Pad JSON to 4-byte boundary with spaces
    let json_pad = (4 - (json.len() % 4)) % 4;
    let json_chunk_len = json.len() + json_pad;

    let bin_chunk_len = if let Some(b) = bin {
        let pad = (4 - (b.len() % 4)) % 4;
        b.len() + pad
    } else {
        0
    };

    let total_len = 12 // header
        + 8 + json_chunk_len // JSON chunk header + data
        + if bin.is_some() { 8 + bin_chunk_len } else { 0 }; // BIN chunk

    let mut out = Vec::with_capacity(total_len);

    // GLB header
    out.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic "glTF"
    out.extend_from_slice(&2u32.to_le_bytes()); // version 2
    out.extend_from_slice(&(total_len as u32).to_le_bytes()); // total length

    // JSON chunk
    out.extend_from_slice(&(json_chunk_len as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    out.extend_from_slice(json);
    out.extend(std::iter::repeat_n(b' ', json_pad));

    // BIN chunk
    if let Some(b) = bin {
        let bin_pad = (4 - (b.len() % 4)) % 4;
        out.extend_from_slice(&(bin_chunk_len as u32).to_le_bytes());
        out.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
        out.extend_from_slice(b);
        out.extend(std::iter::repeat_n(0, bin_pad));
    }

    out
}

/// Whether a `pbrMetallicRoughness` block carries no information beyond the
/// glTF defaults.
///
/// Used to tell "this material has no metal-rough data" from "this material is
/// deliberately a rough metal", which decides whether a
/// `KHR_materials_pbrSpecularGlossiness` material may override it.
///
/// The distinction is not whether the block is *present* but whether it says
/// anything. This previously tested for a literally empty object, and most
/// spec-gloss exporters do not write one — they write the defaults out in full:
///
/// ```json
/// "pbrMetallicRoughness": {
///   "baseColorFactor": [1,1,1,1], "metallicFactor": 1.0, "roughnessFactor": 1.0
/// }
/// ```
///
/// which is exactly "nothing", spelled at length. Reading that as authored
/// intent left every surface of such a model fully metallic — no diffuse
/// response at all, lit only by environment reflection, which renders a stone
/// interior as a flat wash of the sky. Two Sketchfab scans did that; a
/// conventional metal-rough model beside them was fine.
fn pbr_block_is_default(pbr: &serde_json::Value) -> bool {
    let Some(o) = pbr.as_object() else {
        return true;
    };
    if o.is_empty() {
        return true;
    }
    // A texture is information, whatever the factors say.
    if o.contains_key("baseColorTexture") || o.contains_key("metallicRoughnessTexture") {
        return false;
    }
    let is_default_f = |key: &str| {
        o.get(key)
            .map(|v| v.as_f64().is_some_and(|f| (f - 1.0).abs() < 1e-6))
            .unwrap_or(true)
    };
    if !is_default_f("metallicFactor") || !is_default_f("roughnessFactor") {
        return false;
    }
    let base_default = o
        .get("baseColorFactor")
        .map(|v| {
            v.as_array().is_some_and(|a| {
                a.len() == 4
                    && a.iter()
                        .all(|c| c.as_f64().is_some_and(|f| (f - 1.0).abs() < 1e-6))
            })
        })
        .unwrap_or(true);
    // Any other key (emissive strength, extensions on the block, …) is
    // information we should not talk over.
    let known = ["baseColorFactor", "metallicFactor", "roughnessFactor"];
    base_default && o.keys().all(|k| known.contains(&k.as_str()))
}

#[cfg(test)]
mod default_pbr_block_tests {
    use super::pbr_block_is_default;
    use serde_json::json;

    /// The shape every spec-gloss exporter in the wild actually writes: the
    /// defaults, spelled out. Reading this as authored intent is what made two
    /// scanned models render as mirrors.
    #[test]
    fn explicit_defaults_say_nothing() {
        assert!(pbr_block_is_default(&json!({
            "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
            "metallicFactor": 1.0,
            "roughnessFactor": 1.0
        })));
    }

    #[test]
    fn an_absent_or_empty_block_says_nothing() {
        assert!(pbr_block_is_default(&json!({})));
        assert!(pbr_block_is_default(&json!(null)));
    }

    /// A deliberately rough metal must survive: the whole point of the check is
    /// to tell it apart from a placeholder.
    #[test]
    fn authored_values_are_respected() {
        assert!(!pbr_block_is_default(&json!({ "metallicFactor": 0.0 })));
        assert!(!pbr_block_is_default(&json!({ "roughnessFactor": 0.4 })));
        assert!(!pbr_block_is_default(&json!({
            "baseColorFactor": [0.8, 0.2, 0.2, 1.0]
        })));
    }

    /// A texture is information whatever the factors say — a spec-gloss
    /// override would throw it away.
    #[test]
    fn a_texture_counts_as_authored() {
        assert!(!pbr_block_is_default(&json!({
            "metallicFactor": 1.0,
            "metallicRoughnessTexture": { "index": 0 }
        })));
        assert!(!pbr_block_is_default(&json!({
            "baseColorTexture": { "index": 2 }
        })));
    }

    /// Anything we do not recognise is information too. Better to leave a
    /// material alone than to talk over a field this code has never seen.
    #[test]
    fn an_unknown_key_counts_as_authored() {
        assert!(!pbr_block_is_default(&json!({
            "metallicFactor": 1.0,
            "extensions": { "SOMETHING_new": {} }
        })));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_rmip::{dds::Description, mip_count, RmipFormat};

    /// `n` BC1 textures of `size`² with a full mip chain.
    fn texture_set(n: usize, size: u32) -> Vec<Description> {
        (0..n)
            .map(|_| Description {
                format: RmipFormat::Bc1RgbaUnormSrgb,
                width: size,
                height: size,
                mips: mip_count(size, size),
            })
            .collect()
    }

    #[test]
    fn a_modest_texture_set_keeps_the_requested_size() {
        // Ten 2K maps is ~28 MB — nowhere near the budget, so nothing moves.
        assert_eq!(choose_texture_cap(&texture_set(10, 2048), 2048), 2048);
    }

    #[test]
    fn an_oversized_set_steps_down_until_it_fits() {
        // 337 × 2048² BC1 with mips ≈ 950 MB, over the 512 MB budget; one
        // halving quarters it to ~240 MB, which fits.
        let set = texture_set(337, 2048);
        let cap = choose_texture_cap(&set, 2048);
        assert_eq!(cap, 1024);

        let total: usize = set.iter().map(|d| d.size_at(cap)).sum();
        assert!(
            total <= TEXTURE_BUDGET_BYTES,
            "still over budget: {total} bytes"
        );
    }

    #[test]
    fn the_budget_never_reduces_below_the_floor() {
        // A set so large no sane cap fits it: stop at the floor rather than
        // grinding every texture down to nothing.
        assert_eq!(
            choose_texture_cap(&texture_set(100_000, 2048), 2048),
            MIN_TEXTURE_SIZE
        );
    }

    #[test]
    fn a_model_with_no_measurable_textures_is_left_alone() {
        // Embedded PNGs and other non-DDS sources aren't measured, so they
        // must not drag the cap down for everything else.
        assert_eq!(choose_texture_cap(&[], 2048), 2048);
    }

    // ─── Material round trip ────────────────────────────────────────────

    /// Materials survive the intermediate GLB only if the writer and this
    /// reader agree key for key. These lock that agreement down for the
    /// channels glTF has no standard home for, which are the ones that
    /// silently vanished before the converters started going through here.
    #[test]
    fn legacy_and_extended_channels_survive_the_glb_round_trip() {
        use crate::glb_build::{build_glb, MaterialBundle, PbrMaterialDef, TextureRef};

        let texture = |uri: &str| TextureRef {
            uri: uri.to_string(),
            embedded: None,
        };
        let bundle = MaterialBundle {
            textures: vec![
                texture("/tmp/base.png"),
                texture("/tmp/opacity.png"),
                texture("/tmp/specular.png"),
                texture("/tmp/rough.png"),
                texture("/tmp/metal.png"),
                texture("/tmp/coat.png"),
            ],
            materials: vec![PbrMaterialDef {
                name: "Antenna_Plastic".into(),
                base_color: [0.25, 0.5, 0.75, 1.0],
                base_color_texture: Some(0),
                normal_texture: None,
                metallic: 0.25,
                roughness: 0.6,
                emissive: [1.0, 0.5, 0.0],
                emissive_texture: None,
                occlusion_texture: None,
                opacity_texture: Some(1),
                specular_texture: Some(2),
                roughness_texture: Some(3),
                metallic_texture: Some(4),
                alpha: crate::glb_build::AlphaKind::Blend,
                double_sided: false,
                advanced: renzora::core::PbrAdvanced {
                    clearcoat: 0.8,
                    clearcoat_roughness: 0.15,
                    clearcoat_texture: Some("/tmp/coat.png".into()),
                    specular_transmission: 0.4,
                    ior: 1.7,
                    anisotropy_strength: 0.3,
                    reflectance: 0.25,
                    unlit: true,
                    ..Default::default()
                },
            }],
        };

        let positions = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let normals = [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let texcoords = [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0];
        let glb = build_glb(&positions, &normals, &texcoords, &[0, 1, 2], &bundle).unwrap();

        let out = extract_glb_materials(&glb);
        assert_eq!(out.len(), 1);
        let m = &out[0];

        assert_eq!(m.name, "Antenna_Plastic");
        assert_eq!(m.base_color, [0.25, 0.5, 0.75, 1.0]);
        assert_eq!(m.metallic, 0.25);
        assert_eq!(m.roughness, 0.6);
        assert_eq!(m.emissive, [1.0, 0.5, 0.0]);
        assert!(matches!(m.alpha_mode, ExtractedAlphaMode::Blend));

        // The reader rewrites every texture reference to the `.rmip` beside
        // it, so compare on the stem.
        assert_eq!(m.base_color_texture.as_deref(), Some("/tmp/base.rmip"));
        assert_eq!(m.opacity_texture.as_deref(), Some("/tmp/opacity.rmip"));
        assert_eq!(m.specular_texture.as_deref(), Some("/tmp/specular.rmip"));
        assert_eq!(m.roughness_texture.as_deref(), Some("/tmp/rough.rmip"));
        assert_eq!(m.metallic_texture.as_deref(), Some("/tmp/metal.rmip"));

        assert_eq!(m.advanced.clearcoat, 0.8);
        assert_eq!(m.advanced.clearcoat_roughness, 0.15);
        assert_eq!(m.advanced.clearcoat_texture.as_deref(), Some("/tmp/coat.rmip"));
        assert_eq!(m.advanced.specular_transmission, 0.4);
        assert_eq!(m.advanced.ior, 1.7);
        assert_eq!(m.advanced.anisotropy_strength, 0.3);
        // `specularFactor` is halved on the way in, doubled on the way out.
        assert!((m.advanced.reflectance - 0.25).abs() < 1e-6);
        assert!(m.advanced.unlit);
    }
}

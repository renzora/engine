//! GLTF/GLB passthrough — reads and re-exports (or copies) the file.

use std::path::Path;

use renzora_rmip::bake::TextureRole;

use crate::convert::{
    ExtractedAlphaMode, ExtractedPbrMaterial, ExtractedTexture, ImportError, ImportResult,
    ProgressFn,
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
        return Ok(ImportResult {
            glb_bytes: crate::glb_compat::strip_unsupported_extensions(&bytes),
            warnings: Vec::new(),
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

    let (rewritten, extracted_textures, warnings) =
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

    Ok(ImportResult {
        glb_bytes: crate::glb_compat::strip_unsupported_extensions(&rewritten),
        warnings,
        extracted_textures,
        extracted_materials,
    })
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
        // Detection: presence of the extension AND no explicit metalRough fields
        // (everything reads as the glTF default) — that's the unambiguous
        // "spec-gloss-only" case where we should override the metalRough
        // defaults rather than respect them.
        let spec_gloss = mat
            .get("extensions")
            .and_then(|e| e.get("KHR_materials_pbrSpecularGlossiness"));
        let pbr_block_empty = pbr
            .map(|p| p.as_object().is_none_or(|o| o.is_empty()))
            .unwrap_or(true);

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
            roughness_texture: None,
            metallic_texture: None,
            emissive_texture,
            occlusion_texture,
            specular_glossiness_texture,
            opacity_texture: None,
            specular_texture: None,
            advanced,
            alpha_mode,
            alpha_cutoff,
            double_sided,
        });
    }
    out
}

/// Parse a GLB, pull every `bufferView`-backed image out of the BIN chunk,
/// and rewrite those image entries to reference external URIs instead.
/// Returns the (possibly rewritten) GLB bytes, the extracted texture list,
/// and any non-fatal warnings. On fatal parse failure returns an error and
/// the caller falls back to passthrough.
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

    // ── Phase 1 (serial): pull each embedded image out of the BIN chunk,
    // rewrite its URI, emit the original bytes, and queue a bake job. The
    // GLB-JSON mutation and name dedup must stay single-threaded; only the
    // expensive bake is parallelized below.
    let mut jobs: Vec<BakeJob> = Vec::new();
    for (i, image) in root.images.iter_mut().enumerate() {
        // Skip images that already live as external files; nothing to do.
        if image.uri.is_some() {
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

        // Detect the source image format. The GLB references the texture
        // by URI under its original extension so Bevy's GLB loader can
        // decode it via its own image loader (anything else trips a
        // settings-type mismatch — Bevy hardcodes `ImageLoaderSettings`
        // for embedded URIs). Materials separately reference a `.rmip`
        // file under the same stem.
        let extension = match image.mime_type.as_ref().map(|m| m.0.as_str()) {
            Some("image/png") => "png",
            Some("image/jpeg") => "jpg",
            Some("image/webp") => "webp",
            _ => sniff_image_extension(raw),
        };

        let mut name = format!("image_{}", i);
        let mut n = 1;
        while used_names.contains(&name) {
            n += 1;
            name = format!("image_{}_{}", i, n);
        }
        used_names.insert(name.clone());

        // GLB references the original-extension file. Bevy loads this
        // through its own image loader and we discard the result later
        // (the resolver swaps StandardMaterial for GraphMaterial), but
        // the load has to *succeed* for Bevy not to flood the log with
        // settings-mismatch errors.
        image.uri = Some(format!("textures/{}.{}", name, extension));
        image.mime_type = None;

        // Original encoded bytes — what Bevy's GLB loader reads.
        extracted.push(ExtractedTexture {
            name: name.clone(),
            extension: extension.to_string(),
            data: raw.to_vec(),
        });

        let role = roles.get(i).copied().unwrap_or(TextureRole::Color);
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
                data,
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


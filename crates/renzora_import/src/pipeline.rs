//! The import pipeline as one call: convert, write, and announce.
//!
//! [`convert_to_glb`](crate::convert_to_glb) produces bytes, textures and
//! materials in memory and stops there, which is right — it knows nothing about
//! projects. This is the other half: where those go on disk, and the event that
//! turns each extracted material into a `.material` file.
//!
//! It lives here rather than beside one of its callers because it has two. A
//! model dropped into the viewport and a model installed from the marketplace
//! must arrive identically — same GLB, same `textures/`, same materials — and
//! the surest way to guarantee that is for there to be one function rather than
//! two that agree today.

use std::path::Path;

use renzora::bevy::prelude::*;

/// Run the import pipeline on `source`, write the result to `dest`, dump
/// extracted textures under `<model_dir>/textures/`, and fire one
/// `PbrMaterialExtracted` event per material so `renzora_shader::material`
/// writes a `.material` file per entry.
///
/// Logs and falls back to a plain file copy on failure — the GLB still loads
/// for the user, just without per-material editable graphs.
pub fn run_import_pipeline(
    world: &mut renzora::bevy::ecs::world::World,
    source: &Path,
    dest: &Path,
    model_dir: &Path,
    project_path: &Path,
) {
    use crate::{convert_to_glb, ImportSettings};

    // Skip mesh optimization for the drop path — these reorder triangle
    // buffers and are only meaningful for re-importing source files. The
    // drop pipeline is for getting an existing GLB into the project quickly.
    let settings = ImportSettings {
        optimize_vertex_cache: false,
        optimize_overdraw: false,
        optimize_vertex_fetch: false,
        ..Default::default()
    };

    let result = match convert_to_glb(source, &settings) {
        Ok(r) => r,
        Err(e) => {
            warn!(
                "[import] convert failed for {:?}: {}; falling back to plain copy",
                source, e
            );
            if source != dest {
                if let Err(ce) = std::fs::copy(source, dest) {
                    error!("[import] copy fallback failed: {}", ce);
                }
            }
            return;
        }
    };

    if let Err(e) = std::fs::write(dest, &result.glb_bytes) {
        error!("[import] write GLB to {:?}: {}", dest, e);
        return;
    }

    if !result.extracted_textures.is_empty() {
        let tex_dir = model_dir.join("textures");
        if let Err(e) = std::fs::create_dir_all(&tex_dir) {
            warn!("[import] create textures dir: {}", e);
        } else {
            for tex in &result.extracted_textures {
                let tex_path = tex_dir.join(format!("{}.{}", tex.name, tex.extension));
                if let Err(e) = tex.write_to(&tex_path) {
                    warn!("[import] write texture '{}': {}", tex.name, e);
                }
            }
        }
    }

    if !result.extracted_materials.is_empty() {
        let mat_dir = model_dir.join("materials");
        // Texture URIs from the converter are relative to the model folder
        // (e.g. `textures/diffuse.png`). The material observer wants
        // project-relative paths so the resolver can find them — prefix with
        // the model folder's location under the project root.
        let model_rel = model_dir
            .strip_prefix(project_path)
            .ok()
            .and_then(|p| p.to_str())
            .map(|s| s.replace('\\', "/"))
            .unwrap_or_default();
        let prefix = |uri: &Option<String>| -> Option<String> {
            uri.as_ref().map(|u| {
                if model_rel.is_empty() {
                    u.clone()
                } else {
                    format!("{}/{}", model_rel, u)
                }
            })
        };

        for mat in &result.extracted_materials {
            world.trigger(renzora::core::PbrMaterialExtracted {
                name: mat.name.clone(),
                output_dir: mat_dir.clone(),
                project_root: project_path.to_path_buf(),
                base_color: mat.base_color,
                metallic: mat.metallic,
                roughness: mat.roughness,
                emissive: mat.emissive,
                base_color_texture: prefix(&mat.base_color_texture),
                normal_texture: prefix(&mat.normal_texture),
                metallic_roughness_texture: prefix(&mat.metallic_roughness_texture),
                roughness_texture: prefix(&mat.roughness_texture),
                metallic_texture: prefix(&mat.metallic_texture),
                emissive_texture: prefix(&mat.emissive_texture),
                occlusion_texture: prefix(&mat.occlusion_texture),
                specular_glossiness_texture: prefix(&mat.specular_glossiness_texture),
                opacity_texture: prefix(&mat.opacity_texture),
                specular_texture: prefix(&mat.specular_texture),
                advanced: mat.advanced.rewrite_textures(prefix),
                alpha_mode: match mat.alpha_mode {
                    crate::ExtractedAlphaMode::Opaque => {
                        renzora::core::PbrAlphaMode::Opaque
                    }
                    crate::ExtractedAlphaMode::Mask => renzora::core::PbrAlphaMode::Mask,
                    crate::ExtractedAlphaMode::Blend => renzora::core::PbrAlphaMode::Blend,
                },
                alpha_cutoff: mat.alpha_cutoff,
                double_sided: mat.double_sided,
            });
        }
    }
}

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

/// Put every model file under `root` through the import pipeline, in place.
///
/// `root` may be a single file or a folder; both are walked the same way,
/// because the difference is the packaging a downloader happened to produce
/// rather than anything about the asset.
///
/// Each source becomes a sibling `.glb` with its textures in `textures/` and a
/// `.material` per material, which is the layout the engine loads. A source that
/// is **already** GLB still goes through: the converter is what extracts the
/// materials, and skipping it would leave a model that renders untextured.
///
/// Failures are logged per file and the rest continue. A model the pipeline
/// cannot read is still on disk exactly as it was written, which is strictly
/// better than reporting failure and leaving nothing.
///
/// # Why this is here and not beside a caller
///
/// It has three now, and they must agree exactly: a marketplace install, a model
/// dropped into the viewport, and anything draining
/// [`renzora::ImportInPlaceQueue`] — which is how a plugin that cannot link this
/// crate asks for an import. Three copies that agree today is the shape that
/// stops agreeing quietly.
pub fn import_tree_in_place(world: &mut renzora::bevy::ecs::world::World, root: &Path) {
    let Some(project) = world.get_resource::<renzora::CurrentProject>() else {
        warn!("[import] no project open, so {:?} cannot be imported", root);
        return;
    };
    let project_path = project.path.clone();

    let mut sources: Vec<std::path::PathBuf> = Vec::new();
    collect_models(root, &mut sources);
    if sources.is_empty() {
        return;
    }
    for source in sources {
        let Some(model_dir) = source.parent().map(Path::to_path_buf) else {
            continue;
        };
        let dest = source.with_extension("glb");
        run_import_pipeline(world, &source, &dest, &model_dir, &project_path);
        // The source is replaced by its GLB, not kept beside it — two files
        // describing one model is what makes an asset browser show it twice.
        if dest != source {
            if let Err(e) = std::fs::remove_file(&source) {
                warn!("[import] could not remove {}: {e}", source.display());
            }
        }
    }
}

/// Every importable file at or under `path`, depth-first.
fn collect_models(path: &Path, out: &mut Vec<std::path::PathBuf>) {
    if path.is_file() {
        if crate::detect_format(path).is_some() {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_models(&entry.path(), out);
    }
}

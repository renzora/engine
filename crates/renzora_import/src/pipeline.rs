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

/// Run the import pipeline on `source`, write the model beside `dest` (as
/// `<stem>.rvg`, see `model_file`), dump extracted textures under
/// `<model_dir>/textures/`, and fire one `PbrMaterialExtracted` event per
/// material so `renzora_shader::material` writes a `.material` file per entry.
///
/// Returns the file written, which the caller loads: `dest` with the
/// extension of what was stored. Logs and falls back to a plain file copy to
/// `dest` on failure, so the model still loads, just without per-material
/// editable graphs; `None` only when nothing could be written at all.
pub fn run_import_pipeline(
    world: &mut renzora::bevy::ecs::world::World,
    source: &Path,
    dest: &Path,
    model_dir: &Path,
    project_path: &Path,
) -> Option<std::path::PathBuf> {
    let done = convert_in_place(source, dest, model_dir, None)?;
    announce_materials(world, &done, project_path);
    Some(done.written)
}

/// What [`convert_in_place`] wrote, and the materials still to announce.
pub struct ConvertedInPlace {
    /// The model file written: `dest` with the extension of what was stored.
    pub written: std::path::PathBuf,
    pub model_dir: std::path::PathBuf,
    pub materials: Vec<crate::ExtractedPbrMaterial>,
}

/// The half of [`run_import_pipeline`] that needs no `World`: convert `source`,
/// write the model beside `dest` and its textures under `model_dir/textures/`.
/// Safe on any thread. The conversion runs in the companion when `remote` has
/// one connected (the same `model_convert` job the import window uses), and
/// here otherwise.
///
/// Falls back to a plain copy to `dest` when the conversion fails, as
/// [`run_import_pipeline`] does; `None` only when nothing could be written.
pub fn convert_in_place(
    source: &Path,
    dest: &Path,
    model_dir: &Path,
    remote: Option<&renzora::CompanionRemote>,
) -> Option<ConvertedInPlace> {
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

    let in_companion = remote.and_then(|remote| {
        let out_dir = std::env::temp_dir().join(format!(
            "renzora-in-place-{}-{:016x}",
            std::process::id(),
            renzora::SceneEntityId::new_random().0
        ));
        let args = crate::remote::ConvertArgs { source: source.to_path_buf(), settings: settings.clone(), out_dir: out_dir.clone() };
        let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let answer = remote.run(crate::remote::CONVERT_KIND, name, &args, &mut |_, _| {})?;
        let result = answer
            .and_then(|json| serde_json::from_str::<crate::remote::Converted>(&json).map_err(|e| e.to_string()))
            .and_then(|converted| converted.into_result().map_err(|e| e.to_string()));
        let _ = std::fs::remove_dir_all(&out_dir);
        match result {
            Ok(result) => Some(Ok(result)),
            // Converted here instead, which reports its own failure.
            Err(why) => {
                warn!("[import] the companion could not convert {:?} ({why}); converting it here", source);
                None
            }
        }
    });
    let result = match in_companion.unwrap_or_else(|| convert_to_glb(source, &settings)) {
        Ok(r) => r,
        Err(e) => {
            warn!(
                "[import] convert failed for {:?}: {}; falling back to plain copy",
                source, e
            );
            if source != dest {
                if let Err(ce) = std::fs::copy(source, dest) {
                    error!("[import] copy fallback failed: {}", ce);
                    return None;
                }
            }
            return Some(ConvertedInPlace { written: dest.to_path_buf(), model_dir: model_dir.to_path_buf(), materials: Vec::new() });
        }
    };

    let (bytes, extension) = crate::model_file(&result.glb_bytes);
    let dest = dest.with_extension(extension);
    if let Err(e) = std::fs::write(&dest, &bytes) {
        error!("[import] write model to {:?}: {}", dest, e);
        return None;
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

    Some(ConvertedInPlace { written: dest, model_dir: model_dir.to_path_buf(), materials: result.extracted_materials })
}

/// The half of [`run_import_pipeline`] that needs the `World`: one
/// `PbrMaterialExtracted` per material [`convert_in_place`] found, so a
/// `.material` file is written for each.
pub fn announce_materials(
    world: &mut renzora::bevy::ecs::world::World,
    done: &ConvertedInPlace,
    project_path: &Path,
) {
    let model_dir = done.model_dir.as_path();
    if !done.materials.is_empty() {
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

        for mat in &done.materials {
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

/// [`import_tree_in_place`] without the `World`, for a worker thread: convert
/// every model under `root` (in the companion when `remote` has one), write
/// the files, and replace each source with its model file. Each model's
/// materials are passed to `done` to be announced on the main thread with
/// [`announce_materials`].
///
/// A marketplace model install used to run the whole of this inside a command
/// on the main thread, and an FBX with a few 4K textures froze the editor for
/// seconds while it converted.
pub fn import_tree_in_background(
    root: &Path,
    remote: Option<&renzora::CompanionRemote>,
    done: &mut dyn FnMut(ConvertedInPlace),
) {
    let mut sources: Vec<std::path::PathBuf> = Vec::new();
    collect_models(root, &mut sources);
    for source in sources {
        let Some(model_dir) = source.parent().map(Path::to_path_buf) else {
            continue;
        };
        let Some(converted) = convert_in_place(&source, &source, &model_dir, remote) else {
            continue;
        };
        // As in `import_tree_in_place`: the model file replaces its source.
        if converted.written != source {
            if let Err(e) = std::fs::remove_file(&source) {
                warn!("[import] could not remove {}: {e}", source.display());
            }
        }
        done(converted);
    }
}

/// Put every model file under `root` through the import pipeline, in place.
///
/// `root` may be a single file or a folder; both are walked the same way,
/// because the difference is the packaging a downloader happened to produce
/// rather than anything about the asset.
///
/// Each source becomes a sibling `.rvg` with its textures in `textures/` and a
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
        let Some(written) = run_import_pipeline(world, &source, &source, &model_dir, &project_path) else {
            continue;
        };
        // The source is replaced by its model file, not kept beside it: two
        // files describing one model is what makes an asset browser show it
        // twice.
        if written != source {
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

//! Model import pipeline: converts foreign 3D formats (OBJ, FBX, USD/USDZ)
//! to GLB at import time so the existing glTF pipeline handles everything downstream.

pub mod glb_builder;
mod fbx;
mod obj;
mod usd;

use crate::core::{ConvertAxes, ModelImportSettings};
use glb_builder::GlbBuilder;
use std::path::{Path, PathBuf};

/// Check whether a file extension is a convertible (non-glTF) model format.
pub fn is_convertible_model(ext: &str) -> bool {
    matches!(ext, "obj" | "fbx" | "usd" | "usdz")
}

/// Convert a foreign model file to GLB, writing the result alongside the source.
///
/// Returns the path to the generated `.glb` file.
/// If a `.glb` already exists and the source hasn't changed, returns the cached path.
pub fn convert_to_glb(source: &Path, settings: &ModelImportSettings) -> Result<PathBuf, String> {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let glb_path = source.with_extension("glb");

    // Skip conversion if GLB already exists and is newer than source
    if glb_path.exists() {
        if let (Ok(src_meta), Ok(glb_meta)) =
            (std::fs::metadata(source), std::fs::metadata(&glb_path))
        {
            if let (Ok(src_time), Ok(glb_time)) = (src_meta.modified(), glb_meta.modified()) {
                if glb_time >= src_time {
                    log::info!(
                        "Using cached GLB: {}",
                        glb_path.display()
                    );
                    return Ok(glb_path);
                }
            }
        }
    }

    log::info!("Converting {} to GLB...", source.display());

    let mut builder = GlbBuilder::new();

    // Apply import settings
    if (settings.scale - 1.0).abs() > f32::EPSILON {
        builder.set_root_scale(settings.scale);
    }

    match settings.convert_axes {
        ConvertAxes::ZUpToYUp => {
            // Rotate -90° around X: quat = [-sin(45°), 0, 0, cos(45°)]
            builder.set_root_rotation([
                -std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
            ]);
        }
        ConvertAxes::YUpToZUp => {
            // Rotate +90° around X
            builder.set_root_rotation([
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
            ]);
        }
        ConvertAxes::FlipX => {
            builder.set_root_scale(-1.0); // This is a hack; proper flip would need per-vertex
        }
        ConvertAxes::FlipZ => {
            // Rotate 180° around Y
            builder.set_root_rotation([0.0, 1.0, 0.0, 0.0]);
        }
        ConvertAxes::None => {}
    }

    // Dispatch to format-specific converter
    match ext.as_str() {
        "obj" => obj::convert_obj(source, &mut builder)?,
        "fbx" => fbx::convert_fbx(source, &mut builder)?,
        "usd" | "usdz" => usd::convert_usd(source, &mut builder)?,
        _ => return Err(format!("Unsupported format: {}", ext)),
    }

    // Build GLB binary
    let glb_data = builder.build();

    // Write to disk
    std::fs::write(&glb_path, &glb_data)
        .map_err(|e| format!("Failed to write GLB: {}", e))?;

    log::info!(
        "Converted {} -> {} ({} bytes)",
        source.display(),
        glb_path.display(),
        glb_data.len()
    );

    Ok(glb_path)
}

/// Find the GLB sibling for a non-glTF model (for drag preview without re-converting).
pub fn find_glb_sibling(source: &Path) -> Option<PathBuf> {
    let glb_path = source.with_extension("glb");
    if glb_path.exists() {
        Some(glb_path)
    } else {
        None
    }
}

/// Copy sidecar files (MTL for OBJ, textures) alongside the main model file.
pub fn copy_sidecar_files(source: &Path, dest_dir: &Path) -> Vec<PathBuf> {
    let mut copied = Vec::new();
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let source_dir = source.parent().unwrap_or(Path::new("."));

    match ext.as_str() {
        "obj" => {
            // Look for .mtl file with same stem
            let mtl_path = source.with_extension("mtl");
            if mtl_path.exists() {
                if let Some(mtl_name) = mtl_path.file_name() {
                    let dest = dest_dir.join(mtl_name);
                    if std::fs::copy(&mtl_path, &dest).is_ok() {
                        copied.push(dest);
                    }
                }

                // Parse MTL to find referenced textures
                if let Ok(mtl_content) = std::fs::read_to_string(&mtl_path) {
                    for line in mtl_content.lines() {
                        let trimmed = line.trim();
                        // Look for texture map directives
                        if trimmed.starts_with("map_Kd")
                            || trimmed.starts_with("map_Ks")
                            || trimmed.starts_with("map_Ka")
                            || trimmed.starts_with("map_Bump")
                            || trimmed.starts_with("map_d")
                            || trimmed.starts_with("bump")
                        {
                            if let Some(tex_name) = trimmed.split_whitespace().last() {
                                let tex_source = source_dir.join(tex_name);
                                if tex_source.exists() {
                                    if let Some(fname) = tex_source.file_name() {
                                        let dest = dest_dir.join(fname);
                                        if !dest.exists() {
                                            if std::fs::copy(&tex_source, &dest).is_ok() {
                                                copied.push(dest);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        "fbx" => {
            // FBX may reference external textures - look for common texture files nearby
            copy_nearby_textures(source_dir, dest_dir, &mut copied);
        }
        _ => {}
    }

    copied
}

/// Copy image files from source_dir to dest_dir (for formats that reference external textures).
fn copy_nearby_textures(source_dir: &Path, dest_dir: &Path, copied: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(source_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if matches!(
                    ext_lower.as_str(),
                    "png" | "jpg" | "jpeg" | "tga" | "bmp" | "webp"
                ) {
                    if let Some(fname) = path.file_name() {
                        let dest = dest_dir.join(fname);
                        if !dest.exists() {
                            if std::fs::copy(&path, &dest).is_ok() {
                                copied.push(dest);
                            }
                        }
                    }
                }
            }
        }
    }
}

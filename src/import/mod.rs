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

    // Build root rotation: compose axis conversion with user rotation offset
    let axes_quat = match settings.convert_axes {
        ConvertAxes::ZUpToYUp => {
            // Rotate -90° around X
            Some([
                -std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
            ])
        }
        ConvertAxes::YUpToZUp => {
            // Rotate +90° around X
            Some([
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
            ])
        }
        ConvertAxes::FlipX => {
            builder.set_root_scale(-1.0); // This is a hack; proper flip would need per-vertex
            None
        }
        ConvertAxes::FlipZ => {
            // Rotate 180° around Y
            Some([0.0, 1.0, 0.0, 0.0])
        }
        ConvertAxes::None => None,
    };

    let (rx, ry, rz) = settings.rotation_offset;
    let has_rotation_offset = rx.abs() > f32::EPSILON || ry.abs() > f32::EPSILON || rz.abs() > f32::EPSILON;
    let offset_quat = if has_rotation_offset {
        Some(euler_to_quat(rx, ry, rz))
    } else {
        None
    };

    // Compose: axes_quat * offset_quat (axes applied first, then user offset)
    let final_rotation = match (axes_quat, offset_quat) {
        (Some(a), Some(b)) => Some(quat_mul(a, b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    if let Some(rot) = final_rotation {
        builder.set_root_rotation(rot);
    }

    // Apply translation offset
    let (tx, ty, tz) = settings.translation_offset;
    if tx.abs() > f32::EPSILON || ty.abs() > f32::EPSILON || tz.abs() > f32::EPSILON {
        builder.set_root_translation([tx, ty, tz]);
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

/// Convert Euler angles (degrees, XYZ order) to quaternion [x, y, z, w].
fn euler_to_quat(x_deg: f32, y_deg: f32, z_deg: f32) -> [f32; 4] {
    let (hx, hy, hz) = (
        x_deg.to_radians() * 0.5,
        y_deg.to_radians() * 0.5,
        z_deg.to_radians() * 0.5,
    );
    let (sx, cx) = hx.sin_cos();
    let (sy, cy) = hy.sin_cos();
    let (sz, cz) = hz.sin_cos();

    // XYZ rotation order
    [
        sx * cy * cz + cx * sy * sz,
        cx * sy * cz - sx * cy * sz,
        cx * cy * sz + sx * sy * cz,
        cx * cy * cz - sx * sy * sz,
    ]
}

/// Multiply two quaternions: result = a * b (apply b then a).
/// Quaternions are [x, y, z, w].
fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
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

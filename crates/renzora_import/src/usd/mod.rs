#![allow(unused_mut)]

//! Pure-Rust USD/USDA/USDC/USDZ parser.
//!
//! Parses Universal Scene Description files into a rich intermediate
//! representation suitable for game engine import. Supports:
//!
//! - **USDA** -- text format (full mesh/material/skeleton/animation extraction)
//! - **USDC** -- binary Crate format (full mesh/material/skeleton/animation extraction)
//! - **USDZ** -- ZIP archive containing USDA/USDC + embedded textures
//!
//! The output is a [`UsdStage`] containing the full scene graph with meshes,
//! materials, textures, skeletons, animations, lights, and cameras.

pub mod crate_format;
pub mod usda;
mod usdz;
pub mod scene;
pub mod mesh;
pub mod material;
pub mod skeleton;
pub mod animation;
pub mod lights;
pub mod camera;
pub mod xform;
pub mod texture;
mod glb;

use std::path::Path;

pub use scene::*;

use crate::anim_extract::AnimExtractResult;
use crate::convert::{ImportError, ImportResult};
use crate::settings::ImportSettings;

/// Errors that can occur during USD parsing.
#[derive(Debug, thiserror::Error)]
pub enum UsdError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("invalid data: {0}")]
    InvalidData(String),
}

/// Result type alias for USD operations.
pub type UsdResult<T> = Result<T, UsdError>;

/// The primary entry point. Parse any USD file (.usd, .usda, .usdc, .usdz)
/// and return a fully resolved scene.
pub fn parse(path: &Path) -> UsdResult<UsdStage> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "usdz" => usdz::parse(path),
        "usda" => {
            let content = std::fs::read_to_string(path)?;
            usda::parse(&content)
        }
        "usdc" => {
            let data = std::fs::read(path)?;
            crate_format::parse(&data)
        }
        "usd" => {
            // Ambiguous -- try text first, then binary
            let data = std::fs::read(path)?;
            if data.starts_with(b"PXR-USDC") {
                crate_format::parse(&data)
            } else if let Ok(text) = std::str::from_utf8(&data) {
                if text.contains("#usda") || text.contains("def ") {
                    return usda::parse(text);
                }
                Err(UsdError::Parse("Unable to detect USD format".into()))
            } else {
                Err(UsdError::Parse("Unable to detect USD format".into()))
            }
        }
        _ => Err(UsdError::Unsupported(format!("Unknown extension: {}", ext))),
    }
}

/// Convert a parsed USD stage to GLB bytes.
pub fn stage_to_glb(stage: &UsdStage) -> UsdResult<Vec<u8>> {
    glb::convert(stage)
}

// ---------------------------------------------------------------------------
// Import bridge functions (used by renzora_import's convert pipeline)
// ---------------------------------------------------------------------------

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let stage = parse(path).map_err(|e| match e {
        UsdError::Io(io) => ImportError::Io(io),
        UsdError::Parse(msg) => ImportError::ParseError(msg),
        UsdError::Unsupported(msg) => ImportError::UnsupportedFormat(msg),
        UsdError::InvalidData(msg) => ImportError::ParseError(msg),
    })?;

    let mut warnings = stage.warnings.clone();

    let glb_bytes = stage_to_glb(&stage).map_err(|e| {
        ImportError::ConversionError(format!("USD to GLB conversion failed: {}", e))
    })?;

    let (extracted_textures, extracted_materials) =
        collect_usd_textures_and_materials(&stage, path, settings, &mut warnings);

    Ok(ImportResult {
        glb_bytes,
        warnings,
        extracted_textures,
        extracted_materials,
    })
}

/// Funnel parsed USD materials and embedded textures into the shared
/// [`ImportResult`] fields. USDZ archives embed textures inline; plain
/// USD/USDA/USDC reference external files, which we read from disk.
fn collect_usd_textures_and_materials(
    stage: &scene::UsdStage,
    usd_path: &Path,
    settings: &ImportSettings,
    warnings: &mut Vec<String>,
) -> (
    Vec<crate::convert::ExtractedTexture>,
    Vec<crate::convert::ExtractedPbrMaterial>,
) {
    use crate::convert::{ExtractedPbrMaterial, ExtractedTexture};
    use scene::{TextureRef as UsdTextureRef, TextureSource};

    if !settings.extract_textures && !settings.extract_materials {
        return (Vec::new(), Vec::new());
    }

    let mut extracted_textures: Vec<ExtractedTexture> = Vec::new();
    let mut extracted_materials: Vec<ExtractedPbrMaterial> = Vec::new();
    // Map a resolved source key (embedded idx or file path) → URI.
    let mut source_uri: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let usd_dir = usd_path.parent().unwrap_or(Path::new("."));

    let mut resolve_texture = |tex_ref: &UsdTextureRef,
                               extracted_textures: &mut Vec<ExtractedTexture>,
                               source_uri: &mut std::collections::HashMap<String, String>,
                               used_names: &mut std::collections::HashSet<String>,
                               warnings: &mut Vec<String>|
     -> Option<String> {
        if !settings.extract_textures {
            return None;
        }
        let (key, data, hint_ext, stem): (String, Vec<u8>, Option<String>, String) =
            match &tex_ref.source {
                TextureSource::Embedded(idx) => {
                    let Some(tex) = stage.textures.get(*idx) else {
                        warnings.push(format!("USD texture index {} out of range", idx));
                        return None;
                    };
                    let hint = match tex.mime_type.as_str() {
                        "image/png" => Some("png".to_string()),
                        "image/jpeg" | "image/jpg" => Some("jpg".to_string()),
                        "image/webp" => Some("webp".to_string()),
                        _ => None,
                    };
                    let stem = std::path::Path::new(&tex.name)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("texture")
                        .to_string();
                    (format!("embedded:{}", idx), tex.data.clone(), hint, stem)
                }
                TextureSource::File(rel) => {
                    let abs = usd_dir.join(rel);
                    let data = match std::fs::read(&abs) {
                        Ok(d) => d,
                        Err(e) => {
                            warnings.push(format!("texture '{}': {}", rel, e));
                            return None;
                        }
                    };
                    let hint = std::path::Path::new(rel)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase());
                    let stem = std::path::Path::new(rel)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("texture")
                        .to_string();
                    (format!("file:{}", rel), data, hint, stem)
                }
            };

        if let Some(uri) = source_uri.get(&key) {
            return Some(uri.clone());
        }

        let extension = hint_ext.unwrap_or_else(|| sniff_image_ext(&data).to_string());
        let base = sanitize_name(&stem);
        let mut name = base.clone();
        let mut n = 1;
        while used_names.contains(&name) {
            n += 1;
            name = format!("{}_{}", base, n);
        }
        used_names.insert(name.clone());

        let uri = format!("textures/{}.{}", name, extension);
        source_uri.insert(key, uri.clone());
        extracted_textures.push(ExtractedTexture {
            name,
            extension,
            data,
        });
        Some(uri)
    };

    for mat in &stage.materials {
        let base_color_uri = mat.diffuse_texture.as_ref().and_then(|t| {
            resolve_texture(
                t,
                &mut extracted_textures,
                &mut source_uri,
                &mut used_names,
                warnings,
            )
        });
        let normal_uri = mat.normal_texture.as_ref().and_then(|t| {
            resolve_texture(
                t,
                &mut extracted_textures,
                &mut source_uri,
                &mut used_names,
                warnings,
            )
        });

        if settings.extract_materials {
            extracted_materials.push(ExtractedPbrMaterial {
                name: mat.name.clone(),
                base_color: [
                    mat.diffuse_color[0],
                    mat.diffuse_color[1],
                    mat.diffuse_color[2],
                    mat.opacity,
                ],
                metallic: mat.metallic,
                roughness: mat.roughness,
                base_color_texture: base_color_uri,
                normal_texture: normal_uri,
            });
        }
    }

    (extracted_textures, extracted_materials)
}

fn sanitize_name(input: &str) -> String {
    if input.is_empty() {
        return "texture".into();
    }
    input
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn sniff_image_ext(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) { "png" }
    else if data.starts_with(&[0xFF, 0xD8, 0xFF]) { "jpg" }
    else if data.starts_with(b"DDS ") { "dds" }
    else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") { "gif" }
    else if data.starts_with(b"BM") { "bmp" }
    else if data.starts_with(&[0x52, 0x49, 0x46, 0x46]) && data.get(8..12) == Some(b"WEBP") { "webp" }
    else { "bin" }
}

/// Extract animations directly from a USD file (for animation-only files
/// that have no geometry and would fail GLB conversion).
pub fn extract_animations_from_usd(
    path: &Path,
    output_dir: &Path,
) -> Result<AnimExtractResult, String> {
    let stage = parse(path)
        .map_err(|e| format!("USD parse error: {}", e))?;

    if stage.animations.is_empty() {
        return Ok(AnimExtractResult {
            written_files: Vec::new(),
            warnings: vec!["No animations found in USD file".into()],
        });
    }

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create animation output dir: {}", e))?;

    let mut written_files = Vec::new();
    let mut warnings = stage.warnings.clone();

    for anim in &stage.animations {
        let clip = animation::to_anim_clip(anim);

        if clip.tracks.is_empty() {
            warnings.push(format!("Animation '{}' has no tracks, skipping", clip.name));
            continue;
        }

        let file_name = if clip.name.is_empty() {
            format!("clip_{}.anim", written_files.len())
        } else {
            format!("{}.anim", sanitize_filename(&clip.name))
        };

        let out_path = output_dir.join(&file_name);
        renzora::write_anim_file(&clip, &out_path)
            .map_err(|e| format!("Failed to write animation '{}': {}", file_name, e))?;

        written_files.push(out_path.display().to_string());
    }

    Ok(AnimExtractResult {
        written_files,
        warnings,
    })
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

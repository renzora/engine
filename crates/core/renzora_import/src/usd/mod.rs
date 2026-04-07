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

pub fn convert(path: &Path, _settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let stage = parse(path).map_err(|e| match e {
        UsdError::Io(io) => ImportError::Io(io),
        UsdError::Parse(msg) => ImportError::ParseError(msg),
        UsdError::Unsupported(msg) => ImportError::UnsupportedFormat(msg),
        UsdError::InvalidData(msg) => ImportError::ParseError(msg),
    })?;

    let warnings = stage.warnings.clone();

    let glb_bytes = stage_to_glb(&stage).map_err(|e| {
        ImportError::ConversionError(format!("USD to GLB conversion failed: {}", e))
    })?;

    Ok(ImportResult {
        glb_bytes,
        warnings,
    })
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
        renzora_core::write_anim_file(&clip, &out_path)
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

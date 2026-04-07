//! Pure-Rust USD/USDA/USDC/USDZ parser.
//!
//! Parses Universal Scene Description files into a rich intermediate
//! representation suitable for game engine import. Supports:
//!
//! - **USDA** — text format (full mesh/material/skeleton/animation extraction)
//! - **USDC** — binary Crate format (full mesh/material/skeleton/animation extraction)
//! - **USDZ** — ZIP archive containing USDA/USDC + embedded textures
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
            // Ambiguous — try text first, then binary
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

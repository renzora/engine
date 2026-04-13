#![allow(unused_mut)]

//! USDZ archive parser.
//!
//! USDZ is a ZIP archive (uncompressed) containing one USDA/USDC file
//! plus embedded textures (PNG, JPEG, EXR).

use std::path::Path;

use super::scene::*;
use super::{UsdError, UsdResult};

/// Parse a USDZ file.
pub fn parse(path: &Path) -> UsdResult<UsdStage> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| UsdError::Parse(format!("Failed to read USDZ archive: {}", e)))?;

    let mut usd_content: Option<UsdContent> = None;
    let mut textures: Vec<UsdTexture> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| UsdError::Parse(format!("Failed to read USDZ entry: {}", e)))?;

        let name = entry.name().to_string();
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();

        log::debug!("USDZ entry [{}]: '{}' ({} bytes, ext='{}')", i, name, entry.size(), ext);

        match ext.as_str() {
            "usda" => {
                let mut content = String::new();
                std::io::Read::read_to_string(&mut entry, &mut content)
                    .map_err(|e| UsdError::Parse(format!("Failed to read USDA: {}", e)))?;
                usd_content = Some(UsdContent::Text(content));
            }
            "usdc" | "usd" => {
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut data)
                    .map_err(|e| UsdError::Parse(format!("Failed to read USDC: {}", e)))?;

                if data.starts_with(b"PXR-USDC") {
                    usd_content = Some(UsdContent::Binary(data));
                } else if let Ok(text) = String::from_utf8(data) {
                    if text.contains("#usda") || text.contains("def ") {
                        usd_content = Some(UsdContent::Text(text));
                    }
                }
            }
            "png" | "jpg" | "jpeg" | "exr" | "hdr" => {
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut data)
                    .map_err(|e| UsdError::Parse(format!("Failed to read texture: {}", e)))?;

                let mime_type = match ext.as_str() {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "exr" => "image/x-exr",
                    "hdr" => "image/vnd.radiance",
                    _ => "application/octet-stream",
                };

                textures.push(UsdTexture {
                    name,
                    mime_type: mime_type.to_string(),
                    data,
                });
            }
            _ => {}
        }
    }

    let mut stage = match usd_content {
        Some(UsdContent::Text(text)) => super::usda::parse(&text)?,
        Some(UsdContent::Binary(data)) => super::crate_format::parse(&data)?,
        None => {
            return Err(UsdError::Parse(
                "No USD file found in USDZ archive".into(),
            ));
        }
    };

    // Merge embedded textures and resolve texture references
    let texture_offset = stage.textures.len();
    stage.textures.extend(textures);

    // Try to resolve file-path texture references to embedded textures
    for mat in &mut stage.materials {
        resolve_texture_ref(&mut mat.diffuse_texture, &stage.textures, texture_offset);
        resolve_texture_ref(&mut mat.emissive_texture, &stage.textures, texture_offset);
        resolve_texture_ref(&mut mat.metallic_texture, &stage.textures, texture_offset);
        resolve_texture_ref(&mut mat.roughness_texture, &stage.textures, texture_offset);
        resolve_texture_ref(&mut mat.normal_texture, &stage.textures, texture_offset);
        resolve_texture_ref(&mut mat.opacity_texture, &stage.textures, texture_offset);
        resolve_texture_ref(&mut mat.occlusion_texture, &stage.textures, texture_offset);
    }

    stage.warnings.extend(warnings);
    Ok(stage)
}

enum UsdContent {
    Text(String),
    Binary(Vec<u8>),
}

fn resolve_texture_ref(
    tex_ref: &mut Option<TextureRef>,
    textures: &[UsdTexture],
    offset: usize,
) {
    if let Some(ref mut tr) = tex_ref {
        if let TextureSource::File(ref path) = tr.source {
            // Try to find a matching embedded texture by filename
            let filename = path.rsplit('/').next().unwrap_or(path);
            for (i, tex) in textures.iter().enumerate().skip(offset) {
                let tex_filename = tex.name.rsplit('/').next().unwrap_or(&tex.name);
                if tex_filename == filename {
                    tr.source = TextureSource::Embedded(i);
                    return;
                }
            }
        }
    }
}

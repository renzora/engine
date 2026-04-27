//! Unified conversion pipeline: any supported format → GLB bytes.

use std::path::Path;

use crate::formats::{detect_format, ModelFormat};
use crate::settings::ImportSettings;

/// A texture pulled out of an embedded source file (e.g. an FBX with the
/// image bytes stored inline). The caller writes `data` to
/// `<model_dir>/textures/<name>.<extension>`; the GLB references it by URI.
#[derive(Clone)]
pub struct ExtractedTexture {
    /// File stem (no extension), already sanitized for the filesystem.
    pub name: String,
    /// File extension without the dot, e.g. `"png"` or `"jpg"`.
    pub extension: String,
    pub data: Vec<u8>,
}

/// A PBR material pulled out of the source file. The caller turns this into
/// a `.material` file (and decides which on-disk format to use) — this struct
/// is deliberately just plain data so `renzora_import` stays independent of
/// the material graph implementation.
#[derive(Clone, Debug)]
pub struct ExtractedPbrMaterial {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// glTF emissive factor (`emissiveFactor`). Multiplies emissive_texture
    /// when present, or used directly when the texture is absent.
    pub emissive: [f32; 3],
    /// Project-relative URI to the base-color texture, e.g.
    /// `"models/character/textures/diffuse.png"`. `None` if the source had no map.
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
    /// Combined glTF metallic-roughness map. Channels: G = roughness, B = metallic.
    pub metallic_roughness_texture: Option<String>,
    pub emissive_texture: Option<String>,
    /// Ambient occlusion map. Bevy reads only the R channel.
    pub occlusion_texture: Option<String>,
    /// glTF spec-gloss `specularGlossinessTexture` (RGB = specular color,
    /// A = glossiness). Spec-gloss-only — `None` for metal-rough materials.
    /// The graph builder routes the inverted alpha channel into the
    /// `roughness` pin so per-pixel gloss (puddles vs dry stone) survives
    /// the spec-gloss → metal-rough conversion.
    pub specular_glossiness_texture: Option<String>,
    /// glTF `alphaMode`: how transparency is rendered.
    pub alpha_mode: ExtractedAlphaMode,
    /// glTF `alphaCutoff` — discard threshold for `Mask` mode. Ignored otherwise.
    pub alpha_cutoff: f32,
    /// glTF `doubleSided` — render back faces too. Glass, foliage, fabric.
    pub double_sided: bool,
}

/// Mirrors the glTF 2.0 `alphaMode` enum. Importers populate this from the
/// source file; downstream the material resolver maps it onto Bevy's
/// `AlphaMode` so transparency renders correctly without artist intervention.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExtractedAlphaMode {
    Opaque,
    Mask,
    Blend,
}

impl Default for ExtractedAlphaMode {
    fn default() -> Self { Self::Opaque }
}

/// Result of a successful import.
pub struct ImportResult {
    /// The GLB binary data, ready to write to disk.
    pub glb_bytes: Vec<u8>,
    /// Non-fatal warnings encountered during conversion.
    pub warnings: Vec<String>,
    /// Textures extracted from the source file. Empty for formats that don't
    /// embed textures or when the source had none.
    pub extracted_textures: Vec<ExtractedTexture>,
    /// Plain PBR material info. Downstream (editor-side) code turns these
    /// into `.material` graph files.
    pub extracted_materials: Vec<ExtractedPbrMaterial>,
}

impl Default for ImportResult {
    fn default() -> Self {
        Self {
            glb_bytes: Vec::new(),
            warnings: Vec::new(),
            extracted_textures: Vec::new(),
            extracted_materials: Vec::new(),
        }
    }
}

/// Errors that can occur during import.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("file not found: {0}")]
    FileNotFound(String),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("conversion error: {0}")]
    ConversionError(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convert any supported 3D model file to GLB binary data.
pub fn convert_to_glb(
    source_path: &Path,
    settings: &ImportSettings,
) -> Result<ImportResult, ImportError> {
    if !source_path.exists() {
        return Err(ImportError::FileNotFound(
            source_path.display().to_string(),
        ));
    }

    let format = detect_format(source_path).ok_or_else(|| {
        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("(none)")
            .to_string();
        ImportError::UnsupportedFormat(ext)
    })?;

    match format {
        ModelFormat::Glb => crate::gltf_pass::convert_glb(source_path, settings),
        ModelFormat::Gltf => crate::gltf_pass::convert_gltf(source_path, settings),
        ModelFormat::Obj => crate::obj::convert(source_path, settings),
        ModelFormat::Stl => crate::stl::convert(source_path, settings),
        ModelFormat::Ply => crate::ply::convert(source_path, settings),
        ModelFormat::Fbx => crate::fbx::convert(source_path, settings),
        ModelFormat::Usd | ModelFormat::Usdz => crate::usd::convert(source_path, settings),
        ModelFormat::Abc => crate::abc::convert(source_path, settings),
        ModelFormat::Dae => crate::dae::convert(source_path, settings),
        ModelFormat::Bvh => crate::bvh::convert(source_path, settings),
        ModelFormat::Blend => crate::blend::convert(source_path, settings),
    }
}

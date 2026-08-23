//! Unified conversion pipeline: any supported format → GLB bytes.

use std::path::Path;

use crate::formats::{detect_format, ModelFormat};
use crate::settings::ImportSettings;

/// Where an [`ExtractedTexture`]'s bytes come from.
#[derive(Clone, Debug)]
pub enum TextureSource {
    /// Image bytes lifted out of the source file, which stored them inline.
    Embedded(Vec<u8>),
    /// A file sitting beside the source model, referenced by it rather than
    /// packed into it. Carried as a path and copied on write instead of being
    /// read into memory: a single scene can reference well over a gigabyte of
    /// external textures, and buffering all of it just to hand it straight back
    /// to the filesystem is a needless way to run a machine out of RAM.
    File(std::path::PathBuf),
    /// A block-compressed DDS repacked into a `.rmip` as it's written.
    ///
    /// Same reason as [`TextureSource::File`] for holding a path: the repack
    /// runs one file at a time at write, so peak memory is one texture rather
    /// than the whole set. See [`renzora_rmip::dds`] for why this is a copy of
    /// the block data and not a decode.
    DdsToRmip {
        path: std::path::PathBuf,
        /// Colour space for the block format, taken from the texture's role in
        /// the material — DDS itself doesn't record it.
        srgb: bool,
        /// Longest-side cap, honoured by dropping leading mip levels. `0`
        /// keeps the file's native size.
        max_size: u32,
    },
    /// A DDS written back out as a DDS, clamped to `max_size`. This is the
    /// copy the intermediate GLB's own materials point at, so it has to stay a
    /// format Bevy's image loader reads — but it must not escape the clamp the
    /// `.rmip` beside it is under.
    DdsClamped {
        path: std::path::PathBuf,
        max_size: u32,
    },
}

/// A texture belonging to an imported model — either embedded in the source
/// file or referenced by it from disk. The caller writes it to
/// `<model_dir>/textures/<name>.<extension>`; the GLB references it by URI.
#[derive(Clone)]
pub struct ExtractedTexture {
    /// File stem (no extension), already sanitized for the filesystem.
    pub name: String,
    /// File extension without the dot, e.g. `"png"` or `"jpg"`.
    pub extension: String,
    pub source: TextureSource,
}

impl ExtractedTexture {
    /// Write this texture to `path`, copying from disk when it was never
    /// buffered.
    pub fn write_to(&self, path: &Path) -> std::io::Result<()> {
        match &self.source {
            TextureSource::Embedded(bytes) => std::fs::write(path, bytes),
            TextureSource::File(src) => std::fs::copy(src, path).map(|_| ()),
            TextureSource::DdsToRmip {
                path: src,
                srgb,
                max_size,
            } => {
                let bytes = std::fs::read(src)?;
                let out = renzora_rmip::dds::transcode(&bytes, *srgb, *max_size).map_err(|e| {
                    std::io::Error::other(format!("{}: {e}", src.display()))
                })?;
                std::fs::write(path, &out.bytes)
            }
            TextureSource::DdsClamped {
                path: src,
                max_size,
            } => {
                let bytes = std::fs::read(src)?;
                let out = renzora_rmip::dds::clamp(&bytes, *max_size)
                    .map_err(|e| std::io::Error::other(format!("{}: {e}", src.display())))?;
                std::fs::write(path, &out)
            }
        }
    }
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
    /// Standalone roughness map (OBJ `map_Pr`, USD UsdPreviewSurface). Its `r`
    /// channel feeds `roughness`. Used when the source keeps roughness and
    /// metallic in separate images rather than the packed glTF MR texture.
    pub roughness_texture: Option<String>,
    /// Standalone metallic map (OBJ `map_Pm`, USD). `r` → `metallic`.
    pub metallic_texture: Option<String>,
    pub emissive_texture: Option<String>,
    /// Ambient occlusion map. Bevy reads only the R channel.
    pub occlusion_texture: Option<String>,
    /// glTF spec-gloss `specularGlossinessTexture` (RGB = specular color,
    /// A = glossiness). Spec-gloss-only — `None` for metal-rough materials.
    /// The graph builder routes the inverted alpha channel into the
    /// `roughness` pin so per-pixel gloss (puddles vs dry stone) survives
    /// the spec-gloss → metal-rough conversion.
    pub specular_glossiness_texture: Option<String>,
    /// Standalone opacity/alpha map (legacy FBX `TransparentColor` /
    /// `TransparencyFactor`, which have no glTF metal-rough equivalent). The
    /// graph builder samples its `r` channel into the `alpha` pin, so a cloud
    /// shell or cutout that drives transparency through a dedicated grayscale
    /// mask — not the base-color alpha channel — actually punches through.
    pub opacity_texture: Option<String>,
    /// Standalone specular/reflectivity mask (legacy FBX `SpecularColor` /
    /// `ReflectionColor`). The builder routes its `r` channel into `metallic`
    /// and the inverse into `roughness`, approximating a pre-PBR specular map:
    /// bright (water, polished) → smooth + reflective, dark (land, matte) →
    /// rough + dielectric.
    pub specular_texture: Option<String>,
    /// Extended PBR channels (clearcoat, transmission, anisotropy, ior, …).
    /// Texture URIs here are model-relative, like the fields above; the import
    /// bridge rewrites them to project-relative when firing the event. Shares
    /// the contract type so no per-layer conversion is needed.
    pub advanced: renzora::core::PbrAdvanced,
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
#[derive(Default)]
pub enum ExtractedAlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}


/// Result of a successful import.
#[derive(Default)]
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


/// What a format converter hands back: a GLB, plus anything it noticed on the
/// way that the user should hear about.
///
/// Converters stop here deliberately. Everything past this point — texture
/// roles, writing `.rmip`, the memory budget, reading materials back out — is
/// the same for every format, and lives in
/// [`crate::gltf_pass::finish_converted_glb`].
pub(crate) struct ConvertedGlb {
    pub glb_bytes: Vec<u8>,
    pub warnings: Vec<String>,
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

/// Progress callback for long per-asset work — currently texture baking,
/// which dominates import time for texture-heavy models. Called as
/// `(done, total, current_item_name)` once per texture. Lets the UI show a
/// moving "[12/73] Compressing textures: …" bar instead of sitting at 100%
/// for the whole multi-minute bake.
///
/// `Sync` because textures bake in parallel across a rayon pool — the
/// callback is invoked from multiple worker threads as each texture finishes,
/// so it must be shareable (the UI side typically locks an mpsc sender).
pub type ProgressFn<'a> = dyn Fn(usize, usize, &str) + Sync + 'a;

/// Convert any supported 3D model file to GLB binary data.
pub fn convert_to_glb(
    source_path: &Path,
    settings: &ImportSettings,
) -> Result<ImportResult, ImportError> {
    convert_to_glb_with_progress(source_path, settings, &|_, _, _| {})
}

/// Like [`convert_to_glb`] but reports per-texture baking progress through
/// `progress`. Only the glTF/GLB paths emit progress today (they're the ones
/// that bake textures); other formats ignore the callback.
pub fn convert_to_glb_with_progress(
    source_path: &Path,
    settings: &ImportSettings,
    progress: &ProgressFn,
) -> Result<ImportResult, ImportError> {
    if !source_path.exists() {
        return Err(ImportError::FileNotFound(source_path.display().to_string()));
    }

    let format = detect_format(source_path).ok_or_else(|| {
        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("(none)")
            .to_string();
        ImportError::UnsupportedFormat(ext)
    })?;

    // glTF sources are already a GLB, so they run the pass themselves (they
    // have their own reading and camera-stripping to do first). Every other
    // format converts geometry and materials to a GLB and then joins the same
    // pipeline — that shared step is what decides texture roles, writes
    // `.rmip`, applies the memory budget, and reads materials back out.
    //
    // Formats used to do all of that individually, which is how FBX ended up
    // the only one with no resolution clamp and no `.rmip` output at all.
    let converted = match format {
        ModelFormat::Glb => return crate::gltf_pass::convert_glb(source_path, settings, progress),
        ModelFormat::Gltf => {
            return crate::gltf_pass::convert_gltf(source_path, settings, progress)
        }
        // Blender is invoked out-of-process and hands back a GLB file, which
        // goes through the glTF importer like any other.
        ModelFormat::Blend => return crate::blend::convert(source_path, settings, progress),
        // No geometry at all — errors so the caller falls back to extracting
        // animations from it.
        ModelFormat::Bvh => return crate::bvh::convert(source_path, settings),

        ModelFormat::Obj => crate::obj::convert(source_path, settings)?,
        ModelFormat::Stl => crate::stl::convert(source_path, settings)?,
        ModelFormat::Ply => crate::ply::convert(source_path, settings)?,
        ModelFormat::Fbx => crate::fbx::convert(source_path, settings)?,
        ModelFormat::Usd | ModelFormat::Usdz => crate::usd::convert(source_path, settings)?,
        ModelFormat::Abc => crate::abc::convert(source_path, settings)?,
        ModelFormat::Dae => crate::dae::convert(source_path, settings)?,
    };

    Ok(crate::gltf_pass::finish_converted_glb(
        converted.glb_bytes,
        settings,
        progress,
        converted.warnings,
    ))
}

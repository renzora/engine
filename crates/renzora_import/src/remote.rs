//! Converting a model in the companion process instead of the editor.
//!
//! Conversion is the heavy part of an import: parsing the source, baking every
//! texture to `.rmip`, optimizing the meshes. The editor's import worker hands
//! it to the companion when one is connected (`renzora::CompanionRemote`) and
//! does it itself otherwise; both ends call into this module, so the two paths
//! run the same code.
//!
//! What crosses the link is small: arguments in, and a description of the
//! result out. The GLB and every texture are written to `out_dir` in their
//! final form, and the result names them by path, because a model's bytes as
//! JSON would be several times their size and pass through the editor's link
//! thread on the way.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::convert::{convert_to_glb_with_progress, ExtractedPbrMaterial, ExtractedTexture, ImportResult, TextureSource};
use crate::optimize::{optimize_glb, MeshOptSettings};
use crate::settings::ImportSettings;

/// The job kind the companion runs this under.
pub const CONVERT_KIND: &str = "model_convert";
/// The job kind that builds a staged GLB's `.rvg`; see [`build_model_file`].
pub const BUILD_KIND: &str = "model_build";

/// What the editor sends.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConvertArgs {
    pub source: PathBuf,
    pub settings: ImportSettings,
    /// An empty folder the companion writes into. The editor deletes it once
    /// the import has copied what it needs.
    pub out_dir: PathBuf,
}

/// What comes back.
#[derive(Serialize, Deserialize, Clone)]
pub struct Converted {
    pub glb: PathBuf,
    pub warnings: Vec<String>,
    /// Every texture as [`TextureSource::File`] inside `out_dir`.
    pub textures: Vec<ExtractedTexture>,
    pub materials: Vec<ExtractedPbrMaterial>,
    /// The meshes were optimized already, as the settings asked.
    pub optimized: bool,
}

/// The mesh optimizations an import's settings ask for.
pub fn mesh_opt_settings(settings: &ImportSettings) -> MeshOptSettings {
    MeshOptSettings {
        vertex_cache: settings.optimize_vertex_cache,
        overdraw: settings.optimize_overdraw,
        vertex_fetch: settings.optimize_vertex_fetch,
        ..Default::default()
    }
}

/// The companion's side: convert, optimize, and write everything to
/// `out_dir`. `progress` gets the texture baker's reports, as the editor's
/// own conversion passes them to its progress bar.
pub fn convert_to_files(args: &ConvertArgs, progress: &crate::ProgressFn) -> Result<Converted, String> {
    std::fs::create_dir_all(&args.out_dir).map_err(|e| format!("could not create {}: {e}", args.out_dir.display()))?;
    let result = convert_to_glb_with_progress(&args.source, &args.settings, progress).map_err(|e| e.to_string())?;
    let mut warnings = result.warnings;

    let opt = mesh_opt_settings(&args.settings);
    let glb_bytes = if opt.any_enabled() {
        match optimize_glb(&result.glb_bytes, &opt) {
            Ok(optimized) => optimized,
            Err(e) => {
                // As the editor does: an unoptimized mesh is slower, not wrong.
                warnings.push(format!("mesh optimization failed: {e}"));
                result.glb_bytes
            }
        }
    } else {
        result.glb_bytes
    };
    let glb = args.out_dir.join("model.glb");
    std::fs::write(&glb, &glb_bytes).map_err(|e| format!("could not write {}: {e}", glb.display()))?;

    let tex_dir = args.out_dir.join("textures");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("could not create {}: {e}", tex_dir.display()))?;
    let mut textures = Vec::with_capacity(result.extracted_textures.len());
    for (i, texture) in result.extracted_textures.into_iter().enumerate() {
        // Numbered, so two textures with one name cannot overwrite each other
        // here; the editor writes each under its own name as before.
        let path = tex_dir.join(format!("{i}.{}", texture.extension));
        texture.write_to(&path).map_err(|e| format!("could not write texture {}: {e}", texture.name))?;
        textures.push(ExtractedTexture { source: TextureSource::File(path), ..texture });
    }

    Ok(Converted {
        glb,
        warnings,
        textures,
        materials: result.extracted_materials,
        optimized: opt.any_enabled(),
    })
}

impl Converted {
    /// The editor's side: the result as the editor's own conversion returns
    /// it. Whether it still needs optimizing is [`Self::optimized`].
    pub fn into_result(self) -> std::io::Result<ImportResult> {
        Ok(ImportResult {
            glb_bytes: std::fs::read(&self.glb)?,
            warnings: self.warnings,
            extracted_textures: self.textures,
            extracted_materials: self.materials,
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// The companion's conversion, sent through JSON as the link sends it and
    /// read back, gives the editor the same model its own conversion would.
    #[test]
    fn a_model_converted_in_the_companion_matches_one_converted_here() {
        let dir = std::env::temp_dir().join(format!("rz-remote-convert-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("tri.obj");
        std::fs::write(&source, "v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nf 1//1 2//1 3//1\n").unwrap();
        let settings = ImportSettings::default();

        let args = ConvertArgs { source: source.clone(), settings: settings.clone(), out_dir: dir.join("out") };
        let wire = serde_json::to_string(&convert_to_files(&args, &|_, _, _| {}).unwrap()).unwrap();
        let converted: Converted = serde_json::from_str(&wire).unwrap();
        assert!(converted.optimized, "the default settings optimize");
        let remote = converted.into_result().unwrap();

        let local = convert_to_glb_with_progress(&source, &settings, &|_, _, _| {}).unwrap();
        let local_glb = optimize_glb(&local.glb_bytes, &mesh_opt_settings(&settings)).unwrap();
        assert_eq!(remote.glb_bytes, local_glb);
        assert_eq!(remote.extracted_materials.len(), local.extracted_materials.len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Turn a staged GLB into the file the model is stored as, beside it: its
/// `.rvg`, or the GLB itself if that cannot be built (see
/// [`crate::model_file`]). Returns the stored file's extension.
///
/// Slow for a big model (seconds for a whole city block). The editor runs it
/// in the companion when one is connected, under [`BUILD_KIND`].
pub fn build_model_file(glb_path: &Path) -> std::io::Result<&'static str> {
    let glb = std::fs::read(glb_path)?;
    let (bytes, extension) = crate::model_file(&glb);
    let path = glb_path.with_extension(extension);
    std::fs::write(&path, bytes)?;
    if path != glb_path {
        std::fs::remove_file(glb_path)?;
    }
    Ok(extension)
}

//! Model formats a plugin teaches the importer.
//!
//! The importer converts every source format to a GLB first and runs one
//! shared pipeline on that GLB: the preview, texture baking, `.material`
//! extraction, animations, the `.rvg` it stores. So a format needs exactly one
//! thing from whoever adds it: the GLB. A plugin registers a name, the file
//! extensions it reads and a converter, and from then on the format is
//! importable everywhere a built-in one is: the Import dialog's file filter,
//! drag and drop from the desktop, an import into the project in place. Its
//! models get the same treatment as an FBX.
//!
//! ```ignore
//! renzora::register_import_format(renzora::ImportFormat {
//!     name: "3D Manufacturing Format",
//!     extensions: &["3mf"],
//!     convert: my_3mf_to_glb,
//! });
//! ```
//!
//! A static registry rather than a resource, because conversion runs on the
//! importer's worker thread, where there is no world. It lives in this crate,
//! inside the shared `renzora_dylib` image, so a plugin and the importer see
//! one registry. Plugin formats are converted in the editor process: the
//! companion process that converts large built-in formats loads no plugins.

use std::path::Path;
use std::sync::RwLock;

/// The user's choices in the Import window that a converter should honour.
#[derive(Clone, Copy, Debug)]
pub struct ImportConvertOptions {
    /// Uniform scale on top of the format's own units.
    pub scale: f32,
    /// The user said the source is Z up. A format that defines its up axis
    /// (most do) can ignore this; one that does not should swap to Y up when
    /// it is set.
    pub z_up: bool,
    /// Flip the V texture coordinate (`1 - v`).
    pub flip_uvs: bool,
    /// The user wants normals made for geometry that has none.
    pub generate_normals: bool,
}

impl Default for ImportConvertOptions {
    fn default() -> Self {
        Self { scale: 1.0, z_up: false, flip_uvs: false, generate_normals: true }
    }
}

/// What a converter produces: a binary glTF in Y-up metres, with materials
/// named as the source names them (each becomes an editable `.material`), and
/// anything worth telling the user.
pub struct ConvertedModel {
    pub glb: Vec<u8>,
    pub warnings: Vec<String>,
}

/// Source file in, GLB out. Runs on the importer's worker thread.
pub type ImportConvertFn = fn(&Path, &ImportConvertOptions) -> Result<ConvertedModel, String>;

/// One model format, as a plugin registers it.
#[derive(Clone, Copy)]
pub struct ImportFormat {
    /// Shown in the Import window, such as "3MF (3D Manufacturing Format)".
    pub name: &'static str,
    /// Lowercase, without the dot.
    pub extensions: &'static [&'static str],
    pub convert: ImportConvertFn,
}

static FORMATS: RwLock<Vec<ImportFormat>> = RwLock::new(Vec::new());

/// Teach the importer a format. A format registered later for an extension
/// takes it over from an earlier one; a built-in format keeps its own.
pub fn register_import_format(format: ImportFormat) {
    if let Ok(mut formats) = FORMATS.write() {
        formats.push(format);
    }
}

/// The registered format for an extension (any case, no dot).
pub fn import_format_for(extension: &str) -> Option<ImportFormat> {
    let ext = extension.to_ascii_lowercase();
    FORMATS
        .read()
        .ok()?
        .iter()
        .rev()
        .find(|f| f.extensions.contains(&ext.as_str()))
        .copied()
}

/// Every registered format, oldest first.
pub fn import_formats() -> Vec<ImportFormat> {
    FORMATS.read().map(|f| f.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(_: &Path, _: &ImportConvertOptions) -> Result<ConvertedModel, String> {
        Ok(ConvertedModel { glb: b"glTF".to_vec(), warnings: Vec::new() })
    }

    #[test]
    fn a_registered_format_is_found_by_extension_in_any_case() {
        register_import_format(ImportFormat { name: "Test Format", extensions: &["tstfmt"], convert });
        let found = import_format_for("TSTFMT").map(|f| f.name);
        assert_eq!(found, Some("Test Format"));
        assert!(import_format_for("nothing").is_none());
        assert!(import_formats().iter().any(|f| f.name == "Test Format"));
    }
}

//! Web stand-in for the `ufbx`-backed FBX importer.
//!
//! `ufbx` is a C library and has no wasm build, so the browser editor cannot
//! import FBX. This module exists so that fact costs exactly one `#[path]`
//! switch in `lib.rs` instead of `#[cfg]` at every call site: it mirrors
//! `fbx_ufbx`'s public surface and reports the format as unsupported.
//!
//! Nothing else about FBX is web-hostile — it is one C dependency. If someone
//! wants FBX in the browser later, a pure-Rust parser or a wasm build of ufbx
//! drops in behind these two signatures without touching callers.
//!
//! GLB/glTF, OBJ, PLY and STL importers are all pure Rust and work on the web
//! unchanged, so this is a gap in one format, not in importing generally.

use std::path::Path;

use crate::anim_extract::AnimExtractResult;
use crate::convert::{ImportError, ImportResult};
use crate::settings::ImportSettings;

const WHY: &str = "FBX import needs the native ufbx library, which has no web \
                   build — convert to glTF/GLB, or import on desktop";

pub fn convert(_path: &Path, _settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    Err(ImportError::UnsupportedFormat(WHY.to_string()))
}

pub fn extract_animations(
    _path: &Path,
    _output_dir: &Path,
    _settings: &ImportSettings,
) -> Result<AnimExtractResult, String> {
    Err(WHY.to_string())
}

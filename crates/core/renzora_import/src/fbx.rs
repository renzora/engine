//! FBX → GLB converter.
//!
//! Historically this file contained a hand-rolled binary FBX parser based on
//! fbxcel-dom. It's now a thin delegate over [`fbx_ufbx`], which uses the
//! `ufbx` crate to handle every FBX version (3.0 – 7.7), binary and ASCII,
//! with all exporter-specific quirks normalized.

use std::path::Path;

use crate::convert::{ImportError, ImportResult};
use crate::settings::ImportSettings;

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    crate::fbx_ufbx::convert(path, settings)
}

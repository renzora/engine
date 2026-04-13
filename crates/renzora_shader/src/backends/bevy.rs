//! Bevy WGSL backend — handles Bevy's extended WGSL with `#import` preprocessor
//! directives. Skips naga validation since Bevy's naga_oil handles imports at runtime.
//!
//! The registry's post-compile step handles injecting `ShaderUniforms` if missing.

use crate::backend::{ShaderBackend, ShaderCompileError};

pub struct BevyBackend;

impl ShaderBackend for BevyBackend {
    fn name(&self) -> &str {
        "Bevy"
    }

    fn file_extensions(&self) -> &[&str] {
        &["bevy.wgsl"]
    }

    fn to_wgsl(&self, source: &str) -> Result<String, ShaderCompileError> {
        // Passthrough — Bevy's naga_oil preprocessor handles #import at runtime
        Ok(source.to_string())
    }
}

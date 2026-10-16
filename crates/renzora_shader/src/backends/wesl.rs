//! WESL backend: the shader language Bevy 0.20 compiles everything with.
//!
//! Passed through untouched. WESL imports (`import bevy_pbr::render::...;`)
//! resolve against the running app's shader library and the project's other
//! `.wesl` files, so the compiler can only run when the pipeline is built;
//! errors show up there, in the log and the Compiler Log panel.
//!
//! The registry's post-compile step handles injecting `ShaderUniforms` and
//! `@param` constants after the imports.

use crate::backend::{ShaderBackend, ShaderCompileError};

pub struct WeslBackend;

impl ShaderBackend for WeslBackend {
    fn name(&self) -> &str {
        "WESL"
    }

    fn file_extensions(&self) -> &[&str] {
        &["wesl"]
    }

    fn to_wgsl(&self, source: &str) -> Result<String, ShaderCompileError> {
        Ok(source.to_string())
    }
}

//! Bevy WGSL backend: the naga_oil dialect (`#import`, `#ifdef`,
//! `#{MATERIAL_BIND_GROUP}`) Bevy shaders were written in before 0.20.
//!
//! Bevy 0.20 no longer preprocesses it, so this translates it to WESL, which
//! Bevy compiles at pipeline time. Not validated here: the imports only
//! resolve against the running app's shader library.
//!
//! The registry's post-compile step handles injecting `ShaderUniforms` if missing.

use crate::backend::{ShaderBackend, ShaderCompileError};
use renzora::shader_source::Severity;

pub struct BevyBackend;

impl ShaderBackend for BevyBackend {
    fn name(&self) -> &str {
        "Bevy"
    }

    fn file_extensions(&self) -> &[&str] {
        &["bevy.wgsl"]
    }

    fn to_wgsl(&self, source: &str) -> Result<String, ShaderCompileError> {
        let t = renzora::shader_source::to_wesl(source);
        // An error means part of the shader could not be translated; the
        // result would fail to compile anyway, so say where, here.
        if let Some(d) = t.diagnostics.iter().find(|d| d.severity == Severity::Error) {
            return Err(ShaderCompileError {
                message: d.message.clone(),
                line: Some(d.line),
                column: None,
            });
        }
        Ok(t.source)
    }
}

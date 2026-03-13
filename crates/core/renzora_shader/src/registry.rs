//! Shader backend registry — manages available backends and dispatches compilation.
//!
//! After any backend produces WGSL, the registry post-processes the output to
//! ensure it declares the `ShaderUniforms` bind group that `CodeShaderMaterial` requires.

use bevy::prelude::*;

use crate::backend::{ShaderBackend, ShaderCompileError};

/// The `ShaderUniforms` struct + binding declaration required by `CodeShaderMaterial`.
const SHADER_UNIFORMS_BLOCK: &str = r#"
struct ShaderUniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    mouse: vec4<f32>,
    frame: u32,
    _pad: vec3<f32>,
}

@group(3) @binding(0) var<uniform> uniforms: ShaderUniforms;
"#;

/// Resource holding all registered shader backends.
#[derive(Resource, Default)]
pub struct ShaderBackendRegistry {
    backends: Vec<Box<dyn ShaderBackend>>,
}

impl ShaderBackendRegistry {
    /// Register a new shader backend.
    pub fn register(&mut self, backend: Box<dyn ShaderBackend>) {
        info!("[shader] Registered backend: {}", backend.name());
        self.backends.push(backend);
    }

    /// Find a backend by file extension.
    pub fn find_by_extension(&self, ext: &str) -> Option<&dyn ShaderBackend> {
        self.backends
            .iter()
            .find(|b| b.file_extensions().contains(&ext))
            .map(|b| b.as_ref())
    }

    /// Find a backend by language name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Option<&dyn ShaderBackend> {
        let lower = name.to_lowercase();
        self.backends
            .iter()
            .find(|b| b.name().to_lowercase() == lower)
            .map(|b| b.as_ref())
    }

    /// Compile source using the named backend, returning Bevy-compatible WGSL.
    ///
    /// After the backend produces WGSL, the registry ensures the `ShaderUniforms`
    /// bind group is declared so the output works with `CodeShaderMaterial`.
    pub fn compile(&self, language: &str, source: &str) -> Result<String, ShaderCompileError> {
        let backend = self.find_by_name(language).ok_or_else(|| ShaderCompileError {
            message: format!("Unknown shader language: '{}'", language),
            line: None,
            column: None,
        })?;
        let wgsl = backend.to_wgsl(source)?;
        let wgsl = ensure_uniforms_binding(wgsl);

        // Inject @param constants so user-defined parameters resolve as WGSL variables
        let params = crate::file::extract_params(source);
        let param_block = crate::file::params_to_wgsl(&params);
        if param_block.is_empty() {
            Ok(wgsl)
        } else {
            Ok(inject_param_constants(wgsl, &param_block))
        }
    }

    /// List all registered backend names.
    pub fn languages(&self) -> Vec<&str> {
        self.backends.iter().map(|b| b.name()).collect()
    }
}

/// Inject `@param` constant declarations into WGSL output.
/// Inserts after the `ShaderUniforms` binding (or after `#import` lines).
fn inject_param_constants(wgsl: String, param_block: &str) -> String {
    // Insert after the @group(3) @binding(0) line if present
    if let Some(pos) = wgsl.find("var<uniform> uniforms: ShaderUniforms;") {
        if let Some(line_end) = wgsl[pos..].find('\n') {
            let insert_at = pos + line_end + 1;
            let mut output = wgsl;
            output.insert_str(insert_at, param_block);
            return output;
        }
    }

    // Fallback: insert after last #import, or at top
    let insert_pos = wgsl
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let t = line.trim();
            t.starts_with("#import") || t.starts_with("#define_import_path")
        })
        .last()
        .map(|(i, _)| {
            wgsl.lines()
                .take(i + 1)
                .map(|l| l.len() + 1)
                .sum::<usize>()
        })
        .unwrap_or(0);

    let mut output = wgsl;
    output.insert_str(insert_pos, param_block);
    output
}

/// Ensure the WGSL output declares the `ShaderUniforms` struct and
/// `@group(3) @binding(0)` binding. If already present, returns as-is.
fn ensure_uniforms_binding(wgsl: String) -> String {
    if wgsl.contains("@group(3)") {
        return wgsl;
    }

    // Insert after the last #import line, or at the top
    let insert_pos = wgsl
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let t = line.trim();
            t.starts_with("#import") || t.starts_with("#define_import_path")
        })
        .last()
        .map(|(i, _)| {
            wgsl.lines()
                .take(i + 1)
                .map(|l| l.len() + 1)
                .sum::<usize>()
        })
        .unwrap_or(0);

    let mut output = wgsl;
    output.insert_str(insert_pos, SHADER_UNIFORMS_BLOCK);
    output
}

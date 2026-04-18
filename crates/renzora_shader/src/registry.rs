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
    /// Bevy-mode shaders that manage their own bind groups are passed through as-is.
    pub fn compile(&self, language: &str, source: &str) -> Result<String, ShaderCompileError> {
        let wgsl = self.transpile(language, source)?;

        // Inject @param constants so user-defined parameters resolve as WGSL variables
        let params = crate::file::extract_params(source);
        let param_block = crate::file::params_to_wgsl(&params);
        if param_block.is_empty() {
            Ok(wgsl)
        } else {
            Ok(inject_param_constants(wgsl, &param_block))
        }
    }

    /// Transpile source to WGSL with uniform injection but **without** `@param` constant
    /// injection. Use this when you need to inject params separately (e.g. from edited
    /// values in the shader properties panel).
    pub fn transpile(&self, language: &str, source: &str) -> Result<String, ShaderCompileError> {
        let backend = self.find_by_name(language).ok_or_else(|| ShaderCompileError {
            message: format!("Unknown shader language: '{}'", language),
            line: None,
            column: None,
        })?;
        let wgsl = backend.to_wgsl(source)?;

        // Only inject ShaderUniforms if the shader actually uses `uniforms.*`
        let needs_uniforms = wgsl.contains("uniforms.");
        if needs_uniforms {
            Ok(ensure_uniforms_binding(wgsl))
        } else {
            Ok(wgsl)
        }
    }

    /// List all registered backend names.
    pub fn languages(&self) -> Vec<&str> {
        self.backends.iter().map(|b| b.name()).collect()
    }
}

/// Check if the shader declares its own bindings at the material bind group
/// (group 3 or #{MATERIAL_BIND_GROUP}), which would conflict with CodeShaderMaterial's layout.
pub fn has_custom_material_bindings(wgsl: &str) -> bool {
    // Handle both single-line (`@group(3) @binding(0) var<uniform> ...`) and multi-line
    // declarations where `@group(3)` is on one line and `var<uniform>` on the next.
    let mut saw_group3 = false;

    for line in wgsl.lines() {
        let trimmed = line.trim();

        let has_group3 = trimmed.contains("@group(3)") || trimmed.contains("MATERIAL_BIND_GROUP");
        let has_var = trimmed.contains("var ") || trimmed.contains("var<");
        let is_ours = trimmed.contains("uniforms: ShaderUniforms");

        // Single-line declaration
        if has_group3 && has_var && !is_ours {
            return true;
        }

        if has_group3 && !has_var {
            // @group(3) on its own line — the next var belongs to it
            saw_group3 = true;
        } else if saw_group3 && has_var && !is_ours {
            return true;
        } else if saw_group3 && !trimmed.is_empty() && !trimmed.starts_with("@binding") && !trimmed.starts_with("//") {
            saw_group3 = false;
        }
    }
    false
}

/// Inject `@param` constant declarations into WGSL output.
/// Inserts after the `ShaderUniforms` binding (or after `#import` lines).
pub fn inject_param_constants(wgsl: String, param_block: &str) -> String {
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
    let mut output = wgsl;
    output.insert_str(find_after_last_import(&output), param_block);
    output
}

/// Ensure the WGSL output declares the `ShaderUniforms` struct and
/// `@group(3) @binding(0)` binding. If already present, returns as-is.
fn ensure_uniforms_binding(wgsl: String) -> String {
    if wgsl.contains("@group(3)") {
        return wgsl;
    }

    let mut output = wgsl;
    output.insert_str(find_after_last_import(&output), SHADER_UNIFORMS_BLOCK);
    output
}

/// Find the byte position right after the last `#import` / `#define_import_path` line.
/// Handles both `\n` and `\r\n` correctly (the old `lines().len() + 1` was wrong on Windows).
fn find_after_last_import(source: &str) -> usize {
    let mut last_import_end = 0;
    let mut found = false;

    for (byte_pos, line) in line_byte_ranges(source) {
        let t = line.trim();
        if t.starts_with("#import") || t.starts_with("#define_import_path") {
            last_import_end = byte_pos + line.len();
            // Skip past the newline character(s)
            if source[last_import_end..].starts_with("\r\n") {
                last_import_end += 2;
            } else if source[last_import_end..].starts_with('\n') {
                last_import_end += 1;
            }
            found = true;
        }
    }

    if found { last_import_end } else { 0 }
}

/// Iterate lines with their byte offset in the source string.
fn line_byte_ranges(source: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut pos = 0;
    source.split('\n').map(move |line| {
        let start = pos;
        // +1 for the '\n' we split on (the line itself doesn't include it)
        pos += line.len() + 1;
        // Strip trailing \r if present (Windows CRLF)
        let trimmed = line.strip_suffix('\r').unwrap_or(line);
        (start, trimmed)
    })
}

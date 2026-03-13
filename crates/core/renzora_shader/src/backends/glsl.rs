//! GLSL backend — transpile GLSL fragment shaders to WGSL via naga.

use crate::backend::{ShaderBackend, ShaderCompileError};

pub struct GlslBackend;

impl ShaderBackend for GlslBackend {
    fn name(&self) -> &str {
        "GLSL"
    }

    fn file_extensions(&self) -> &[&str] {
        &["glsl", "frag", "vert"]
    }

    fn to_wgsl(&self, source: &str) -> Result<String, ShaderCompileError> {
        glsl_to_wgsl(source)
    }
}

/// Convert GLSL fragment shader source to WGSL.
pub fn glsl_to_wgsl(source: &str) -> Result<String, ShaderCompileError> {
    // Parse GLSL
    let mut frontend = naga::front::glsl::Frontend::default();
    let options = naga::front::glsl::Options::from(naga::ShaderStage::Fragment);

    let module = frontend.parse(&options, source).map_err(|errors| {
        let messages: Vec<String> = errors.errors.iter().map(|e| format!("{}", e)).collect();
        ShaderCompileError {
            message: messages.join("\n"),
            line: None,
            column: None,
        }
    })?;

    // Validate the module
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|err| ShaderCompileError {
        message: format!("Validation error: {}", err),
        line: None,
        column: None,
    })?;

    // Write WGSL
    let mut wgsl = String::new();
    let mut writer = naga::back::wgsl::Writer::new(&mut wgsl, naga::back::wgsl::WriterFlags::empty());
    writer.write(&module, &info).map_err(|err| ShaderCompileError {
        message: format!("WGSL write error: {}", err),
        line: None,
        column: None,
    })?;

    Ok(wgsl)
}

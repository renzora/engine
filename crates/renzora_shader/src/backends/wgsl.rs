//! WGSL backend — pure WGSL with naga validation.

use crate::backend::{ShaderBackend, ShaderCompileError};

pub struct WgslBackend;

impl ShaderBackend for WgslBackend {
    fn name(&self) -> &str {
        "WGSL"
    }

    fn file_extensions(&self) -> &[&str] {
        &["wgsl"]
    }

    fn to_wgsl(&self, source: &str) -> Result<String, ShaderCompileError> {
        // Validate using naga's WGSL parser
        match naga::front::wgsl::parse_str(source) {
            Ok(_module) => Ok(source.to_string()),
            Err(err) => {
                let msg = err.emit_to_string(source);
                Err(ShaderCompileError {
                    message: msg,
                    line: None,
                    column: None,
                })
            }
        }
    }
}

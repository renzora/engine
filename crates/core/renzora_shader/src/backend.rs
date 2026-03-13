//! Core shader backend trait for transpiling shader languages to WGSL.

use std::fmt;

/// Error produced during shader compilation/transpilation.
#[derive(Debug, Clone)]
pub struct ShaderCompileError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl fmt::Display for ShaderCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(line), Some(col)) = (self.line, self.column) {
            write!(f, "[{}:{}] {}", line, col, self.message)
        } else if let Some(line) = self.line {
            write!(f, "[line {}] {}", line, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

/// Mapping from a backend-specific uniform name to its WGSL equivalent.
#[derive(Debug, Clone)]
pub struct UniformMapping {
    pub source_name: &'static str,
    pub wgsl_name: &'static str,
    pub glsl_type: &'static str,
    pub description: &'static str,
}

/// A syntax highlighting rule for the editor.
#[derive(Debug, Clone)]
pub struct SyntaxRule {
    pub token: &'static str,
    pub category: SyntaxCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxCategory {
    Keyword,
    Type,
    BuiltinFunction,
    Uniform,
}

/// Trait for shader language backends that transpile to WGSL.
///
/// Implementations handle converting source code from their language
/// (WGSL, GLSL, HLSL, ShaderToy, etc.) into valid WGSL for Bevy's GPU pipeline.
pub trait ShaderBackend: Send + Sync + 'static {
    /// Human-readable name for this backend (e.g. "WGSL", "GLSL", "ShaderToy").
    fn name(&self) -> &str;

    /// File extensions this backend handles (e.g. &["wgsl"], &["glsl", "frag", "vert"]).
    fn file_extensions(&self) -> &[&str];

    /// Transpile source code to WGSL.
    fn to_wgsl(&self, source: &str) -> Result<String, ShaderCompileError>;

    /// Built-in uniforms this language provides (e.g. ShaderToy's iTime, iResolution).
    fn builtin_uniforms(&self) -> &[UniformMapping] {
        &[]
    }

    /// Optional syntax highlighting rules for the code editor.
    fn syntax_tokens(&self) -> Option<&[SyntaxRule]> {
        None
    }
}

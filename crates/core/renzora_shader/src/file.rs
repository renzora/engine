//! `.shader` file format — JSON-serialized code shader definition.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// The type of shader being authored — affects compilation and preview behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShaderType {
    /// Fragment-only shader (ShaderToy-style). Previews on a mesh via `CodeShaderMaterial`.
    #[default]
    Fragment,
    /// Full Bevy material with custom bind groups and optional vertex shader.
    /// Cannot preview via `CodeShaderMaterial` — compile-check only.
    Material,
    /// Full-screen post-process effect.
    PostProcess,
}

impl ShaderType {
    pub const ALL: &[ShaderType] = &[
        ShaderType::Fragment,
        ShaderType::Material,
        ShaderType::PostProcess,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ShaderType::Fragment => "Fragment",
            ShaderType::Material => "Material",
            ShaderType::PostProcess => "Post-Process",
        }
    }
}

/// A `.shader` file stores a code-authored shader with its source, language,
/// and exposed parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderFile {
    /// Language identifier (e.g. "WGSL", "GLSL", "ShaderToy").
    pub language: String,
    /// The type of shader (Fragment, Material, Post-Process).
    #[serde(default)]
    pub shader_type: ShaderType,
    /// Raw shader source code.
    pub shader_source: String,
    /// Cached compiled WGSL (not serialized — recomputed at load time).
    #[serde(skip)]
    pub compiled_wgsl: Option<String>,
    /// User-defined parameters exposed to the inspector/overrides.
    #[serde(default)]
    pub params: HashMap<String, ShaderParam>,
}

impl Default for ShaderFile {
    fn default() -> Self {
        Self {
            language: "Bevy".into(),
            shader_type: ShaderType::default(),
            shader_source: DEFAULT_BEVY_SOURCE.into(),
            compiled_wgsl: None,
            params: HashMap::new(),
        }
    }
}

/// A user-defined shader parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderParam {
    pub param_type: ParamType,
    pub default_value: ParamValue,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub description: String,
}

/// Supported parameter types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParamType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Color,
    Int,
    Bool,
}

/// Parameter value variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Color([f32; 4]),
    Int(i32),
    Bool(bool),
}

impl Default for ParamValue {
    fn default() -> Self {
        Self::Float(0.0)
    }
}

/// Extract `@param` annotations from shader source comments.
///
/// Supported formats:
/// ```text
/// // @param name float default [min max] description
/// // @param speed float 1.0 0.0 10.0 Animation speed
/// // @param tint color 1.0 0.5 0.0 1.0
/// // @param offset vec2 0.0 0.0
/// // @param enabled bool true
/// // @param count int 4
/// ```
pub fn extract_params(source: &str) -> HashMap<String, ShaderParam> {
    let mut params = HashMap::new();

    for line in source.lines() {
        let trimmed = line.trim();
        // Match // @param or /* @param
        let content = if let Some(rest) = trimmed.strip_prefix("//") {
            rest.trim()
        } else if let Some(rest) = trimmed.strip_prefix("/*") {
            rest.trim().trim_end_matches("*/").trim()
        } else {
            continue;
        };

        let Some(rest) = content.strip_prefix("@param") else { continue };
        let rest = rest.trim();
        if rest.is_empty() { continue; }

        let tokens: Vec<&str> = rest.split_whitespace().collect();
        if tokens.len() < 2 { continue; }

        let name = tokens[0].to_string();
        let type_str = tokens[1].to_lowercase();

        let (param_type, default_value, min, max, desc_start) = match type_str.as_str() {
            "float" | "f32" => {
                let default = tokens.get(2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let min = tokens.get(3).and_then(|s| s.parse::<f32>().ok());
                let max = tokens.get(4).and_then(|s| s.parse::<f32>().ok());
                let desc_idx = if max.is_some() { 5 } else if min.is_some() { 4 } else { 3 };
                (ParamType::Float, ParamValue::Float(default), min, max, desc_idx)
            }
            "vec2" => {
                let x = tokens.get(2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let y = tokens.get(3).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                (ParamType::Vec2, ParamValue::Vec2([x, y]), None, None, 4)
            }
            "vec3" => {
                let x = tokens.get(2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let y = tokens.get(3).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let z = tokens.get(4).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                (ParamType::Vec3, ParamValue::Vec3([x, y, z]), None, None, 5)
            }
            "vec4" => {
                let x = tokens.get(2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let y = tokens.get(3).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let z = tokens.get(4).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let w = tokens.get(5).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                (ParamType::Vec4, ParamValue::Vec4([x, y, z, w]), None, None, 6)
            }
            "color" | "colour" => {
                let r = tokens.get(2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
                let g = tokens.get(3).and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
                let b = tokens.get(4).and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
                let a = tokens.get(5).and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
                (ParamType::Color, ParamValue::Color([r, g, b, a]), None, None, 6)
            }
            "int" | "i32" => {
                let default = tokens.get(2).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                (ParamType::Int, ParamValue::Int(default), None, None, 3)
            }
            "bool" => {
                let default = tokens.get(2).map(|s| *s == "true").unwrap_or(false);
                (ParamType::Bool, ParamValue::Bool(default), None, None, 3)
            }
            _ => continue,
        };

        let description = if desc_start < tokens.len() {
            tokens[desc_start..].join(" ")
        } else {
            String::new()
        };

        params.insert(name, ShaderParam {
            param_type,
            default_value,
            min,
            max,
            description,
        });
    }

    params
}

/// Generate WGSL constant declarations from extracted `@param` annotations.
/// These are injected into compiled WGSL so param names resolve as variables.
pub fn params_to_wgsl(params: &HashMap<String, ShaderParam>) -> String {
    if params.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push("\n// @param constants".to_string());

    // Sort for deterministic output
    let mut sorted: Vec<_> = params.iter().collect();
    sorted.sort_by_key(|(name, _)| name.to_string());

    for (name, param) in sorted {
        let decl = match &param.default_value {
            ParamValue::Float(v) => format!("const {}: f32 = {:.6};", name, v),
            ParamValue::Vec2(v) => format!("const {}: vec2<f32> = vec2<f32>({:.6}, {:.6});", name, v[0], v[1]),
            ParamValue::Vec3(v) => format!("const {}: vec3<f32> = vec3<f32>({:.6}, {:.6}, {:.6});", name, v[0], v[1], v[2]),
            ParamValue::Vec4(v) => format!("const {}: vec4<f32> = vec4<f32>({:.6}, {:.6}, {:.6}, {:.6});", name, v[0], v[1], v[2], v[3]),
            ParamValue::Color(v) => format!("const {}: vec4<f32> = vec4<f32>({:.6}, {:.6}, {:.6}, {:.6});", name, v[0], v[1], v[2], v[3]),
            ParamValue::Int(v) => format!("const {}: i32 = {};", name, v),
            ParamValue::Bool(v) => format!("const {}: bool = {};", name, v),
        };
        lines.push(decl);
    }

    lines.push(String::new());
    lines.join("\n")
}

/// Auto-detect shader language from source content.
/// Returns the best-guess language name for the backend registry.
pub fn detect_language(source: &str) -> &'static str {
    // ShaderToy: has mainImage signature or iResolution/iTime globals
    if source.contains("mainImage") || source.contains("iResolution") || source.contains("iTime") {
        return "ShaderToy";
    }

    // Bevy WGSL: uses #import directives (naga_oil)
    if source.contains("#import bevy_") || source.contains("#import bevy::") {
        return "Bevy";
    }

    // GLSL: C-style signatures, #version, or void main()
    if source.contains("#version")
        || source.contains("void main")
        || source.contains("gl_Frag")
        || source.contains("uniform ")
        || source.contains("varying ")
    {
        return "GLSL";
    }

    // WGSL: @fragment, fn fragment, var<uniform>, struct keywords
    if source.contains("@fragment")
        || source.contains("@vertex")
        || source.contains("@compute")
        || source.contains("var<uniform>")
        || source.contains("var<storage")
    {
        return "WGSL";
    }

    // Default to Bevy (most common in this engine)
    "Bevy"
}

const DEFAULT_BEVY_SOURCE: &str = r#"#import bevy_pbr::forward_io::VertexOutput

// @param speed float 1.0 0.1 5.0 Animation speed
// @param tint color 1.0 0.8 0.6 1.0

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = uniforms.time * speed;

    let r = 0.5 + 0.5 * cos(t + uv.x * 6.283);
    let g = 0.5 + 0.5 * cos(t + uv.y * 6.283 + 2.094);
    let b = 0.5 + 0.5 * cos(t + (uv.x + uv.y) * 3.141 + 4.189);

    return vec4<f32>(r * tint.r, g * tint.g, b * tint.b, 1.0);
}
"#;

pub const DEFAULT_MATERIAL_SOURCE: &str = r#"#import bevy_pbr::forward_io::VertexOutput

struct MyMaterial {
    color: vec4<f32>,
}

@group(3) @binding(0)
var<uniform> material: MyMaterial;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    return material.color * vec4<f32>(uv, 0.5, 1.0);
}
"#;

pub const DEFAULT_POST_PROCESS_SOURCE: &str = r#"#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0)
var screen_texture: texture_2d<f32>;
@group(0) @binding(1)
var screen_sampler: sampler;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, screen_sampler, in.uv);
    let gray = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
    return vec4<f32>(vec3<f32>(gray), 1.0);
}
"#;

/// Get the default source template for a given shader type.
pub fn default_source_for_type(shader_type: ShaderType) -> &'static str {
    match shader_type {
        ShaderType::Fragment => DEFAULT_BEVY_SOURCE,
        ShaderType::Material => DEFAULT_MATERIAL_SOURCE,
        ShaderType::PostProcess => DEFAULT_POST_PROCESS_SOURCE,
    }
}

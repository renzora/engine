//! ShaderToy backend — wraps ShaderToy-style GLSL into a proper fragment shader,
//! then transpiles to WGSL via the GLSL backend.
//!
//! The registry's post-compile step handles injecting the `ShaderUniforms` bind group.

use crate::backend::{ShaderBackend, ShaderCompileError, UniformMapping};
use crate::backends::glsl;

/// Built-in ShaderToy uniform mappings.
static SHADERTOY_UNIFORMS: &[UniformMapping] = &[
    UniformMapping {
        source_name: "iTime",
        wgsl_name: "uniforms.time",
        glsl_type: "float",
        description: "Playback time in seconds",
    },
    UniformMapping {
        source_name: "iResolution",
        wgsl_name: "uniforms.resolution",
        glsl_type: "vec3",
        description: "Viewport resolution (width, height, aspect)",
    },
    UniformMapping {
        source_name: "iMouse",
        wgsl_name: "uniforms.mouse",
        glsl_type: "vec4",
        description: "Mouse pixel coords (xy: current, zw: click)",
    },
    UniformMapping {
        source_name: "iTimeDelta",
        wgsl_name: "uniforms.delta_time",
        glsl_type: "float",
        description: "Frame delta time",
    },
    UniformMapping {
        source_name: "iFrame",
        wgsl_name: "uniforms.frame",
        glsl_type: "int",
        description: "Frame counter",
    },
];

pub struct ShaderToyBackend;

impl ShaderBackend for ShaderToyBackend {
    fn name(&self) -> &str {
        "ShaderToy"
    }

    fn file_extensions(&self) -> &[&str] {
        &["shadertoy"]
    }

    fn to_wgsl(&self, source: &str) -> Result<String, ShaderCompileError> {
        // Step 1: Wrap ShaderToy source in proper GLSL
        let glsl_source = wrap_shadertoy(source);

        // Step 2: Transpile GLSL → WGSL via naga
        let wgsl = glsl::glsl_to_wgsl(&glsl_source)?;

        // Step 3: Remap naga's generated uniform globals to our struct fields
        Ok(remap_uniforms(&wgsl))
    }

    fn builtin_uniforms(&self) -> &[UniformMapping] {
        SHADERTOY_UNIFORMS
    }
}

/// Wrap ShaderToy `mainImage(out vec4 fragColor, in vec2 fragCoord)` into proper GLSL.
///
/// Instead of calling `mainImage` via an `out` parameter (which naga's IR validator
/// struggles with for scalar-to-vector operations), we inline the body of `mainImage`
/// directly into `main()` so `fragColor` is a plain local variable.
fn wrap_shadertoy(source: &str) -> String {
    let rewritten = preprocess_shadertoy(source);

    // Split into helper functions (before mainImage) and mainImage body
    let (helpers, main_body) = extract_main_image_body(&rewritten);

    format!(
        r#"#version 450

precision highp float;

// ShaderToy uniforms in a proper uniform block
layout(set = 0, binding = 0) uniform ShaderToyUniforms {{
    float iTime;
    float iTimeDelta;
    vec3 iResolution;
    vec4 iMouse;
    int iFrame;
}} _st;

layout(location = 0) out vec4 outColor;

{helpers}

void main() {{
    vec4 fragColor = vec4(0.0);
    vec2 fragCoord = gl_FragCoord.xy;

{main_body}

    outColor = fragColor;
}}
"#
    )
}

/// Extract the body of `mainImage(out vec4 fragColor, in vec2 fragCoord)` from the source.
/// Returns (helper_functions, mainImage_body).
/// If `mainImage` isn't found, returns the whole source as the body (best-effort).
fn extract_main_image_body(source: &str) -> (String, String) {
    // Find `void mainImage(` — may have varying whitespace
    let main_image_pat = "void mainImage";
    let Some(sig_start) = source.find(main_image_pat) else {
        // No mainImage found — wrap everything as the body
        return (String::new(), source.to_string());
    };

    let helpers = source[..sig_start].to_string();

    // Find the opening brace of mainImage
    let after_sig = &source[sig_start..];
    let Some(brace_offset) = after_sig.find('{') else {
        return (helpers, String::new());
    };

    let body_start = sig_start + brace_offset + 1;

    // Find matching closing brace
    let mut depth = 1;
    let mut body_end = body_start;
    for (i, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = body_start + i;
                    break;
                }
            }
            _ => {}
        }
    }

    let body = source[body_start..body_end].to_string();

    // Anything after mainImage's closing brace (unusual but handle it)
    let remainder = source[body_end + 1..].trim();
    let full_helpers = if remainder.is_empty() {
        helpers
    } else {
        format!("{}\n{}", helpers, remainder)
    };

    (full_helpers, body)
}

/// Run all ShaderToy-specific source preprocessing:
/// 1. Expand `#define` macros (naga has no C preprocessor)
/// 2. Fix naga-incompatible GLSL patterns
/// 3. Rewrite uniform references to use the UBO instance
fn preprocess_shadertoy(source: &str) -> String {
    let expanded = expand_defines(source);
    let stripped = strip_float_suffixes(&expanded);
    let fixed = fix_mat2_from_vec4(&stripped);
    preprocess_shadertoy_uniforms(&fixed)
}

/// Rewrite `mat2(<single_expr>)` → `mat2((<single_expr>).xy, (<single_expr>).zw)`.
///
/// GLSL allows `mat2(vec4)` to construct a 2×2 matrix from 4 components,
/// but naga's GLSL frontend doesn't support this constructor form.
/// This is extremely common in code-golfed ShaderToy shaders.
fn fix_mat2_from_vec4(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let pat: Vec<char> = "mat2(".chars().collect();
    let pat_len = pat.len();
    let mut i = 0;

    while i < len {
        if i + pat_len <= len && &chars[i..i + pat_len] == pat.as_slice() {
            // Check word boundary before `mat2(`
            let before_ok = if i == 0 {
                true
            } else {
                let c = chars[i - 1];
                !c.is_alphanumeric() && c != '_'
            };

            if before_ok {
                // Parse the argument list
                let args_start = i + pat_len; // position right after '('
                if let Some((args, end)) = parse_mat2_args(&chars, args_start) {
                    if args.len() == 1 && looks_like_vec4_expr(&args[0]) {
                        // Single argument that's likely a vec4 → rewrite
                        let expr = args[0].trim();
                        result.push_str(&format!("mat2(({expr}).xy, ({expr}).zw)"));
                        i = end;
                        continue;
                    }
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Parse comma-separated arguments inside `mat2(...)`, respecting nested parens.
/// `start` is the position right after the opening `(`.
/// Returns (vec_of_args, position_after_closing_paren).
fn parse_mat2_args(chars: &[char], start: usize) -> Option<(Vec<String>, usize)> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 1;
    let mut i = start;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    args.push(current.trim().to_string());
                    return Some((args, i + 1));
                }
                current.push(c);
            }
            ',' if depth == 1 => {
                args.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
        i += 1;
    }
    None
}

/// Heuristic: does this expression likely produce a vec4?
/// Matches common ShaderToy patterns like `cos(...)`, `sin(...)`, `vec4(...)`, etc.
fn looks_like_vec4_expr(expr: &str) -> bool {
    let trimmed = expr.trim();
    // If it contains `vec4` anywhere, it's likely vec4-producing
    if trimmed.contains("vec4") {
        return true;
    }
    // If it's a function call wrapping something with vec4 inside
    // e.g. cos(a+vec4(...))
    if trimmed.starts_with("cos(") || trimmed.starts_with("sin(")
        || trimmed.starts_with("abs(") || trimmed.starts_with("floor(")
        || trimmed.starts_with("ceil(") || trimmed.starts_with("fract(")
    {
        return true;
    }
    // If it's a single identifier, it might be anything — don't rewrite
    // If it has multiple args (contains comma at depth 0), it's not a single vec4
    false
}

/// Remove `f` suffixes from float literals (e.g. `0.5f` → `0.5`).
/// Some ShaderToy shaders use these but naga's GLSL parser may reject them.
fn strip_float_suffixes(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if (chars[i] == 'f' || chars[i] == 'F') && i > 0 {
            // Check if preceded by a digit or '.' (part of a float literal)
            let prev = chars[i - 1];
            let is_float_suffix = (prev.is_ascii_digit() || prev == '.')
                && (i + 1 >= len || {
                    let next = chars[i + 1];
                    !next.is_alphanumeric() && next != '_'
                });

            if is_float_suffix {
                // Skip the 'f' suffix
                i += 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Simple `#define` expansion for ShaderToy compatibility.
/// Supports object-like macros (`#define FOO 42`) and function-like macros
/// (`#define R(a) mat2(cos(a+vec4(0,33,11,0)))`).
fn expand_defines(source: &str) -> String {
    let mut defines: Vec<(String, Vec<String>, String)> = Vec::new(); // (name, params, body)
    let mut output_lines = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("#define ") {
            let rest = rest.trim();
            // Check for function-like macro: NAME(args)
            if let Some(paren_pos) = rest.find('(') {
                let name = rest[..paren_pos].trim().to_string();
                // Make sure there's no space before '(' — it must be right after the name
                if rest.as_bytes().get(paren_pos.saturating_sub(1)).map_or(false, |c| c.is_ascii_alphanumeric() || *c == b'_')
                    || paren_pos == name.len()
                {
                    if let Some(close_paren) = rest[paren_pos..].find(')') {
                        let params_str = &rest[paren_pos + 1..paren_pos + close_paren];
                        let params: Vec<String> = params_str.split(',').map(|s| s.trim().to_string()).collect();
                        let body = rest[paren_pos + close_paren + 1..].trim().to_string();
                        defines.push((name, params, body));
                        continue;
                    }
                }
            }
            // Object-like macro: NAME body
            if let Some(space_pos) = rest.find(|c: char| c.is_whitespace()) {
                let name = rest[..space_pos].to_string();
                let body = rest[space_pos..].trim().to_string();
                defines.push((name, Vec::new(), body));
            }
            continue;
        }
        output_lines.push(line.to_string());
    }

    // Expand macros in the remaining source
    let mut result = output_lines.join("\n");
    // Multiple passes to handle nested macros
    for _ in 0..4 {
        let prev = result.clone();
        for (name, params, body) in &defines {
            if params.is_empty() {
                // Object-like macro: whole-word replace
                result = replace_whole_word(&result, name, body);
            } else {
                // Function-like macro: find NAME(...) and expand
                result = expand_function_macro(&result, name, params, body);
            }
        }
        if result == prev {
            break;
        }
    }

    result
}

/// Expand a function-like macro invocation.
fn expand_function_macro(source: &str, name: &str, params: &[String], body: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    let name_len = name_chars.len();
    let mut i = 0;

    while i < chars.len() {
        // Try to match macro name
        if i + name_len < chars.len()
            && &chars[i..i + name_len] == name_chars.as_slice()
        {
            let before_ok = if i == 0 {
                true
            } else {
                let c = chars[i - 1];
                !c.is_alphanumeric() && c != '_'
            };

            if before_ok && chars[i + name_len] == '(' {
                // Parse arguments
                if let Some((args, end)) = parse_macro_args(&chars, i + name_len) {
                    if args.len() == params.len() {
                        // Substitute parameters in body
                        let mut expanded = body.to_string();
                        for (param, arg) in params.iter().zip(args.iter()) {
                            expanded = replace_whole_word(&expanded, param, arg);
                        }
                        result.push_str(&expanded);
                        i = end;
                        continue;
                    }
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Parse macro arguments from a `(...)` invocation, handling nested parens.
fn parse_macro_args(chars: &[char], start: usize) -> Option<(Vec<String>, usize)> {
    if chars.get(start) != Some(&'(') {
        return None;
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut i = start;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '(' => {
                depth += 1;
                if depth > 1 {
                    current.push(c);
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    args.push(current.trim().to_string());
                    return Some((args, i + 1));
                }
                current.push(c);
            }
            ',' if depth == 1 => {
                args.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
        i += 1;
    }

    None
}

/// Replace whole-word occurrences of `name` with `replacement`.
fn replace_whole_word(source: &str, name: &str, replacement: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    let name_len = name_chars.len();
    let mut i = 0;

    while i < chars.len() {
        if i + name_len <= chars.len() && &chars[i..i + name_len] == name_chars.as_slice() {
            let before_ok = if i == 0 {
                true
            } else {
                let c = chars[i - 1];
                !c.is_alphanumeric() && c != '_'
            };
            let after_ok = if i + name_len >= chars.len() {
                true
            } else {
                let c = chars[i + name_len];
                !c.is_alphanumeric() && c != '_'
            };

            if before_ok && after_ok {
                output.push_str(replacement);
                i += name_len;
                continue;
            }
        }
        output.push(chars[i]);
        i += 1;
    }

    output
}

/// Rewrite ShaderToy uniform references to use the UBO instance.
/// Replaces bare `iTime` with `_st.iTime` etc., being careful not to
/// replace inside identifiers (e.g. `myiTime` should not be touched).
fn preprocess_shadertoy_uniforms(source: &str) -> String {
    let uniforms = ["iResolution", "iTimeDelta", "iTime", "iMouse", "iFrame"];
    let mut result = source.to_string();

    for name in &uniforms {
        let replacement = format!("_st.{}", name);
        // Replace whole-word occurrences only.
        // A simple boundary check: the char before/after must not be alphanumeric or '_'.
        let mut output = String::with_capacity(result.len());
        let chars: Vec<char> = result.chars().collect();
        let name_chars: Vec<char> = name.chars().collect();
        let name_len = name_chars.len();
        let mut i = 0;

        while i < chars.len() {
            if i + name_len <= chars.len() && &chars[i..i + name_len] == name_chars.as_slice() {
                let before_ok = if i == 0 {
                    true
                } else {
                    let c = chars[i - 1];
                    !c.is_alphanumeric() && c != '_'
                };
                let after_ok = if i + name_len >= chars.len() {
                    true
                } else {
                    let c = chars[i + name_len];
                    !c.is_alphanumeric() && c != '_'
                };

                if before_ok && after_ok {
                    output.push_str(&replacement);
                    i += name_len;
                    continue;
                }
            }
            output.push(chars[i]);
            i += 1;
        }

        result = output;
    }

    result
}

/// Remap naga-generated UBO member access to our `uniforms.*` struct fields.
///
/// Naga translates the GLSL UBO instance `_st` into a WGSL `var<uniform>` named `_st`
/// with struct member access like `_st.member.iTime`. We remap these to
/// `uniforms.time`, `uniforms.resolution`, etc.
fn remap_uniforms(wgsl: &str) -> String {
    let mut result = wgsl.to_string();

    // Naga may generate `_st.member.X` or `_st.X` depending on version.
    // Handle both patterns. Order matters: iTimeDelta before iTime.
    for (from, to) in [
        ("_st.member.iTimeDelta", "uniforms.delta_time"),
        ("_st.member.iTime", "uniforms.time"),
        ("_st.member.iResolution", "vec3<f32>(uniforms.resolution, uniforms.resolution.x / uniforms.resolution.y)"),
        ("_st.member.iMouse", "uniforms.mouse"),
        ("_st.member.iFrame", "i32(uniforms.frame)"),
        ("_st.iTimeDelta", "uniforms.delta_time"),
        ("_st.iTime", "uniforms.time"),
        ("_st.iResolution", "vec3<f32>(uniforms.resolution, uniforms.resolution.x / uniforms.resolution.y)"),
        ("_st.iMouse", "uniforms.mouse"),
        ("_st.iFrame", "i32(uniforms.frame)"),
    ] {
        result = result.replace(from, to);
    }

    result
}

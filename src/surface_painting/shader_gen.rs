//! Dynamic splatmap shader generation.
//!
//! Generates a combined WGSL fragment shader that calls each layer's shader
//! function and blends results by splatmap weights.

use bevy::prelude::*;

use super::data::{MaterialLayer, PaintableSurfaceData};
use super::material::SPLATMAP_FRAG_SHADER_HANDLE;

/// Default splatmap shader (used at startup before any dynamic generation).
pub const DEFAULT_SPLATMAP_SHADER: &str = include_str!("../../assets/shaders/splatmap_blend.wgsl");

/// Generate a combined WGSL fragment shader that blends up to 4 layers.
///
/// Layers with a `cached_shader_source` get their `layer_main` function inlined.
/// Layers without a shader source use a grey fallback.
pub fn generate_splatmap_shader(layers: &[MaterialLayer]) -> String {
    let mut out = String::with_capacity(4096);

    // Header — unlit, only needs VertexOutput and globals for time
    out.push_str("// AUTO-GENERATED SPLATMAP SHADER\n");
    out.push_str("#import bevy_pbr::{\n");
    out.push_str("    mesh_view_bindings::globals,\n");
    out.push_str("    forward_io::VertexOutput,\n");
    out.push_str("}\n\n");

    // Material bindings (must match SplatmapMaterial AsBindGroup layout)
    // layer_colors are declared for bind group compatibility but unused
    for i in 0..4 {
        out.push_str(&format!(
            "@group(3) @binding({}) var<uniform> layer_colors_{}: vec4<f32>;\n",
            i, i
        ));
    }
    for i in 0..4 {
        out.push_str(&format!(
            "@group(3) @binding({}) var<uniform> layer_props_{}: vec4<f32>;\n",
            i + 4, i
        ));
    }
    out.push_str("@group(3) @binding(8) var splatmap_texture: texture_2d<f32>;\n");
    out.push_str("@group(3) @binding(9) var splatmap_sampler: sampler;\n\n");

    // Generate per-layer functions
    for i in 0..4 {
        let layer = layers.get(i);
        let has_shader = layer
            .and_then(|l| l.cached_shader_source.as_ref())
            .is_some();

        if has_shader {
            let source = layer.unwrap().cached_shader_source.as_ref().unwrap();
            let main_body = extract_function_body(source, "layer_main");

            // Inline the layer_main function with a unique name
            out.push_str(&format!(
                "fn layer_{}_main(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {{\n",
                i
            ));
            if let Some(body) = &main_body {
                out.push_str(body);
                out.push('\n');
            } else {
                // Extraction failed — grey fallback
                out.push_str("    return vec4<f32>(0.5, 0.5, 0.5, 1.0);\n");
            }
            out.push_str("}\n\n");

            // Wrapper
            out.push_str(&format!(
                "fn layer_{}(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {{\n",
                i
            ));
            out.push_str(&format!(
                "    return layer_{}_main(uv, world_pos, world_normal, time);\n",
                i
            ));
            out.push_str("}\n\n");
        } else {
            // No shader source — grey fallback
            out.push_str(&format!(
                "fn layer_{}(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {{\n",
                i
            ));
            out.push_str("    return vec4<f32>(0.5, 0.5, 0.5, 1.0);\n");
            out.push_str("}\n\n");
        }
    }

    // Fragment function — unlit blending
    out.push_str("@fragment\n");
    out.push_str("fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {\n");
    out.push_str("    let weights = textureSample(splatmap_texture, splatmap_sampler, in.uv);\n");
    out.push_str("    let total = weights.r + weights.g + weights.b + weights.a + 0.001;\n");
    out.push_str("    let w = weights / total;\n\n");
    out.push_str("    let time = globals.time;\n");
    out.push_str("    let world_pos = in.world_position.xyz;\n");
    out.push_str("    let world_normal = normalize(in.world_normal);\n\n");
    out.push_str("    let l0 = layer_0(in.uv, world_pos, world_normal, time);\n");
    out.push_str("    let l1 = layer_1(in.uv, world_pos, world_normal, time);\n");
    out.push_str("    let l2 = layer_2(in.uv, world_pos, world_normal, time);\n");
    out.push_str("    let l3 = layer_3(in.uv, world_pos, world_normal, time);\n\n");
    out.push_str("    let blended = l0 * w.r + l1 * w.g + l2 * w.b + l3 * w.a;\n");
    out.push_str("    return vec4<f32>(blended.rgb, 1.0);\n");
    out.push_str("}\n");

    out
}

/// Extract the body of a named function from WGSL source.
/// Returns the content between the outermost `{` and `}` of the function.
fn extract_function_body(source: &str, fn_name: &str) -> Option<String> {
    let pattern = format!("fn {}", fn_name);
    let start_idx = source.find(&pattern)?;

    // Find the opening brace
    let after_fn = &source[start_idx..];
    let brace_start = after_fn.find('{')?;
    let body_start = start_idx + brace_start + 1;

    // Track brace depth to find matching close
    let mut depth = 1i32;
    let mut end_idx = body_start;
    for (i, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = body_start + i;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return None;
    }

    Some(source[body_start..end_idx].to_string())
}

/// System that regenerates the splatmap shader when layer sources change.
pub fn splatmap_shader_regen_system(
    mut shaders: ResMut<Assets<Shader>>,
    mut paintable_query: Query<&mut PaintableSurfaceData>,
) {
    for mut surface in paintable_query.iter_mut() {
        if !surface.shader_dirty {
            continue;
        }

        // Load/cache .wgsl sources for layers that have paths
        for layer in &mut surface.layers {
            if let Some(path) = &layer.texture_path {
                if path.ends_with(".wgsl") && layer.cached_shader_source.is_none() {
                    match std::fs::read_to_string(path) {
                        Ok(source) => {
                            info!("Loaded layer shader: {}", path);
                            layer.cached_shader_source = Some(source);
                        }
                        Err(e) => {
                            warn!("Failed to read layer shader '{}': {}", path, e);
                        }
                    }
                }
            } else {
                layer.cached_shader_source = None;
            }
        }

        // Always generate the composed shader from layer sources
        let wgsl = generate_splatmap_shader(&surface.layers);
        info!("Generated splatmap shader ({} bytes)", wgsl.len());

        // Hot-reload by inserting at the UUID handle
        let _ = shaders.insert(
            &SPLATMAP_FRAG_SHADER_HANDLE,
            Shader::from_wgsl(wgsl, "splatmap://generated"),
        );

        surface.shader_dirty = false;
    }
}

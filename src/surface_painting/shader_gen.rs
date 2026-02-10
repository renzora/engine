//! Dynamic splatmap shader generation.
//!
//! Generates a combined WGSL fragment shader that calls each layer's shader
//! function and blends results by splatmap weights, with PBR lighting.

use bevy::prelude::*;

use super::data::{MaterialLayer, PaintableSurfaceData};
use super::material::SPLATMAP_FRAG_SHADER_HANDLE;

/// Default PBR splatmap shader (used at startup before any dynamic generation).
pub const DEFAULT_SPLATMAP_SHADER: &str = include_str!("../../assets/shaders/splatmap_blend.wgsl");

/// Generate a combined WGSL fragment shader that blends up to 4 layers.
///
/// Layers with a `cached_shader_source` get their `layer_main` function inlined.
/// Layers without a shader source use the uniform color fallback.
pub fn generate_splatmap_shader(layers: &[MaterialLayer]) -> String {
    let mut out = String::with_capacity(4096);

    // Header + imports
    out.push_str("// AUTO-GENERATED SPLATMAP SHADER\n");
    out.push_str("#import bevy_pbr::{\n");
    out.push_str("    pbr_functions::apply_pbr_lighting,\n");
    out.push_str("    pbr_types::PbrInput,\n");
    out.push_str("    pbr_types::pbr_input_new,\n");
    out.push_str("    mesh_view_bindings::globals,\n");
    out.push_str("    forward_io::VertexOutput,\n");
    out.push_str("}\n\n");

    // Material bindings (must match SplatmapMaterial AsBindGroup layout)
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

    // LayerResult struct
    out.push_str("struct LayerResult {\n");
    out.push_str("    color: vec4<f32>,\n");
    out.push_str("    metallic: f32,\n");
    out.push_str("    roughness: f32,\n");
    out.push_str("}\n\n");

    // Generate per-layer functions
    for i in 0..4 {
        let layer = layers.get(i);
        let has_shader = layer
            .and_then(|l| l.cached_shader_source.as_ref())
            .is_some();

        if has_shader {
            let source = layer.unwrap().cached_shader_source.as_ref().unwrap();
            let extracted = extract_layer_functions(source);

            // Inline the layer_main function with a unique name
            out.push_str(&format!(
                "fn layer_{}_main(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {{\n",
                i
            ));
            if let Some(body) = &extracted.main_body {
                out.push_str(body);
                out.push('\n');
            } else {
                // Fallback: use uniform color if extraction failed
                out.push_str(&format!("    return layer_colors_{};\n", i));
            }
            out.push_str("}\n\n");

            // Inline the optional layer_pbr function
            let has_pbr = extracted.pbr_body.is_some();
            if let Some(pbr_body) = &extracted.pbr_body {
                out.push_str(&format!(
                    "fn layer_{}_pbr(uv: vec2<f32>, world_pos: vec3<f32>) -> vec2<f32> {{\n",
                    i
                ));
                out.push_str(pbr_body);
                out.push('\n');
                out.push_str("}\n\n");
            }

            // Wrapper function
            out.push_str(&format!(
                "fn layer_{}(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> LayerResult {{\n",
                i
            ));
            out.push_str(&format!(
                "    let c = layer_{}_main(uv, world_pos, world_normal, time);\n",
                i
            ));
            if has_pbr {
                out.push_str(&format!(
                    "    let pbr = layer_{}_pbr(uv, world_pos);\n",
                    i
                ));
                out.push_str("    return LayerResult(c, pbr.x, pbr.y);\n");
            } else {
                out.push_str(&format!(
                    "    return LayerResult(c, layer_props_{}.x, layer_props_{}.y);\n",
                    i, i
                ));
            }
            out.push_str("}\n\n");
        } else {
            // Color-only layer: use uniform color and props
            out.push_str(&format!(
                "fn layer_{}(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> LayerResult {{\n",
                i
            ));
            out.push_str(&format!(
                "    return LayerResult(layer_colors_{}, layer_props_{}.x, layer_props_{}.y);\n",
                i, i, i
            ));
            out.push_str("}\n\n");
        }
    }

    // Fragment function
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
    out.push_str("    var pbr_input: PbrInput = pbr_input_new();\n");
    out.push_str("    pbr_input.material.base_color = l0.color * w.r + l1.color * w.g + l2.color * w.b + l3.color * w.a;\n");
    out.push_str("    pbr_input.material.metallic = l0.metallic * w.r + l1.metallic * w.g + l2.metallic * w.b + l3.metallic * w.a;\n");
    out.push_str("    pbr_input.material.perceptual_roughness = l0.roughness * w.r + l1.roughness * w.g + l2.roughness * w.b + l3.roughness * w.a;\n");
    out.push_str("    pbr_input.diffuse_occlusion = vec3<f32>(1.0);\n");
    out.push_str("    pbr_input.world_normal = world_normal;\n");
    out.push_str("    pbr_input.world_position = in.world_position;\n");
    out.push_str("    pbr_input.frag_coord = in.position;\n\n");
    out.push_str("    var color = apply_pbr_lighting(pbr_input);\n");
    out.push_str("    color.a = 1.0;\n");
    out.push_str("    return color;\n");
    out.push_str("}\n");

    out
}

/// Extracted function bodies from a layer .wgsl file.
struct ExtractedLayerFunctions {
    /// Body of `fn layer_main(...)` (everything inside the braces)
    main_body: Option<String>,
    /// Body of `fn layer_pbr(...)` if present
    pbr_body: Option<String>,
}

/// Extract the bodies of `fn layer_main(...)` and optionally `fn layer_pbr(...)`
/// from a `.wgsl` layer shader source.
fn extract_layer_functions(source: &str) -> ExtractedLayerFunctions {
    ExtractedLayerFunctions {
        main_body: extract_function_body(source, "layer_main"),
        pbr_body: extract_function_body(source, "layer_pbr"),
    }
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
        let mut any_shader = false;
        for layer in &mut surface.layers {
            if let Some(path) = &layer.texture_path {
                if path.ends_with(".wgsl") && layer.cached_shader_source.is_none() {
                    // Try to read from disk
                    match std::fs::read_to_string(path) {
                        Ok(source) => {
                            layer.cached_shader_source = Some(source);
                        }
                        Err(e) => {
                            warn!("Failed to read layer shader '{}': {}", path, e);
                        }
                    }
                }
                if layer.cached_shader_source.is_some() {
                    any_shader = true;
                }
            } else {
                layer.cached_shader_source = None;
            }
        }

        // Generate combined shader
        let wgsl = if any_shader {
            generate_splatmap_shader(&surface.layers)
        } else {
            // No custom shaders — use the default PBR fallback
            DEFAULT_SPLATMAP_SHADER.to_string()
        };

        // Hot-reload by inserting at the UUID handle
        let _ = shaders.insert(
            &SPLATMAP_FRAG_SHADER_HANDLE,
            Shader::from_wgsl(wgsl, "splatmap://generated"),
        );

        surface.shader_dirty = false;
    }
}

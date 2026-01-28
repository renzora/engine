//! WGSL code generation from material blueprint graphs
//!
//! Compiles a BlueprintGraph (with graph_type == Material) into WGSL shader code
//! that can be used with Bevy's material system.

use std::collections::{HashMap, HashSet};
use super::{BlueprintGraph, BlueprintNode, NodeId, PinId, PinValue};

/// Result of WGSL code generation
pub struct WgslCodegenResult {
    /// Generated fragment shader code
    pub fragment_shader: String,
    /// Texture bindings required by the shader
    pub texture_bindings: Vec<TextureBinding>,
    /// Uniform bindings required by the shader
    pub uniform_bindings: Vec<UniformBinding>,
    /// Whether this is a PBR or unlit material
    pub is_pbr: bool,
    /// Errors encountered during generation
    pub errors: Vec<String>,
    /// Warnings
    pub warnings: Vec<String>,
}

/// A texture binding used by the shader
#[derive(Debug, Clone)]
pub struct TextureBinding {
    /// Variable name in the shader
    pub name: String,
    /// Binding index
    pub binding: u32,
    /// Asset path to the texture
    pub asset_path: String,
}

/// A uniform binding used by the shader
#[derive(Debug, Clone)]
pub struct UniformBinding {
    /// Variable name in the shader
    pub name: String,
    /// Binding index
    pub binding: u32,
    /// WGSL type (e.g., "f32", "vec3<f32>")
    pub wgsl_type: String,
}

/// Context for WGSL code generation
struct WgslCodegenContext<'a> {
    graph: &'a BlueprintGraph,
    /// Generated variable names for node outputs
    output_vars: HashMap<PinId, String>,
    /// Counter for generating unique variable names
    var_counter: usize,
    /// Set of nodes that have been processed
    processed_nodes: HashSet<NodeId>,
    /// Texture bindings accumulated during generation
    texture_bindings: Vec<TextureBinding>,
    /// Next texture binding index
    next_texture_binding: u32,
    /// Code lines for the fragment function body
    fragment_lines: Vec<String>,
    /// Whether noise functions are used (need to include helpers)
    uses_noise: bool,
}

impl<'a> WgslCodegenContext<'a> {
    fn new(graph: &'a BlueprintGraph) -> Self {
        Self {
            graph,
            output_vars: HashMap::new(),
            var_counter: 0,
            processed_nodes: HashSet::new(),
            texture_bindings: Vec::new(),
            next_texture_binding: 1, // 0 is typically for uniforms
            fragment_lines: Vec::new(),
            uses_noise: false,
        }
    }

    fn next_var(&mut self, prefix: &str) -> String {
        let name = format!("{}_{}", prefix, self.var_counter);
        self.var_counter += 1;
        name
    }

    /// Get the value expression for an input pin
    fn get_input_value(&mut self, node: &BlueprintNode, pin_name: &str) -> String {
        let pin_id = PinId::input(node.id, pin_name);

        // Check for connection
        if let Some(conn) = self.graph.connection_to(&pin_id) {
            if let Some(var_name) = self.output_vars.get(&conn.from) {
                return var_name.clone();
            }

            // Generate source node
            if let Some(source_node) = self.graph.get_node(conn.from.node_id) {
                self.generate_node(source_node);
                if let Some(var_name) = self.output_vars.get(&conn.from) {
                    return var_name.clone();
                }
            }
        }

        // Use default value from input_values or pin default
        if let Some(value) = node.get_input_value(pin_name) {
            return value.to_wgsl();
        }

        // Fallback
        "0.0".to_string()
    }

    /// Generate WGSL code for a node
    fn generate_node(&mut self, node: &BlueprintNode) {
        if self.processed_nodes.contains(&node.id) {
            return;
        }
        self.processed_nodes.insert(node.id);

        match node.node_type.as_str() {
            // ==================== INPUT NODES ====================
            "shader/uv" => {
                self.output_vars.insert(PinId::output(node.id, "uv"), "in.uv".to_string());
                self.output_vars.insert(PinId::output(node.id, "u"), "in.uv.x".to_string());
                self.output_vars.insert(PinId::output(node.id, "v"), "in.uv.y".to_string());
            }

            "shader/world_position" => {
                self.output_vars.insert(PinId::output(node.id, "position"), "in.world_position".to_string());
                self.output_vars.insert(PinId::output(node.id, "x"), "in.world_position.x".to_string());
                self.output_vars.insert(PinId::output(node.id, "y"), "in.world_position.y".to_string());
                self.output_vars.insert(PinId::output(node.id, "z"), "in.world_position.z".to_string());
            }

            "shader/world_normal" => {
                self.output_vars.insert(PinId::output(node.id, "normal"), "in.world_normal".to_string());
                self.output_vars.insert(PinId::output(node.id, "x"), "in.world_normal.x".to_string());
                self.output_vars.insert(PinId::output(node.id, "y"), "in.world_normal.y".to_string());
                self.output_vars.insert(PinId::output(node.id, "z"), "in.world_normal.z".to_string());
            }

            "shader/view_direction" => {
                let var = self.next_var("view_dir");
                self.fragment_lines.push(format!(
                    "    let {} = normalize(view.world_position.xyz - in.world_position);",
                    var
                ));
                self.output_vars.insert(PinId::output(node.id, "direction"), var);
            }

            "shader/time" => {
                self.output_vars.insert(PinId::output(node.id, "time"), "globals.time".to_string());
                let sin_var = self.next_var("sin_time");
                let cos_var = self.next_var("cos_time");
                self.fragment_lines.push(format!("    let {} = sin(globals.time);", sin_var));
                self.fragment_lines.push(format!("    let {} = cos(globals.time);", cos_var));
                self.output_vars.insert(PinId::output(node.id, "sin_time"), sin_var);
                self.output_vars.insert(PinId::output(node.id, "cos_time"), cos_var);
            }

            "shader/vertex_color" => {
                // Note: vertex colors may need mesh attribute support
                self.output_vars.insert(PinId::output(node.id, "color"), "in.color".to_string());
                self.output_vars.insert(PinId::output(node.id, "r"), "in.color.r".to_string());
                self.output_vars.insert(PinId::output(node.id, "g"), "in.color.g".to_string());
                self.output_vars.insert(PinId::output(node.id, "b"), "in.color.b".to_string());
                self.output_vars.insert(PinId::output(node.id, "a"), "in.color.a".to_string());
            }

            // ==================== TEXTURE NODES ====================
            "shader/texture" => {
                let path = node.input_values.get("path")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.clone()) } else { None })
                    .or_else(|| node.input_values.get("texture")
                        .and_then(|v| if let PinValue::Texture2D(s) = v { Some(s.clone()) } else { None }))
                    .unwrap_or_default();

                let binding = self.next_texture_binding;
                self.next_texture_binding += 1;

                let tex_name = format!("material_texture_{}", binding);
                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding,
                    asset_path: path,
                });

                self.output_vars.insert(PinId::output(node.id, "texture"), tex_name);
            }

            "shader/sample_texture" => {
                let tex = self.get_input_value(node, "texture");
                let uv = self.get_input_value(node, "uv");

                let color_var = self.next_var("tex_sample");
                self.fragment_lines.push(format!(
                    "    let {} = textureSample({}, {}_sampler, {});",
                    color_var, tex, tex, uv
                ));

                self.output_vars.insert(PinId::output(node.id, "color"), color_var.clone());
                self.output_vars.insert(PinId::output(node.id, "rgb"), format!("{}.rgb", color_var));
                self.output_vars.insert(PinId::output(node.id, "r"), format!("{}.r", color_var));
                self.output_vars.insert(PinId::output(node.id, "g"), format!("{}.g", color_var));
                self.output_vars.insert(PinId::output(node.id, "b"), format!("{}.b", color_var));
                self.output_vars.insert(PinId::output(node.id, "a"), format!("{}.a", color_var));
            }

            // ==================== MATH NODES ====================
            "math/add" | "shader/add" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("add");
                self.fragment_lines.push(format!("    let {} = {} + {};", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/subtract" | "shader/subtract" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("sub");
                self.fragment_lines.push(format!("    let {} = {} - {};", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/multiply" | "shader/multiply" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("mul");
                self.fragment_lines.push(format!("    let {} = {} * {};", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/divide" | "shader/divide" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("div");
                self.fragment_lines.push(format!("    let {} = {} / {};", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/lerp" | "shader/lerp" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("lerp");
                self.fragment_lines.push(format!("    let {} = mix({}, {}, {});", result_var, a, b, t));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/clamp" | "shader/clamp" => {
                let value = self.get_input_value(node, "value");
                let min_val = self.get_input_value(node, "min");
                let max_val = self.get_input_value(node, "max");
                let result_var = self.next_var("clamp");
                self.fragment_lines.push(format!("    let {} = clamp({}, {}, {});", result_var, value, min_val, max_val));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/abs" | "shader/abs" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("abs");
                self.fragment_lines.push(format!("    let {} = abs({});", result_var, value));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/min" | "shader/min" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("min");
                self.fragment_lines.push(format!("    let {} = min({}, {});", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/max" | "shader/max" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("max");
                self.fragment_lines.push(format!("    let {} = max({}, {});", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/sin" | "shader/sin" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("sin");
                self.fragment_lines.push(format!("    let {} = sin({});", result_var, value));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/cos" | "shader/cos" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("cos");
                self.fragment_lines.push(format!("    let {} = cos({});", result_var, value));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/dot" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("dot");
                self.fragment_lines.push(format!("    let {} = dot({}, {});", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/cross" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("cross");
                self.fragment_lines.push(format!("    let {} = cross({}, {});", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/normalize" => {
                let v = self.get_input_value(node, "v");
                let result_var = self.next_var("norm");
                self.fragment_lines.push(format!("    let {} = normalize({});", result_var, v));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/length" => {
                let v = self.get_input_value(node, "v");
                let result_var = self.next_var("len");
                self.fragment_lines.push(format!("    let {} = length({});", result_var, v));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/distance" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("dist");
                self.fragment_lines.push(format!("    let {} = distance({}, {});", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/reflect" => {
                let incident = self.get_input_value(node, "incident");
                let normal = self.get_input_value(node, "normal");
                let result_var = self.next_var("reflect");
                self.fragment_lines.push(format!("    let {} = reflect({}, {});", result_var, incident, normal));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/fresnel" => {
                let normal = self.get_input_value(node, "normal");
                let view = self.get_input_value(node, "view");
                let power = self.get_input_value(node, "power");
                let result_var = self.next_var("fresnel");
                self.fragment_lines.push(format!(
                    "    let {} = pow(1.0 - saturate(dot({}, {})), {});",
                    result_var, normal, view, power
                ));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/pow" => {
                let base = self.get_input_value(node, "base");
                let exp = self.get_input_value(node, "exp");
                let result_var = self.next_var("pow");
                self.fragment_lines.push(format!("    let {} = pow({}, {});", result_var, base, exp));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/smoothstep" => {
                let edge0 = self.get_input_value(node, "edge0");
                let edge1 = self.get_input_value(node, "edge1");
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("smooth");
                self.fragment_lines.push(format!("    let {} = smoothstep({}, {}, {});", result_var, edge0, edge1, x));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/step" => {
                let edge = self.get_input_value(node, "edge");
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("step");
                self.fragment_lines.push(format!("    let {} = step({}, {});", result_var, edge, x));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/fract" => {
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("fract");
                self.fragment_lines.push(format!("    let {} = fract({});", result_var, x));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/floor" => {
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("floor");
                self.fragment_lines.push(format!("    let {} = floor({});", result_var, x));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/ceil" => {
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("ceil");
                self.fragment_lines.push(format!("    let {} = ceil({});", result_var, x));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/one_minus" => {
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("one_minus");
                self.fragment_lines.push(format!("    let {} = 1.0 - {};", result_var, x));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/saturate" => {
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("saturate");
                self.fragment_lines.push(format!("    let {} = saturate({});", result_var, x));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            // ==================== VECTOR NODES ====================
            "shader/make_vec2" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let result_var = self.next_var("vec2");
                self.fragment_lines.push(format!("    let {} = vec2<f32>({}, {});", result_var, x, y));
                self.output_vars.insert(PinId::output(node.id, "v"), result_var);
            }

            "shader/make_vec3" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let result_var = self.next_var("vec3");
                self.fragment_lines.push(format!("    let {} = vec3<f32>({}, {}, {});", result_var, x, y, z));
                self.output_vars.insert(PinId::output(node.id, "v"), result_var);
            }

            "shader/make_vec4" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let w = self.get_input_value(node, "w");
                let result_var = self.next_var("vec4");
                self.fragment_lines.push(format!("    let {} = vec4<f32>({}, {}, {}, {});", result_var, x, y, z, w));
                self.output_vars.insert(PinId::output(node.id, "v"), result_var);
            }

            "shader/make_color" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                let result_var = self.next_var("color");
                self.fragment_lines.push(format!("    let {} = vec4<f32>({}, {}, {}, {});", result_var, r, g, b, a));
                self.output_vars.insert(PinId::output(node.id, "color"), result_var);
            }

            "shader/split_vec2" => {
                let v = self.get_input_value(node, "v");
                self.output_vars.insert(PinId::output(node.id, "x"), format!("{}.x", v));
                self.output_vars.insert(PinId::output(node.id, "y"), format!("{}.y", v));
            }

            "shader/split_vec3" => {
                let v = self.get_input_value(node, "v");
                self.output_vars.insert(PinId::output(node.id, "x"), format!("{}.x", v));
                self.output_vars.insert(PinId::output(node.id, "y"), format!("{}.y", v));
                self.output_vars.insert(PinId::output(node.id, "z"), format!("{}.z", v));
            }

            "shader/split_color" => {
                let color = self.get_input_value(node, "color");
                self.output_vars.insert(PinId::output(node.id, "r"), format!("{}.r", color));
                self.output_vars.insert(PinId::output(node.id, "g"), format!("{}.g", color));
                self.output_vars.insert(PinId::output(node.id, "b"), format!("{}.b", color));
                self.output_vars.insert(PinId::output(node.id, "a"), format!("{}.a", color));
            }

            "shader/color" => {
                let color = self.get_input_value(node, "color");
                self.output_vars.insert(PinId::output(node.id, "color"), color.clone());
                // For rgb output, extract the xyz/rgb components
                let rgb_var = self.next_var("rgb");
                self.fragment_lines.push(format!("    let {} = {}.rgb;", rgb_var, color));
                self.output_vars.insert(PinId::output(node.id, "rgb"), rgb_var);
            }

            "shader/float" => {
                let value = self.get_input_value(node, "value");
                self.output_vars.insert(PinId::output(node.id, "value"), value);
            }

            // ==================== NOISE NODES ====================
            "shader/noise_simple" => {
                self.uses_noise = true;
                let uv = self.get_input_value(node, "uv");
                let scale = self.get_input_value(node, "scale");
                let result_var = self.next_var("noise");
                self.fragment_lines.push(format!("    let {} = simple_noise({} * {});", result_var, uv, scale));
                self.output_vars.insert(PinId::output(node.id, "value"), result_var);
            }

            "shader/noise_gradient" => {
                self.uses_noise = true;
                let uv = self.get_input_value(node, "uv");
                let scale = self.get_input_value(node, "scale");
                let result_var = self.next_var("gnoise");
                self.fragment_lines.push(format!("    let {} = gradient_noise({} * {});", result_var, uv, scale));
                self.output_vars.insert(PinId::output(node.id, "value"), result_var);
            }

            "shader/noise_voronoi" => {
                self.uses_noise = true;
                let uv = self.get_input_value(node, "uv");
                let scale = self.get_input_value(node, "scale");
                let dist_var = self.next_var("vor_dist");
                let cell_var = self.next_var("vor_cell");
                self.fragment_lines.push(format!("    let vor_result_{} = voronoi_noise({} * {});", self.var_counter, uv, scale));
                self.fragment_lines.push(format!("    let {} = vor_result_{}.x;", dist_var, self.var_counter));
                self.fragment_lines.push(format!("    let {} = vor_result_{}.y;", cell_var, self.var_counter));
                self.var_counter += 1;
                self.output_vars.insert(PinId::output(node.id, "distance"), dist_var);
                self.output_vars.insert(PinId::output(node.id, "cell"), cell_var);
            }

            "shader/checkerboard" => {
                let uv = self.get_input_value(node, "uv");
                let scale = self.get_input_value(node, "scale");
                let result_var = self.next_var("checker");
                self.fragment_lines.push(format!(
                    "    let {} = abs(fract({}.x * {}) - 0.5) + abs(fract({}.y * {}) - 0.5);",
                    result_var, uv, scale, uv, scale
                ));
                let final_var = self.next_var("checker_final");
                self.fragment_lines.push(format!(
                    "    let {} = step(0.5, fract(floor({}.x * {}) + floor({}.y * {})));",
                    final_var, uv, scale, uv, scale
                ));
                self.output_vars.insert(PinId::output(node.id, "value"), final_var);
            }

            "shader/gradient" => {
                let uv = self.get_input_value(node, "uv");
                let direction = self.get_input_value(node, "direction");
                let result_var = self.next_var("gradient");
                self.fragment_lines.push(format!("    let {} = dot({}, normalize({}));", result_var, uv, direction));
                self.output_vars.insert(PinId::output(node.id, "value"), result_var);
            }

            _ => {}
        }
    }
}

/// Generate noise helper functions for WGSL
fn generate_noise_helpers() -> String {
    r#"
// Simple hash function for noise
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// Simple noise
fn simple_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(hash21(i + vec2<f32>(0.0, 0.0)), hash21(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

// Gradient noise (Perlin-like)
fn gradient_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);

    return mix(
        mix(
            dot(hash22(i + vec2<f32>(0.0, 0.0)) * 2.0 - 1.0, f - vec2<f32>(0.0, 0.0)),
            dot(hash22(i + vec2<f32>(1.0, 0.0)) * 2.0 - 1.0, f - vec2<f32>(1.0, 0.0)),
            u.x
        ),
        mix(
            dot(hash22(i + vec2<f32>(0.0, 1.0)) * 2.0 - 1.0, f - vec2<f32>(0.0, 1.0)),
            dot(hash22(i + vec2<f32>(1.0, 1.0)) * 2.0 - 1.0, f - vec2<f32>(1.0, 1.0)),
            u.x
        ),
        u.y
    ) * 0.5 + 0.5;
}

// Voronoi noise - returns (distance, cell_id)
fn voronoi_noise(p: vec2<f32>) -> vec2<f32> {
    let n = floor(p);
    let f = fract(p);

    var min_dist = 8.0;
    var cell_id = 0.0;

    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let g = vec2<f32>(f32(i), f32(j));
            let o = hash22(n + g);
            let r = g + o - f;
            let d = dot(r, r);

            if (d < min_dist) {
                min_dist = d;
                cell_id = hash21(n + g);
            }
        }
    }

    return vec2<f32>(sqrt(min_dist), cell_id);
}
"#.to_string()
}

/// Generate WGSL code from a material blueprint graph
pub fn generate_wgsl_code(graph: &BlueprintGraph) -> WgslCodegenResult {
    let mut ctx = WgslCodegenContext::new(graph);
    let mut errors = Vec::new();
    let warnings = Vec::new();

    // Find the output node
    let output_node = graph.nodes.iter()
        .find(|n| n.node_type == "shader/pbr_output" || n.node_type == "shader/unlit_output");

    let Some(output_node) = output_node else {
        errors.push("Material blueprint must have an output node (PBR Output or Unlit Output)".to_string());
        return WgslCodegenResult {
            fragment_shader: String::new(),
            texture_bindings: Vec::new(),
            uniform_bindings: Vec::new(),
            is_pbr: false,
            errors,
            warnings,
        };
    };

    let is_pbr = output_node.node_type == "shader/pbr_output";

    // Generate code by processing the output node (which will recursively process dependencies)
    if is_pbr {
        // Get all PBR inputs
        let base_color = ctx.get_input_value(output_node, "base_color");
        let metallic = ctx.get_input_value(output_node, "metallic");
        let roughness = ctx.get_input_value(output_node, "roughness");
        let emissive = ctx.get_input_value(output_node, "emissive");
        let ao = ctx.get_input_value(output_node, "ao");
        let alpha = ctx.get_input_value(output_node, "alpha");

        // Check for normal map input
        let has_normal = graph.connection_to(&PinId::input(output_node.id, "normal")).is_some();
        let normal = if has_normal {
            ctx.get_input_value(output_node, "normal")
        } else {
            "in.world_normal".to_string()
        };

        // Generate PBR output
        ctx.fragment_lines.push(String::new());
        ctx.fragment_lines.push("    // PBR Output".to_string());
        ctx.fragment_lines.push(format!("    var pbr_input: PbrInput = pbr_input_new();"));
        ctx.fragment_lines.push(format!("    pbr_input.material.base_color = {};", base_color));
        ctx.fragment_lines.push(format!("    pbr_input.material.metallic = {};", metallic));
        ctx.fragment_lines.push(format!("    pbr_input.material.perceptual_roughness = {};", roughness));
        ctx.fragment_lines.push(format!("    pbr_input.material.emissive = {}.rgb * {}.a;", emissive, emissive));
        ctx.fragment_lines.push(format!("    pbr_input.occlusion = vec3<f32>({});", ao));
        ctx.fragment_lines.push(format!("    pbr_input.world_normal = normalize({});", normal));
        ctx.fragment_lines.push(format!("    pbr_input.world_position = vec4<f32>(in.world_position, 1.0);"));
        ctx.fragment_lines.push(format!("    pbr_input.frag_coord = in.position;"));
        ctx.fragment_lines.push(String::new());
        ctx.fragment_lines.push(format!("    var color = pbr(pbr_input);"));
        ctx.fragment_lines.push(format!("    color.a = {};", alpha));
        ctx.fragment_lines.push("    return color;".to_string());
    } else {
        // Unlit output
        let color = ctx.get_input_value(output_node, "color");
        let alpha = ctx.get_input_value(output_node, "alpha");

        ctx.fragment_lines.push(String::new());
        ctx.fragment_lines.push("    // Unlit Output".to_string());
        ctx.fragment_lines.push(format!("    return vec4<f32>({}.rgb, {});", color, alpha));
    }

    // Generate texture bindings
    let mut binding_declarations = Vec::new();
    for binding in &ctx.texture_bindings {
        binding_declarations.push(format!(
            "@group(2) @binding({}) var {}: texture_2d<f32>;",
            binding.binding, binding.name
        ));
        binding_declarations.push(format!(
            "@group(2) @binding({}) var {}_sampler: sampler;",
            binding.binding + 100, binding.name
        ));
    }

    // Assemble full fragment shader
    let noise_helpers = if ctx.uses_noise {
        generate_noise_helpers()
    } else {
        String::new()
    };

    let pbr_imports = if is_pbr {
        r#"#import bevy_pbr::{
    pbr_functions::pbr,
    pbr_types::PbrInput,
    pbr_types::pbr_input_new,
    mesh_view_bindings::view,
    mesh_view_bindings::globals,
}
"#
    } else {
        "#import bevy_pbr::mesh_view_bindings::globals\n"
    };

    let fragment_shader = format!(
        r#"// Auto-generated by Material Blueprint
// DO NOT EDIT - changes will be overwritten

{pbr_imports}
{bindings}
{noise}
struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}};

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {{
{body}
}}
"#,
        pbr_imports = pbr_imports,
        bindings = binding_declarations.join("\n"),
        noise = noise_helpers,
        body = ctx.fragment_lines.join("\n")
    );

    WgslCodegenResult {
        fragment_shader,
        texture_bindings: ctx.texture_bindings,
        uniform_bindings: Vec::new(),
        is_pbr,
        errors,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::{BlueprintGraph, BlueprintType};

    #[test]
    fn test_empty_material_error() {
        let graph = BlueprintGraph::new_material("test");
        let result = generate_wgsl_code(&graph);
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("output node"));
    }
}

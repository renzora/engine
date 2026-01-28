# Material Blueprint System Design

This document outlines how to extend the blueprint system to support visual shader/material creation.

## Overview

The material blueprint system reuses the existing `BlueprintGraph` infrastructure but:
- Uses different node types (shader-specific)
- Compiles to WGSL instead of Rhai
- Produces a Bevy `Material` asset instead of entity behavior

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      BlueprintGraph                              │
│                                                                  │
│  ┌──────────────┐                    ┌──────────────┐           │
│  │ graph_type:  │                    │ graph_type:  │           │
│  │ Behavior     │                    │ Material     │           │
│  └──────┬───────┘                    └──────┬───────┘           │
│         │                                   │                    │
│         ▼                                   ▼                    │
│  ┌──────────────┐                    ┌──────────────┐           │
│  │ codegen_rhai │                    │ codegen_wgsl │           │
│  └──────┬───────┘                    └──────┬───────┘           │
│         │                                   │                    │
│         ▼                                   ▼                    │
│    .rhai script                     WGSL shader code            │
│         │                                   │                    │
│         ▼                                   ▼                    │
│   Rhai execution                    Bevy Material asset          │
└─────────────────────────────────────────────────────────────────┘
```

## Graph Type Extension

```rust
// In graph.rs - add to BlueprintGraph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlueprintType {
    #[default]
    Behavior,  // Compiles to Rhai
    Material,  // Compiles to WGSL
}

pub struct BlueprintGraph {
    pub name: String,
    pub graph_type: BlueprintType,  // NEW
    pub nodes: Vec<BlueprintNode>,
    pub connections: Vec<Connection>,
    pub variables: Vec<BlueprintVariable>,
    next_node_id: u64,
}
```

## Material Node Types

### Category: `input/` - Vertex & Fragment Inputs

```rust
// UV coordinates (0-1 range)
"shader/uv" => {
    outputs: [
        ("uv", Vec2),      // Full UV
        ("u", Float),      // U component
        ("v", Float),      // V component
    ]
}

// World position of fragment
"shader/world_position" => {
    outputs: [
        ("position", Vec3),
        ("x", Float),
        ("y", Float),
        ("z", Float),
    ]
}

// Surface normal
"shader/normal" => {
    outputs: [
        ("normal", Vec3),
        ("x", Float),
        ("y", Float),
        ("z", Float),
    ]
}

// View direction (camera to fragment)
"shader/view_direction" => {
    outputs: [("direction", Vec3)]
}

// Time (for animated materials)
"shader/time" => {
    outputs: [
        ("time", Float),      // Total elapsed
        ("sin_time", Float),  // sin(time)
        ("cos_time", Float),  // cos(time)
    ]
}

// Camera position
"shader/camera_position" => {
    outputs: [("position", Vec3)]
}

// Vertex color (if mesh has vertex colors)
"shader/vertex_color" => {
    outputs: [("color", Color)]
}
```

### Category: `texture/` - Texture Sampling

```rust
// Sample a 2D texture
"shader/sample_texture" => {
    inputs: [
        ("texture", Texture2D),  // NEW pin type
        ("uv", Vec2),
    ],
    outputs: [
        ("color", Color),   // RGBA
        ("rgb", Vec3),      // RGB only
        ("r", Float),
        ("g", Float),
        ("b", Float),
        ("a", Float),
    ]
}

// Texture asset reference (drag from asset browser)
"shader/texture" => {
    properties: [("path", String)],  // Asset path
    outputs: [("texture", Texture2D)]
}
```

### Category: `math/` - Reuse Existing + Add Shader-Specific

Reuse from behavior blueprints:
- `math/add`, `math/subtract`, `math/multiply`, `math/divide`
- `math/lerp`, `math/clamp`, `math/abs`, `math/min`, `math/max`
- `math/sin`, `math/cos`

Add shader-specific:
```rust
"shader/dot" => {
    inputs: [("a", Vec3), ("b", Vec3)],
    outputs: [("result", Float)]
}

"shader/cross" => {
    inputs: [("a", Vec3), ("b", Vec3)],
    outputs: [("result", Vec3)]
}

"shader/normalize" => {
    inputs: [("v", Vec3)],
    outputs: [("result", Vec3)]
}

"shader/length" => {
    inputs: [("v", Vec3)],
    outputs: [("result", Float)]
}

"shader/reflect" => {
    inputs: [("incident", Vec3), ("normal", Vec3)],
    outputs: [("result", Vec3)]
}

"shader/fresnel" => {
    inputs: [
        ("normal", Vec3),
        ("view", Vec3),
        ("power", Float, default=5.0),
    ],
    outputs: [("result", Float)]
}

"shader/pow" => {
    inputs: [("base", Float), ("exp", Float)],
    outputs: [("result", Float)]
}

"shader/smoothstep" => {
    inputs: [("edge0", Float), ("edge1", Float), ("x", Float)],
    outputs: [("result", Float)]
}

"shader/fract" => {
    inputs: [("x", Float)],
    outputs: [("result", Float)]
}

"shader/floor" => {
    inputs: [("x", Float)],
    outputs: [("result", Float)]
}
```

### Category: `vector/` - Vector Operations

```rust
"shader/split_vec2" => {
    inputs: [("v", Vec2)],
    outputs: [("x", Float), ("y", Float)]
}

"shader/split_vec3" => {
    inputs: [("v", Vec3)],
    outputs: [("x", Float), ("y", Float), ("z", Float)]
}

"shader/split_color" => {
    inputs: [("color", Color)],
    outputs: [("r", Float), ("g", Float), ("b", Float), ("a", Float)]
}

"shader/make_vec2" => {
    inputs: [("x", Float), ("y", Float)],
    outputs: [("v", Vec2)]
}

"shader/make_vec3" => {
    inputs: [("x", Float), ("y", Float), ("z", Float)],
    outputs: [("v", Vec3)]
}

"shader/make_color" => {
    inputs: [("r", Float), ("g", Float), ("b", Float), ("a", Float, default=1.0)],
    outputs: [("color", Color)]
}
```

### Category: `noise/` - Procedural Patterns

```rust
"shader/noise_simplex" => {
    inputs: [("uv", Vec2), ("scale", Float, default=1.0)],
    outputs: [("value", Float)]  // 0-1
}

"shader/noise_voronoi" => {
    inputs: [("uv", Vec2), ("scale", Float)],
    outputs: [
        ("distance", Float),
        ("cell_color", Vec3),
    ]
}

"shader/noise_fbm" => {
    inputs: [
        ("uv", Vec2),
        ("octaves", Int, default=4),
        ("lacunarity", Float, default=2.0),
        ("gain", Float, default=0.5),
    ],
    outputs: [("value", Float)]
}

"shader/gradient" => {
    inputs: [
        ("uv", Vec2),
        ("direction", Vec2, default=[1,0]),  // Gradient direction
    ],
    outputs: [("value", Float)]  // 0-1 along direction
}
```

### Category: `output/` - Material Outputs (PBR)

```rust
// Main PBR output - REQUIRED, only one per material
"shader/pbr_output" => {
    inputs: [
        ("base_color", Color, default=[1,1,1,1]),
        ("metallic", Float, default=0.0),
        ("roughness", Float, default=0.5),
        ("normal", Vec3),           // Tangent-space normal
        ("emissive", Color, default=[0,0,0,1]),
        ("ambient_occlusion", Float, default=1.0),
        ("alpha", Float, default=1.0),
    ]
}

// Unlit output - for UI, particles, etc.
"shader/unlit_output" => {
    inputs: [
        ("color", Color),
        ("alpha", Float, default=1.0),
    ]
}
```

## Pin Type Extensions

```rust
// In graph.rs
pub enum PinType {
    // Existing
    Flow, Float, Int, Bool, String, Vec2, Vec3, Color, Any,

    // New for materials
    Vec4,           // 4D vector
    Texture2D,      // Texture reference
    TextureCube,    // Cubemap reference
    Sampler,        // Texture sampler
}
```

## WGSL Code Generation

### New file: `src/blueprint/codegen_wgsl.rs`

```rust
//! WGSL code generation from material blueprint graphs

use super::{BlueprintGraph, BlueprintNode, NodeId, PinId, PinValue};
use std::collections::{HashMap, HashSet};

/// Result of WGSL code generation
pub struct WgslCodegenResult {
    /// Generated vertex shader code
    pub vertex_shader: String,
    /// Generated fragment shader code
    pub fragment_shader: String,
    /// Texture bindings required
    pub texture_bindings: Vec<TextureBinding>,
    /// Uniform bindings required
    pub uniform_bindings: Vec<UniformBinding>,
    /// Errors
    pub errors: Vec<String>,
    /// Warnings
    pub warnings: Vec<String>,
}

pub struct TextureBinding {
    pub name: String,
    pub binding: u32,
    pub asset_path: String,
}

pub struct UniformBinding {
    pub name: String,
    pub binding: u32,
    pub value_type: String,
}

struct WgslCodegenContext<'a> {
    graph: &'a BlueprintGraph,
    output_vars: HashMap<PinId, String>,
    var_counter: usize,
    processed_nodes: HashSet<NodeId>,
    texture_bindings: Vec<TextureBinding>,
    next_texture_binding: u32,
}

impl<'a> WgslCodegenContext<'a> {
    fn new(graph: &'a BlueprintGraph) -> Self {
        Self {
            graph,
            output_vars: HashMap::new(),
            var_counter: 0,
            processed_nodes: HashSet::new(),
            texture_bindings: Vec::new(),
            next_texture_binding: 1, // 0 is usually for uniforms
        }
    }

    fn next_var(&mut self, prefix: &str) -> String {
        let name = format!("{}_{}", prefix, self.var_counter);
        self.var_counter += 1;
        name
    }

    fn get_input_value(&mut self, node: &BlueprintNode, pin_name: &str, lines: &mut Vec<String>) -> String {
        let pin_id = PinId::input(node.id, pin_name);

        // Check for connection
        if let Some(conn) = self.graph.connection_to(&pin_id) {
            if let Some(var_name) = self.output_vars.get(&conn.from) {
                return var_name.clone();
            }
            // Generate source node
            if let Some(source_node) = self.graph.get_node(conn.from.node_id) {
                self.generate_node(source_node, lines);
                if let Some(var_name) = self.output_vars.get(&conn.from) {
                    return var_name.clone();
                }
            }
        }

        // Use default value
        if let Some(value) = node.get_input_value(pin_name) {
            return value.to_wgsl();
        }

        "0.0".to_string()
    }

    fn generate_node(&mut self, node: &BlueprintNode, lines: &mut Vec<String>) {
        if self.processed_nodes.contains(&node.id) {
            return;
        }
        self.processed_nodes.insert(node.id);

        match node.node_type.as_str() {
            // === INPUT NODES ===
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

            "shader/normal" => {
                self.output_vars.insert(PinId::output(node.id, "normal"), "in.world_normal".to_string());
                self.output_vars.insert(PinId::output(node.id, "x"), "in.world_normal.x".to_string());
                self.output_vars.insert(PinId::output(node.id, "y"), "in.world_normal.y".to_string());
                self.output_vars.insert(PinId::output(node.id, "z"), "in.world_normal.z".to_string());
            }

            "shader/time" => {
                self.output_vars.insert(PinId::output(node.id, "time"), "globals.time".to_string());
                let sin_var = self.next_var("sin_time");
                let cos_var = self.next_var("cos_time");
                lines.push(format!("    let {} = sin(globals.time);", sin_var));
                lines.push(format!("    let {} = cos(globals.time);", cos_var));
                self.output_vars.insert(PinId::output(node.id, "sin_time"), sin_var);
                self.output_vars.insert(PinId::output(node.id, "cos_time"), cos_var);
            }

            // === TEXTURE NODES ===
            "shader/texture" => {
                let path = node.input_values.get("path")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_default();

                let binding = self.next_texture_binding;
                self.next_texture_binding += 1;

                let tex_name = format!("texture_{}", binding);
                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding,
                    asset_path: path,
                });

                self.output_vars.insert(PinId::output(node.id, "texture"), tex_name);
            }

            "shader/sample_texture" => {
                let tex = self.get_input_value(node, "texture", lines);
                let uv = self.get_input_value(node, "uv", lines);

                let color_var = self.next_var("tex_color");
                lines.push(format!("    let {} = textureSample({}, sampler_{}, {});",
                    color_var, tex, tex, uv));

                self.output_vars.insert(PinId::output(node.id, "color"), color_var.clone());
                self.output_vars.insert(PinId::output(node.id, "rgb"), format!("{}.rgb", color_var));
                self.output_vars.insert(PinId::output(node.id, "r"), format!("{}.r", color_var));
                self.output_vars.insert(PinId::output(node.id, "g"), format!("{}.g", color_var));
                self.output_vars.insert(PinId::output(node.id, "b"), format!("{}.b", color_var));
                self.output_vars.insert(PinId::output(node.id, "a"), format!("{}.a", color_var));
            }

            // === MATH NODES ===
            "math/add" | "shader/add" => {
                let a = self.get_input_value(node, "a", lines);
                let b = self.get_input_value(node, "b", lines);
                let result_var = self.next_var("add");
                lines.push(format!("    let {} = {} + {};", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/multiply" | "shader/multiply" => {
                let a = self.get_input_value(node, "a", lines);
                let b = self.get_input_value(node, "b", lines);
                let result_var = self.next_var("mul");
                lines.push(format!("    let {} = {} * {};", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "math/lerp" | "shader/lerp" => {
                let a = self.get_input_value(node, "a", lines);
                let b = self.get_input_value(node, "b", lines);
                let t = self.get_input_value(node, "t", lines);
                let result_var = self.next_var("lerp");
                lines.push(format!("    let {} = mix({}, {}, {});", result_var, a, b, t));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/dot" => {
                let a = self.get_input_value(node, "a", lines);
                let b = self.get_input_value(node, "b", lines);
                let result_var = self.next_var("dot");
                lines.push(format!("    let {} = dot({}, {});", result_var, a, b));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/normalize" => {
                let v = self.get_input_value(node, "v", lines);
                let result_var = self.next_var("norm");
                lines.push(format!("    let {} = normalize({});", result_var, v));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/fresnel" => {
                let normal = self.get_input_value(node, "normal", lines);
                let view = self.get_input_value(node, "view", lines);
                let power = self.get_input_value(node, "power", lines);
                let result_var = self.next_var("fresnel");
                lines.push(format!("    let {} = pow(1.0 - saturate(dot({}, {})), {});",
                    result_var, normal, view, power));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            "shader/smoothstep" => {
                let edge0 = self.get_input_value(node, "edge0", lines);
                let edge1 = self.get_input_value(node, "edge1", lines);
                let x = self.get_input_value(node, "x", lines);
                let result_var = self.next_var("smooth");
                lines.push(format!("    let {} = smoothstep({}, {}, {});", result_var, edge0, edge1, x));
                self.output_vars.insert(PinId::output(node.id, "result"), result_var);
            }

            // === VECTOR NODES ===
            "shader/make_vec3" => {
                let x = self.get_input_value(node, "x", lines);
                let y = self.get_input_value(node, "y", lines);
                let z = self.get_input_value(node, "z", lines);
                let result_var = self.next_var("vec3");
                lines.push(format!("    let {} = vec3<f32>({}, {}, {});", result_var, x, y, z));
                self.output_vars.insert(PinId::output(node.id, "v"), result_var);
            }

            "shader/make_color" => {
                let r = self.get_input_value(node, "r", lines);
                let g = self.get_input_value(node, "g", lines);
                let b = self.get_input_value(node, "b", lines);
                let a = self.get_input_value(node, "a", lines);
                let result_var = self.next_var("color");
                lines.push(format!("    let {} = vec4<f32>({}, {}, {}, {});", result_var, r, g, b, a));
                self.output_vars.insert(PinId::output(node.id, "color"), result_var);
            }

            "shader/split_vec3" => {
                let v = self.get_input_value(node, "v", lines);
                self.output_vars.insert(PinId::output(node.id, "x"), format!("{}.x", v));
                self.output_vars.insert(PinId::output(node.id, "y"), format!("{}.y", v));
                self.output_vars.insert(PinId::output(node.id, "z"), format!("{}.z", v));
            }

            // === NOISE NODES ===
            "shader/noise_simplex" => {
                let uv = self.get_input_value(node, "uv", lines);
                let scale = self.get_input_value(node, "scale", lines);
                let result_var = self.next_var("noise");
                // Note: simplex_noise would need to be defined in the shader preamble
                lines.push(format!("    let {} = simplex_noise_2d({} * {});", result_var, uv, scale));
                self.output_vars.insert(PinId::output(node.id, "value"), result_var);
            }

            _ => {}
        }
    }
}

impl PinValue {
    /// Convert to WGSL code representation
    pub fn to_wgsl(&self) -> String {
        match self {
            PinValue::Flow => String::new(),
            PinValue::Float(v) => format!("{:.6}", v),
            PinValue::Int(v) => format!("{}i", v),
            PinValue::Bool(v) => format!("{}", v),
            PinValue::String(v) => format!("\"{}\"", v),
            PinValue::Vec2(v) => format!("vec2<f32>({:.6}, {:.6})", v[0], v[1]),
            PinValue::Vec3(v) => format!("vec3<f32>({:.6}, {:.6}, {:.6})", v[0], v[1], v[2]),
            PinValue::Color(v) => format!("vec4<f32>({:.6}, {:.6}, {:.6}, {:.6})", v[0], v[1], v[2], v[3]),
        }
    }
}

/// Generate WGSL code from a material blueprint graph
pub fn generate_wgsl_code(graph: &BlueprintGraph) -> WgslCodegenResult {
    let mut ctx = WgslCodegenContext::new(graph);
    let mut fragment_lines = Vec::new();
    let mut errors = Vec::new();
    let warnings = Vec::new();

    // Find the output node
    let output_node = graph.nodes.iter()
        .find(|n| n.node_type == "shader/pbr_output" || n.node_type == "shader/unlit_output");

    let Some(output_node) = output_node else {
        errors.push("Material blueprint must have an output node (PBR Output or Unlit Output)".to_string());
        return WgslCodegenResult {
            vertex_shader: String::new(),
            fragment_shader: String::new(),
            texture_bindings: Vec::new(),
            uniform_bindings: Vec::new(),
            errors,
            warnings,
        };
    };

    // Generate code by traversing from output back to inputs
    let is_pbr = output_node.node_type == "shader/pbr_output";

    if is_pbr {
        // Get all PBR inputs
        let base_color = ctx.get_input_value(output_node, "base_color", &mut fragment_lines);
        let metallic = ctx.get_input_value(output_node, "metallic", &mut fragment_lines);
        let roughness = ctx.get_input_value(output_node, "roughness", &mut fragment_lines);
        let emissive = ctx.get_input_value(output_node, "emissive", &mut fragment_lines);
        let ao = ctx.get_input_value(output_node, "ambient_occlusion", &mut fragment_lines);
        let alpha = ctx.get_input_value(output_node, "alpha", &mut fragment_lines);

        // Generate PBR output assignment
        fragment_lines.push(String::new());
        fragment_lines.push("    // PBR Output".to_string());
        fragment_lines.push(format!("    var output: PbrOutput;"));
        fragment_lines.push(format!("    output.base_color = {};", base_color));
        fragment_lines.push(format!("    output.metallic = {};", metallic));
        fragment_lines.push(format!("    output.roughness = {};", roughness));
        fragment_lines.push(format!("    output.emissive = {};", emissive));
        fragment_lines.push(format!("    output.ambient_occlusion = {};", ao));
        fragment_lines.push(format!("    output.alpha = {};", alpha));
        fragment_lines.push("    return output;".to_string());
    } else {
        // Unlit output
        let color = ctx.get_input_value(output_node, "color", &mut fragment_lines);
        let alpha = ctx.get_input_value(output_node, "alpha", &mut fragment_lines);

        fragment_lines.push(String::new());
        fragment_lines.push(format!("    return vec4<f32>({}.rgb, {});", color, alpha));
    }

    // Generate texture bindings
    let mut binding_declarations = Vec::new();
    for binding in &ctx.texture_bindings {
        binding_declarations.push(format!(
            "@group(1) @binding({}) var {}: texture_2d<f32>;",
            binding.binding, binding.name
        ));
        binding_declarations.push(format!(
            "@group(1) @binding({}) var sampler_{}: sampler;",
            binding.binding + 100, binding.name  // Samplers at offset
        ));
    }

    // Assemble full fragment shader
    let fragment_shader = format!(r#"
// Auto-generated by Material Blueprint
// DO NOT EDIT - changes will be overwritten

#import bevy_pbr::mesh_view_bindings
#import bevy_pbr::mesh_bindings

{}

struct VertexOutput {{
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}};

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {{
{}
}}
"#,
        binding_declarations.join("\n"),
        fragment_lines.join("\n")
    );

    WgslCodegenResult {
        vertex_shader: String::new(), // Use default vertex shader
        fragment_shader,
        texture_bindings: ctx.texture_bindings,
        uniform_bindings: Vec::new(),
        errors,
        warnings,
    }
}
```

## Creating a Bevy Material from Generated Code

```rust
// In src/blueprint/material.rs

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use super::codegen_wgsl::{generate_wgsl_code, WgslCodegenResult};
use super::BlueprintGraph;

/// A material generated from a blueprint graph
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct BlueprintMaterial {
    #[uniform(0)]
    pub base_color: Color,

    // Texture bindings are dynamic based on the blueprint
    #[texture(1)]
    #[sampler(2)]
    pub texture_0: Option<Handle<Image>>,

    #[texture(3)]
    #[sampler(4)]
    pub texture_1: Option<Handle<Image>>,

    // ... more texture slots as needed

    /// The generated shader code
    #[dependency]
    shader: Handle<Shader>,
}

impl Material for BlueprintMaterial {
    fn fragment_shader() -> ShaderRef {
        // This would need to return the generated shader
        ShaderRef::Default
    }
}

/// Compile a material blueprint and create a Bevy material
pub fn compile_material_blueprint(
    graph: &BlueprintGraph,
    shaders: &mut Assets<Shader>,
    asset_server: &AssetServer,
) -> Result<BlueprintMaterial, Vec<String>> {
    let result = generate_wgsl_code(graph);

    if !result.errors.is_empty() {
        return Err(result.errors);
    }

    // Create shader asset from generated code
    let shader = Shader::from_wgsl(result.fragment_shader, "generated_material.wgsl");
    let shader_handle = shaders.add(shader);

    // Load textures
    let texture_0 = result.texture_bindings.get(0)
        .map(|b| asset_server.load(&b.asset_path));
    let texture_1 = result.texture_bindings.get(1)
        .map(|b| asset_server.load(&b.asset_path));

    Ok(BlueprintMaterial {
        base_color: Color::WHITE,
        texture_0,
        texture_1,
        shader: shader_handle,
    })
}
```

## Example: Simple Gradient Sky Material

Blueprint nodes for a gradient sky:

```
[UV] ──────────────────┐
        │              │
        │ (v output)   │
        ▼              │
    [Split Vec2]       │
        │              │
        │ (y output)   │
        ▼              │
    [Smoothstep]       │
     edge0: 0.3        │
     edge1: 0.7        │
        │              │
        │ (result)     │
        ▼              │
    [Lerp] ◄───────────┘
     a: [Color: Sky Blue]
     b: [Color: Deep Blue]
        │
        │ (result)
        ▼
  [Unlit Output]
     color ◄───┘
```

Generated WGSL:

```wgsl
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let smooth_0 = smoothstep(0.3, 0.7, in.uv.y);
    let lerp_0 = mix(
        vec4<f32>(0.529, 0.808, 0.922, 1.0),  // Sky blue
        vec4<f32>(0.098, 0.098, 0.439, 1.0),  // Deep blue
        smooth_0
    );

    return lerp_0;
}
```

## UI Integration

The existing blueprint canvas can be reused with minimal changes:

1. **Graph Type Toggle**: Add dropdown in toolbar to switch between Behavior/Material
2. **Node Library Filter**: Show only relevant nodes based on graph type
3. **Compile Button**: Call appropriate codegen (Rhai vs WGSL)
4. **Preview Panel**: Show material preview on a sphere/cube for material blueprints
5. **Output**: Save as `.material` file alongside `.blueprint`

## File Structure

```
project/
├── blueprints/
│   ├── player_controller.blueprint    (Behavior)
│   └── stylized_water.blueprint       (Material)
├── materials/
│   └── stylized_water.material        (Generated WGSL + metadata)
└── scripts/
    └── player_controller.rhai         (Generated Rhai)
```

## Summary

This design allows the existing blueprint infrastructure to be extended for materials by:

1. Adding a `graph_type` field to distinguish behavior vs material graphs
2. Creating shader-specific node types in a new `shader/` category
3. Adding a WGSL codegen backend parallel to the existing Rhai codegen
4. Creating a `BlueprintMaterial` that uses the generated WGSL
5. Reusing the canvas UI with graph-type-aware node filtering

The math nodes can be shared between both graph types since operations like add, multiply, lerp work identically in Rhai and WGSL.

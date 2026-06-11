# Custom Material Nodes

How Renzora's material node graph is built, how to add a new built-in node to the engine, and how to plug in your own shader-language backend.

## Where this lives

Materials are owned by the **`renzora_shader`** crate (`crates/renzora_shader`). It registers two runtime plugins through `renzora::add!`, so they ship in both the editor and exported games:

- **`ShaderPlugin`** (`renzora_shader::ShaderPlugin`) — installs the code-shader runtime and registers the built-in shader-language backends (Bevy / WGSL / GLSL / ShaderToy).
- **`MaterialPlugin`** (`renzora_shader::material::MaterialPlugin`) — installs the node-graph material runtime (`GraphMaterialPlugin`) and the `MaterialResolverPlugin` that turns a `.material` graph into a renderable Bevy material.

The two editor front-ends are separate, editor-only plugins: `renzora_material_editor::MaterialEditorPlugin` (the visual node graph) and `renzora_shader_editor::ShaderEditorPlugin` (the code editor). Both are registered with `renzora::add!(..., Editor)`.

## How a material graph compiles

A material is a **node graph** (`MaterialGraph`) saved to a `.material` file (JSON). At resolve time the compiler:

1. Finds the graph's single output node (`output/surface`, `output/terrain_layer`, `output/vegetation`, or `output/unlit`).
2. Walks **backwards** through the connections, visiting each upstream node.
3. Dispatches on the node's `node_type` string to emit a small WGSL snippet, naming intermediate variables and inserting type casts as needed.
4. Composes the snippets into a complete, Bevy-compatible material shader and hands it to the resolver, which builds a `GraphMaterial` (a `StandardMaterial` extension).

Material nodes are therefore **compile-time**: they produce WGSL, not runtime logic. When a graph is saved through the editor, codegen runs once and the resulting `.wgsl` path is stored in the graph's `wgsl_path` field so the runtime can skip codegen entirely.

> **Important:** the set of graph node types is **fixed and compiled into `renzora_shader`** (`material/nodes.rs::ALL_NODES`, dispatched in `material/codegen.rs`). There is **no runtime trait or plugin API for registering new graph node types** — adding a brand-new built-in node is an engine-source change (see *Adding a built-in node*). To make reusable nodes without editing the engine, use a **Material Function** (a saved subgraph). To author arbitrary shader logic outside the graph, write a **code shader** and, if needed, a custom **`ShaderBackend`**.

## The graph data model

All of these types live in `renzora_shader::material::graph`:

| Type | Purpose |
|------|---------|
| `MaterialGraph` | The whole material: `name`, `domain`, `nodes`, `connections`, `alpha_mode`, `double_sided`, `wgsl_path`. |
| `MaterialNode` | One placed node: `id: u64`, `node_type: String`, `position: [f32; 2]`, `input_values: HashMap<String, PinValue>`. |
| `Connection` | A wire: `from_node`, `from_pin`, `to_node`, `to_pin`. An input pin accepts exactly one connection. |
| `PinTemplate` | A node's declared pin: `name`, `label`, `pin_type`, `direction` (`PinDir::Input`/`Output`), `default_value`. |
| `PinValue` | A concrete value stored on an input pin (`Float`, `Vec2`, `Vec3`, `Vec4`, `Color`, `Bool`, `Int`, `TexturePath`, `String`, `None`). |
| `MaterialDomain` | `Surface`, `TerrainLayer`, `Vegetation`, or `Unlit` — selects the output node and shader template. |

A node only stores *overrides* for its input pins in `input_values`. Pins with no override and no incoming connection fall back to the `default_value` declared on the node's `PinTemplate`.

### Pin types

`PinType` (in `material::graph`) determines the WGSL type a pin compiles to:

| `PinType` | WGSL type | Notes |
|-----------|-----------|-------|
| `Float` | `f32` | Scalar |
| `Vec2` | `vec2<f32>` | UVs, 2D vectors |
| `Vec3` | `vec3<f32>` | Color (no alpha), normals, positions |
| `Vec4` | `vec4<f32>` | Vector with alpha |
| `Color` | `vec4<f32>` | Same WGSL type as `Vec4`, edited with a color picker |
| `Bool` | `bool` | Static switches / flags |
| `Texture2D` | `texture_2d<f32>` | Texture reference (a `TexturePath`) |
| `Sampler` | `sampler` | Texture sampler |
| `String` | — | Never reaches WGSL; used for parameter names, function paths, etc. |

All numeric/vector/color types are **freely inter-convertible at the graph level** — `PinType::compatible` allows the wire and `PinType::cast_expr` inserts the WGSL coercion (widening copies the scalar into every lane; narrowing takes the leading components). `String` pins only connect to other strings.

## Node types and categories

Every node is identified by a namespaced `node_type` string (e.g. `"math/add"`, `"texture/sample"`, `"output/surface"`). `renzora_shader` ships over 150 built-in nodes across these categories (`nodes::categories()`):

| Category | `node_type` prefix | Examples |
|----------|--------------------|----------|
| Input | `input/` | `input/uv`, `input/world_position`, `input/world_normal`, `input/view_direction`, `input/time`, `input/camera_position`, `input/uv_panner` |
| Parameter | `param/` | `param/float`, `param/color`, `param/vec2`, `param/vec3`, `param/vec4`, `param/bool` |
| Texture | `texture/` | `texture/sample`, `texture/sample_normal`, `texture/triplanar`, `texture/sample_lod`, `texture/sample_cubemap`, `texture/sample_2d_array`, `texture/sample_3d` |
| Math | `math/` | `math/add`, `math/multiply`, `math/lerp`, `math/clamp`, `math/smoothstep`, `math/remap`, `math/sin`, `math/pow` |
| Vector | `vector/` | `vector/dot`, `vector/cross`, `vector/normalize`, `vector/split_vec3`, `vector/combine_vec3`, `vector/reflect`, `vector/swizzle` |
| Color | `color/` | `color/constant`, `color/float`, `color/lerp`, `color/fresnel`, `color/rgb_to_hsv`, `color/hue_shift` |
| Procedural | `procedural/` | Perlin / Simplex / Voronoi / FBM / ridged / billow noise, checkerboard, brick, gradients, domain warp |
| Animation | `animation/` | `animation/uv_scroll`, `animation/flow_map`, `animation/sine_wave`, `animation/wind`, `animation/flipbook_uv` |
| Utility | `utility/` | `utility/depth_fade`, `utility/dither`, `utility/hash`, derivatives (`dpdx`/`dpdy`/`fwidth`), masks |
| Control | `control/` | `control/if`, `control/static_switch`, comparisons (`greater_than`, `equal`, …), boolean ops |
| Scene | `scene/` | `scene/pixel_depth`, `scene/scene_depth`, `scene/screen_uv`, `scene/env_map_sample`, `scene/motion_vector` |
| Output | `output/` | `output/surface`, `output/terrain_layer`, `output/vegetation`, `output/unlit` |

Look up a definition at runtime with `nodes::node_def(node_type)` or list a category with `nodes::nodes_in_category(category)`.

### Output nodes and domains

`output/surface` maps 1:1 onto Bevy's `StandardMaterial`. Beyond the core PBR pins (`base_color`, `metallic`, `roughness`, `normal`, `emissive`, `ao`, `alpha`, `reflectance`) it exposes transmission (`specular_transmission`, `diffuse_transmission`, `thickness`, `ior`, `attenuation_distance`), clearcoat (`clearcoat`, `clearcoat_roughness`), and anisotropy (`anisotropy_strength`, `anisotropy_rotation`). **Disconnected pins keep the `StandardMaterial` defaults.** Connecting either transmission pin makes the resolver flip on Bevy's transmissive pass.

## Adding a built-in node

Because nodes are compiled into the engine, a new built-in node is three edits in `crates/renzora_shader`:

**1. Declare the node** in `material/nodes.rs` as a `MaterialNodeDef` static:

```rust
pub static POSTERIZE: MaterialNodeDef = MaterialNodeDef {
    node_type: "math/posterize",
    display_name: "Posterize",
    category: CAT_MATH,
    description: "Quantize a value into N discrete steps",
    pins: || {
        vec![
            PinTemplate::input("value", "Value", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::input("steps", "Steps", PinType::Float)
                .with_default(PinValue::Float(4.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: [120, 120, 120], // graph header RGB
};
```

**2. Register it** by adding `&POSTERIZE` to the `ALL_NODES` slice (in the matching category block).

**3. Generate WGSL** by adding an arm to the `match node.node_type.as_str()` in `material/codegen.rs`:

```rust
"math/posterize" => {
    let value = self.input(node, "value"); // resolves the wire or the pin default
    let steps = self.input(node, "steps");
    let v = self.next_var("posterize");    // unique WGSL variable name
    self.emit(format!("    let {v} = floor({value} * {steps}) / {steps};"));
    self.set_out(id, "result", v);         // publish this node's "result" output
}
```

The codegen helpers are the contract every arm uses:

| Helper | Does |
|--------|------|
| `self.input(node, "pin")` | Returns the WGSL expression feeding an input pin (an upstream output var, or the pin's literal default), already cast to the pin's type. |
| `self.next_var("prefix")` | Allocates a fresh, collision-free WGSL identifier. |
| `self.emit(line)` | Appends a line to the function body. |
| `self.set_out(id, "pin", expr)` | Records the WGSL expression for one of this node's outputs so downstream nodes can read it. |

> The `node_type` string in the `MaterialNodeDef` and the string in the `codegen.rs` match arm **must match exactly** — that string is the only link between the two.

## Parameters and material instances

`param/*` nodes are named, graph-boundary inputs. The `name` pin is the parameter identifier; the `default` pin is the value baked into the master shader. A graph may declare up to **`codegen::MAX_PARAMETER_SLOTS` (32)** parameters. The compiler returns the discovered parameter list (`Vec<MaterialParam>`) so a **material instance** — a derived `.material` that references a master — can override each default by name without recompiling the shader.

## Material functions (reusable subgraphs)

A `MaterialFunction` is the no-Rust way to package graph logic as a reusable "custom node". It is a named subgraph bracketed by a `function/input_point` node (its inputs) and a `function/output_point` node (its outputs). Other graphs invoke it with a **`function/call`** node. At compile time each call **inlines a WGSL helper function** at module scope and invokes it at the call site, so the function composes like any other node while staying visually encapsulated. The compiler detects and reports recursive cycles.

## The `.material` file

A `.material` file is a JSON-serialized `MaterialGraph`. Master materials and derived instances both use the `.material` extension; the legacy `.material_instance` and `.material_bp` extensions are still loaded for back-compat.

```json
{
  "name": "Glow",
  "domain": "Surface",
  "nodes": [
    { "id": 1, "node_type": "output/surface", "position": [300.0, 0.0], "input_values": {} },
    {
      "id": 2,
      "node_type": "color/constant",
      "position": [0.0, 0.0],
      "input_values": { "color": { "Color": [0.9, 0.2, 0.1, 1.0] } }
    }
  ],
  "connections": [
    { "from_node": 2, "from_pin": "rgb", "to_node": 1, "to_pin": "emissive" }
  ],
  "next_id": 3,
  "alpha_mode": "Opaque",
  "double_sided": false
}
```

`wgsl_path` (omitted above) is added automatically when the editor saves: it points at the precompiled `.wgsl` so the runtime skips codegen. `alpha_mode` is `Opaque`, `{ "Mask": { "cutoff": 0.5 } }`, or `Blend`.

## Custom shader backends (code shaders)

For shader **code** rather than graphs, `renzora_shader` exposes a genuine plugin extension point: the **`ShaderBackend`** trait plus the **`ShaderBackendRegistry`** resource. A backend transpiles some source language into Bevy-compatible WGSL.

```rust
pub trait ShaderBackend: Send + Sync + 'static {
    fn name(&self) -> &str;                 // e.g. "HLSL"
    fn file_extensions(&self) -> &[&str];   // e.g. &["hlsl"]
    fn to_wgsl(&self, source: &str) -> Result<String, ShaderCompileError>;

    // Optional:
    fn builtin_uniforms(&self) -> &[UniformMapping] { &[] }
    fn syntax_tokens(&self) -> Option<&[SyntaxRule]> { None }
}
```

Built-in backends (registered by `ShaderPlugin`):

| Backend | Extensions | What it does |
|---------|------------|--------------|
| `BevyBackend` | `bevy.wgsl` | Passthrough — Bevy's `naga_oil` resolves `#import` directives at runtime. |
| `WgslBackend` | `wgsl` | Plain WGSL, validated with naga. |
| `GlslBackend` | `glsl`, `frag`, `vert` | GLSL fragment shaders transpiled to WGSL via naga. |
| `ShaderToyBackend` | `shadertoy` | Wraps a ShaderToy `mainImage(...)` in GLSL, transpiles to WGSL, and maps `iTime`/`iResolution`/`iMouse`/`iTimeDelta`/`iFrame` onto the engine's `ShaderUniforms`. |

Register your own backend by mutating the registry resource in a plugin's `build` (the resource is created by `ShaderPlugin`, so run after it):

```rust
use renzora_shader::registry::ShaderBackendRegistry;

impl Plugin for MyHlslPlugin {
    fn build(&self, app: &mut App) {
        let mut reg = app.world_mut().resource_mut::<ShaderBackendRegistry>();
        reg.register(Box::new(HlslBackend));
    }
}
```

After any backend produces WGSL, the registry post-processes it: it injects the `ShaderUniforms` bind group (`@group(3) @binding(0)`) when the shader references `uniforms.*`, and it injects `@param` constants.

### `@param` constants

Code shaders expose editable constants through `@param` comment annotations, which the editor surfaces as inspector controls and the registry compiles into WGSL `const` declarations:

```wgsl
// @param name  type  default [min max] description
// @param speed float 1.0 0.1 5.0 Animation speed
// @param tint  color 1.0 0.8 0.6 1.0
// @param offset vec2 0.0 0.0
// @param enabled bool true
// @param count int 4
```

Supported `@param` types are `float`, `color`, `vec2`, `vec3`, `vec4`, `bool`, and `int`. Code shaders render through `CodeShaderMaterial`.

## Editor integration

- **Material Editor** (`renzora_material_editor`) — the visual node graph. It is **selection-driven**: selecting a mesh loads its `.material` into the graph; you can also open a `.material` standalone from the asset browser. Edits compile live (the panel shows `compiled_wgsl` and any `compile_errors`) and **Apply** writes the file, re-runs codegen, and invalidates the resolver cache so the mesh updates.
- **Shader Editor** (`renzora_shader_editor`) — the code editor for `.wgsl`/`.glsl`/`.frag`/`.vert`/`.shadertoy` files, with auto-compile, a compiler log panel, `@param` property controls, and a live mesh preview (shaders that declare their own material bindings can't use the preview).

The asset browser routes `wgsl`/`glsl`/`vert`/`frag` files to the Shader Editor tab and `.material` files to the Material Editor.

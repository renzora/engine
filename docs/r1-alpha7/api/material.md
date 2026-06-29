# Material API

Reference for Renzora's material system — the `.material` node-graph format, material instances, the full node catalog, and the code-shader backends, all backed by the `renzora_shader` crate.

## How materials work

A Renzora material is a **node graph**, not a fixed list of PBR sliders. You build it in the [Material Editor](/docs/r1-alpha5/editor/materials) (the **Materials** workspace), and it is saved to disk as a `.material` file — JSON-serialized [`MaterialGraph`](#the-material-file-format). At runtime the `renzora_shader` crate's `MaterialResolverPlugin` watches every entity that has a `MaterialRef` component pointing at a `.material` (or `.shader`) file, compiles the graph, and applies the result to the mesh.

There are two compile paths, chosen automatically by the resolver:

- **Trivial graphs** — a texture/factor wired straight into a PBR output pin compile to a plain Bevy `StandardMaterial`. Most imported materials land here.
- **Procedural graphs** — anything with math, noise, animation, or custom logic compile to a `GraphMaterial` (an `ExtendedMaterial<StandardMaterial, SurfaceGraphExt>`) with a generated WGSL fragment shader.

Compiled results are cached per file path in the `MaterialCache` resource, so editing one material in the editor invalidates and recompiles only that material.

> When you save through the editor, codegen also writes the generated WGSL beside the `.material` file and records its project-relative path in the graph's `wgsl_path` field. The runtime follows that link to skip codegen entirely. Legacy files without a `wgsl_path` fall back to live codegen.

## Attaching a material to an entity

Materials are assigned with the `MaterialRef` component (re-exported as `renzora::MaterialRef`). Its single field is the asset-relative path to a `.material` or `.shader` file. Scenes serialize this component, so a material assignment survives save/load.

Per-entity tweaks use the `MaterialOverrides` component, a map of parameter name → `ParamValue` applied on top of the material's defaults:

```rust
ParamValue::Float(f32)
ParamValue::Vec2([f32; 2])
ParamValue::Vec3([f32; 3])
ParamValue::Vec4([f32; 4])
ParamValue::Color([f32; 4])
ParamValue::Int(i32)
ParamValue::Bool(bool)
```

## The `.material` file format

A master `.material` file is a JSON-serialized `MaterialGraph`:

| Field | Type | Notes |
|-------|------|-------|
| `name` | string | Display name |
| `domain` | enum | `Surface`, `TerrainLayer`, `Vegetation`, or `Unlit` |
| `nodes` | array | The graph's nodes (see below) |
| `connections` | array | Wires between pins |
| `next_id` | u64 | Next node id the editor will allocate |
| `alpha_mode` | enum | `Opaque` (default), `{ "Mask": { "cutoff": 0.5 } }`, or `Blend` |
| `double_sided` | bool | Render back faces too (default `false`) |
| `wgsl_path` | string | Optional — link to the precompiled `.wgsl` (omitted when absent) |

Each entry in `nodes` is a `MaterialNode`:

| Field | Type | Notes |
|-------|------|-------|
| `id` | u64 | Unique within the graph |
| `node_type` | string | A registered node type, e.g. `"math/multiply"` |
| `position` | `[f32; 2]` | Editor canvas position |
| `input_values` | object | Per-input-pin constant overrides, keyed by pin name |

Each entry in `connections` is a `Connection` with `from_node`, `from_pin`, `to_node`, `to_pin`. An input pin accepts **one** connection; reconnecting it replaces the previous wire.

Pin constants in `input_values` are `PinValue`s, serialized externally tagged — `{ "Float": 0.5 }`, `{ "Color": [1.0, 0.0, 0.0, 1.0] }`, `{ "Vec3": [0,0,0] }`, `{ "TexturePath": "textures/brick.png" }`, `{ "String": "BaseColor" }`, etc.

```json
{
  "name": "Brick",
  "domain": "Surface",
  "nodes": [
    {
      "id": 1,
      "node_type": "output/surface",
      "position": [300.0, 0.0],
      "input_values": {}
    },
    {
      "id": 2,
      "node_type": "param/color",
      "position": [0.0, 0.0],
      "input_values": {
        "name":    { "String": "BaseColor" },
        "default": { "Color": [0.6, 0.2, 0.15, 1.0] }
      }
    }
  ],
  "connections": [
    { "from_node": 2, "from_pin": "value", "to_node": 1, "to_pin": "base_color" }
  ],
  "next_id": 3,
  "alpha_mode": "Opaque",
  "double_sided": false
}
```

### Pin types

Every pin has a `PinType`. Numeric, vector, and color types are freely inter-connectable — the codegen inserts the right WGSL coercion (widening copies the scalar across components; narrowing takes the leading components).

| PinType | WGSL type | Notes |
|---------|-----------|-------|
| `Float` | `f32` | |
| `Vec2` | `vec2<f32>` | |
| `Vec3` | `vec3<f32>` | |
| `Vec4` | `vec4<f32>` | |
| `Color` | `vec4<f32>` | Treated as Vec4 for casting |
| `Bool` | `bool` | |
| `Texture2D` | `texture_2d<f32>` | Texture asset path |
| `Sampler` | `sampler` | |
| `String` | — | Editor-only (e.g. parameter names); never reaches WGSL |

## Material domains and output nodes

Every graph has exactly one output node, fixed by its `domain`. The output node cannot be deleted. Its input pins are the channels you drive.

### `output/surface` — Surface

Full PBR surface that maps 1:1 onto Bevy's `StandardMaterial`. Disconnected pins keep `StandardMaterial` defaults.

| Pin | Type | Default | Purpose |
|-----|------|---------|---------|
| `base_color` | Color | `[0.8, 0.8, 0.8, 1.0]` | Albedo |
| `metallic` | Float | `0.0` | Dielectric ↔ metal |
| `roughness` | Float | `0.5` | Smooth ↔ matte |
| `normal` | Vec3 | — | Tangent-space normal |
| `emissive` | Vec3 | `[0,0,0]` | Self-illumination |
| `ao` | Float | `1.0` | Ambient occlusion |
| `alpha` | Float | `1.0` | Opacity |
| `reflectance` | Vec3 | `[0.5,0.5,0.5]` | Specular reflectance |
| `specular_transmission` | Float | `0.0` | Refraction (glass, water) |
| `diffuse_transmission` | Float | `0.0` | Foliage / skin |
| `thickness` | Float | `0.0` | Volume thickness |
| `ior` | Float | `1.5` | Index of refraction |
| `attenuation_distance` | Float | `1.0e37` | Volume attenuation |
| `clearcoat` | Float | `0.0` | Second specular layer (car paint) |
| `clearcoat_roughness` | Float | `0.5` | Clearcoat roughness |
| `anisotropy_strength` | Float | `0.0` | Directional specular (brushed metal, hair) |
| `anisotropy_rotation` | Float | `0.0` | Anisotropy direction |

### Other output nodes

| Node type | Domain | Input pins |
|-----------|--------|------------|
| `output/terrain_layer` | Terrain Layer (blended via splatmap) | `base_color`, `metallic`, `roughness`, `normal`, `height` |
| `output/vegetation` | PBR + vertex displacement | `base_color`, `metallic`, `roughness`, `normal`, `emissive`, `ao`, `alpha`, `vertex_offset` |
| `output/unlit` | Unlit (no lighting) | `color`, `alpha` |

## Material instances

A material **instance** is a *derived* `.material` file: instead of a full graph it carries a `master` path and an `overrides` map. Many visually distinct materials can share one compiled master shader — the same idea as Unreal's material instances.

Masters and instances share the `.material` extension; the resolver content-detects which is which (a derived file has a non-empty `master`). Legacy `.material_instance` files still load.

```json
{
  "master": "models/Wood/materials/Wood.material",
  "overrides": {
    "BaseColor": { "Color": [0.45, 0.22, 0.10, 1.0] },
    "Roughness": { "Float": 0.85 }
  }
}
```

- `overrides` keys are the **names authored on the master's `param/*` nodes** (an unnamed node falls back to `FloatParam`, `ColorParam`, …).
- Unknown keys are ignored at resolve time, so renaming a master parameter won't hard-fail every instance.
- For a trivial master the override values are spliced into the matching `param/*` defaults and re-classified into a fresh `StandardMaterial`. For a procedural master the master compiles once and each instance only overwrites the relevant slots of the parameter uniform buffer, so wgpu reuses the same specialized pipeline.

> Expose a value for instances (or for `MaterialOverrides`) by adding a **Parameter** node (`param/float`, `param/color`, …) and giving its `name` pin a stable identifier. Anything wired from a parameter becomes overridable by name.

## Node catalog

Node types are identified by a `category/name` string. Categories appear in the editor's node menu in the order below.

> For the **per-node reference** — every node's inputs, outputs, and exactly what it computes — see [Material Node Reference](/docs/r1-alpha5/api/material-node-reference). The tables below are the quick index.

### Input

| Node type | Name | Description |
|-----------|------|-------------|
| `input/uv` | UV | Texture coordinates (0–1); also `u`, `v` |
| `input/uv_scale` | UV Scale | Scale/offset UVs for tiling |
| `input/uv_polar` | Polar UV | Cartesian → polar (angle, radius) |
| `input/uv_rotator` | UV Rotator | Rotate UVs around a center |
| `input/uv_panner` | UV Panner | Time-driven UV pan |
| `input/world_position` | World Position | Fragment world-space position |
| `input/world_normal` | World Normal | Fragment world-space normal |
| `input/view_direction` | View Direction | Fragment → camera direction |
| `input/time` | Time | `time`, `sin_time`, `cos_time` |
| `input/vertex_color` | Vertex Color | Per-vertex color attribute |
| `input/camera_position` | Camera Position | World-space camera position |
| `input/object_position` | Object Position | Object pivot world position |

### Parameter

Named graph-boundary inputs that material instances and `MaterialOverrides` can replace by name.

| Node type | Name |
|-----------|------|
| `param/float` | Float Parameter |
| `param/color` | Color Parameter |
| `param/vec2` | Vec2 Parameter |
| `param/vec3` | Vec3 Parameter |
| `param/vec4` | Vec4 Parameter |
| `param/bool` | Bool Parameter |

### Texture

| Node type | Name | Description |
|-----------|------|-------------|
| `texture/sample` | Sample Texture | Sample a 2D texture; outputs `color`, `rgb`, `r`, `g`, `b`, `a` |
| `texture/sample_normal` | Sample Normal Map | Sample + decode a normal map (with `strength`) |
| `texture/triplanar` | Triplanar Sample | World-projected sampling, no UV seams |
| `texture/sample_lod` | Sample Texture LOD | Sample at an explicit mip (`textureSampleLevel`) |
| `texture/sample_grad` | Sample Texture Grad | Sample with explicit UV derivatives |
| `texture/sample_cubemap` | Sample Cubemap | Material-local cubemap along a direction |
| `texture/sample_2d_array` | Sample 2D Array | Layered array; `layer` picks the slice |
| `texture/sample_3d` | Sample 3D Texture | Volumetric (UVW) sampling |

> **All texture slots in a graph material share one sampler.** Every slot — 2D, cubemap, array, and 3D — samples with the first 2D texture's sampler settings (or a default linear sampler when there is none). Per-image filter and wrap modes on the other textures are not honored. This keeps the fragment stage under the 16-samplers-per-stage limit that Metal and baseline WebGPU/Vulkan impose; per-slot samplers made graph materials fail pipeline creation on macOS.

### Math

All operate component-wise on the inferred type. Each has a `result` output.

| Node type | Op | Node type | Op |
|-----------|----|-----------|----|
| `math/add` | A + B | `math/saturate` | clamp 0–1 |
| `math/subtract` | A − B | `math/modulo` | A mod B |
| `math/multiply` | A × B | `math/sign` | −1 / 0 / +1 |
| `math/divide` | A / B | `math/atan2` | atan2(y, x) |
| `math/power` | base ^ exp | `math/trunc` | truncate toward zero |
| `math/abs` | absolute value | `math/round` | round to nearest |
| `math/negate` | −value | `math/exp` | e^x |
| `math/one_minus` | 1 − value | `math/log` | ln(x) |
| `math/fract` | fractional part | `math/sqrt` | square root |
| `math/floor` | round down | `math/reciprocal` | 1 / value |
| `math/ceil` | round up | `math/tan` | tangent |
| `math/min` | min(A, B) | `math/asin` | arcsine |
| `math/max` | max(A, B) | `math/acos` | arccosine |
| `math/clamp` | clamp(value, min, max) | `math/radians` | degrees → radians |
| `math/lerp` | mix(A, B, T) | `math/degrees` | radians → degrees |
| `math/smoothstep` | Hermite(edge0, edge1, x) | `math/sin` | sine |
| `math/step` | 0 / 1 step | `math/cos` | cosine |
| `math/remap` | remap range | | |

### Vector

| Node type | Name | Description |
|-----------|------|-------------|
| `vector/split_vec2` | Split Vec2 | Vec2 → `x`, `y` |
| `vector/split_vec3` | Split Vec3 | Vec3 → `x`, `y`, `z` |
| `vector/combine_vec2` | Combine Vec2 | Components → Vec2 |
| `vector/combine_vec3` | Combine Vec3 | Components → Vec3 |
| `vector/combine_vec4` | Combine Vec4 | Components → Vec4 |
| `vector/dot` | Dot Product | Scalar |
| `vector/cross` | Cross Product | Vec3 |
| `vector/normalize` | Normalize | Unit vector |
| `vector/distance` | Distance | Distance between points |
| `vector/length` | Length | Magnitude |
| `vector/reflect` | Reflect | Reflect about a normal |
| `vector/refract` | Refract | Refract with IOR ratio |
| `vector/swizzle` | Swizzle | Reorder vec4 channels (0=X … 4=zero, 5=one) |

### Color

| Node type | Name | Description |
|-----------|------|-------------|
| `color/constant` | Color | Constant color (`color`, `rgb`, `r`, `g`, `b`) |
| `color/float` | Float | Constant scalar |
| `color/vec2` | Vec2 | Constant Vec2 |
| `color/vec3` | Vec3 | Constant Vec3 |
| `color/lerp` | Color Lerp | Blend two colors |
| `color/cosine_palette` | Cosine Palette | IQ-style procedural palette |
| `color/fresnel` | Fresnel | View-angle rim factor |
| `color/srgb_to_linear` | sRGB → Linear | Color space convert |
| `color/linear_to_srgb` | Linear → sRGB | Color space convert |
| `color/rgb_to_hsv` | RGB → HSV | |
| `color/hsv_to_rgb` | HSV → RGB | |
| `color/hue_shift` | Hue Shift | Rotate hue |
| `color/luminance` | Luminance | Perceptual brightness |
| `color/gamma` | Gamma | Gamma curve |
| `color/brightness_contrast` | Brightness/Contrast | |
| `color/saturation` | Saturation | Adjust saturation |
| `color/blend` | Blend | Photoshop-style blend modes |

### Procedural

| Node type | Name | Node type | Name |
|-----------|------|-----------|------|
| `procedural/noise_perlin` | Perlin Noise | `procedural/gradient_radial` | Radial Gradient |
| `procedural/noise_simplex` | Simplex Noise | `procedural/gradient_linear` | Linear Gradient |
| `procedural/noise_voronoi` | Voronoi | `procedural/gradient_angular` | Angular Gradient |
| `procedural/noise_fbm` | FBM | `procedural/gradient_diamond` | Diamond Gradient |
| `procedural/checkerboard` | Checkerboard | `procedural/bump_offset` | Bump Offset (parallax) |
| `procedural/gradient` | Gradient | `procedural/noise_ridged` | Ridged Noise |
| `procedural/brick` | Brick | `procedural/noise_turbulence` | Turbulence |
| `procedural/normal_from_height` | Normal from Height | `procedural/noise_billow` | Billow Noise |
| `procedural/world_normal_from_height` | World Normal from Height | `procedural/noise_white` | White Noise |
| `procedural/domain_warp` | Domain Warp | `procedural/noise_curl` | Curl Noise |
| `procedural/hex_tile` | Hex Tile | | |
| `procedural/noise_triplanar_fbm` | Triplanar FBM | `procedural/noise_triplanar_billow` | Triplanar Billow |
| `procedural/noise_triplanar_ridged` | Triplanar Ridged | `procedural/noise_triplanar_voronoi` | Triplanar Voronoi |
| `procedural/noise_triplanar_turbulence` | Triplanar Turbulence | | |

### Animation

| Node type | Name | Description |
|-----------|------|-------------|
| `animation/uv_scroll` | UV Scroll | Time-driven UV scroll |
| `animation/flow_map` | Flow Map | Flow-map distortion |
| `animation/sine_wave` | Sine Wave | Animated sine |
| `animation/ping_pong` | Ping Pong | Back-and-forth oscillation |
| `animation/wind` | Wind | Vertex sway for vegetation |
| `animation/flipbook_uv` | Flipbook UV | Sprite-sheet frame UVs |

### Utility

| Node type | Name | Description |
|-----------|------|-------------|
| `utility/world_pos_mask` | World Position Mask | Mask by world position |
| `utility/slope_mask` | Slope Mask | Mask by surface slope |
| `utility/depth_fade` | Depth Fade | Distance-based fade |
| `utility/dpdx` | DDX | Screen-space x derivative |
| `utility/dpdy` | DDY | Screen-space y derivative |
| `utility/fwidth` | Fwidth | abs(ddx) + abs(ddy) |
| `utility/dither` | Dither | Ordered dithering |
| `utility/hash` | Hash | Deterministic pseudo-random |

### Control

| Node type | Name | Description |
|-----------|------|-------------|
| `control/if` | If | Select A/B on a condition |
| `control/static_switch` | Static Switch | Compile-time branch |
| `control/component_mask` | Component Mask | Pick vector channels |
| `control/greater_than` | Greater Than | Comparison → bool |
| `control/less_than` | Less Than | Comparison → bool |
| `control/equal` | Equal | Comparison → bool |
| `control/not_equal` | Not Equal | Comparison → bool |
| `control/and` | And | Boolean and |
| `control/or` | Or | Boolean or |
| `control/not` | Not | Boolean not |

### Scene

These read Bevy prepass buffers, so the camera must have the matching prepass enabled.

| Node type | Name | Description |
|-----------|------|-------------|
| `scene/pixel_depth` | Pixel Depth | Linear view-space depth of this fragment |
| `scene/scene_depth` | Scene Depth | Opaque depth buffer (needs `DepthPrepass`) |
| `scene/depth_fade` | Scene Depth Fade | Proximity fade to nearest surface (foam, soft intersection) |
| `scene/scene_normal` | Scene Normal | World normal from the normal prepass |
| `scene/motion_vector` | Motion Vector | Screen-space velocity (needs `MotionVectorPrepass`) |
| `scene/refraction_uv_offset` | Refraction UV Offset | Screen-UV offset for refraction |
| `scene/screen_uv` | Screen UV | Fragment screen-space UV |
| `scene/scene_color` | Scene Color | **Not implemented** — returns magenta (no grab-pass) |
| `scene/env_map_sample` | Environment Map Sample | Sample the scene env cubemap along a direction |
| `scene/env_map_reflect` | Environment Map Reflect | Mirror/glossy reflection off the env cubemap |

> `scene/scene_color` is a placeholder: Bevy doesn't expose a grab-pass texture to custom-material shaders without a dedicated render-graph node, so this node currently outputs magenta.

### Custom

| Node type | Name | Description |
|-----------|------|-------------|
| `custom/code` | Custom Code | Inline WGSL escape hatch — write shader logic the node library can't express |

`custom/code` takes four `vec4` inputs (`a`, `b`, `c`, `d`) plus a `code` string pin, and exposes `result` (`vec4`) and its `rgb`/`x`/`y`/`z`/`w` channels. The snippet runs in a generated helper function with the inputs in scope and assigns the `result` (pre-seeded to opaque black):

```wgsl
result = a * b + vec4<f32>(sin(c.x), 0.0, 0.0, 1.0);
```

It's the in-graph counterpart to a full [code shader](#code-shaders): reach for it when you only need a few lines of WGSL inside an otherwise node-based material.

## Material functions (subgraphs)

A reusable subgraph is stored as a `.material_function` JSON file (a `MaterialFunction`: a `name` plus a `MaterialGraph`). The resolver loads `*.material_function` files sibling to a material. Inside a function graph you use the bracket nodes instead of an output node:

| Node type | Role |
|-----------|------|
| `function/input_point` | Outputs the call site's four `Vec4` inputs (`in_0`…`in_3`) |
| `function/output_point` | Receives the four `Vec4` returns (`out_0`…`out_3`) |
| `function/call` | Invokes a function by name; set `input_values["function"]` to the function name |

At compile time each `function/call` inlines the function's WGSL helper at module scope and invokes it at the call site, so functions compose like ordinary nodes while staying visually encapsulated.

## Code shaders

Besides node graphs, `MaterialRef` can point at a `.shader` file — a hand-written shader. `renzora_shader` ships four transpiling backends, all targeting WGSL for Bevy's pipeline:

| Backend (`language`) | Handles | Notes |
|----------------------|---------|-------|
| `Bevy` | WGSL using `#import bevy_*` | Bevy-flavored WGSL (naga_oil); default |
| `WGSL` | raw WGSL | `@fragment`, `var<uniform>`, … |
| `GLSL` | `.glsl`, `.frag`, `.vert` | `#version`, `void main()`, `uniform`/`varying` |
| `ShaderToy` | ShaderToy-style fragment code | `mainImage`, `iTime`, `iResolution` |

A `.shader` file is a JSON `ShaderFile`:

| Field | Type | Notes |
|-------|------|-------|
| `language` | string | One of the backend names above |
| `shader_type` | enum | `Fragment` (default), `Material`, or `PostProcess` |
| `shader_source` | string | Raw source code |
| `params` | object | Parameters parsed from `@param` annotations |

The compiler transpiles to WGSL, injects a `ShaderUniforms` bind group (`time`, `delta_time`, `resolution`, `mouse`, `frame`) at `@group(3) @binding(0)` when the source references `uniforms.*`, and turns `@param` comment annotations into WGSL `const` declarations so parameter names resolve as variables. A `Fragment` shader previews on a mesh via `CodeShaderMaterial`; `Material` and `PostProcess` types are compile-checked.

```wgsl
#import bevy_pbr::forward_io::VertexOutput

// @param speed float 1.0 0.1 5.0 Animation speed
// @param tint  color 1.0 0.8 0.6 1.0

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = uniforms.time * speed;
    let r = 0.5 + 0.5 * cos(t + in.uv.x * 6.283);
    let g = 0.5 + 0.5 * cos(t + in.uv.y * 6.283 + 2.094);
    let b = 0.5 + 0.5 * cos(t + (in.uv.x + in.uv.y) * 3.141 + 4.189);
    return vec4<f32>(r * tint.r, g * tint.g, b * tint.b, 1.0);
}
```

`@param` syntax is `// @param <name> <type> <default> [min max] [description]`, where `<type>` is `float`, `vec2`, `vec3`, `vec4`, `color`, `int`, or `bool`. Author and preview code shaders in the **Shader Editor** (the asset browser routes `.wgsl`/`.glsl`/`.vert`/`.frag` there).

## Scripting

Material authoring is done in the graph, not in scripts. Lua registers exactly one material-related global:

```lua
-- Set the base color of the material on this entity. r, g, b, a are 0.0–1.0
-- floats; a is optional and defaults to 1.0.
set_material_color(1.0, 0.0, 0.0)        -- red
set_material_color(0.0, 0.4, 1.0, 0.5)   -- semi-transparent blue
```

> `set_material_color` is **Lua-only** — it is not part of the [Rhai](/docs/r1-alpha5/scripting/rhai) subset. There are **no** `set_material_property`, `set_material_emissive`, or `swap_material` functions. In this release `set_material_color` emits a script action that the renderer does not yet consume, so for reliable runtime variation use **material instances** / `MaterialOverrides` or edit the graph directly.

To add your own node types to the graph, see [Custom Material Nodes](/docs/r1-alpha5/extending/material-nodes).

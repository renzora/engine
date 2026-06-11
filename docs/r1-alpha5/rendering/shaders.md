# WGSL Shaders

Author code shaders in WGSL, GLSL, or ShaderToy syntax and preview them live in the editor — Renzora transpiles everything to WGSL and runs it through one shared material runtime.

## The shader material runtime

Code shaders live in the `renzora_shader` crate. Its `ShaderPlugin` (registered with `renzora::add!(ShaderPlugin)`, so it runs in both the editor and exported games) provides:

- **`CodeShaderMaterial`** — a per-entity Bevy `Material` whose fragment shader is authored in code and compiled to WGSL at runtime. Each instance carries its own `Handle<Shader>`, and `Material::specialize()` swaps the fragment shader per pipeline key, so different entities can run different code shaders at once.
- **`ShaderBackendRegistry`** — the set of language backends that transpile source to WGSL.
- **`ShaderCache`** — caches compiled `Handle<Shader>` assets by source hash so identical sources don't recompile.

> This page is about **code-authored shader materials** and the shader editor. Two related systems are documented separately: visual PBR materials in the [Material Editor](../editor/materials.md), and fullscreen camera effects in [Post-Processing Effects](../extending/post-processing.md). They are not the same code path.

## Shader languages and backends

`ShaderPlugin` registers four backends. Every backend implements one trait — `name()`, `file_extensions()`, and `to_wgsl()` — and all of them ultimately produce WGSL for Bevy's pipeline.

| Backend | Language name | Extensions | What it does |
|---|---|---|---|
| `BevyBackend` | `Bevy` | `bevy.wgsl` | Passthrough. Bevy's `naga_oil` preprocessor resolves `#import` directives at runtime, so the source is not pre-validated. |
| `WgslBackend` | `WGSL` | `wgsl` | Plain WGSL, validated up-front with `naga`'s WGSL parser. |
| `GlslBackend` | `GLSL` | `glsl`, `frag`, `vert` | Parses a GLSL fragment shader with `naga`, validates, and writes WGSL. |
| `ShaderToyBackend` | `ShaderToy` | `shadertoy` | Wraps a `mainImage(...)` body into GLSL, transpiles via `naga`, then remaps ShaderToy uniforms onto the engine's uniform struct. |

The editor picks a backend from the shader's stored `language` field. When that is unknown it auto-detects from the source: `mainImage`/`iResolution`/`iTime` ⇒ ShaderToy, `#import bevy_` ⇒ Bevy, `#version`/`void main`/`gl_Frag` ⇒ GLSL, `@fragment`/`var<uniform>` ⇒ WGSL, otherwise Bevy.

> The asset browser routes `.wgsl`, `.glsl`, `.vert`, and `.frag` files to the **Shader** editor tab (double-click). There is **no `renzora::` shader import module** — older docs referencing `#import renzora::common_uniforms`, `renzora::noise`, or `renzora::math` are wrong; those modules do not exist.

## WGSL basics

WGSL is the WebGPU Shading Language used by `wgpu`. A quick refresher on the syntax:

```wgsl
// Types
var x: f32 = 1.0;
var v: vec3<f32> = vec3<f32>(1.0, 2.0, 3.0);
var m: mat4x4<f32>;

// Functions
fn my_func(a: f32, b: f32) -> f32 {
    return a + b;
}

// Swizzling
var color: vec4<f32> = vec4<f32>(1.0, 0.5, 0.0, 1.0);
var rgb: vec3<f32> = color.rgb;
var rr: vec2<f32> = color.xx;
```

## Writing a fragment shader

A code shader is fragment-only: you write the `@fragment` entry point named `fragment`, and the engine supplies the vertex stage and the uniform block. Import Bevy's `VertexOutput` for the interpolated per-pixel inputs (`in.uv`, `in.world_position`, `in.world_normal`, …). This is the default Bevy-WGSL template:

```wgsl
#import bevy_pbr::forward_io::VertexOutput

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
```

`uniforms` and the `speed`/`tint` constants in the example are injected for you — see the next two sections.

To read camera/view data (world-space camera position, view-projection matrix, etc.), import Bevy's view bindings rather than declaring your own:

```wgsl
#import bevy_pbr::mesh_view_bindings::view
// e.g. view.world_position, view.clip_from_world
```

## Built-in uniforms

`CodeShaderMaterial` provides a single uniform block at the material bind group (`@group(3) @binding(0)`). The registry injects this declaration automatically **if** your shader references `uniforms.` and doesn't already declare its own `@group(3)` binding:

```wgsl
struct ShaderUniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    mouse: vec4<f32>,
    frame: u32,
    _pad: vec3<f32>,
}

@group(3) @binding(0) var<uniform> uniforms: ShaderUniforms;
```

| Field | Type | Updated each frame? | Notes |
|---|---|---|---|
| `uniforms.time` | `f32` | yes | Elapsed seconds. |
| `uniforms.delta_time` | `f32` | yes | Seconds since last frame. |
| `uniforms.frame` | `u32` | yes | Frame counter (wraps). |
| `uniforms.resolution` | `vec2<f32>` | no | Present in the struct; currently fixed (defaults to 512×512). |
| `uniforms.mouse` | `vec4<f32>` | no | Present in the struct; currently fixed at zero. |
| `uniforms._pad` | `vec3<f32>` | — | Padding for 16-byte alignment; do not use. |

> If your shader declares its own bindings at `@group(3)` (a full custom material with textures/samplers), the engine treats it as a self-managed material and does **not** inject `ShaderUniforms`. Such shaders compile but cannot use the editor's mesh preview (see below).

## Exposing parameters with `@param`

Annotate shader source with `// @param` comments to declare tweakable constants. The engine parses them and **injects WGSL `const` declarations** into the compiled output, so the parameter name resolves as an ordinary WGSL variable. In the editor, each `@param` also becomes an editable control in the **Shader Properties** panel.

Syntax (works after `//` or inside `/* ... */`):

```text
// @param <name> <type> <default> [min max] [description]
```

```wgsl
// @param speed float 1.0 0.0 10.0 Animation speed
// @param tint color 1.0 0.5 0.0 1.0
// @param offset vec2 0.0 0.0
// @param count int 4
// @param enabled bool true
```

Supported types and the WGSL `const` each one generates:

| `@param` type | Aliases | Extra args | Generated WGSL constant |
|---|---|---|---|
| `float` | `f32` | `[min] [max]` | `const name: f32 = ...;` |
| `int` | `i32` | — | `const name: i32 = ...;` |
| `bool` | — | — | `const name: bool = ...;` |
| `vec2` | — | — | `const name: vec2<f32> = vec2<f32>(...);` |
| `vec3` | — | — | `const name: vec3<f32> = vec3<f32>(...);` |
| `vec4` | — | — | `const name: vec4<f32> = vec4<f32>(...);` |
| `color` | `colour` | — | `const name: vec4<f32> = vec4<f32>(r, g, b, a);` |

The optional `min`/`max` (floats only) set the slider bounds in the inspector. Anything after the recognized tokens becomes the parameter's description.

> Because parameters are compiled in as `const` values, editing one in the Shader Properties panel **re-injects the constants and recompiles** rather than feeding a live uniform. The editor does this incrementally — it re-injects against the already-transpiled WGSL instead of re-transpiling the whole source — so the preview updates immediately.

In the editor, `@param`s are grouped by kind (Float, Color, Vector, Integer, Boolean) and rendered as scrubbable number fields, colour swatches, or checkboxes. If a shader has no `@param` annotations, the panel shows a hint to add some.

## ShaderToy shaders

Paste a ShaderToy `mainImage` body and the ShaderToy backend wraps it, transpiles GLSL→WGSL, and remaps the ShaderToy globals onto `ShaderUniforms`:

| ShaderToy | Maps to | Notes |
|---|---|---|
| `iTime` | `uniforms.time` | Playback seconds. |
| `iTimeDelta` | `uniforms.delta_time` | Frame delta. |
| `iResolution` | `vec3<f32>(uniforms.resolution, aspect)` | Width, height, aspect. |
| `iMouse` | `uniforms.mouse` | Currently fixed at zero. |
| `iFrame` | `i32(uniforms.frame)` | Frame counter. |

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec3 col = 0.5 + 0.5 * cos(iTime + uv.xyx + vec3(0.0, 2.0, 4.0));
    fragColor = vec4(col, 1.0);
}
```

The backend also runs basic preprocessing for code-golfed ShaderToy shaders: it expands `#define` macros (object- and function-like), strips `f` float suffixes, and rewrites `mat2(vec4)` constructors that `naga` doesn't accept. Heavily exotic shaders may still fail to transpile — the compiler log will tell you.

## GLSL shaders

The GLSL backend parses a fragment-stage shader, validates it, and emits WGSL. Provide a standard `void main()` writing to an `out vec4`. Validation is strict (full `naga` capabilities), so target portable GLSL.

## The shader editor

Open a shader file to author and preview it. The editor (`renzora_shader_editor`, editor-only) registers three native panels:

- **Shader Preview** (`shader_preview`) — renders the compiled shader to an offscreen texture on a selectable mesh and shows it in an image node. Mesh options: Quad, Sphere (default), Cube, Cylinder, Capsule, Torus, Cone, Tetrahedron, Plane. The preview camera only activates when there is a compiled, preview-compatible shader.
- **Compiler Log** (`shader_compiler_log`) — a green "Compiled successfully" line, or a scrollable list of errors with a `[line:col]` location prefix in monospace.
- **Shader Properties** (`shader_properties`) — the `@param` editors described above.

Auto-compile is on by default, so edits recompile as you go.

> Only **Fragment**-type shaders preview on the mesh. **Material** shaders (custom bind groups, textures/samplers) and **Post-Process** shaders can't use the mesh preview — the panel shows an explanation instead. Author fullscreen effects with the `#[post_process]` macro (see [Post-Processing Effects](../extending/post-processing.md)).

## The `.shader` file format

A `.shader` file is JSON. Its key fields:

| Field | Type | Meaning |
|---|---|---|
| `language` | string | Backend name: `"Bevy"`, `"WGSL"`, `"GLSL"`, or `"ShaderToy"`. |
| `shader_type` | enum | `Fragment`, `Material`, or `PostProcess` (defaults to `Fragment`). |
| `shader_source` | string | The raw shader source. |
| `params` | map | User parameters (also recoverable from `@param` annotations). |

`compiled_wgsl` is not serialized — it is recomputed when the file loads.

```json
{
  "language": "WGSL",
  "shader_type": "Fragment",
  "shader_source": "#import bevy_pbr::forward_io::VertexOutput\n// @param speed float 1.0 0.1 5.0\n@fragment\nfn fragment(in: VertexOutput) -> @location(0) vec4<f32> {\n    return vec4<f32>(vec3<f32>(uniforms.time * speed % 1.0), 1.0);\n}",
  "params": {}
}
```

## Debugging

- **Compile errors** appear in the Compiler Log panel with a `[line:col]` (or `[line N]`) prefix when the backend reports a position. WGSL is validated by `naga`'s WGSL frontend; GLSL/ShaderToy by the GLSL frontend plus a full validation pass.
- **Visual debugging** — output an intermediate value as colour: `return vec4<f32>(value, value, value, 1.0);`.
- **Layout mismatches** — if you hand-declare bindings at `@group(3)`, the shader becomes preview-incompatible; verify it in a scene viewport instead.
- **RenderDoc** — attach RenderDoc to capture a frame and inspect pipeline state and shader I/O.

## Related

- [Post-Processing Effects](../extending/post-processing.md) — fullscreen camera shaders via `#[post_process]`.
- [Material Editor](../editor/materials.md) — visual PBR node-graph materials (`.material`).
- [Render Pipeline](pipeline.md) — where these passes sit in the frame.

# Shaders

Author code shaders in WESL, WGSL, GLSL or ShaderToy syntax and preview them live in the editor. Renzora runs every one of them through one shared material runtime, and shaders written for older versions of the engine keep working.

## The shader material runtime

Code shaders live in the `renzora_shader` crate. Its `ShaderPlugin` (registered with `renzora::add!(ShaderPlugin)`, so it runs in both the editor and exported games) provides:

- **`CodeShaderMaterial`**: a per-entity Bevy `Material` whose fragment shader is authored in code. Each instance carries its own `Handle<Shader>`, and `Material::specialize()` swaps the fragment shader per pipeline key, so different entities can run different code shaders at once.
- **`ShaderBackendRegistry`**: the set of language backends that turn source into something Bevy compiles.
- **`ShaderCache`**: caches compiled `Handle<Shader>` assets by source hash so identical sources don't recompile.

> This page is about **code-authored shader materials** and the shader editor. Two related systems are documented separately: visual PBR materials in the [Material Editor](../editor/materials.md), and fullscreen camera effects in [Post-Processing Effects](../extending/post-processing.md). They are not the same code path.

## WESL, and shaders from older versions

Bevy 0.20, which the engine is built on, compiles every shader with **WESL**, a superset of WGSL that adds imports and conditional compilation. Write new shaders as `.wesl`:

```wesl
import bevy_pbr::render::forward_io::VertexOutput;
import super::noise::fbm;            // another .wesl file in the same folder

@if(VERTEX_COLORS)
const USE_TINT = true;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> color: vec4<f32>;
```

- Imports end with `;` and come first, before any declaration.
- A module's name is its path. Bevy's own are under their crate and folder: `bevy_pbr::render::forward_io`, `bevy_pbr::render::mesh_view_bindings`, `bevy_core_pipeline::fullscreen_vertex_shader`. A project file is imported relative to the importer, as `super::noise`.
- `@if(FLAG)`, `@elif` and `@else` gate a declaration, struct member, parameter or statement. Pipeline flags such as `VERTEX_UVS_A` and `VERTEX_COLORS` are the same flags `#ifdef` tested; a flag nobody sets counts as false.
- Numeric pipeline values are `constants::NAME` (they were `#{NAME}`).

**Shaders in the old dialect still work.** Engine versions before r1-alpha8 used Bevy's previous preprocessor, which spelled these `#import`, `#ifdef`/`#else`/`#endif` and `#{NAME}`. Everything that may still be written that way is translated to WESL when it loads:

- a `.wgsl` file in a project, a theme folder or a plugin, through the engine's `.wgsl` asset loader;
- the shader embedded in a `.material` saved by an earlier version;
- a shader a plugin builds from a string with `renzora::shader_source::shader(source, path)`.

The translation renames Bevy modules to their new paths (`bevy_pbr::forward_io` becomes `bevy_pbr::render::forward_io`), turns `#ifdef` blocks into `@if`, and rewrites the calls whose signature changed in a mechanical way, such as `alpha_discard`. What it cannot fix is logged with the file and line, for example a call to a Bevy shader function that 0.20 removed. A shader with no directives at all is plain WGSL and is used as it is.

## Shader languages and backends

`ShaderPlugin` registers five backends. Every backend implements one trait (`name()`, `file_extensions()` and `to_wgsl()`) and hands Bevy either WGSL or WESL.

| Backend | Language name | Extensions | What it does |
|---|---|---|---|
| `WeslBackend` | `WESL` | `wesl` | Passthrough. Imports resolve against the running app's shader library and the project's files, so errors appear when the pipeline is built. The default for new shaders. |
| `BevyBackend` | `Bevy` | `bevy.wgsl` | The pre-0.20 dialect (`#import`, `#ifdef`). Translated to WESL. |
| `WgslBackend` | `WGSL` | `wgsl` | Plain WGSL, validated up front with `naga`'s WGSL parser. |
| `GlslBackend` | `GLSL` | `glsl`, `frag`, `vert` | Parses a GLSL fragment shader with `naga`, validates, and writes WGSL. |
| `ShaderToyBackend` | `ShaderToy` | `shadertoy` | Wraps a `mainImage(...)` body into GLSL, transpiles via `naga`, then remaps ShaderToy uniforms onto the engine's uniform struct. |

The editor picks a backend from the shader's stored `language` field. When that is unknown it detects one from the source: `mainImage`/`iResolution`/`iTime` is ShaderToy; `#version`/`void main`/`gl_Frag` is GLSL; any old-dialect directive is Bevy; an `import a::b;` statement is WESL; `@fragment`/`var<uniform>` is WGSL; anything else is WESL. A file named `.wesl` is always WESL.

> The asset browser routes `.wesl`, `.wgsl`, `.glsl`, `.vert` and `.frag` files to the **Shader** editor tab (double-click). There is **no `renzora::` shader import module**: older docs referencing `#import renzora::common_uniforms`, `renzora::noise` or `renzora::math` are wrong; those modules do not exist.

## Writing a fragment shader

A code shader is fragment-only: you write the `@fragment` entry point named `fragment`, and the engine supplies the vertex stage. Import Bevy's `VertexOutput` for the interpolated per-pixel inputs (`in.uv`, `in.world_position`, `in.world_normal` and so on). This is the template a new shader starts from:

```wesl
import bevy_pbr::render::forward_io::VertexOutput;

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

`uniforms` and the `speed`/`tint` constants in the example are injected for you; see the next two sections.

To read camera and time data, import Bevy's view bindings rather than declaring your own:

```wesl
import bevy_pbr::render::mesh_view_bindings::{view, globals};
// view.world_position, view.clip_from_world, globals.time
```

`in.uv` only exists on a mesh with UVs. A shader meant for any mesh can fall back on `@if(VERTEX_UVS_A)`:

```wesl
var coord = in.world_position.xz;
@if(VERTEX_UVS_A)
coord = in.uv;
```

## Applying a shader to a mesh

Save the shader as a `.wesl` (or `.wgsl`) file anywhere in the project, then drag it from the asset browser onto a mesh in the viewport, or onto a row in the Hierarchy. It is assigned exactly like a `.material`: the mesh's material reference points at the file, it is saved with the scene, and Undo puts the previous material back.

A `.wesl` file can import its neighbours, so shared code (a noise library, say) lives in one file that several shaders `import super::noise::fbm;` from. The imported file needs no `fragment` of its own.

A code shader is unlit: the colour your `fragment` returns is the colour on screen, and scene lights do not touch it. A shader that wants shading works it out itself, for example from `in.world_normal` and a light direction exposed as a `@param`. `discard;` cuts real holes in the mesh.

## Built-in uniforms

`CodeShaderMaterial` provides a single uniform block at the material bind group (`@group(3) @binding(0)`). The registry injects this declaration automatically, after the imports, **if** your shader references `uniforms.` and doesn't already declare its own material bindings (`@group(3)` or `@group(constants::MATERIAL_BIND_GROUP)`):

```wesl
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
| `uniforms._pad` | `vec3<f32>` | n/a | Padding for 16-byte alignment; do not use. |

Bevy's own `globals.time` (from `mesh_view_bindings`) works too and needs no injection.

> If your shader declares its own material bindings (a full custom material with textures and samplers), the engine treats it as a self-managed material and does **not** inject `ShaderUniforms`. Such shaders compile but cannot use the editor's mesh preview (see below).

## Exposing parameters with `@param`

Annotate shader source with `// @param` comments to declare tweakable constants. The engine parses them and **injects `const` declarations** into the compiled output (after the imports, which WESL requires to come first), so the parameter name resolves as an ordinary variable. In the editor, each `@param` also becomes an editable control in the **Shader Properties** panel.

Syntax (works after `//` or inside `/* ... */`):

```text
// @param <name> <type> <default> [min max] [description]
```

```wesl
// @param speed float 1.0 0.0 10.0 Animation speed
// @param tint color 1.0 0.5 0.0 1.0
// @param offset vec2 0.0 0.0
// @param count int 4
// @param enabled bool true
```

Supported types and the `const` each one generates:

| `@param` type | Aliases | Extra args | Generated constant |
|---|---|---|---|
| `float` | `f32` | `[min] [max]` | `const name: f32 = ...;` |
| `int` | `i32` | none | `const name: i32 = ...;` |
| `bool` | none | none | `const name: bool = ...;` |
| `vec2` | none | none | `const name: vec2<f32> = vec2<f32>(...);` |
| `vec3` | none | none | `const name: vec3<f32> = vec3<f32>(...);` |
| `vec4` | none | none | `const name: vec4<f32> = vec4<f32>(...);` |
| `color` | `colour` | none | `const name: vec4<f32> = vec4<f32>(r, g, b, a);` |

The optional `min`/`max` (floats only) set the slider bounds in the inspector. Anything after the recognized tokens becomes the parameter's description.

> Because parameters are compiled in as `const` values, editing one in the Shader Properties panel **re-injects the constants and recompiles** rather than feeding a live uniform. The editor does this incrementally, re-injecting against the already-translated source instead of starting over, so the preview updates immediately.

In the editor, `@param`s are grouped by kind (Float, Color, Vector, Integer, Boolean) and rendered as scrubbable number fields, colour swatches, or checkboxes. If a shader has no `@param` annotations, the panel shows a hint to add some.

## ShaderToy shaders

Paste a ShaderToy `mainImage` body and the ShaderToy backend wraps it, transpiles GLSL to WGSL, and remaps the ShaderToy globals onto `ShaderUniforms`:

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

The backend also runs basic preprocessing for code-golfed ShaderToy shaders: it expands `#define` macros (object- and function-like), strips `f` float suffixes, and rewrites `mat2(vec4)` constructors that `naga` doesn't accept. Heavily exotic shaders may still fail to transpile; the compiler log will tell you.

## GLSL shaders

The GLSL backend parses a fragment-stage shader, validates it, and emits WGSL. Provide a standard `void main()` writing to an `out vec4`. Validation is strict (full `naga` capabilities), so target portable GLSL. This is the engine's own translation: Bevy 0.20 itself no longer reads GLSL.

## The shader editor

Open a shader file to author and preview it. The editor (`renzora_shader_editor`, editor-only) registers three native panels:

- **Shader Preview** (`shader_preview`): renders the compiled shader to an offscreen texture on a selectable mesh and shows it in an image node. Mesh options: Quad, Sphere (default), Cube, Cylinder, Capsule, Torus, Cone, Tetrahedron, Plane. The preview resolves relative imports against the folder of the file being edited. The preview camera only activates when there is a compiled, preview-compatible shader.
- **Compiler Log** (`shader_compiler_log`): a green "Compiled successfully" line, or a scrollable list of errors with a `[line:col]` location prefix in monospace.
- **Shader Properties** (`shader_properties`): the `@param` editors described above.

Auto-compile is on by default, so edits recompile as you go. A WESL shader's imports only resolve against the running app, so its errors come from the pipeline (the log) rather than from the Compiler Log's up-front check.

> Only **Fragment**-type shaders preview on the mesh. **Material** shaders (custom bind groups, textures and samplers) and **Post-Process** shaders can't use the mesh preview; the panel shows an explanation instead. Write fullscreen effects as effect files for a Post Process component (see [Post-Processing Effects](../extending/post-processing.md)).

## The `.shader` file format

A `.shader` file is JSON. Its key fields:

| Field | Type | Meaning |
|---|---|---|
| `language` | string | Backend name: `"WESL"`, `"Bevy"`, `"WGSL"`, `"GLSL"` or `"ShaderToy"`. |
| `shader_type` | enum | `Fragment`, `Material` or `PostProcess` (defaults to `Fragment`). |
| `shader_source` | string | The raw shader source. |
| `params` | map | User parameters (also recoverable from `@param` annotations). |

`compiled_wgsl` is not serialized; it is recomputed when the file loads. A `.shader` saved with `"language": "Bevy"` keeps loading through the translation described above.

```json
{
  "language": "WESL",
  "shader_type": "Fragment",
  "shader_source": "import bevy_pbr::render::forward_io::VertexOutput;\n// @param speed float 1.0 0.1 5.0\n@fragment\nfn fragment(in: VertexOutput) -> @location(0) vec4<f32> {\n    return vec4<f32>(vec3<f32>(uniforms.time * speed % 1.0), 1.0);\n}",
  "params": {}
}
```

## Debugging

- **Compile errors** appear in the Compiler Log panel with a `[line:col]` (or `[line N]`) prefix when the backend reports a position. WGSL is validated by `naga`'s WGSL frontend; GLSL and ShaderToy by the GLSL frontend plus a full validation pass. WESL, and translated old-dialect shaders, are compiled by Bevy when the pipeline is built, and their errors are logged.
- **Translation notes**: a shader in the old dialect that needed a hand fix says so in the log, with the file and line.
- **Visual debugging**: output an intermediate value as colour: `return vec4<f32>(value, value, value, 1.0);`.
- **Layout mismatches**: if you hand-declare material bindings, the shader becomes preview-incompatible; verify it in a scene viewport instead.
- **RenderDoc**: attach RenderDoc to capture a frame and inspect pipeline state and shader I/O.

## Related

- [Post-Processing Effects](../extending/post-processing.md): fullscreen camera shaders as effect files on a Post Process component.
- [Material Editor](../editor/materials.md): visual PBR node-graph materials (`.material`).
- [Render Pipeline](pipeline.md): where these passes sit in the frame.

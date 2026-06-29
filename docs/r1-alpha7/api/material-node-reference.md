# Material Node Reference

The complete catalog of **every** node in the Renzora material graph, grouped by
category, with each node's inputs, outputs, and exactly what it computes.

This page is the per-node companion to the [Material API](/docs/r1-alpha5/api/material)
(file format, instances, domains) and the [Material Editor](/docs/r1-alpha5/editor/materials)
(how to wire nodes). To add a brand-new built-in node, see
[Custom Material Nodes](/docs/r1-alpha5/extending/material-nodes).

All nodes live in `renzora_shader` — declared in `material/nodes.rs` (`ALL_NODES`)
and compiled to WGSL in `material/codegen.rs`. There are **13 categories** and
roughly **124 node types**.

## How to read this page

Every node is identified by a `category/name` string (e.g. `math/multiply`). For
each node you'll see its **inputs**, **outputs**, and **what it does**. Inputs are
written `name (type = default)`; an input with no default takes whatever you wire
into it (or a zero/identity value if left unconnected).

A few rules apply everywhere:

- **Pin types coerce automatically.** `Float`, `Vec2`, `Vec3`, `Vec4`, and `Color`
  are freely inter-connectable. Wiring a scalar into a vector copies it across every
  lane; wiring a wider vector into a narrower pin takes the leading components. So a
  `math/multiply` works on floats *or* colors with no extra nodes.
- **Unconnected UV inputs default to the mesh UVs.** Texture and pattern nodes that
  take a `uv` pin fall back to the mesh's UV attribute (`mat_uv`) when you leave it
  empty, so the simplest possible graph still works.
- **Most nodes are component-wise.** Math nodes operate per-channel on whatever type
  flows through them.
- **Nodes are compile-time.** Each node emits a WGSL snippet; the graph compiles to
  one shader. "Runtime branch" vs "compile-time branch" (see *Control*) is the one
  place this distinction is visible.

---

## Input

Sources of per-fragment data. Header color: blue. None of these have inputs unless
noted; they're where data *enters* the graph.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `input/uv` — **UV** | — | `uv` (Vec2), `u`, `v` (Float) | The mesh's texture coordinates (0–1). `u`/`v` are the split channels. |
| `input/uv_scale` — **UV Scale** | `uv` (Vec2), `scale` (Vec2 = 2,2), `offset` (Vec2 = 0,0) | `uv` (Vec2) | Tiling: `uv * scale + offset`. |
| `input/uv_polar` — **Polar UV** | `uv` (Vec2), `center` (Vec2 = 0.5,0.5) | `uv` (Vec2), `angle`, `radius` (Float) | Cartesian → polar around `center`. `angle` is `0..1`, `radius` is distance. For spirals, pies, radial sweeps. |
| `input/uv_rotator` — **UV Rotator** | `uv` (Vec2), `angle` (Float = 0, radians), `center` (Vec2 = 0.5,0.5) | `uv` (Vec2) | Rotates UVs around `center`. |
| `input/uv_panner` — **UV Panner** | `uv` (Vec2), `speed` (Vec2 = 0.1,0), `time_offset` (Float = 0) | `uv` (Vec2) | Time-driven scroll: `uv + speed * (time + offset)`. Matches Unreal's Panner. |
| `input/world_position` — **World Position** | — | `position` (Vec3), `x`, `y`, `z` (Float) | Fragment world-space position. |
| `input/world_normal` — **World Normal** | — | `normal` (Vec3), `x`, `y`, `z` (Float) | Fragment world-space surface normal. |
| `input/view_direction` — **View Direction** | — | `direction` (Vec3) | Normalized fragment → camera direction. |
| `input/time` — **Time** | — | `time`, `sin_time`, `cos_time` (Float) | Seconds since start, plus its sine and cosine for cheap oscillation. |
| `input/vertex_color` — **Vertex Color** | — | `color` (Color), `r`, `g`, `b`, `a` (Float) | Per-vertex color attribute (defaults to white on meshes without one). |
| `input/camera_position` — **Camera Position** | — | `position` (Vec3) | World-space camera position. |
| `input/object_position` — **Object Position** | — | `position` (Vec3) | The object's pivot in world space (handy for wind anchoring). |

---

## Parameter

Named graph-boundary inputs. The `name` pin is the identifier a **material
instance** or a `MaterialOverrides` component overrides by name; the `default` pin
is the value baked into the master shader. A graph may declare up to **32**
parameters. Header color: purple.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `param/float` — **Float Parameter** | `name` (String), `default` (Float) | `value` (Float) | Overridable scalar. |
| `param/color` — **Color Parameter** | `name` (String), `default` (Color) | `value` (Color) | Overridable color. |
| `param/vec2` — **Vec2 Parameter** | `name` (String), `default` (Vec2) | `value` (Vec2) | Overridable 2-vector. |
| `param/vec3` — **Vec3 Parameter** | `name` (String), `default` (Vec3) | `value` (Vec3) | Overridable 3-vector. |
| `param/vec4` — **Vec4 Parameter** | `name` (String), `default` (Vec4) | `value` (Vec4) | Overridable 4-vector. |
| `param/bool` — **Bool Parameter** | `name` (String), `default` (Bool) | `value` (Bool) | Overridable flag. |

> Give a parameter a **stable `name`** — that string is the key instances use to
> override it. Anything wired downstream of a parameter becomes tweakable per
> instance without recompiling the shader.

---

## Texture

Sample images. Each texture node holds its own texture reference (a `TexturePath`),
set in the inspector. 2D nodes default the `uv` pin to the mesh UVs. Header color:
tan.

> **All texture slots in a graph share one sampler** — the first 2D texture's
> filter/wrap settings (or a default linear sampler). This keeps the fragment stage
> under the 16-sampler limit that Metal and baseline Vulkan/WebGPU impose.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `texture/sample` — **Sample Texture** | `uv` (Vec2 = mesh UV) | `color` (Color), `rgb` (Vec3), `r`, `g`, `b`, `a` (Float) | Standard 2D `textureSample`. |
| `texture/sample_normal` — **Sample Normal Map** | `uv` (Vec2 = mesh UV), `strength` (Float = 1) | `normal` (Vec3) | Samples and decodes a tangent-space normal map (`rgb*2-1`), scales XY by `strength`, renormalizes. |
| `texture/triplanar` — **Triplanar Sample** | `scale` (Float = 1), `sharpness` (Float = 2) | `color` (Color), `rgb` (Vec3) | Projects the texture along world X/Y/Z and blends by the world normal — **no UVs, no seams**. `sharpness` tightens the blend. |
| `texture/sample_lod` — **Sample Texture LOD** | `uv` (Vec2 = mesh UV), `lod` (Float = 0) | `color`, `rgb`, `r`, `g`, `b`, `a` | `textureSampleLevel` at an explicit mip. Blur reflections with a roughness-driven LOD, or sample mip 0 inside a branch/loop where automatic derivatives are invalid. |
| `texture/sample_grad` — **Sample Texture Grad** | `uv` (Vec2 = mesh UV), `ddx` (Vec2), `ddy` (Vec2) | `color`, `rgb`, `r`, `g`, `b`, `a` | `textureSampleGrad` with explicit derivatives. Fixes mip selection when UVs are rotated or polar-warped — crisp anisotropic filtering. |
| `texture/sample_cubemap` — **Sample Cubemap** | `direction` (Vec3 = 0,1,0), `lod` (Float = 0) | `color`, `rgb`, `a` | Samples a **material-local** cubemap along a direction. Separate from the scene env map, so a graph can carry its own stylized sky/reflection. `lod` = glossiness. |
| `texture/sample_2d_array` — **Sample 2D Array** | `uv` (Vec2 = mesh UV), `layer` (Float = 0) | `color`, `rgb`, `r`, `g`, `b`, `a` | Layered array; `layer` (rounded to nearest int) picks the slice. For terrain layer stacks, skin/variant atlases, mask banks. |
| `texture/sample_3d` — **Sample 3D Texture** | `uvw` (Vec3 = 0.5,0.5,0.5) | `color`, `rgb`, `r`, `g`, `b`, `a` | Samples a volume texture at a `0..1³` coordinate. For volume fog, caustic/scattering LUTs, 3D color grading. |

---

## Math

Every Math node is **component-wise** and type-generic — the same `math/multiply`
multiplies two floats, two colors, or a float and a vector. Each has a single
`result` output. Header color: grey.

### Binary / multi-input

| Node | Inputs | What it does |
|------|--------|--------------|
| `math/add` — **Add** | `a`, `b` | `a + b` |
| `math/subtract` — **Subtract** | `a`, `b` | `a - b` |
| `math/multiply` — **Multiply** | `a`, `b` | `a * b` |
| `math/divide` — **Divide** | `a`, `b` | `a / b` (guards against divide-by-zero) |
| `math/power` — **Power** | `base`, `exp` | `pow(abs(base), exp)` |
| `math/min` — **Min** | `a`, `b` | `min(a, b)` |
| `math/max` — **Max** | `a`, `b` | `max(a, b)` |
| `math/modulo` — **Modulo** | `a`, `b` | floating-point remainder `a mod b` |
| `math/atan2` — **Atan2** | `y`, `x` | `atan2(y, x)` in radians |
| `math/clamp` — **Clamp** | `value`, `min`, `max` | `clamp(value, min, max)` |
| `math/lerp` — **Lerp** | `a`, `b`, `t` | `mix(a, b, t)` — linear blend |
| `math/smoothstep` — **Smoothstep** | `edge0`, `edge1`, `value` | Hermite ease between the edges |
| `math/step` — **Step** | `edge`, `value` | `0` if `value < edge`, else `1` |
| `math/remap` — **Remap** | `value`, `in_min`, `in_max`, `out_min`, `out_max` | Rescales from one range to another |

### Unary

| Node | What it does | Node | What it does |
|------|--------------|------|--------------|
| `math/abs` — **Abs** | absolute value | `math/negate` — **Negate** | `-value` |
| `math/one_minus` — **One Minus** | `1 - value` | `math/fract` — **Fract** | fractional part |
| `math/floor` — **Floor** | round down | `math/ceil` — **Ceil** | round up |
| `math/saturate` — **Saturate** | clamp to `0..1` | `math/sign` — **Sign** | `-1` / `0` / `+1` |
| `math/trunc` — **Trunc** | truncate toward zero | `math/round` — **Round** | round to nearest |
| `math/exp` — **Exp** | `e^x` | `math/log` — **Log** | `ln(x)` (guards `x>0`) |
| `math/sqrt` — **Sqrt** | `sqrt(max(x,0))` | `math/reciprocal` — **Reciprocal** | `1 / value` |
| `math/sin` — **Sin** | sine | `math/cos` — **Cos** | cosine |
| `math/tan` — **Tan** | tangent | `math/asin` — **Asin** | arcsine (input clamped ±1) |
| `math/acos` — **Acos** | arccosine (input clamped ±1) | `math/radians` — **To Radians** | degrees → radians |
| `math/degrees` — **To Degrees** | radians → degrees | | |

---

## Vector

Build, split, and operate on vectors. Header color: green.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `vector/split_vec2` — **Split Vec2** | `vector` (Vec2) | `x`, `y` (Float) | Unpack a Vec2. |
| `vector/split_vec3` — **Split Vec3** | `vector` (Vec3) | `x`, `y`, `z` (Float) | Unpack a Vec3. |
| `vector/combine_vec2` — **Combine Vec2** | `x`, `y` (Float) | `vector` (Vec2) | Pack a Vec2. |
| `vector/combine_vec3` — **Combine Vec3** | `x`, `y`, `z` (Float) | `vector` (Vec3) | Pack a Vec3. |
| `vector/combine_vec4` — **Combine Vec4** | `x`, `y`, `z`, `w` (Float, `w` = 1) | `vector` (Vec4) | Pack a Vec4. |
| `vector/dot` — **Dot Product** | `a`, `b` | `result` (Float) | `dot(a, b)`. |
| `vector/cross` — **Cross Product** | `a` (Vec3 = 1,0,0), `b` (Vec3 = 0,1,0) | `result` (Vec3) | `cross(a, b)`. |
| `vector/normalize` — **Normalize** | `vector` | `result` | Unit-length vector. |
| `vector/distance` — **Distance** | `a`, `b` | `result` (Float) | `distance(a, b)`. |
| `vector/length` — **Length** | `vector` | `result` (Float) | `length(vector)`. |
| `vector/reflect` — **Reflect** | `incident`, `normal` | `result` (Vec3) | `reflect(incident, normal)`. |
| `vector/refract` — **Refract** | `incident`, `normal` (=0,1,0), `eta` (Float = 1) | `result` (Vec3) | `refract(...)` with index-of-refraction ratio `eta`. |
| `vector/swizzle` — **Swizzle** | `vector` (Vec4), `out_x/y/z/w` (Int) | `vector` (Vec4) | Rearranges channels — each output picks `0=X, 1=Y, 2=Z, 3=W, 4=zero, 5=one`. |

---

## Color

Constants, color-space conversion, and grading. Header color: amber.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `color/constant` — **Color** | `color` (Color value) | `color`, `rgb`, `r`, `g`, `b`, `a` | A constant color (edited with a picker). |
| `color/float` — **Float** | `value` | `value` (Float) | A constant scalar. |
| `color/vec2` — **Vec2** | `value` | `value` (Vec2) | A constant Vec2. |
| `color/vec3` — **Vec3** | `value` | `value` (Vec3) | A constant Vec3. |
| `color/lerp` — **Color Lerp** | `a`, `b` (Color), `t` (Float) | `color` (Color) | `mix(a, b, t)` between two colors. |
| `color/cosine_palette` — **Cosine Palette** | `t` (Float), `a`, `b`, `c`, `d` (Vec3) | `color` (Vec3) | IQ cosine palette `a + b·cos(2π(c·t + d))` — smooth procedural gradients from four control vectors. |
| `color/fresnel` — **Fresnel** | `power` (Float = 5) | `result` (Float) | `pow(1 - max(dot(view, normal), 0), power)` — bright at grazing angles. Rim light, water edges. |
| `color/srgb_to_linear` — **sRGB → Linear** | `color` | `result` | Piecewise sRGB → linear decode. |
| `color/linear_to_srgb` — **Linear → sRGB** | `color` | `result` | Piecewise linear → sRGB encode. |
| `color/rgb_to_hsv` — **RGB → HSV** | `rgb` (Vec3) | `hsv`, `h`, `s`, `v` | Convert to hue/saturation/value. |
| `color/hsv_to_rgb` — **HSV → RGB** | `hsv` (Vec3) | `rgb` (Vec3) | Convert back to RGB. |
| `color/hue_shift` — **Hue Shift** | `rgb` (Vec3), `shift` (Float) | `rgb` (Vec3) | Rotate hue by `shift` (`0..1` = full circle). |
| `color/luminance` — **Luminance** | `rgb` (Vec3) | `value` (Float) | Rec.709 perceptual brightness. |
| `color/gamma` — **Gamma** | `color`, `gamma` (Float = 2.2) | `result` | `pow(color, gamma)` per channel. |
| `color/brightness_contrast` — **Brightness / Contrast** | `color`, `brightness` (=0), `contrast` (=1) | `result` | Additive brightness, contrast pivoted on 0.5 grey. |
| `color/saturation` — **Saturation** | `color`, `saturation` (=1) | `result` | Mix toward luminance: `0` = greyscale, `1` = original, `>1` = supersaturated. |
| `color/blend` — **Blend** | `base`, `blend` (Color), `opacity` (=1), `mode` (Int = 0) | `result` | Photoshop-style composite. `mode`: `0` normal, `1` multiply, `2` screen, `3` overlay, `4` add, `5` subtract, `6` soft-light, `7` hard-light, `8` difference, `9` divide. |

---

## Procedural

Patterns generated in-shader — no texture needed. Header color: violet.

### Noise

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `procedural/noise_perlin` — **Perlin Noise** | `uv` (=mesh UV), `scale` (Float = 10) | `value` (Float) | Smooth gradient noise, `0..1`. |
| `procedural/noise_simplex` — **Simplex Noise** | `uv`, `scale` (=10) | `value` | Gradient noise with fewer directional artifacts. |
| `procedural/noise_voronoi` — **Voronoi** | `uv`, `scale` (=5) | `distance` (F1), `f2` (F2), `edge`, `cell_id` | Cell/Worley noise — nearest-point distance, second-nearest, edge distance, and a per-cell random id. |
| `procedural/noise_fbm` — **FBM Noise** | `uv`, `scale` (=5), `octaves` (=4), `lacunarity` (=2), `persistence` (=0.5) | `value` | Fractal Brownian Motion — layered noise for clouds, terrain, marble. |
| `procedural/noise_ridged` — **Ridged FBM** | `uv`, `scale`, `octaves`, `lacunarity`, `persistence` | `value` | Sharp crests — mountain ridges, cumulus billows, cracks. |
| `procedural/noise_turbulence` — **Turbulence** | same as FBM | `value` | `\|noise\|` accumulated — fire, smoke, turbulent flow. |
| `procedural/noise_billow` — **Billow Noise** | same as FBM | `value` | `\|noise\|²` accumulated — puffy cumulus clouds, stone pores. |
| `procedural/noise_white` — **White Noise** | `uv`, `scale` (=50) | `value` | Uncorrelated random per UV — grain, sparkle. |
| `procedural/noise_curl` — **Curl Noise** | `uv`, `scale` (=3), `epsilon` (=0.01) | `flow` (Vec2) | Divergence-free 2D flow field for fluid-like advection and swirly UV distortion. |

### Triplanar noise

World-space noise projected onto X/Y/Z planes and blended by the world normal —
**no UVs, no seams**, works on any topology. All take `scale` (=1), `octaves`,
`lacunarity` (=2), `persistence` (=0.5), `sharpness` (=4) and output `value`.

| Node | What it does |
|------|--------------|
| `procedural/noise_triplanar_fbm` — **Triplanar FBM** | Seamless FBM on spheres, terrain, sculpts. |
| `procedural/noise_triplanar_ridged` — **Triplanar Ridged** | Seamless mountain/cumulus ridges. |
| `procedural/noise_triplanar_turbulence` — **Triplanar Turbulence** | Seamless fire/smoke/flow. |
| `procedural/noise_triplanar_billow` — **Triplanar Billow** | Seamless puffy cumulus / stone pores. |
| `procedural/noise_triplanar_voronoi` — **Triplanar Voronoi** | Seamless cracked-surface / cell pattern. Outputs `distance`, `cell_id`. |

### Patterns & gradients

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `procedural/checkerboard` — **Checkerboard** | `uv`, `scale` (=8) | `value` | Alternating 0/1 checker. |
| `procedural/gradient` — **Gradient** | `uv` | `u`, `v` (Float) | Raw `0..1` ramp along U or V. |
| `procedural/brick` — **Brick** | `uv`, `scale` (Vec2 = 4,8), `mortar` (Float = 0.05) | `value` | Row-offset brick pattern with mortar lines. |
| `procedural/gradient_radial` — **Radial Gradient** | `uv`, `center`, `radius` (=0.5), `softness` (=0.3) | `value` | 0 at center → 1 at `radius`, soft falloff. |
| `procedural/gradient_linear` — **Linear Gradient** | `uv`, `angle`, `center` | `value` | Ramp along a direction (`angle` radians, 0 = +X). |
| `procedural/gradient_angular` — **Angular Gradient** | `uv`, `center`, `offset` | `value` | Sweeps `0..1` around the center — pie / compass / clock wipes. |
| `procedural/gradient_diamond` — **Diamond Gradient** | `uv`, `center`, `size` (=0.5) | `value` | Diamond (Manhattan-distance) falloff. |

### Height, warp & anti-tiling

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `procedural/normal_from_height` — **Normal From Height (tangent)** | `height`, `strength` (=1) | `normal` (Vec3) | Tangent-space normal (Z = up) from a height value via screen-space derivatives. |
| `procedural/world_normal_from_height` — **World Normal From Height** | `height`, `strength` (=1) | `normal` (Vec3) | **World-space** perturbed normal — reconstructs a tangent frame per fragment, so it works on any orientation. Wire straight into Surface Output's `normal` (water, stone, procedural displacement). |
| `procedural/domain_warp` — **Domain Warp** | `uv`, `scale` (=1.5), `strength` (=0.35), `offset` (Vec2 = 5.2,1.3) | `uv` (Vec2) | Distorts UVs with FBM noise — organic cloud / marble / fluid shapes. |
| `procedural/bump_offset` — **Bump Offset** | `uv`, `height`, `reference` (=0.5), `strength` (=0.05) | `uv` (Vec2) | Cheap parallax — displaces UVs along the view vector by a height value for fake depth. |
| `procedural/hex_tile` — **Hex Tile UV** | `uv`, `scale` (=1), `variation` (=1) | `uv1`, `uv2`, `uv3` (Vec2), `w1`, `w2`, `w3` (Float) | Hexagonal anti-tiling — sample one texture with all three UVs and blend by the weights to kill visible repetition. `variation` = rotation scramble. |

---

## Animation

Time-driven motion. Header color: teal.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `animation/uv_scroll` — **UV Scroll** | `uv`, `speed` (Vec2 = 0.1,0) | `uv` (Vec2) | `uv + speed * time`. |
| `animation/flow_map` — **Flow Map** | `uv`, `flow` (Vec2), `speed` (=1), `strength` (=0.1) | `uv1`, `uv2` (Vec2), `blend` (Float) | Two-phase distortion + crossfade weight for realistic flowing water. Sample twice and `mix` by `blend`. |
| `animation/sine_wave` — **Sine Wave** | `frequency` (=1), `amplitude` (=1), `offset` (=0) | `value` (Float) | `sin(time·frequency + offset) · amplitude`. |
| `animation/ping_pong` — **Ping Pong** | `speed` (=1) | `value` (Float) | Triangular `0 → 1 → 0` wave. |
| `animation/wind` — **Wind** | `strength` (=0.3), `speed` (=1), `direction` (Vec2 = 1,0), `turbulence` (=0.2), `mask` (=1) | `displacement` (Vec3) | Vegetation sway. **Vertex domain** — wire into a Vegetation Output's `vertex_offset`. |
| `animation/flipbook_uv` — **Flipbook UV** | `uv`, `frame`, `cols` (=4), `rows` (=4) | `uv` (Vec2) | Sub-rect UV for one frame of a `cols × rows` sprite sheet. Drive `frame` by `time·fps` to play. |

---

## Utility

Masks, derivatives, and helpers. Header color: slate.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `utility/world_pos_mask` — **World Position Mask** | `height` (=10), `falloff` (=2) | `mask` (Float) | Mask by world Y height — snow on peaks, water lines. |
| `utility/slope_mask` — **Slope Mask** | `threshold` (=0.5), `falloff` (=0.2) | `mask` (Float) | Mask by surface slope — cliffs vs flat ground (smoothstep on `world_normal.y`). |
| `utility/depth_fade` — **Depth Fade** | `distance` (=1) | `fade` (Float) | Simple height-based fade. For true scene-depth proximity fade use `scene/depth_fade`. |
| `utility/dpdx` — **DDX** | `value` | `result` | Screen-space derivative along X. |
| `utility/dpdy` — **DDY** | `value` | `result` | Screen-space derivative along Y. |
| `utility/fwidth` — **FWidth** | `value` | `result` | `abs(ddx) + abs(ddy)` — pixel footprint for anti-aliasing. |
| `utility/dither` — **Dither** | — | `value` (Float) | 4×4 Bayer ordered dither from screen position — transparency-to-coverage. |
| `utility/hash` — **Hash** | `value` (Vec2) | `result` (Float) | Deterministic `0..1` pseudo-random hash. |

---

## Control

Branching and boolean logic. Header color: yellow.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `control/if` — **If** | `condition`, `threshold` (=0.5), `if_true` (Vec4 = 1,1,1,1), `if_false` (Vec4 = 0,0,0,1) | `result` (Vec4) | **Runtime** select: `condition > threshold ? if_true : if_false`. Both branches execute. |
| `control/static_switch` — **Static Switch** | `a` (Vec4), `b` (Vec4), `use_a` (Bool = true) | `result` (Vec4) | **Compile-time** branch — the unused side's subgraph is stripped from the shader entirely. Use for shader permutations. Set `use_a` in the node's values. |
| `control/component_mask` — **Component Mask** | `vector` (Vec4), `keep_r/g/b/a` (Bool) | `vector` (Vec4) | Zeroes the channels you toggle off. |
| `control/greater_than` — **Greater Than** | `a`, `b` | `result` | `1.0` if `a > b`, else `0.0`. |
| `control/less_than` — **Less Than** | `a`, `b` | `result` | `1.0` if `a < b`, else `0.0`. |
| `control/equal` — **Equal** | `a`, `b`, `epsilon` (=0.001) | `result` | `1.0` if `\|a-b\| < epsilon`. |
| `control/not_equal` — **Not Equal** | `a`, `b`, `epsilon` (=0.001) | `result` | `1.0` if `\|a-b\| ≥ epsilon`. |
| `control/and` — **And** | `a`, `b` | `result` | `min(a, b)` — float-boolean AND. |
| `control/or` — **Or** | `a`, `b` | `result` | `max(a, b)` — float-boolean OR. |
| `control/not` — **Not** | `value` | `result` | `1 - value`. |

---

## Scene

Read render-pass buffers and the environment. **These need the matching prepass
enabled on the camera** (depth/normal/motion); when a prepass is missing they return
a safe sentinel rather than failing. Header color: cyan.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `scene/pixel_depth` — **Pixel Depth** | — | `depth` (Float) | This fragment's linear view-space depth (distance from camera). |
| `scene/scene_depth` — **Scene Depth** | — | `depth` (Float) | The opaque-pass depth buffer at this fragment. Needs **DepthPrepass**; returns a large value otherwise. |
| `scene/depth_fade` — **Scene Depth Fade** | `distance` (=1) | `fade` (Float) | Proximity fade to the nearest opaque surface: 0 at contact → 1 when `distance` units behind. Shoreline foam, soft intersections. |
| `scene/scene_normal` — **Scene Normal** | — | `normal` (Vec3) | World normal from the normal prepass. Needs **NormalPrepass**; returns +Y otherwise. Wetness masks, edge detection. |
| `scene/motion_vector` — **Motion Vector** | — | `velocity` (Vec2), `speed` (Float) | Screen-space motion (Δ NDC since last frame). Needs **MotionVectorPrepass**. Motion blur masks, speed lines. |
| `scene/refraction_uv_offset` — **Refraction UV Offset** | `normal` (Vec3), `strength` (=0.05) | `offset` (Vec2) | Screen-UV offset from a distorting normal, for refraction. |
| `scene/screen_uv` — **Screen UV** | — | `uv` (Vec2) | Fragment screen-space UV (0,0 top-left → 1,1 bottom-right). |
| `scene/scene_color` — **Scene Color (stub)** | `uv` | `color`, `rgb` | **Not generally implemented** — Bevy doesn't expose a grab-pass to custom-material shaders without a render-graph node. Returns magenta as a placeholder. |
| `scene/env_map_sample` — **Environment Map Sample** | `direction` (Vec3 = 0,1,0), `mip_level` (=0) | `color`, `rgb` | Samples the scene environment cubemap along a direction. Works with loaded skyboxes and the procedural atmosphere. `mip_level` = blur/roughness. |
| `scene/env_map_reflect` — **Environment Map Reflect** | `normal` (Vec3), `mip_level` (=0) | `color`, `rgb` | Reflects the view direction off `normal` and samples the env map — mirror/glossy reflection. `mip_level` = glossiness. |

---

## Custom

The escape hatch when no node expresses what you need. Header color: red.

**`custom/code` — Custom Code.** Inputs: `code` (String, default `result = a;`),
`a`, `b`, `c`, `d` (Vec4). Outputs: `result` (Vec4), `rgb` (Vec3), `x`, `y`, `z`,
`w` (Float). Your snippet runs inside a generated helper `mat_custom_<id>(a,b,c,d)`
with the four inputs in scope and `result` pre-seeded to opaque black; assign
`result` to produce the output.

```wgsl
result = a * b + vec4<f32>(sin(c.x), 0.0, 0.0, 1.0);
```

It's the in-graph counterpart to a full code shader — reach for it when you only
need a few lines of WGSL inside an otherwise node-based material.

---

## Functions (reusable subgraphs)

A **material function** packages graph logic as a reusable node without writing
Rust. It's a named subgraph saved as a `.material_function` file (in
`assets/material_functions/`), bracketed by input/output point nodes and invoked by
a call node. These appear in the editor under the *Control* menu.

| Node | Inputs | Outputs | What it does |
|------|--------|---------|--------------|
| `function/input_point` — **Function Inputs** | — | `in_0`…`in_3` (Vec4) | Inside a function only: the call site's four inputs. Use `split_vec*` to unpack scalars. |
| `function/output_point` — **Function Outputs** | `out_0`…`out_3` (Vec4) | — | Inside a function only: what the function returns. |
| `function/call` — **Function Call** | `in_0`…`in_3` (Vec4) | `out_0`…`out_3` (Vec4) | Invokes a function by name (set `input_values["function"]`). At compile time it inlines the function's WGSL helper at module scope, so it composes like any node. Recursive cycles are detected and reported. |

---

## Output

Every graph has **exactly one** output node, fixed by its `domain`. It can't be
deleted, and it's where the material's final channels are assembled. **A pin only
takes effect when it's connected (or has an override) — disconnected pins keep
Bevy's `StandardMaterial` defaults.** Header color: dark red.

### `output/surface` — Surface Output

The full PBR master; maps 1:1 onto Bevy's `StandardMaterial`.

| Pin | Type | Default | Drives |
|-----|------|---------|--------|
| `base_color` | Color | `0.8, 0.8, 0.8, 1` | Albedo |
| `metallic` | Float | `0` | Dielectric ↔ metal |
| `roughness` | Float | `0.5` | Smooth ↔ matte |
| `normal` | Vec3 | — | Surface normal |
| `emissive` | Vec3 | `0` | Self-illumination |
| `ao` | Float | `1` | Ambient occlusion |
| `alpha` | Float | `1` | Opacity |
| `reflectance` | Vec3 | `0.5` | Dielectric specular reflectance |
| `specular_transmission` | Float | `0` | Refraction (glass, water) |
| `diffuse_transmission` | Float | `0` | Light through thin surfaces (foliage, skin) |
| `thickness` | Float | `0` | Volume thickness |
| `ior` | Float | `1.5` | Index of refraction |
| `attenuation_distance` | Float | `1e37` | Volume attenuation distance |
| `clearcoat` | Float | `0` | Second specular layer (car paint) |
| `clearcoat_roughness` | Float | `0.5` | Clearcoat roughness |
| `anisotropy_strength` | Float | `0` | Directional specular (brushed metal, hair) |
| `anisotropy_rotation` | Float | `0` | Anisotropy direction |

> Connecting either transmission pin makes the resolver flip on Bevy's transmissive
> pass automatically.

### `output/terrain_layer` — Terrain Layer Output

A paintable terrain layer, blended through the splatmap. Pins: `base_color`,
`metallic`, `roughness`, `normal`, `height` (=0.5). Compiles to the layer functions
the terrain shader blends per-pixel.

### `output/vegetation` — Vegetation Output

Surface PBR **plus** a `vertex_offset` (Vec3 = 0) pin. When `vertex_offset` is
connected, a custom vertex shader is generated that displaces the world position —
wire `animation/wind` (or any Vec3) into it for sway. All the surface pins above
also apply.

### `output/unlit` — Unlit Output

Flat, lighting-free color. Pins: `color` (Color = 1,1,1,1), `alpha` (Float = 1).
The resolver sets `unlit = true` so lighting is skipped — for UI bits, holograms,
and stylized effects.

---

## See also

- [Material API](/docs/r1-alpha5/api/material) — the `.material` file format, pin
  types, material instances, and code shaders.
- [Material Editor](/docs/r1-alpha5/editor/materials) — the visual workflow for
  wiring these nodes.
- [Custom Material Nodes](/docs/r1-alpha5/extending/material-nodes) — how to add a
  new built-in node to the engine, and how the codegen works.

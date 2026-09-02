//! The WGSL text library — helper functions the emitters call by name, plus
//! the binding declarations and import block every generated shader needs.
//!
//! Every helper here is emitted *conditionally*, keyed off a `uses_*` flag the
//! node emitters set. A graph that never touches HSV never pays for
//! `mat_rgb_to_hsv`, and — more importantly on macOS — a graph that never
//! samples a cubemap does not declare the binding.
//!
//! The one thing that is always declared is the parameter UBO at binding 118:
//! `AsBindGroup` on `SurfaceGraphExt` requires that slot to be bound whether
//! or not the graph reads a parameter, and wgpu rejects the pipeline at draw
//! time if it isn't.

use super::ctx::Ctx;

pub(crate) fn noise_helpers(ctx: &Ctx) -> String {
    let mut s = String::new();
    if ctx.uses_noise || ctx.uses_hash {
        s.push_str(
            r#"
fn mat_hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}
"#,
        );
    }
    if ctx.uses_noise {
        s.push_str(
            r#"
// Random gradient for Perlin-style noise
fn mat_hash_grad(p: vec2<f32>) -> vec2<f32> {
    let k = vec2<f32>(
        dot(p, vec2<f32>(127.1, 311.7)),
        dot(p, vec2<f32>(269.5, 183.3)),
    );
    return fract(sin(k) * 43758.5453) * 2.0 - 1.0;
}

// Gradient (Perlin) noise with C2-continuous quintic interpolation.
// Returns [0, 1]. Much less grid-aligned artifact than value noise.
fn mat_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);

    let g00 = dot(mat_hash_grad(i + vec2<f32>(0.0, 0.0)), f - vec2<f32>(0.0, 0.0));
    let g10 = dot(mat_hash_grad(i + vec2<f32>(1.0, 0.0)), f - vec2<f32>(1.0, 0.0));
    let g01 = dot(mat_hash_grad(i + vec2<f32>(0.0, 1.0)), f - vec2<f32>(0.0, 1.0));
    let g11 = dot(mat_hash_grad(i + vec2<f32>(1.0, 1.0)), f - vec2<f32>(1.0, 1.0));

    return mix(mix(g00, g10, u.x), mix(g01, g11, u.x), u.y) * 0.5 + 0.5;
}
"#,
        );
    }
    if ctx.uses_fbm {
        s.push_str(
            r#"
// FBM with inter-octave rotation — breaks grid-aligned artifacts of basic noise
fn mat_fbm(uv: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var p = uv;
    var value = 0.0;
    var amplitude = 0.5;
    let c = cos(0.77); let sn = sin(0.77);
    let r = mat2x2<f32>(c, sn, -sn, c);
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + mat_noise(p) * amplitude;
        p = r * p * lacunarity + vec2<f32>(37.1, 17.3);
        amplitude = amplitude * persistence;
    }
    return value;
}
"#,
        );
    }
    if ctx.uses_fbm_ridged {
        s.push_str(
            r#"
fn mat_fbm_ridged(uv: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var p = uv;
    var value = 0.0;
    var amplitude = 0.5;
    var total = 0.0;
    let c = cos(1.13); let sn = sin(1.13);
    let r = mat2x2<f32>(c, sn, -sn, c);
    for (var i = 0; i < octaves; i = i + 1) {
        let n = mat_noise(p);
        value = value + (1.0 - abs(n * 2.0 - 1.0)) * amplitude;
        total = total + amplitude;
        p = r * p * lacunarity + vec2<f32>(21.7, 43.9);
        amplitude = amplitude * persistence;
    }
    return value / max(total, 0.000001);
}
"#,
        );
    }
    if ctx.uses_fbm_turbulence {
        s.push_str(
            r#"
fn mat_fbm_turbulence(uv: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var p = uv;
    var value = 0.0;
    var amplitude = 0.5;
    var total = 0.0;
    let c = cos(0.63); let sn = sin(0.63);
    let r = mat2x2<f32>(c, sn, -sn, c);
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + abs(mat_noise(p) * 2.0 - 1.0) * amplitude;
        total = total + amplitude;
        p = r * p * lacunarity + vec2<f32>(53.1, 29.7);
        amplitude = amplitude * persistence;
    }
    return value / max(total, 0.000001);
}
"#,
        );
    }
    if ctx.uses_fbm_billow {
        s.push_str(
            r#"
fn mat_fbm_billow(uv: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var p = uv;
    var value = 0.0;
    var amplitude = 0.5;
    var total = 0.0;
    let c = cos(0.91); let sn = sin(0.91);
    let r = mat2x2<f32>(c, sn, -sn, c);
    for (var i = 0; i < octaves; i = i + 1) {
        let n = abs(mat_noise(p) * 2.0 - 1.0);
        value = value + n * n * amplitude;
        total = total + amplitude;
        p = r * p * lacunarity + vec2<f32>(13.7, 61.1);
        amplitude = amplitude * persistence;
    }
    return value / max(total, 0.000001);
}
"#,
        );
    }
    if ctx.uses_curl {
        s.push_str(
            r#"
fn mat_curl_noise(uv: vec2<f32>, eps: f32) -> vec2<f32> {
    let e = max(eps, 0.0001);
    let n1 = mat_noise(uv + vec2<f32>(0.0, e));
    let n2 = mat_noise(uv - vec2<f32>(0.0, e));
    let n3 = mat_noise(uv + vec2<f32>(e, 0.0));
    let n4 = mat_noise(uv - vec2<f32>(e, 0.0));
    // 2D curl: (∂n/∂y, -∂n/∂x)
    return vec2<f32>((n1 - n2) / (2.0 * e), -(n3 - n4) / (2.0 * e));
}
"#,
        );
    }
    if ctx.uses_voronoi {
        s.push_str(
            r#"
fn mat_voronoi(p: vec2<f32>) -> vec2<f32> {
    let n = floor(p);
    let f = fract(p);
    var min_dist = 8.0;
    var cell = 0.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let g = vec2<f32>(f32(i), f32(j));
            let o = vec2<f32>(mat_hash(n + g), mat_hash(n + g + vec2<f32>(57.0, 113.0)));
            let d = length(g + o - f);
            if (d < min_dist) {
                min_dist = d;
                cell = mat_hash(n + g + vec2<f32>(234.0, 567.0));
            }
        }
    }
    return vec2<f32>(min_dist, cell);
}
"#,
        );
    }
    if ctx.uses_voronoi_full {
        s.push_str(
            r#"
// Extended Voronoi — returns (F1, F2, edge_dist, cell_id).
// Edge distance uses a second pass that compares F1 neighbors as in IQ's article.
fn mat_voronoi_full(p: vec2<f32>) -> vec4<f32> {
    let n = floor(p);
    let f = fract(p);

    // Pass 1: find nearest cell F1
    var f1 = 8.0;
    var f2 = 8.0;
    var nearest = vec2<f32>(0.0);
    var cell = 0.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let g = vec2<f32>(f32(i), f32(j));
            let o = vec2<f32>(mat_hash(n + g), mat_hash(n + g + vec2<f32>(57.0, 113.0)));
            let r = g + o - f;
            let d = dot(r, r);
            if (d < f1) {
                f2 = f1;
                f1 = d;
                nearest = r;
                cell = mat_hash(n + g + vec2<f32>(234.0, 567.0));
            } else if (d < f2) {
                f2 = d;
            }
        }
    }

    // Pass 2: edge distance — minimum of dot((r_i + nearest)/2, normalize(r_i - nearest))
    var edge = 8.0;
    for (var j = -2; j <= 2; j = j + 1) {
        for (var i = -2; i <= 2; i = i + 1) {
            let g = vec2<f32>(f32(i), f32(j));
            let o = vec2<f32>(mat_hash(n + g), mat_hash(n + g + vec2<f32>(57.0, 113.0)));
            let r = g + o - f;
            let diff = r - nearest;
            if (dot(diff, diff) > 0.00001) {
                let e = dot(0.5 * (nearest + r), normalize(diff));
                if (e < edge) { edge = e; }
            }
        }
    }

    return vec4<f32>(sqrt(f1), sqrt(f2), edge, cell);
}
"#,
        );
    }
    if ctx.uses_srgb {
        s.push_str(
            r#"
fn mat_srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let cutoff = vec3<f32>(0.04045);
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= cutoff);
}

fn mat_linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let cutoff = vec3<f32>(0.0031308);
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(hi, lo, c <= cutoff);
}
"#,
        );
    }
    if ctx.uses_hsv {
        s.push_str(r#"
fn mat_rgb_to_hsv(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = select(vec4<f32>(c.bg, K.wz), vec4<f32>(c.gb, K.xy), c.g >= c.b);
    let q = select(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), c.r >= p.x);
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(
        abs(q.z + (q.w - q.y) / (6.0 * d + e)),
        d / (q.x + e),
        q.x
    );
}

fn mat_hsv_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(vec3<f32>(c.x) + K.xyz) * 6.0 - vec3<f32>(K.w));
    return c.z * mix(vec3<f32>(K.x), clamp(p - vec3<f32>(K.x), vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}
"#);
    }
    if ctx.uses_scene_depth {
        s.push_str(
            r#"
fn mat_linearize_depth(ndc_depth: f32) -> f32 {
    let near = view.clip_from_view[3][2];
    let far_factor = view.clip_from_view[2][2];
    return near / (far_factor + ndc_depth);
}
"#,
        );
    }
    if ctx.uses_hex_tile {
        // Hex anti-tiling (Heitz & Neyret 2018 / "Hex Tiling" by Jasper Flick).
        // Decomposes UV space into hex cells, rotates UV inside each cell by a
        // pseudo-random angle keyed to the cell's integer position, and returns
        // three overlapping samples with barycentric weights for the triangle
        // formed by the three nearest hex centers. A consumer samples its
        // texture three times at uv_a/uv_b/uv_c and combines by w.x/w.y/w.z.
        s.push_str(r#"
struct HexTile {
    uv_a: vec2<f32>,
    uv_b: vec2<f32>,
    uv_c: vec2<f32>,
    w: vec3<f32>,
};

fn mat_hex_cell_uv(cell: vec2<f32>, local: vec2<f32>, variation: f32) -> vec2<f32> {
    let ang = mat_hash(cell) * 6.2831853 * variation;
    let cs = vec2<f32>(cos(ang), sin(ang));
    let off = vec2<f32>(mat_hash(cell + vec2<f32>(17.0, 83.0)), mat_hash(cell + vec2<f32>(47.0, 29.0)));
    let r = vec2<f32>(local.x * cs.x - local.y * cs.y, local.x * cs.y + local.y * cs.x);
    return r + off;
}

fn mat_hex_tile(uv: vec2<f32>, variation: f32) -> HexTile {
    // Skew UV into hex-grid axes (flat-topped hex basis).
    let skew = mat2x2<f32>(1.0, 0.0, 0.5, 0.8660254);
    let inv_skew = mat2x2<f32>(1.0, 0.0, -0.5773503, 1.1547005);
    let hex_uv = inv_skew * uv;
    let base = floor(hex_uv);
    let f = fract(hex_uv);

    // Three corners of the unit quad whose barycentric triangle our sample falls into.
    var c1: vec2<f32>;
    var c2: vec2<f32>;
    var c3: vec2<f32>;
    var w: vec3<f32>;
    if (f.x + f.y < 1.0) {
        c1 = base + vec2<f32>(0.0, 0.0);
        c2 = base + vec2<f32>(1.0, 0.0);
        c3 = base + vec2<f32>(0.0, 1.0);
        w = vec3<f32>(1.0 - f.x - f.y, f.x, f.y);
    } else {
        c1 = base + vec2<f32>(1.0, 1.0);
        c2 = base + vec2<f32>(0.0, 1.0);
        c3 = base + vec2<f32>(1.0, 0.0);
        w = vec3<f32>(f.x + f.y - 1.0, 1.0 - f.x, 1.0 - f.y);
    }

    // Local offset of the input point from each hex center, back in world UV space.
    let p = uv;
    let p1 = p - skew * c1;
    let p2 = p - skew * c2;
    let p3 = p - skew * c3;

    var out: HexTile;
    out.uv_a = mat_hex_cell_uv(c1, p1, variation);
    out.uv_b = mat_hex_cell_uv(c2, p2, variation);
    out.uv_c = mat_hex_cell_uv(c3, p3, variation);
    // Gain-corrected weights preserve variance after blending three rotated samples.
    let w2 = w * w;
    let s = w2.x + w2.y + w2.z;
    out.w = w2 / max(s, 0.00001);
    return out;
}
"#);
    }
    if ctx.uses_blend {
        s.push_str(
            r#"
fn mat_blend(base: vec4<f32>, blnd: vec4<f32>, opacity: f32, mode: i32) -> vec4<f32> {
    let b = base.rgb;
    let t = blnd.rgb;
    var r: vec3<f32>;
    switch mode {
        case 1: { r = b * t; }                                              // multiply
        case 2: { r = vec3<f32>(1.0) - (vec3<f32>(1.0) - b) * (vec3<f32>(1.0) - t); } // screen
        case 3: {                                                            // overlay
            let lt = 2.0 * b * t;
            let gt = vec3<f32>(1.0) - 2.0 * (vec3<f32>(1.0) - b) * (vec3<f32>(1.0) - t);
            r = select(gt, lt, b < vec3<f32>(0.5));
        }
        case 4: { r = b + t; }                                              // add
        case 5: { r = b - t; }                                              // subtract
        case 6: {                                                            // soft-light
            r = (vec3<f32>(1.0) - 2.0 * t) * b * b + 2.0 * t * b;
        }
        case 7: {                                                            // hard-light
            let lt = 2.0 * b * t;
            let gt = vec3<f32>(1.0) - 2.0 * (vec3<f32>(1.0) - b) * (vec3<f32>(1.0) - t);
            r = select(gt, lt, t < vec3<f32>(0.5));
        }
        case 8: { r = abs(b - t); }                                         // difference
        case 9: { r = b / max(t, vec3<f32>(0.000001)); }                    // divide
        default: { r = t; }                                                 // normal
    }
    return vec4<f32>(mix(b, r, opacity), base.a);
}
"#,
        );
    }
    s
}

pub(crate) fn emit_module_prelude(ctx: &Ctx, s: &mut String) {
    for chunk in &ctx.module_prelude {
        s.push_str(chunk);
        // Chunks carry no trailing newline of their own; without one the
        // next chunk's `fn` glues onto this chunk's closing brace.
        if !chunk.ends_with('\n') {
            s.push('\n');
        }
    }
}

/// Steep-parallax march over `graph_displacement`, plus one interpolation step
/// (parallax *occlusion* mapping — Bevy's default method for `depth_map`).
///
/// Deliberately not Bevy's `parallaxed_uv`: that one reads `depth_map_texture`
/// at a fixed StandardMaterial binding, and our height comes out of the graph.
/// The march is the same algorithm against a different source.
///
/// Height convention is the one every PBR texture set ships — white is the
/// peak — so it is inverted into a depth here. Bevy's `depth_map` is the other
/// way round, which is exactly why a displacement graph never takes the
/// StandardMaterial fast path: one convention, whatever the graph looks like.
pub(crate) fn parallax_helper_wgsl(depth_scale: f32) -> String {
    format!(
        r#"
const GRAPH_PARALLAX_SCALE: f32 = {depth_scale:.6};
const GRAPH_PARALLAX_MAX_LAYERS: f32 = 16.0;

fn graph_parallax_uv(in: VertexOutput, original_uv: vec2<f32>, Vt: vec3<f32>) -> vec2<f32> {{
    // Shallow view angles need more layers; head-on needs one. `max` keeps a
    // surface exactly edge-on from dividing by zero.
    let view_steepness = max(abs(Vt.z), 0.0001);
    let layer_count = mix(GRAPH_PARALLAX_MAX_LAYERS, 1.0, view_steepness);
    let layer_depth = 1.0 / layer_count;
    let delta_uv = GRAPH_PARALLAX_SCALE * layer_depth * Vt.xy * vec2<f32>(1.0, -1.0) / view_steepness;

    var uv = original_uv;
    var current_layer_depth = 0.0;
    var texture_depth = 1.0 - graph_displacement(in, uv);
    for (var i = 0; texture_depth > current_layer_depth && i <= i32(layer_count); i++) {{
        current_layer_depth += layer_depth;
        uv += delta_uv;
        texture_depth = 1.0 - graph_displacement(in, uv);
    }}

    // Interpolate across the layer the ray crossed, so the relief doesn't
    // read as a staircase at the sampling resolution.
    let previous_uv = uv - delta_uv;
    let next_depth = texture_depth - current_layer_depth;
    let previous_depth = (1.0 - graph_displacement(in, previous_uv)) - current_layer_depth + layer_depth;
    let denom = next_depth - previous_depth;
    let weight = select(0.0, next_depth / denom, abs(denom) > 0.0001);
    return mix(uv, previous_uv, weight);
}}
"#
    )
}

/// The parallax march, emitted into `fragment` right after the aliases so every
/// later texture read picks up the offset UV.
///
/// Gated on `VERTEX_TANGENTS` because the view ray has to be expressed in
/// tangent space and there is no tangent frame without them — the same gate
/// Bevy puts on its own parallax block. A mesh without tangents keeps the raw
/// UV rather than rendering something wrong.
pub(crate) const PARALLAX_FRAGMENT_WGSL: &str = r#"#ifdef VERTEX_TANGENTS
    {
        let pom_tbn = pbr_functions::calculate_tbn_mikktspace(in.world_normal, in.world_tangent);
        let pom_vt = vec3<f32>(dot(pbr_input.V, pom_tbn[0]), dot(pbr_input.V, pom_tbn[1]), dot(pbr_input.V, pom_tbn[2]));
        // Flipped to point into the surface, which is the direction the march walks.
        mat_uv = graph_parallax_uv(in, mat_uv, -pom_vt);
    }
#endif
"#;

/// WGSL snippet that aliases mesh-conditional VertexOutput fields. Generated
/// graph code references `mat_uv` / `mat_vertex_color` instead of `in.uv` /
/// `in.color` so a mesh without those attributes still compiles — the
/// `#ifdef` falls back to a sane default (zeroed UV, white vertex color).
pub(crate) fn fragment_input_aliases() -> String {
    // `var`, not `let`: the parallax block reassigns `mat_uv` in place so that
    // every downstream sampler picks up the offset without knowing about it.
    r#"#ifdef VERTEX_UVS_A
    var mat_uv = in.uv;
#else
    var mat_uv = vec2<f32>(0.0, 0.0);
#endif
#ifdef VERTEX_COLORS
    let mat_vertex_color = in.color;
#else
    let mat_vertex_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
"#
    .to_string()
}

pub(crate) fn texture_bindings_wgsl(ctx: &Ctx) -> String {
    // Extension-material texture slots live at bindings 100..113 in group 3,
    // alongside StandardMaterial's own bindings (which occupy 0..~30). The
    // extension's AsBindGroup (see `SurfaceGraphExt`) declares the same
    // offsets, so the shader and the CPU-side bind group match.
    //
    // Bevy 0.18 merges base-material + extension bindings into a single bind
    // group 3 (`MATERIAL_BIND_GROUP_INDEX`), filtering duplicates. As long as
    // our bindings don't collide with StandardMaterial's, they coexist fine.
    let mut s = String::new();
    // One sampler (binding 101) is shared by every slot — 2D, cube, array and
    // 3D alike. Metal allows only 16 sampler states per stage, and the mesh
    // view + StandardMaterial bindings already use 14; per-slot samplers made
    // the pipeline unbuildable on macOS. The CPU side mirrors this: only
    // `texture_0` carries a `#[sampler]` attribute in `SurfaceGraphExt`.
    s.push_str("@group(3) @binding(101) var texture_sampler: sampler;\n");
    // Slots 0..3 live on bindings 100..106 (even). Slots 4..5 live on 114 and
    // 116 — the cubemap/array/3D slots sit between them at 108-112, so we
    // can't keep the linear `100 + slot*2` formula past slot 3.
    const D2_BINDINGS: [u32; 6] = [100, 102, 104, 106, 114, 116];
    for (slot, tex_binding) in D2_BINDINGS.iter().enumerate() {
        s.push_str(&format!(
            "@group(3) @binding({tex_binding}) var texture_{slot}: texture_2d<f32>;\n",
        ));
    }
    // Cubemap / 2D-array / 3D bindings are only declared when the graph
    // actually samples them — their @binding slots exist on the layout either
    // way (Bevy's fallback image handles that) but emitting unused `var`s
    // would add harmless-but-noisy lines to every shader.
    if ctx.uses_cube_0 {
        s.push_str("@group(3) @binding(108) var cube_0: texture_cube<f32>;\n");
    }
    if ctx.uses_array_0 {
        s.push_str("@group(3) @binding(110) var array_0: texture_2d_array<f32>;\n");
    }
    if ctx.uses_volume_0 {
        s.push_str("@group(3) @binding(112) var volume_0: texture_3d<f32>;\n");
    }
    // Always declare the parameter UBO at @binding(118) — the
    // `AsBindGroup` derive on `SurfaceGraphExt` requires this slot to be
    // bound regardless of whether the graph reads any params, otherwise
    // wgpu rejects the pipeline at draw time. Keep the slot count in sync
    // with `surface_ext::SURFACE_GRAPH_PARAM_SLOTS`.
    s.push_str("struct SurfaceGraphParams { slots: array<vec4<f32>, 32>, }\n");
    s.push_str("@group(3) @binding(118) var<uniform> material_params: SurfaceGraphParams;\n");
    s
}

/// Emit the common import block shared by the Surface and Unlit shader
/// templates. Kept as a helper so both code paths stay in sync when we add /
/// remove imports based on which nodes the graph uses.
pub(crate) fn emit_ext_shader_header(ctx: &Ctx, shader: &mut String) {
    // The extension-hook pattern: import StandardMaterial's PbrInput builder +
    // the full lighting pipeline. This is the seam documented in the Bevy PBR
    // source (`extended_material.rs` + example usage).
    shader.push_str("#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material\n");
    shader.push_str("#import bevy_pbr::pbr_functions\n");
    shader.push_str("#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}\n");
    shader.push_str("#import bevy_pbr::mesh_view_bindings::{view, globals}\n");

    shader.push_str("#import bevy_pbr::mesh_functions\n");
    if ctx.uses_scene_depth || ctx.uses_scene_normal || ctx.uses_motion_vector {
        shader.push_str("#import bevy_pbr::prepass_utils\n");
    }
    if ctx.uses_transmission {
        shader.push_str("#import bevy_pbr::mesh_view_bindings::{view_transmission_texture, view_transmission_sampler}\n");
    }
    if ctx.uses_env_map {
        shader.push_str("#ifdef ENVIRONMENT_MAP\n");
        shader.push_str("#ifdef MULTIPLE_LIGHT_PROBES_IN_ARRAY\n");
        shader.push_str("#import bevy_pbr::mesh_view_bindings::{specular_environment_maps, environment_map_sampler}\n");
        shader.push_str("#else\n");
        shader.push_str("#import bevy_pbr::mesh_view_bindings::{specular_environment_map, environment_map_sampler}\n");
        shader.push_str("#endif\n");
        shader.push_str("#endif\n");
    }
    shader.push('\n');
}

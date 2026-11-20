// Terrain surface: up to four textured layers placed by slope and height.
//
// Layer 0 covers everything. Each later layer is painted over what is below it
// wherever the surface's slope and height fall inside that layer's bands, with
// a soft edge on each band and noise that breaks the edge up so it never reads
// as a contour line. Where two layers meet, their height maps decide which one
// shows: grass settles into the cracks of a rock face instead of fading
// through it. See `renzora_terrain::surface` for the rules' meaning.
//
// Every texture read is `textureSampleGrad`, with the gradients taken once at
// the top of the shader: a layer is only sampled where it has weight, and an
// implicit-derivative sample inside that branch would pick its mip from
// neighbours that took the other branch.

import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;
import bevy_pbr::render::pbr_functions;
@if(PREPASS_PIPELINE) import bevy_pbr::prepass::io::FragmentOutput;
@if(PREPASS_PIPELINE && !MESHLET_MESH_MATERIAL_PASS) import bevy_pbr::prepass::io::VertexOutput;
@if(PREPASS_PIPELINE) import bevy_pbr::deferred::functions::deferred_output;
@if(!PREPASS_PIPELINE) import bevy_pbr::render::forward_io::FragmentOutput;
@if(!PREPASS_PIPELINE && !MESHLET_MESH_MATERIAL_PASS) import bevy_pbr::render::forward_io::VertexOutput;
// Drawn as virtual geometry (see `renzora_terrain::clusters`), the shader runs
// once per covered pixel from a fullscreen pass and rebuilds what the vertex
// stage would have passed from the visibility buffer, as Bevy's own PBR
// fragment does.
@if(MESHLET_MESH_MATERIAL_PASS) import bevy_pbr::meshlet::visibility_buffer_resolve::{VertexOutput, resolve_vertex_output};
import bevy_pbr::render::mesh_view_bindings::view;

struct SurfaceLayer {
    // rgb = tint (or the colour itself without textures), a = 1 / tile size in metres
    tint_scale: vec4<f32>,
    // x = from, y = to, z = blend, all in degrees
    slope: vec4<f32>,
    // x = from, y = to, z = blend in metres, w = edge breakup 0..1
    height: vec4<f32>,
    // x = enabled, y = triplanar, z = has textures, w = unused
    flags: vec4<f32>,
};

struct SurfaceParams {
    layers: array<SurfaceLayer, 4>,
    // x = layer count, y = macro variation strength, z = macro variation size
    // in metres, w = distance at which the far tiling fully takes over
    settings: vec4<f32>,
    // x = height blending 0..1, y = normal strength, z = anti-tiling on (1)
    // or off (0), w = unused
    look: vec4<f32>,
};

@group(3) @binding(100) var<uniform> surface: SurfaceParams;
@group(3) @binding(101) var albedo_array: texture_2d_array<f32>;
@group(3) @binding(102) var surface_sampler: sampler;
@group(3) @binding(103) var normal_array: texture_2d_array<f32>;
// R = ambient occlusion, G = roughness, B = height. Terrain is never metal, so
// the build packs each layer's height map into the channel a metallic map
// would use, and the shader treats metallic as zero.
@group(3) @binding(104) var arm_array: texture_2d_array<f32>;

// ── Noise ──────────────────────────────────────────────────────────────────

fn hash2(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    var q = fract(vec3<f32>(p.x, p.y, p.x) * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.xx + q.yz) * q.zy);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        v += amp * value_noise(q);
        q = q * 2.03 + vec2<f32>(17.0, 31.0);
        amp *= 0.5;
    }
    return v / 0.9375;
}

/// 1 inside `lo..hi`, 0 outside, with a `blend`-wide ramp on each edge.
fn band(x: f32, lo: f32, hi: f32, blend: f32) -> f32 {
    let b = max(blend, 0.0001);
    return smoothstep(lo - b, lo + b, x) * (1.0 - smoothstep(hi - b, hi + b, x));
}

// ── Sampling ───────────────────────────────────────────────────────────────

struct Texel {
    albedo: vec3<f32>,
    normal: vec3<f32>,
    arm: vec3<f32>,
};

/// What every sample needs besides its own coordinates: the world-space
/// position gradients (see the note at the top), how strongly normal maps
/// tilt the surface, and whether to break up the tiling.
struct Ctx {
    dx: vec3<f32>,
    dy: vec3<f32>,
    normal_gain: f32,
    hex: bool,
};

fn empty_texel() -> Texel {
    var t: Texel;
    t.albedo = vec3<f32>(0.0);
    t.normal = vec3<f32>(0.0, 0.0, 1.0);
    t.arm = vec3<f32>(0.0);
    return t;
}

/// One tap of one layer. `uv` and its gradients are already scaled.
/// Returns the tangent-space normal unpacked, for the caller to orient.
/// Not named `read`: that is the access mode in `var<storage, read>`, and
/// the virtual geometry pass imports bindings that use it, where a function
/// of that name made the whole pipeline fail and the terrain drew unshaded.
fn sample_maps(layer: u32, uv: vec2<f32>, gx: vec2<f32>, gy: vec2<f32>, gain: f32) -> Texel {
    var t: Texel;
    t.albedo = textureSampleGrad(albedo_array, surface_sampler, uv, layer, gx, gy).rgb;
    let n = textureSampleGrad(normal_array, surface_sampler, uv, layer, gx, gy).xyz * 2.0 - 1.0;
    t.normal = vec3<f32>(n.xy * gain, n.z);
    t.arm = textureSampleGrad(arm_array, surface_sampler, uv, layer, gx, gy).rgb;
    return t;
}

/// The triangle of hex centres a point falls between, and its barycentric
/// weights. The texture is laid down once per hex cell, each copy turned and
/// shifted at random, and a pixel blends the three copies around it: no two
/// neighbouring cells show the same patch, so the repeat has nothing regular
/// left to show. After Mikkelsen, "Practical Real-Time Hex-Tiling" (2022).
struct HexCell {
    w: vec3<f32>,
    v1: vec2<f32>,
    v2: vec2<f32>,
    v3: vec2<f32>,
};

fn hex_cell(uv: vec2<f32>) -> HexCell {
    let s = uv * 3.4641016;
    let skewed = vec2<f32>(s.x, -0.57735027 * s.x + 1.15470054 * s.y);
    let corner = floor(skewed);
    let f = fract(skewed);
    let z = 1.0 - f.x - f.y;
    let upper = step(0.0, -z);
    let flip = 2.0 * upper - 1.0;
    var h: HexCell;
    h.w = vec3<f32>(-z * flip, upper - f.y * flip, upper - f.x * flip);
    h.v1 = corner + vec2<f32>(upper, upper);
    h.v2 = corner + vec2<f32>(upper, 1.0 - upper);
    h.v3 = corner + vec2<f32>(1.0 - upper, upper);
    return h;
}

/// One hex cell's copy of the texture: turned by up to `spin` of a full turn
/// and shifted by a random fraction of a tile. The normal is turned back, or
/// the light would come from a different side in every cell.
fn sample_cell(layer: u32, uv: vec2<f32>, gx: vec2<f32>, gy: vec2<f32>, cell: vec2<f32>, spin: f32, gain: f32) -> Texel {
    let r = hash22(cell);
    let angle = (r.x - 0.5) * 6.2831853 * spin;
    let c = cos(angle);
    let s = sin(angle);
    let turn = mat2x2<f32>(c, s, -s, c);
    let shift = hash22(cell + vec2<f32>(17.31, 5.17));
    var t = sample_maps(layer, turn * uv + shift, turn * gx, turn * gy, gain);
    // The normal's tilt in image space is (x, -y): green points up the image,
    // against v. Turned back by the transpose, then repacked.
    let tilt = transpose(turn) * vec2<f32>(t.normal.x, -t.normal.y);
    t.normal = vec3<f32>(tilt.x, -tilt.y, t.normal.z);
    return t;
}

/// One planar projection, hex-tiled when anti-tiling is on. The three copies
/// are weighted by their own height (the ARM blue channel), sharply: where
/// copies meet, the taller pebble or tuft wins, so the seams follow the
/// texture's own shapes and most of the ground shows one copy at full
/// contrast rather than a washed-out average of three.
fn sample_planar(layer: u32, uv: vec2<f32>, gx: vec2<f32>, gy: vec2<f32>, spin: f32, ctx: Ctx) -> Texel {
    if !ctx.hex {
        return sample_maps(layer, uv, gx, gy, ctx.normal_gain);
    }
    let h = hex_cell(uv * 0.5);
    let a = sample_cell(layer, uv, gx, gy, h.v1, spin, ctx.normal_gain);
    let b = sample_cell(layer, uv, gx, gy, h.v2, spin, ctx.normal_gain);
    let c = sample_cell(layer, uv, gx, gy, h.v3, spin, ctx.normal_gain);
    var w = h.w * (vec3<f32>(a.arm.b, b.arm.b, c.arm.b) + 0.35);
    w = pow(max(w, vec3<f32>(0.0)), vec3<f32>(6.0));
    w = w / max(w.x + w.y + w.z, 1e-6);
    var t: Texel;
    t.albedo = a.albedo * w.x + b.albedo * w.y + c.albedo * w.z;
    t.normal = a.normal * w.x + b.normal * w.y + c.normal * w.z;
    t.arm = a.arm * w.x + b.arm * w.y + c.arm * w.z;
    return t;
}

/// Whiteout blend of a tangent normal from a top-down projection onto the
/// surface normal. OpenGL normals: green points up the image, which is toward
/// -z with the image's v running along +z.
fn orient_top(t: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    return normalize(vec3<f32>(t.x + n.x, abs(t.z) * n.y, -t.y + n.z));
}

/// Projected from above, at two scales: the far one (a quarter the frequency,
/// rotated) fades in with distance so the repeat stops showing and distant
/// ground keeps detail of its own size, and the near one keeps full detail
/// underfoot.
fn sample_top(layer: u32, scale: f32, p: vec3<f32>, ctx: Ctx, n: vec3<f32>, far: f32) -> Texel {
    let uv = p.xz * scale;
    var t = sample_planar(layer, uv, ctx.dx.xz * scale, ctx.dy.xz * scale, 1.0, ctx);
    // Only without anti-tiling: hex tiling already breaks up the repeat the far
    // copy was for, and a second hex-tiled sample nested here (eighteen reads
    // of a layer in one branch) drew the whole terrain black on an RTX 3060,
    // with no error anywhere. It halves the cost of a textured layer too.
    if far > 0.001 && !ctx.hex {
        let rot = mat2x2<f32>(0.8, -0.6, 0.6, 0.8);
        let s2 = scale * 0.27;
        let f = sample_planar(
            layer,
            rot * (p.xz * s2) + vec2<f32>(0.37, 0.71),
            rot * (ctx.dx.xz * s2),
            rot * (ctx.dy.xz * s2),
            1.0,
            ctx,
        );
        // The far copy is turned by `rot`; turn its normal back as a cell's is.
        let tilt = transpose(rot) * vec2<f32>(f.normal.x, -f.normal.y);
        let far_normal = vec3<f32>(tilt.x, -tilt.y, f.normal.z);
        t.albedo = mix(t.albedo, f.albedo, far);
        t.arm = mix(t.arm, f.arm, far);
        t.normal = mix(t.normal, far_normal, far);
    }
    t.normal = orient_top(t.normal, n);
    return t;
}

/// Projected along all three axes and blended by the normal, so a cliff face
/// gets texture at its own scale instead of a smear stretched from above. The
/// side projections are shifted per hex cell but never turned, so strata in a
/// rock texture stay level as they do in a real cliff.
fn sample_triplanar(layer: u32, scale: f32, p: vec3<f32>, ctx: Ctx, n: vec3<f32>) -> Texel {
    var w = pow(abs(n), vec3<f32>(4.0));
    w = w / (w.x + w.y + w.z);
    // An axis with almost no weight is skipped (the rest are renormalized):
    // on a hillside one or two projections carry the whole pixel, and each
    // costs three hex taps of three maps.
    w = select(w, vec3<f32>(0.0), w < vec3<f32>(0.02));
    w = w / max(w.x + w.y + w.z, 1e-5);
    var tx = empty_texel();
    var ty = empty_texel();
    var tz = empty_texel();
    // v runs down the image, so the side projections take -y to stand the
    // texture upright.
    if w.x > 0.0 {
        tx = sample_planar(layer, vec2<f32>(p.z, -p.y) * scale, vec2<f32>(ctx.dx.z, -ctx.dx.y) * scale, vec2<f32>(ctx.dy.z, -ctx.dy.y) * scale, 0.0, ctx);
    }
    if w.y > 0.0 {
        ty = sample_planar(layer, p.xz * scale, ctx.dx.xz * scale, ctx.dy.xz * scale, 1.0, ctx);
    }
    if w.z > 0.0 {
        tz = sample_planar(layer, vec2<f32>(p.x, -p.y) * scale, vec2<f32>(ctx.dx.x, -ctx.dx.y) * scale, vec2<f32>(ctx.dy.x, -ctx.dy.y) * scale, 0.0, ctx);
    }
    var t: Texel;
    t.albedo = tx.albedo * w.x + ty.albedo * w.y + tz.albedo * w.z;
    t.arm = tx.arm * w.x + ty.arm * w.y + tz.arm * w.z;
    let nx = vec3<f32>(abs(tx.normal.z) * n.x, tx.normal.y + n.y, tx.normal.x + n.z);
    let ny = vec3<f32>(ty.normal.x + n.x, abs(ty.normal.z) * n.y, -ty.normal.y + n.z);
    let nz = vec3<f32>(tz.normal.x + n.x, tz.normal.y + n.y, abs(tz.normal.z) * n.z);
    t.normal = normalize(nx * w.x + ny * w.y + nz * w.z);
    return t;
}

fn sample_layer(i: u32, p: vec3<f32>, ctx: Ctx, n: vec3<f32>, far: f32) -> Texel {
    let l = surface.layers[i];
    var t: Texel;
    if l.flags.z < 0.5 {
        // No textures (not shipped, or not loaded yet): the colour alone,
        // with a little noise so a flat colour still reads as ground, and a
        // flat height so height blending has nothing to favour.
        let v = 0.85 + 0.3 * value_noise(p.xz * 0.7);
        t.albedo = l.tint_scale.rgb * v;
        t.normal = n;
        t.arm = vec3<f32>(1.0, 0.9, 0.5);
        return t;
    }
    let scale = l.tint_scale.a;
    if l.flags.y > 0.5 {
        t = sample_triplanar(i, scale, p, ctx, n);
    } else {
        t = sample_top(i, scale, p, ctx, n, far);
    }
    t.albedo *= l.tint_scale.rgb;
    return t;
}

/// How much of a layer with band weight `w` and height `above` shows over
/// ground of height `below`. `c` is how much the heights count, `d` how soft
/// the resulting edge is. At `w` 0 the layer never shows and at 1 it always
/// does; in between, the taller of the two wins, so the edge between grass
/// and rock follows the rock's cracks rather than fading evenly.
fn height_mix(below: f32, above: f32, w: f32, c: f32, d: f32) -> f32 {
    let a = c * below + (1.0 - w);
    let b = c * above + w;
    let m = max(a, b) - d;
    let ka = max(a - m, 0.0);
    let kb = max(b - m, 0.0);
    return kb / max(ka + kb, 1e-5);
}

// ── Fragment ───────────────────────────────────────────────────────────────

/// `n` turned toward the eye just enough to face it. A smoothed, normal-mapped
/// normal can point away from the camera on a small step seen edge-on, though
/// the triangle under it faces the camera. The rasterizer clamps that case and
/// the pixel just reads dim; a ray tracer shading from the G-buffer gives such
/// a surface no light at all, so eroded slopes were streaked with black lines
/// under Solari wherever they were seen at a glancing angle.
fn face_eye(n: vec3<f32>, to_eye: vec3<f32>) -> vec3<f32> {
    let facing = dot(n, to_eye);
    let wanted = 0.05;
    if facing >= wanted {
        return n;
    }
    return normalize(n + to_eye * (wanted - facing));
}

@fragment
fn fragment(
    @if(MESHLET_MESH_MATERIAL_PASS) @builtin(position) frag_coord: vec4<f32>,
    @if(!MESHLET_MESH_MATERIAL_PASS) vertex_output: VertexOutput,
    @if(!MESHLET_MESH_MATERIAL_PASS) @builtin(front_facing) front: bool,
) -> FragmentOutput {
    @if(MESHLET_MESH_MATERIAL_PASS) let in = resolve_vertex_output(frag_coord);
    @if(MESHLET_MESH_MATERIAL_PASS) let is_front = true;
    @if(!MESHLET_MESH_MATERIAL_PASS) let in = vertex_output;
    @if(!MESHLET_MESH_MATERIAL_PASS) let is_front = front;

    // A level of detail fading in or out across a band of distance drops
    // pixels in a dither pattern, so neighbouring levels cross-fade instead of
    // popping. Bevy's own fragment does this; this one replaces it.
    @if(VISIBILITY_RANGE_DITHER) pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);

    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let p = in.world_position.xyz;
    let n = normalize(in.world_normal);
    var ctx: Ctx;
    ctx.dx = dpdx(p);
    ctx.dy = dpdy(p);
    ctx.normal_gain = max(surface.look.y, 0.0);
    ctx.hex = surface.look.z > 0.5;

    let slope = degrees(acos(clamp(n.y, -1.0, 1.0)));
    let breakup = fbm(p.xz * 0.08) - 0.5;
    let distance_to_eye = distance(view.world_position, p);
    let far_at = max(surface.settings.w, 1.0);
    let far = smoothstep(far_at * 0.25, far_at, distance_to_eye) * 0.75;

    // Height blending: `c` and `d` of `height_mix`. Full blending leaves a
    // crisp, height-shaped edge; none is an even cross-fade across the band.
    let hb = clamp(surface.look.x, 0.0, 1.0);
    let hc = 0.9 * hb;
    let hd = 0.08 + 0.4 * (1.0 - hb);
    // Below this band weight `height_mix` is zero whatever the heights, so the
    // layer is not sampled at all.
    let unseen = (1.0 - hc - hd) * 0.5;

    var albedo = vec3<f32>(0.5);
    var normal = n;
    var arm = vec3<f32>(1.0, 0.9, 0.5);
    // The height of whatever is on top so far, and how much of the base layer
    // still shows (the vegetation the macro colour drift applies to).
    var top_height = 0.5;
    var base_left = 1.0;
    let count = min(u32(surface.settings.x), 4u);
    for (var i = 0u; i < count; i++) {
        let l = surface.layers[i];
        if l.flags.x < 0.5 {
            continue;
        }
        if i == 0u {
            let t = sample_layer(i, p, ctx, n, far);
            albedo = t.albedo;
            normal = t.normal;
            arm = t.arm;
            top_height = t.arm.b;
            continue;
        }
        // The breakup shifts where each edge falls, by up to a blend width
        // either side, so neighbouring pixels cross it at different places.
        let jitter = breakup * l.height.w * 2.0;
        let s = slope + jitter * max(l.slope.z, 1.0);
        let h = p.y + jitter * max(l.height.z, 0.5);
        let w = band(s, l.slope.x, l.slope.y, l.slope.z) * band(h, l.height.x, l.height.y, l.height.z);
        if w > unseen {
            let t = sample_layer(i, p, ctx, n, far);
            let k = height_mix(top_height, t.arm.b, w, hc, hd);
            albedo = mix(albedo, t.albedo, k);
            normal = normalize(mix(normal, t.normal, k));
            arm = mix(arm, t.arm, k);
            top_height = mix(top_height, t.arm.b, k);
            base_left *= 1.0 - k;
        }
    }

    // Large, slow variation in brightness, plus drier and lusher patches on
    // the base layer: what keeps a hillside from reading as one repeated
    // swatch, and distant hills from being one flat colour.
    let mv = surface.settings.y;
    if mv > 0.0 {
        let size = max(surface.settings.z, 1.0);
        let m1 = fbm(p.xz / size) - 0.5;
        let m2 = fbm(p.xz / (size * 0.29) + vec2<f32>(41.0, 7.0)) - 0.5;
        albedo *= max(1.0 + (m1 * 1.4 + m2 * 0.7) * mv, 0.2);
        let dry = smoothstep(-0.18, 0.18, m1 + 0.6 * m2);
        let drift = mix(vec3<f32>(0.84, 1.0, 0.86), vec3<f32>(1.18, 1.06, 0.76), dry);
        albedo *= mix(vec3<f32>(1.0), drift, clamp(mv * 2.0, 0.0, 1.0) * base_left);
    }

    // Distant ground samples small mips, which average every texture toward
    // its grey mean. A little saturation back with distance keeps far hills
    // reading as grass and rock rather than haze.
    let fade = smoothstep(far_at * 0.15, far_at * 1.5, distance_to_eye);
    let luma = dot(albedo, vec3<f32>(0.2126, 0.7152, 0.0722));
    albedo = max(mix(vec3<f32>(luma), albedo, 1.0 + 0.3 * fade), vec3<f32>(0.0));

    pbr_input.material.base_color = vec4<f32>(clamp(albedo, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    pbr_input.material.perceptual_roughness = clamp(arm.g, 0.05, 1.0);
    pbr_input.material.metallic = 0.0;
    pbr_input.diffuse_occlusion = pbr_input.diffuse_occlusion * arm.r;
    pbr_input.N = face_eye(normal, normalize(view.world_position - p));

    @if(PREPASS_PIPELINE) let out = deferred_output(in, pbr_input);
    @if(!PREPASS_PIPELINE) var out: FragmentOutput;
    @if(!PREPASS_PIPELINE) out.color = pbr_functions::apply_pbr_lighting(pbr_input);
    @if(!PREPASS_PIPELINE) out.color = pbr_functions::main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}

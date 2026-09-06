// Viewport debug visualization material.
//
// Mode values:
//   0 = Normals  (world-space normal mapped to color)
//   1 = Roughness (sample MR texture green channel, fall back to scalar)
//   2 = Metallic (sample MR texture blue channel, fall back to scalar)
//   3 = Depth (distance-based grayscale)
//   4 = UV Checker (procedural)
//   5 = Flat Clay (textures-off: neutral gray with hemisphere shading)
//   6 = Matcap (sculpting view: view-space lighting + screen-space cavity)

#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::{position_world_to_clip, direction_world_to_view}
#import bevy_pbr::mesh_view_bindings as view_bindings

// Self-contained vertex output. We deliberately do NOT use
// `bevy_pbr::forward_io::VertexOutput`: its `uv`/`world_normal` fields are gated
// behind the `VERTEX_UVS_A` / `VERTEX_NORMALS` shader-defs, which this material's
// pipeline doesn't set — so `out.uv = uv` failed to compile ("invalid accessor").
// Declaring our own struct guarantees the fields exist regardless of defs.
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct DebugParams {
    // x = mode, y = scalar_roughness, z = scalar_metallic, w = has_mr_texture (0/1)
    config: vec4<f32>,
    // x = depth_near, y = depth_far, z = checker_scale, w = unused
    extra: vec4<f32>,
};

@group(3) @binding(0) var<uniform> params: DebugParams;
@group(3) @binding(1) var mr_texture: texture_2d<f32>;
@group(3) @binding(2) var mr_sampler: sampler;

// `normal` / `uv` are gated on the standard Bevy mesh shader defs
// (`VERTEX_NORMALS` / `VERTEX_UVS_A`), which the mesh pipeline sets per the
// mesh's actual attributes and uses to build the matching vertex layout. A mesh
// without those attributes still gets a valid pipeline; the missing fields fall
// back to sensible constants below so every visualization mode keeps working.
@vertex
fn vertex(
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
#ifdef VERTEX_NORMALS
    @location(1) normal: vec3<f32>,
#endif
#ifdef VERTEX_UVS_A
    @location(2) uv: vec2<f32>,
#endif
) -> VertexOutput {
    var out: VertexOutput;
    let model = mesh_functions::get_world_from_local(instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(normal, instance_index);
#else
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
#endif
#ifdef VERTEX_UVS_A
    out.uv = uv;
#else
    out.uv = vec2<f32>(0.0);
#endif
    return out;
}

fn mode_normals(n: vec3<f32>) -> vec3<f32> {
    return normalize(n) * 0.5 + vec3<f32>(0.5);
}

fn mode_roughness(uv: vec2<f32>) -> vec3<f32> {
    let has_tex = params.config.w > 0.5;
    var r = params.config.y;
    if (has_tex) {
        r = textureSample(mr_texture, mr_sampler, uv).g;
    }
    return vec3<f32>(r, r, r);
}

fn mode_metallic(uv: vec2<f32>) -> vec3<f32> {
    let has_tex = params.config.w > 0.5;
    var m = params.config.z;
    if (has_tex) {
        m = textureSample(mr_texture, mr_sampler, uv).b;
    }
    // Tint yellow-ish to distinguish from roughness at a glance
    return vec3<f32>(m, m * 0.85, m * 0.2);
}

fn mode_depth(world_pos: vec3<f32>) -> vec3<f32> {
    // Distance from camera, mapped over [near, far] → grayscale (near=white).
    let cam = view_bindings::view.world_position.xyz;
    let d = length(world_pos - cam);
    let n = params.extra.x;
    let f = params.extra.y;
    let t = clamp((d - n) / max(f - n, 0.0001), 0.0, 1.0);
    let v = 1.0 - t;
    return vec3<f32>(v, v, v);
}

fn mode_uv_checker(uv: vec2<f32>) -> vec3<f32> {
    let scale = params.extra.z;
    let scaled = uv * scale;
    let cell = floor(scaled);
    let checker = (cell.x + cell.y) - 2.0 * floor((cell.x + cell.y) * 0.5);
    let frac_uv = fract(scaled);
    // grid lines
    let line_w = 0.03;
    let on_line = frac_uv.x < line_w || frac_uv.y < line_w
        || frac_uv.x > (1.0 - line_w) || frac_uv.y > (1.0 - line_w);
    if (on_line) {
        return vec3<f32>(0.05, 0.05, 0.05);
    }
    // color by cell coords to convey orientation
    let cx = fract(cell.x / scale);
    let cy = fract(cell.y / scale);
    if (checker > 0.5) {
        return vec3<f32>(0.85 - cx * 0.5, 0.2, 0.2 + cx * 0.6);
    } else {
        return vec3<f32>(0.1, 0.85 - cy * 0.5, 0.1);
    }
}

fn mode_flat_clay(world_normal: vec3<f32>) -> vec3<f32> {
    // Hemisphere shading: sky/ground mix by normal.y, plus subtle directional wrap.
    let n = normalize(world_normal);
    let sky = vec3<f32>(0.95, 0.95, 0.98);
    let ground = vec3<f32>(0.35, 0.34, 0.32);
    let hemi_t = n.y * 0.5 + 0.5;
    let hemi = mix(ground, sky, hemi_t);
    let sun_dir = normalize(vec3<f32>(0.3, 0.8, 0.4));
    let wrap = max(dot(n, sun_dir) * 0.6 + 0.4, 0.0);
    return hemi * (0.6 + 0.4 * wrap);
}

// Matcap: the shading a sculptor works under.
//
// Two properties, and neither is available from scene lighting.
//
// First, the lights are fixed to the **camera**, not to the world. A world-lit
// surface changes brightness as you orbit, so half of what you see moving is
// the light rather than the form; with view-space lighting the same curvature
// always reads the same way and orbiting tells you only about the shape. That
// is what a matcap is, and why every sculpting tool has one.
//
// Second, cavity. Diffuse shading is a function of the normal, and a fine
// crease barely changes the normal, so a wrinkle a millimetre deep is invisible
// under any number of lights. Curvature is the *derivative* of the normal, and
// it is large exactly where the surface folds. Screen-space partial derivatives
// give it for free: `dpdx(n.x) + dpdy(n.y)` in view space is the divergence of
// the normal field, negative in a valley and positive on a ridge. Darkening
// valleys and lifting ridges by it is what makes detail legible.
//
// Analytic rather than a sampled matcap image: no asset to ship or to fail to
// load, and the cavity term has to be computed here regardless.
fn mode_matcap(world_normal: vec3<f32>) -> vec3<f32> {
    let n = normalize(direction_world_to_view(normalize(world_normal)));

    // Three-point studio rig, all in view space. The key from over the viewer's
    // left shoulder is the convention every sculpting matcap uses; a form lit
    // from anywhere else reads as unfamiliar even when it is correct.
    let key = max(dot(n, normalize(vec3<f32>(-0.45, 0.55, 0.70))), 0.0);
    let fill = max(dot(n, normalize(vec3<f32>(0.65, -0.25, 0.55))), 0.0);
    // Rim: bright where the surface turns away, which is what draws the
    // silhouette and makes a limb read as round rather than flat.
    let rim = pow(1.0 - clamp(n.z, 0.0, 1.0), 3.0);

    let clay = vec3<f32>(0.62, 0.60, 0.58);
    var color = clay * (0.20 + 0.80 * pow(key, 0.8));
    color += vec3<f32>(0.16, 0.18, 0.24) * fill;
    color += vec3<f32>(0.30, 0.30, 0.33) * rim;

    // Screen-space curvature. Scaled by the fragment's own screen-space size so
    // the effect does not double every time you zoom in: derivatives are
    // per-pixel, and without this a crease looks deeper the closer you get.
    let curvature = (dpdx(n.x) + dpdy(n.y)) / max(fwidth(n.z) + 0.02, 0.02);
    let cavity = clamp(curvature * params.extra.w, -1.0, 1.0);
    // Valleys darken more than ridges brighten: a crease is a shadow, and
    // lifting ridges as hard would wash the form out.
    color *= 1.0 + select(cavity * 0.55, cavity * 0.30, cavity > 0.0);

    return max(color, vec3<f32>(0.0));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let mode = i32(params.config.x + 0.5);
    var color: vec3<f32>;
    if (mode == 0) {
        color = mode_normals(in.world_normal);
    } else if (mode == 1) {
        color = mode_roughness(in.uv);
    } else if (mode == 2) {
        color = mode_metallic(in.uv);
    } else if (mode == 3) {
        color = mode_depth(in.world_position.xyz);
    } else if (mode == 4) {
        color = mode_uv_checker(in.uv);
    } else if (mode == 6) {
        color = mode_matcap(in.world_normal);
    } else {
        color = mode_flat_clay(in.world_normal);
    }
    return vec4<f32>(color, 1.0);
}

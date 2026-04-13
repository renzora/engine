// Viewport debug visualization material.
//
// Mode values:
//   0 = Normals  (world-space normal mapped to color)
//   1 = Roughness (sample MR texture green channel, fall back to scalar)
//   2 = Metallic (sample MR texture blue channel, fall back to scalar)
//   3 = Depth (distance-based grayscale)
//   4 = UV Checker (procedural)
//   5 = Flat Clay (textures-off: neutral gray with hemisphere shading)

#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings as view_bindings

struct DebugParams {
    // x = mode, y = scalar_roughness, z = scalar_metallic, w = has_mr_texture (0/1)
    config: vec4<f32>,
    // x = depth_near, y = depth_far, z = checker_scale, w = unused
    extra: vec4<f32>,
};

@group(3) @binding(0) var<uniform> params: DebugParams;
@group(3) @binding(1) var mr_texture: texture_2d<f32>;
@group(3) @binding(2) var mr_sampler: sampler;

@vertex
fn vertex(
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let model = mesh_functions::get_world_from_local(instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(normal, instance_index);
    out.uv = uv;
    out.instance_index = instance_index;
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
    } else {
        color = mode_flat_clay(in.world_normal);
    }
    return vec4<f32>(color, 1.0);
}

#define_import_path renzora_rt::common

// Push constants matching RtPushConstants in Rust.
struct RtPushConstants {
    frame_index: u32,
    reset: u32,
    gi_max_ray_steps: u32,
    gi_max_distance_bits: u32,
    gi_thickness_bits: u32,
    gi_intensity_bits: u32,
    refl_intensity_bits: u32,
    flags: u32,
    light_dir_x: f32,
    light_dir_y: f32,
    light_dir_z: f32,
    shadow_max_steps: u32,
}

fn pc_gi_enabled(pc: RtPushConstants) -> bool {
    return (pc.flags & 1u) != 0u;
}
fn pc_reflections_enabled(pc: RtPushConstants) -> bool {
    return (pc.flags & 2u) != 0u;
}
fn pc_shadows_enabled(pc: RtPushConstants) -> bool {
    return (pc.flags & 4u) != 0u;
}
fn pc_is_half_res(pc: RtPushConstants) -> bool {
    return (pc.flags & 8u) != 0u;
}
fn pc_gi_max_distance(pc: RtPushConstants) -> f32 {
    return bitcast<f32>(pc.gi_max_distance_bits);
}
fn pc_gi_thickness(pc: RtPushConstants) -> f32 {
    return bitcast<f32>(pc.gi_thickness_bits);
}
fn pc_gi_intensity(pc: RtPushConstants) -> f32 {
    return bitcast<f32>(pc.gi_intensity_bits);
}
fn pc_refl_intensity(pc: RtPushConstants) -> f32 {
    return bitcast<f32>(pc.refl_intensity_bits);
}
fn pc_light_dir(pc: RtPushConstants) -> vec3<f32> {
    return vec3<f32>(pc.light_dir_x, pc.light_dir_y, pc.light_dir_z);
}
fn pc_shadow_max_steps(pc: RtPushConstants) -> u32 {
    return pc.shadow_max_steps;
}

// --- Reconstruction helpers ---

/// Reconstruct world-space position from depth + screen UV + inverse VP matrix.
fn reconstruct_world_position(depth: f32, uv: vec2<f32>, world_from_clip: mat4x4<f32>) -> vec3<f32> {
    // Bevy uses Y-down NDC: uv (0,0) = top-left, ndc (-1,1) = top-left
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, -(uv.y * 2.0 - 1.0), depth, 1.0);
    let world_h = world_from_clip * ndc;
    return world_h.xyz / world_h.w;
}

/// Estimate surface normal from depth buffer using 3-tap cross product.
fn estimate_normal_from_depth(
    depth_tex: texture_depth_2d,
    coord: vec2<i32>,
    world_from_clip: mat4x4<f32>,
    tex_size: vec2<f32>,
) -> vec3<f32> {
    let d_c = textureLoad(depth_tex, coord, 0);
    let d_r = textureLoad(depth_tex, coord + vec2<i32>(1, 0), 0);
    let d_u = textureLoad(depth_tex, coord + vec2<i32>(0, -1), 0); // up = -Y in screen

    let uv_c = (vec2<f32>(coord) + 0.5) / tex_size;
    let uv_r = (vec2<f32>(coord) + vec2<f32>(1.5, 0.5)) / tex_size;
    let uv_u = (vec2<f32>(coord) + vec2<f32>(0.5, -0.5)) / tex_size;

    let p_c = reconstruct_world_position(d_c, uv_c, world_from_clip);
    let p_r = reconstruct_world_position(d_r, uv_r, world_from_clip);
    let p_u = reconstruct_world_position(d_u, uv_u, world_from_clip);

    return normalize(cross(p_r - p_c, p_u - p_c));
}

/// Project world position to screen UV (0..1) + depth.
fn project_to_screen(world_pos: vec3<f32>, clip_from_world: mat4x4<f32>) -> vec3<f32> {
    let clip = clip_from_world * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    // Bevy Y-down NDC: ndc.y=-1 is top of screen (uv.y=0)
    return vec3<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5, ndc.z);
}

// --- RNG ---

fn hash_pcg(seed: u32) -> u32 {
    var state = seed * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand_float(seed: u32) -> f32 {
    return f32(hash_pcg(seed)) / 4294967296.0;
}

/// R2 quasi-random sequence — gives much better distribution than PCG hash.
/// Produces a 2D point in [0,1)^2 for a given sample index.
/// Based on the generalized golden ratio for 2D (plastic constant).
fn r2_sequence(index: u32) -> vec2<f32> {
    // 1/phi2 and 1/phi2^2 where phi2 is the plastic constant ≈ 1.3247
    let alpha = vec2<f32>(0.7548776662, 0.5698402909);
    return fract(vec2<f32>(0.5) + vec2<f32>(f32(index)) * alpha);
}

/// Cosine-weighted hemisphere sample around `normal`.
fn cosine_hemisphere_sample(normal: vec3<f32>, rand_u: f32, rand_v: f32) -> vec3<f32> {
    let r = sqrt(rand_u);
    let phi = 6.283185307 * rand_v;
    let x = r * cos(phi);
    let y = r * sin(phi);
    let z = sqrt(max(0.0, 1.0 - rand_u));

    let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(normal.y) > 0.999);
    let tangent = normalize(cross(up, normal));
    let bitangent = cross(normal, tangent);

    return normalize(tangent * x + bitangent * y + normal * z);
}

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

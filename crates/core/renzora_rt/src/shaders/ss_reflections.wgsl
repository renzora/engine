// Screen-space reflections — reflect view direction off surface normal,
// ray march through the depth buffer with adaptive stride.
// Handles half-res: output coord is trace-res, depth/color are full-res.

#import bevy_render::view::View
#import renzora_rt::common::{
    RtPushConstants, pc_gi_max_distance, pc_gi_thickness,
    reconstruct_world_position, estimate_normal_from_depth,
    project_to_screen, r2_sequence,
}

@group(0) @binding(0) var hdr_color: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(1) var depth_texture: texture_depth_2d;
@group(0) @binding(3) var<uniform> view: View;
@group(0) @binding(8) var refl_output: texture_storage_2d<rgba16float, read_write>;

// Radiance cache for off-screen fallback
@group(0) @binding(15) var<storage, read_write> cache_life: array<u32>;
@group(0) @binding(16) var<storage, read_write> cache_radiance: array<vec4<f32>>;
@group(0) @binding(18) var<storage, read_write> cache_samples: array<u32>;

var<push_constant> pc: RtPushConstants;

const CACHE_SIZE: u32 = 524288u;
const CELL_SIZE: f32 = 1.0;
const COARSE_STRIDE: u32 = 4u;

fn hash_position(pos: vec3<f32>) -> u32 {
    let q = vec3<i32>(floor(pos / CELL_SIZE));
    var h = u32(q.x) * 73856093u;
    h = h ^ (u32(q.y) * 19349663u);
    h = h ^ (u32(q.z) * 83492791u);
    return h % CACHE_SIZE;
}

fn cache_query(world_pos: vec3<f32>) -> vec3<f32> {
    let idx = hash_position(world_pos);
    if cache_samples[idx] > 0u && cache_life[idx] > 0u {
        return cache_radiance[idx].rgb;
    }
    return vec3<f32>(0.0);
}

@compute @workgroup_size(8, 8, 1)
fn ss_reflections(@builtin(global_invocation_id) id: vec3<u32>) {
    // Output is at trace resolution (may be half-res)
    let out_size = vec2<f32>(textureDimensions(refl_output));
    let coord = vec2<i32>(id.xy);

    if any(vec2<f32>(coord) >= out_size) {
        return;
    }

    // Map trace-res coord to normalized UV, then to full-res
    let uv = (vec2<f32>(coord) + 0.5) / out_size;
    let full_size = vec2<f32>(textureDimensions(depth_texture));
    let full_coord = vec2<i32>(uv * full_size);

    let depth = textureLoad(depth_texture, full_coord, 0);
    if depth >= 1.0 || depth <= 0.0 {
        textureStore(refl_output, coord, vec4<f32>(0.0));
        return;
    }

    let world_pos = reconstruct_world_position(depth, uv, view.world_from_clip);
    let normal = estimate_normal_from_depth(depth_texture, full_coord, view.world_from_clip, full_size);

    let view_dir = normalize(view.world_position.xyz - world_pos);
    let reflect_dir = reflect(-view_dir, normal);

    let ray_origin = world_pos + normal * 0.05;
    let max_steps = pc.gi_max_ray_steps;
    let max_dist = pc_gi_max_distance(pc);
    let thickness = pc_gi_thickness(pc);
    let fine_step = max_dist / f32(max_steps);
    let coarse_step = fine_step * f32(COARSE_STRIDE);
    let coarse_steps = max_steps / COARSE_STRIDE;

    var radiance = vec3<f32>(0.0);
    var hit = false;
    var last_world_pos = ray_origin;

    for (var i = 0u; i < coarse_steps; i = i + 1u) {
        let t = f32(i + 1u) * coarse_step;
        let sample_pos = ray_origin + reflect_dir * t;
        last_world_pos = sample_pos;

        let screen = project_to_screen(sample_pos, view.clip_from_world);

        if screen.x < 0.0 || screen.x > 1.0 || screen.y < 0.0 || screen.y > 1.0 {
            break;
        }
        if screen.z < 0.0 {
            break;
        }

        let sample_coord = vec2<i32>(screen.xy * full_size);
        let scene_depth = textureLoad(depth_texture, sample_coord, 0);
        let depth_diff = scene_depth - screen.z;

        if depth_diff > 0.0 && depth_diff < thickness * f32(COARSE_STRIDE) {
            let refine_start = f32(i) * coarse_step;
            for (var j = 0u; j < COARSE_STRIDE + 1u; j = j + 1u) {
                let t_fine = refine_start + f32(j + 1u) * fine_step;
                let fine_pos = ray_origin + reflect_dir * t_fine;
                let fine_screen = project_to_screen(fine_pos, view.clip_from_world);

                if fine_screen.x < 0.0 || fine_screen.x > 1.0 || fine_screen.y < 0.0 || fine_screen.y > 1.0 { continue; }
                if fine_screen.z < 0.0 { continue; }

                let fine_coord = vec2<i32>(fine_screen.xy * full_size);
                let fine_depth = textureLoad(depth_texture, fine_coord, 0);
                let fine_diff = fine_depth - fine_screen.z;

                if fine_diff > 0.0 && fine_diff < thickness {
                    let hit_color = textureLoad(hdr_color, fine_coord);
                    radiance = hit_color.rgb;
                    hit = true;
                    break;
                }
            }
            if hit { break; }
        }
    }

    if !hit {
        radiance = cache_query(last_world_pos);
    }

    textureStore(refl_output, coord, vec4<f32>(radiance, select(0.0, 1.0, hit)));
}

// Screen-space contact shadows — short-range ray march toward light.
//
// Marches from each pixel toward the primary directional light in screen space.
// If the depth buffer is intersected, the pixel is in shadow.
// Complements traditional shadow maps for fine contact detail.

#import bevy_render::view::View
#import renzora_rt::common::{
    RtPushConstants, pc_light_dir, pc_shadow_max_steps,
    reconstruct_world_position, estimate_normal_from_depth,
    project_to_screen,
}

@group(0) @binding(1) var depth_texture: texture_depth_2d;
@group(0) @binding(3) var<uniform> view: View;
@group(0) @binding(10) var shadow_output: texture_storage_2d<r16float, read_write>;

var<push_constant> pc: RtPushConstants;

@compute @workgroup_size(8, 8, 1)
fn ss_shadows(@builtin(global_invocation_id) id: vec3<u32>) {
    let tex_size = vec2<f32>(textureDimensions(depth_texture));
    let coord = vec2<i32>(id.xy);

    if any(vec2<f32>(id.xy) >= tex_size) {
        return;
    }

    let depth = textureLoad(depth_texture, coord, 0);
    if depth >= 1.0 || depth <= 0.0 {
        textureStore(shadow_output, coord, vec4<f32>(1.0, 0.0, 0.0, 0.0));
        return;
    }

    let uv = (vec2<f32>(coord) + 0.5) / tex_size;
    let world_pos = reconstruct_world_position(depth, uv, view.world_from_clip);
    let normal = estimate_normal_from_depth(depth_texture, coord, view.world_from_clip, tex_size);

    // Direction toward the primary directional light (extracted from scene)
    let light_dir = pc_light_dir(pc);

    let ray_origin = world_pos + normal * 0.02;
    let max_steps = pc_shadow_max_steps(pc);
    let max_dist = 5.0;
    let step_size = max_dist / f32(max_steps);
    let thickness = 0.3;

    var shadow = 1.0; // 1.0 = fully lit

    for (var i = 0u; i < max_steps; i = i + 1u) {
        let t = f32(i + 1u) * step_size;
        let sample_pos = ray_origin + light_dir * t;
        let screen = project_to_screen(sample_pos, view.clip_from_world);

        if screen.x < 0.0 || screen.x > 1.0 || screen.y < 0.0 || screen.y > 1.0 {
            break;
        }
        if screen.z < 0.0 {
            break;
        }

        let sample_coord = vec2<i32>(screen.xy * tex_size);
        let scene_depth = textureLoad(depth_texture, sample_coord, 0);

        let depth_diff = scene_depth - screen.z;
        if depth_diff > 0.0 && depth_diff < thickness {
            shadow = 0.0;
            break;
        }
    }

    textureStore(shadow_output, coord, vec4<f32>(shadow, 0.0, 0.0, 0.0));
}

#define_import_path bevy_gaussian_splatting::optical_flow

#import bevy_pbr::{
    forward_io::VertexOutput,
    prepass_utils,
}
#import bevy_render::color_operations::hsv_to_rgb
#import bevy_render::maths::PI_2

#import bevy_gaussian_splatting::bindings::{
    globals,
    previous_view_uniforms,
    view,
}

fn calculate_motion_vector(
    world_position: vec3<f32>,
    previous_world_position: vec3<f32>,
) -> vec2<f32> {
    let world_position_t = vec4<f32>(world_position, 1.0);
    let previous_world_position_t = vec4<f32>(previous_world_position, 1.0);
    let clip_position_t = view.unjittered_clip_from_world * world_position_t;
    let clip_position = clip_position_t.xy / clip_position_t.w;
    let previous_clip_position_t = previous_view_uniforms.clip_from_world * previous_world_position_t;
    let previous_clip_position = previous_clip_position_t.xy / previous_clip_position_t.w;
    // These motion vectors are used as offsets to UV positions and are stored
    // in the range -1,1 to allow offsetting from the one corner to the
    // diagonally-opposite corner in UV coordinates, in either direction.
    // A difference between diagonally-opposite corners of clip space is in the
    // range -2,2, so this needs to be scaled by 0.5. And the V direction goes
    // down where clip space y goes up, so y needs to be flipped.
    return (clip_position - previous_clip_position) * vec2(0.5, -0.5);
}

fn optical_flow_to_rgb(
    motion_vector: vec2<f32>,
) -> vec3<f32> {
    let flow = motion_vector / globals.delta_time;

    let radius = length(flow);
    var angle = atan2(flow.y, flow.x);
    if (angle < 0.0) {
        angle += PI_2;
    }

    // let sigma: f32 = 0.15;
    // let norm_factor = sigma * 2.0;
    // let m = clamp(radius / norm_factor, 0.0, 1.0);
    let m = clamp(radius, 0.0, 1.0);

    let rgb = hsv_to_rgb(vec3<f32>(angle, m, 1.0));
    return rgb;
}

// TODO: support immediate vs. persistent previous_view, aiding with no-smoothness on the pan-orbit camera (required by cpu sort)
// TODO: set clear color to white in optical flow render mode

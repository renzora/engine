#define_import_path bevy_gaussian_splatting::transform

#import bevy_gaussian_splatting::bindings::view

fn world_to_clip(world_pos: vec3<f32>) -> vec4<f32> {
    let homogenous_pos = view.unjittered_clip_from_world * vec4<f32>(world_pos, 1.0);
    return homogenous_pos / (homogenous_pos.w + 0.000000001);
}

fn in_frustum(clip_space_pos: vec3<f32>) -> bool {
    return abs(clip_space_pos.x) < 1.1
        && abs(clip_space_pos.y) < 1.1
        && abs(clip_space_pos.z - 0.5) < 0.5;
}

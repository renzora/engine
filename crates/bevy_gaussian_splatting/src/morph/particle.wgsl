#define_import_path bevy_gaussian_splatting::morph::particle

#import bevy_gaussian_splatting::bindings::{
    gaussian_uniforms,
    globals,
    position_visibility,
}
#import bevy_gaussian_splatting::spherical_harmonics::spherical_harmonics_lookup
#import bevy_gaussian_splatting::transform::{
    world_to_clip,
    in_frustum,
}

struct ParticleBehavior {
    @location(0) indicies: vec4<i32>,
    @location(1) velocity: vec4<f32>,
    @location(2) acceleration: vec4<f32>,
    @location(3) jerk: vec4<f32>,
}

@group(3) @binding(7) var<storage, read_write> particle_behaviors: array<ParticleBehavior>;

@compute @workgroup_size(32, 32)
fn apply_particle_behaviors(
    @builtin(local_invocation_id) gl_LocalInvocationID: vec3<u32>,
    @builtin(global_invocation_id) gl_GlobalInvocationID: vec3<u32>,
) {
    let behavior_index = gl_GlobalInvocationID.x * 32u + gl_GlobalInvocationID.y;
    let behavior = particle_behaviors[behavior_index];

    let point_index = behavior.indicies.x;
    let point = position_visibility[point_index];

    // TODO: add gaussian attribute setters for 4d capability

    let delta_position = behavior.velocity * globals.delta_time + 0.5 * behavior.acceleration * globals.delta_time * globals.delta_time + 1.0 / 6.0 * behavior.jerk * globals.delta_time * globals.delta_time * globals.delta_time;
    let delta_velocity = behavior.acceleration * globals.delta_time + 0.5 * behavior.jerk * globals.delta_time * globals.delta_time;
    let delta_acceleration = behavior.jerk * globals.delta_time;

    let new_position = point + delta_position;
    let new_velocity = behavior.velocity + delta_velocity;
    let new_acceleration = behavior.acceleration + delta_acceleration;

    workgroupBarrier();

    if (behavior.indicies.x < 0) {
        return;
    }

    position_visibility[point_index] = new_position;
    particle_behaviors[behavior_index].velocity = new_velocity;
    particle_behaviors[behavior_index].acceleration = new_acceleration;
}

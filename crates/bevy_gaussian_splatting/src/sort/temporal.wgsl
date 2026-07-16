
@compute @workgroup_size(#{TEMPORAL_SORT_WINDOW_SIZE})
fn temporal_sort_flip(
    @builtin(local_invocation_id) gl_LocalInvocationID: vec3<u32>,
    @builtin(global_invocation_id) gl_GlobalInvocationID: vec3<u32>,
) {
    // let start_index = gl_GlobalInvocationID.x * #{TEMPORAL_SORT_WINDOW_SIZE}u;
    // let end_index = start_index + #{TEMPORAL_SORT_WINDOW_SIZE}u;
}

@compute @workgroup_size(#{TEMPORAL_SORT_WINDOW_SIZE})
fn temporal_sort_flop(
    @builtin(local_invocation_id) gl_LocalInvocationID: vec3<u32>,
    @builtin(global_invocation_id) gl_GlobalInvocationID: vec3<u32>,
) {
    // // TODO: pad sorting buffers to 1.5 window size
    // let start_index = gl_GlobalInvocationID.x * #{TEMPORAL_SORT_WINDOW_SIZE}u + #{TEMPORAL_SORT_WINDOW_SIZE}u / 2u;
    // let end_index = start_index + #{TEMPORAL_SORT_WINDOW_SIZE}u;

    // // pair sort entries in window size
    // for (var i = start_index; i < end_index; i += 2u) {
    //     let pos_a = points[input_entries[i][0]].position_visibility.xyz;
    //     let depth_a = world_to_clip(pos_a).z;
    // }
}

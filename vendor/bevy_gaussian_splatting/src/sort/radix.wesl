import bevy_gaussian_splatting::render::bindings::view;
import bevy_gaussian_splatting::render::bindings::globals;
import bevy_gaussian_splatting::render::bindings::gaussian_uniforms;
import bevy_gaussian_splatting::render::bindings::DrawIndirect;
import bevy_gaussian_splatting::render::bindings::Entry;
import bevy_gaussian_splatting::render::transform::world_to_clip;
import bevy_gaussian_splatting::render::transform::in_frustum;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_position;
@if(!PACKED_F32 && BUFFER_STORAGE) import bevy_gaussian_splatting::render::planar::get_position;
@if(BUFFER_TEXTURE) import bevy_gaussian_splatting::render::texture::get_position;


// The radix sizes are shader defs, which WESL reads as `constants::NAME`.
// Those are abstract integers; `u32(...)` stands where the old source pasted
// the number in front of a `u` suffix (`#{RADIX_BASE}u`).

struct SortingGlobal {
    digit_histogram: array<array<atomic<u32>, constants::RADIX_BASE>, constants::RADIX_DIGIT_PLACES>,
    assignment_counter: atomic<u32>,
    digit_tile_head: array<atomic<u32>, constants::RADIX_BASE>,
}

@group(3) @binding(0) var<uniform> sorting_pass_index: u32;
@group(3) @binding(1) var<storage, read_write> sorting: SortingGlobal;
// Per-tile temporary storage for radix pass C.
@group(3) @binding(2) var<storage, read_write> status_counters: array<array<atomic<u32>, constants::RADIX_BASE>>;
@group(3) @binding(3) var<storage, read_write> draw_indirect: DrawIndirect;
@group(3) @binding(4) var<storage, read_write> input_entries: array<Entry>;
@group(3) @binding(5) var<storage, read_write> output_entries: array<Entry>;

//
// The following three functions (`radix_reset`, `radix_sort_a`, `radix_sort_b`)
// form a standard three-phase GPU sort setup and were already correct.
// They are included here without changes.
//

@compute @workgroup_size(constants::RADIX_BASE, constants::RADIX_DIGIT_PLACES)
fn radix_reset(
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(global_invocation_id) global_id: vec3<u32>,
){
    let b = local_id.x;
    let p = local_id.y;
    atomicStore(&sorting.digit_histogram[p][b], 0u);
    if (p == 0u) {
        atomicStore(&sorting.digit_tile_head[b], 0u);
    }
    if (global_id.x == 0u && global_id.y == 0u) {
        atomicStore(&sorting.assignment_counter, 0u);
        draw_indirect.instance_count = 0u;
    }
}

@compute @workgroup_size(constants::RADIX_BASE, constants::RADIX_DIGIT_PLACES)
fn radix_sort_a(
    @builtin(local_invocation_id) gl_LocalInvocationID: vec3<u32>,
    @builtin(global_invocation_id) gl_GlobalInvocationID: vec3<u32>,
) {
    if (gl_LocalInvocationID.x == 0u && gl_LocalInvocationID.y == 0u && gl_GlobalInvocationID.x == 0u) {
        draw_indirect.vertex_count = 4u;
        atomicStore(&draw_indirect.instance_count, gaussian_uniforms.count);
    }
    workgroupBarrier();

    let thread_index = gl_GlobalInvocationID.x * u32(constants::RADIX_DIGIT_PLACES) + gl_GlobalInvocationID.y;
    let start_entry_index = thread_index * u32(constants::ENTRIES_PER_INVOCATION_A);
    let end_entry_index = start_entry_index + u32(constants::ENTRIES_PER_INVOCATION_A);

    for (var entry_index = start_entry_index; entry_index < end_entry_index; entry_index += 1u) {
        if (entry_index >= gaussian_uniforms.count) { continue; }
        var key: u32 = 0xFFFFFFFFu;
        let position = vec4<f32>(get_position(entry_index), 1.0);
        let transformed_position = (gaussian_uniforms.transform * position).xyz;
        let clip_space_pos = world_to_clip(transformed_position);
        let diff = transformed_position - view.world_position;
        let dist2 = dot(diff, diff);
        let dist_bits = bitcast<u32>(dist2);
        let key_distance = 0xFFFFFFFFu - dist_bits;
        if (in_frustum(clip_space_pos.xyz)) {
            key = key_distance;
        }
        key = key >> u32(constants::RADIX_KEY_SHIFT);
        input_entries[entry_index].key = key;
        input_entries[entry_index].value = entry_index;
        for(var shift = 0u; shift < u32(constants::RADIX_DIGIT_PLACES); shift += 1u) {
            let digit = (key >> (shift * u32(constants::RADIX_BITS_PER_DIGIT))) & (u32(constants::RADIX_BASE) - 1u);
            atomicAdd(&sorting.digit_histogram[shift][digit], 1u);
        }
    }
}

@compute @workgroup_size(1)
fn radix_sort_b(
    @builtin(global_invocation_id) gl_GlobalInvocationID: vec3<u32>,
) {
    var sum = 0u;
    for(var digit = 0u; digit < u32(constants::RADIX_BASE); digit += 1u) {
        let tmp = atomicLoad(&sorting.digit_histogram[gl_GlobalInvocationID.y][digit]);
        atomicStore(&sorting.digit_histogram[gl_GlobalInvocationID.y][digit], sum);
        sum += tmp;
    }
}

// --- SHARED MEMORY for radix pass C ---
var<workgroup> tile_input_entries: array<Entry, constants::WORKGROUP_ENTRIES_C>;
var<workgroup> sorted_tile_entries: array<Entry, constants::WORKGROUP_ENTRIES_C>;
var<workgroup> tile_digit_counts: array<atomic<u32>, constants::RADIX_BASE>;
var<workgroup> local_digit_counts: array<u32, constants::RADIX_BASE>;
var<workgroup> local_digit_offsets: array<u32, constants::RADIX_BASE>;
var<workgroup> tile_entry_count_ws: u32;
const INVALID_KEY: u32 = 0xFFFFFFFFu;

@compute @workgroup_size(constants::WORKGROUP_INVOCATIONS_C)
fn radix_sort_c_count_tiles(
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    let tid = local_id.x;
    let tile_size = u32(constants::WORKGROUP_ENTRIES_C);
    let threads = u32(constants::WORKGROUP_INVOCATIONS_C);
    let global_entry_offset = workgroup_id.y * tile_size;

    if (tid < u32(constants::RADIX_BASE)) {
        atomicStore(&tile_digit_counts[tid], 0u);
    }
    workgroupBarrier();

    for (var i = tid; i < tile_size; i += threads) {
        let idx = global_entry_offset + i;
        if (idx >= gaussian_uniforms.count) {
            continue;
        }

        let entry = input_entries[idx];
        let digit = (entry.key >> (sorting_pass_index * u32(constants::RADIX_BITS_PER_DIGIT))) & (u32(constants::RADIX_BASE) - 1u);
        atomicAdd(&tile_digit_counts[digit], 1u);
    }
    workgroupBarrier();

    if (tid < u32(constants::RADIX_BASE)) {
        let count = atomicLoad(&tile_digit_counts[tid]);
        atomicStore(&status_counters[workgroup_id.y][tid], count);
    }
}

// One workgroup of RADIX_BASE lanes, lane = digit. Each digit's tile-prefix is
// independent (no cross-lane data), so this packs 256 lanes into ~4 full waves instead
// of dispatching 256 single-lane workgroups (1/64 wave occupancy on RDNA).
@compute @workgroup_size(constants::RADIX_BASE)
fn radix_sort_c_scan_tiles(
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let digit = local_id.x;
    if (digit >= u32(constants::RADIX_BASE)) {
        return;
    }

    let tile_size = u32(constants::WORKGROUP_ENTRIES_C);
    let tile_count = (gaussian_uniforms.count + tile_size - 1u) / tile_size;

    var sum = atomicLoad(&sorting.digit_histogram[sorting_pass_index][digit]);
    for (var tile = 0u; tile < tile_count; tile += 1u) {
        let count = atomicLoad(&status_counters[tile][digit]);
        atomicStore(&status_counters[tile][digit], sum);
        sum += count;
    }
}

@compute @workgroup_size(constants::WORKGROUP_INVOCATIONS_C)
fn radix_sort_c_scatter(
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    let tid = local_id.x;
    let tile_size = u32(constants::WORKGROUP_ENTRIES_C);
    let threads = u32(constants::WORKGROUP_INVOCATIONS_C);
    let global_entry_offset = workgroup_id.y * tile_size;

    // Step 1: Parallel load.
    for (var i = tid; i < tile_size; i += threads) {
        let idx = global_entry_offset + i;
        if (idx < gaussian_uniforms.count) {
            tile_input_entries[i] = input_entries[idx];
        } else {
            tile_input_entries[i] = Entry(INVALID_KEY, INVALID_KEY);
        }
    }
    workgroupBarrier();

    // Step 2a: PARALLEL per-digit count of this tile (all lanes, atomic into LDS).
    if (tid < u32(constants::RADIX_BASE)) {
        atomicStore(&tile_digit_counts[tid], 0u);
    }
    workgroupBarrier();
    for (var i = tid; i < tile_size; i += threads) {
        let entry = tile_input_entries[i];
        if (entry.value != INVALID_KEY) { // value sentinel marks padding
            let digit = (entry.key >> (sorting_pass_index * u32(constants::RADIX_BITS_PER_DIGIT))) & (u32(constants::RADIX_BASE) - 1u);
            atomicAdd(&tile_digit_counts[digit], 1u);
        }
    }
    workgroupBarrier();

    // Step 2b: STABLE per-tile placement: must run in input order, so kept on one lane
    // (LSD radix requires per-pass stability). The expensive O(tile_size) COUNT above is
    // now parallel; this lane only does the 256-wide prefix + the ordered placement.
    if (tid == 0u) {
        var entries_in_tile = 0u;
        for (var i = 0u; i < u32(constants::RADIX_BASE); i+=1u) {
            let c = atomicLoad(&tile_digit_counts[i]);
            local_digit_counts[i] = c;
            entries_in_tile += c;
        }
        tile_entry_count_ws = entries_in_tile;

        var sum = 0u;
        for (var i = 0u; i < u32(constants::RADIX_BASE); i+=1u) {
            local_digit_offsets[i] = sum;
            sum += local_digit_counts[i];
        }

        for (var i = 0u; i < tile_size; i+=1u) {
            let entry = tile_input_entries[i];
            if (entry.value == INVALID_KEY) { continue; } // value sentinel marks padding

            let digit = (entry.key >> (sorting_pass_index * u32(constants::RADIX_BITS_PER_DIGIT))) & (u32(constants::RADIX_BASE) - 1u);
            let dest_idx = local_digit_offsets[digit];
            local_digit_offsets[digit] = dest_idx + 1u;
            sorted_tile_entries[dest_idx] = entry;
        }
    }
    workgroupBarrier();

    // Step 3: Parallel write from the locally-sorted tile to global memory.
    if (tid == 0u) {
        var sum = 0u;
        for (var i = 0u; i < u32(constants::RADIX_BASE); i += 1u) {
            local_digit_offsets[i] = sum;
            sum += local_digit_counts[i];
        }
    }
    workgroupBarrier();

    for (var i = tid; i < tile_size; i += threads) {
        if (i < tile_entry_count_ws) {
            let entry = sorted_tile_entries[i];
            let digit = (entry.key >> (sorting_pass_index * u32(constants::RADIX_BITS_PER_DIGIT))) & (u32(constants::RADIX_BASE) - 1u);

            let bin_start_offset = local_digit_offsets[digit];
            let rank_in_bin = i - bin_start_offset;
            let global_base = atomicLoad(&status_counters[workgroup_id.y][digit]);
            let dst = global_base + rank_in_bin;

            if (dst < gaussian_uniforms.count) {
                output_entries[dst] = entry;
            }
        }
    }
    if (sorting_pass_index == u32(constants::RADIX_DIGIT_PLACES) - 1u && tid == 0u) {
        atomicStore(&draw_indirect.instance_count, gaussian_uniforms.count);
    }
}

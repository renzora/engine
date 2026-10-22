// The GPU backend's fixed kernels, the same for every emitter: clearing the
// pool, the once-a-frame reduction the CPU reads back a frame late (live
// count, light, bounds), and the back-to-front bitonic sort. Appended to
// `library.wgsl`, whose bindings and helpers they use, and compiled once.
//
// Each reads the frame record (`step`) rather than a step: capacity, the sort
// size and camera, the reset serial.

@group(1) @binding(0) var<uniform> sort_pass: vec4<u32>;

// Empty the pool: every slot free, the dead list holding all of them in an
// order that hands out slot 0 first, the counters zeroed and stamped with
// the reset's serial so a readback from before it can be told apart.
@compute @workgroup_size(64)
fn reset(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let capacity = step.counts.z;
    if i == 0u {
        atomicStore(&state.counters[C_ALIVE], 0u);
        atomicStore(&state.counters[C_DEAD], capacity);
        atomicStore(&state.counters[C_DROPPED], 0u);
        atomicStore(&state.counters[C_SERIAL], step.seed.w);
    }
    if i < capacity {
        particles[i].lifetime = -1.0;
        state.words[i] = capacity - 1u - i;
    }
}

// An order-preserving map from f32 to u32, so atomicMin and atomicMax on the
// bits order the floats. layout.rs decodes it.
fn order_bits(f: f32) -> u32 {
    let b = bitcast<u32>(f);
    if (b & 0x80000000u) != 0u {
        return ~b;
    }
    return b | 0x80000000u;
}

// Light sums are fixed point at this scale, clamped per workgroup so a sea
// of HDR particles saturates rather than wraps.
const LIGHT_SCALE: f32 = 64.0;
const LIGHT_CLAMP: f32 = 1048576.0;

var<workgroup> wg_sum: array<vec4<f32>, 64>;
var<workgroup> wg_lo: array<vec4<f32>, 64>;
var<workgroup> wg_hi: array<vec4<f32>, 64>;

// What light.rs's PerEmitter aggregate and the renderer's bounds need, over
// every live slot: the summed weight and weighted colour, the position
// bounds, the largest particle radius and the fastest speed. A tree
// reduction per workgroup and one atomic per value per workgroup, so 200k
// particles cost a few thousand atomics, not a million.
@compute @workgroup_size(64)
fn stats(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let big = 3.0e38;
    var sum = vec4<f32>(0.0);
    var lo = vec4<f32>(big, big, big, 0.0);
    var hi = vec4<f32>(-big, -big, -big, 0.0);
    let i = gid.x;
    if i < step.counts.z {
        let p = particles[i];
        if p.lifetime >= 0.0 && finite3(p.position) {
            let c = p.color;
            let a = clamp(c.w, 0.0, 1.0);
            let rgb = max(c.xyz, vec3<f32>(0.0));
            let lum = 0.2126 * rgb.x + 0.7152 * rgb.y + 0.0722 * rgb.z;
            let w = lum * a;
            if finite(w) && finite3(rgb * a) {
                sum = vec4<f32>(w, rgb * a);
            }
            let radius = 0.5 * length(abs(p.size));
            var speed = length(p.velocity);
            if !finite(speed) {
                speed = 0.0;
            }
            lo = vec4<f32>(p.position, speed);
            hi = vec4<f32>(p.position, san(radius));
        }
    }
    wg_sum[li] = sum;
    wg_lo[li] = lo;
    wg_hi[li] = hi;
    workgroupBarrier();
    for (var s = 32u; s > 0u; s = s >> 1u) {
        if li < s {
            wg_sum[li] = wg_sum[li] + wg_sum[li + s];
            let a = wg_lo[li];
            let b = wg_lo[li + s];
            wg_lo[li] = vec4<f32>(min(a.xyz, b.xyz), max(a.w, b.w));
            wg_hi[li] = max(wg_hi[li], wg_hi[li + s]);
        }
        workgroupBarrier();
    }
    if li == 0u {
        let total = wg_sum[0];
        atomicAdd(&state.counters[C_LIGHT], u32(clamp(total.x * LIGHT_SCALE, 0.0, LIGHT_CLAMP)));
        atomicAdd(&state.counters[C_LIGHT + 1u], u32(clamp(total.y * LIGHT_SCALE, 0.0, LIGHT_CLAMP)));
        atomicAdd(&state.counters[C_LIGHT + 2u], u32(clamp(total.z * LIGHT_SCALE, 0.0, LIGHT_CLAMP)));
        atomicAdd(&state.counters[C_LIGHT + 3u], u32(clamp(total.w * LIGHT_SCALE, 0.0, LIGHT_CLAMP)));
        let l = wg_lo[0];
        let h = wg_hi[0];
        if l.x <= h.x {
            atomicMin(&state.counters[C_MIN], order_bits(l.x));
            atomicMin(&state.counters[C_MIN + 1u], order_bits(l.y));
            atomicMin(&state.counters[C_MIN + 2u], order_bits(l.z));
            atomicMax(&state.counters[C_MAX], order_bits(h.x));
            atomicMax(&state.counters[C_MAX + 1u], order_bits(h.y));
            atomicMax(&state.counters[C_MAX + 2u], order_bits(h.z));
            atomicMax(&state.counters[C_MAX_RADIUS], order_bits(h.w));
            atomicMax(&state.counters[C_MAX_SPEED], order_bits(l.w));
        }
    }
}

// Fill the sort with every live particle's key, as a u32 an ascending sort
// puts in draw order (`step.sub.z` says which order, layout.rs's SORT_*):
// minus the squared distance from the camera (farthest first), minus the age
// (oldest first), the age (youngest first), or for a Connect ribbon the strip
// in the top byte and how many spawns ago the particle was born below it
// (each strip contiguous, newest first, the CPU's order by spawn index). The
// floats go through order_bits, so comparing the words compares the floats.
// The pad past the live count is the largest word and sorts to the end.
//
// A Connect ribbon also counts each strip's particles into the header, for
// the vertex shader to place a point along its strip; the CPU zeroes those
// words each frame before this runs.
@compute @workgroup_size(64)
fn sort_keys(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if k >= step.seed.z {
        return;
    }
    let count = atomicLoad(&state.counters[C_ALIVE]);
    var key = 0xFFFFFFFFu;
    var index = 0u;
    if k < count && k < step.counts.z {
        index = state.words[alive_base() + k];
        let p = particles[index];
        let mode = step.sub.z;
        if mode == SORT_CONNECT {
            // `counts.y` carries the strip count in the frame record.
            let groups = clamp(step.counts.y, 1u, 64u - C_GROUPS);
            let g = p.id % groups;
            let ago = min(step.sub.w - 1u - p.id, 0xFFFFFFu);
            key = (g << 24u) | ago;
            atomicAdd(&state.counters[C_GROUPS + g], 1u);
        } else if mode == SORT_OLDEST {
            key = order_bits(-p.age);
        } else if mode == SORT_YOUNGEST {
            key = order_bits(p.age);
        } else {
            let d = p.position - step.camera.xyz;
            let dist = dot(d, d);
            if finite(dist) {
                key = order_bits(-dist);
            } else {
                key = 0xFFFFFFFEu;
            }
        }
    }
    state.words[keys_base() + k] = key;
    state.words[sorted_base() + k] = index;
}

// Empty this emitter's inbox once its children are delivered, for the next
// step's events. A kernel of its own because `deliver` cannot zero the count
// its other threads are still reading.
@compute @workgroup_size(64)
fn clear_inbox(@builtin(global_invocation_id) gid: vec3<u32>) {
    let base = step.sub.x;
    if gid.x == 0u && base != NO_INBOX {
        atomicStore(&events[base], 0u);
        atomicStore(&events[base + 1u], 0u);
    }
}

// One compare-and-swap pass of a bitonic sort: `sort_pass.x` is the partner
// distance j, `sort_pass.y` the block size k.
@compute @workgroup_size(64)
fn sort_step(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let n = step.seed.z;
    let j = sort_pass.x;
    let k = sort_pass.y;
    if i >= n {
        return;
    }
    let l = i ^ j;
    if l <= i || l >= n {
        return;
    }
    let kb = keys_base();
    let sb = sorted_base();
    let ka = state.words[kb + i];
    let kl = state.words[kb + l];
    let ascending = (i & k) == 0u;
    if (ascending && ka > kl) || (!ascending && ka < kl) {
        let ki = state.words[kb + i];
        state.words[kb + i] = state.words[kb + l];
        state.words[kb + l] = ki;
        let si = state.words[sb + i];
        state.words[sb + i] = state.words[sb + l];
        state.words[sb + l] = si;
    }
}

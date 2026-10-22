// The GPU backend's shared library: the buffers every particle kernel binds,
// and a line-for-line transliteration of the CPU code each one must agree
// with. `codegen.rs` puts this text in front of every emitter's generated
// kernels and `kernels.wgsl` in front of the fixed ones, so one copy serves
// both and a kernel cannot see a different hash or noise than its neighbour.
//
// What each block mirrors, and is held to by the tests in `gpu/tests.rs`:
//   pcg .. random4      random.rs (u32 math only, so it is bit-identical)
//   lattice .. curl_fbm noise.rs
//   ease .. gradient    curve.rs, sampling the packed key tables of layout.rs
//   unit_sphere ..      sim/interp.rs's helpers
//   respond .. collide  sim/collide.rs
//   trail_start ..      sim/mod.rs's trail ring
//   interval_range ..   sim/sub.rs, and the event inboxes of layout.rs
//
// Plain WGSL with no imports or conditionals: it is concatenated, not
// imported, so the generated source is self-contained and naga can check it
// without Bevy's shader libraries.

// BEGIN PARTICLE STRUCT
// One particle, 96 bytes. A slot is free while `lifetime` is negative; a
// live particle's lifetime is sanitized to zero or more at birth.
struct Particle {
    position: vec3<f32>,
    age: f32,
    velocity: vec3<f32>,
    lifetime: f32,
    color: vec4<f32>,
    base_color: vec4<f32>,
    size: vec2<f32>,
    base_size: vec2<f32>,
    rotation: f32,
    angular_velocity: f32,
    id: u32,
    seed: f32,
};
// END PARTICLE STRUCT

// One step of one emitter, or the frame record the fixed kernels read.
struct Step {
    // x step length, y seconds into the cycle, z wind turbulence
    time: vec4<f32>,
    // x spawn count, y first spawn index, z capacity, w flags
    counts: vec4<u32>,
    // x seed low word, y seed high word, z sort size (a power of two), w reset serial
    seed: vec4<u32>,
    emitter_velocity: vec4<f32>,
    // the world wind in the emitter's simulation space
    wind: vec4<f32>,
    // the sorting camera in simulation space
    camera: vec4<f32>,
    // the entity's world transform and its inverse
    transform: mat4x4<f32>,
    inverse: mat4x4<f32>,
    params: array<vec4<f32>, 16>,
    // x this emitter's inbox in the event buffer (NO_INBOX for none), y its
    // capacity, z the sort mode, w the next spawn index
    sub: vec4<u32>,
};

// The counters sit in front of the lists so one binding carries both.
struct State {
    counters: array<atomic<u32>, 64>,
    words: array<u32>,
};

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<storage, read_write> state: State;
@group(0) @binding(2) var<storage, read> data: array<vec4<f32>>;
@group(0) @binding(3) var<uniform> step: Step;
// The effect's sub-emitter events, one inbox per GPU target (layout.rs's
// EventLayout). Atomic throughout, entries included: one binding has one
// type, and the counters at each inbox's head must be atomic.
@group(0) @binding(4) var<storage, read_write> events: array<atomic<u32>>;

const C_ALIVE: u32 = 0u;
const C_DEAD: u32 = 1u;
const C_DROPPED: u32 = 2u;
const C_SERIAL: u32 = 3u;
const C_LIGHT: u32 = 4u;
const C_MIN: u32 = 8u;
const C_MAX_RADIUS: u32 = 11u;
const C_MAX: u32 = 12u;
const C_MAX_SPEED: u32 = 15u;

const FLAG_LAST: u32 = 1u;
const FLAG_INVERSE_OK: u32 = 2u;

const EVENT_WORDS: u32 = 16u;
const EVENT_HEADER_WORDS: u32 = 4u;
const NO_INBOX: u32 = 4294967295u;
const MAX_INTERVAL_EVENTS: u32 = 16u;

const SORT_DEPTH: u32 = 1u;
const SORT_OLDEST: u32 = 2u;
const SORT_YOUNGEST: u32 = 3u;
const SORT_CONNECT: u32 = 4u;
const C_GROUPS: u32 = 32u;

// The dead list starts at word 0, the alive list at `capacity`, the sort keys
// at twice that and the sorted indices `sort size` after the keys.
fn alive_base() -> u32 {
    return step.counts.z;
}

fn keys_base() -> u32 {
    return 2u * step.counts.z;
}

fn sorted_base() -> u32 {
    return 2u * step.counts.z + step.seed.z;
}

// A Trail ribbon's history, after the sorted list (layout.rs's trail_word,
// less the header the words array starts after).
fn trail_base() -> u32 {
    return 2u * step.counts.z + 2u * step.seed.z;
}

// ── random.rs ────────────────────────────────────────────────────────────

fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let word = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (word >> 22u) ^ word;
}

fn particle_hash(seed_lo: u32, seed_hi: u32, id: u32, salt: u32, stream: u32) -> u32 {
    var h = pcg(stream);
    h = pcg(h ^ salt);
    h = pcg(h ^ id);
    h = pcg(h ^ seed_lo);
    return pcg(h ^ seed_hi);
}

fn unit_float(h: u32) -> f32 {
    return f32(h >> 8u) * (1.0 / 16777216.0);
}

fn random4(id: u32, salt: u32, stream: u32) -> vec4<f32> {
    let h = particle_hash(step.seed.x, step.seed.y, id, salt, stream);
    return vec4<f32>(
        unit_float(pcg(h)),
        unit_float(pcg(h ^ 0x68E31DA4u)),
        unit_float(pcg(h ^ 0xB5297A4Du)),
        unit_float(pcg(h ^ 0x1B56C4E9u)),
    );
}

fn salt_bits(salt: f32) -> u32 {
    if salt == 0.0 {
        return 0u;
    }
    return bitcast<u32>(salt);
}

// The per-particle seed the procedural shading reads (render/mod.rs's
// particle_seed), worked out once at birth.
fn particle_seed(id: u32) -> f32 {
    return unit_float(pcg(id ^ 0x9E3779B9u));
}

// ── Finite checks ────────────────────────────────────────────────────────
// By bits rather than by comparison, which a compiler allowed to assume no
// NaN may fold away.

fn finite(x: f32) -> bool {
    return (bitcast<u32>(x) & 0x7F800000u) != 0x7F800000u;
}

fn finite3(v: vec3<f32>) -> bool {
    return finite(v.x) && finite(v.y) && finite(v.z);
}

fn san(x: f32) -> f32 {
    if finite(x) {
        return x;
    }
    return 0.0;
}

fn san3(v: vec3<f32>) -> vec3<f32> {
    if finite3(v) {
        return v;
    }
    return vec3<f32>(0.0);
}

// ── noise.rs ─────────────────────────────────────────────────────────────

fn lattice(x: i32, y: i32, z: i32, channel: u32) -> f32 {
    let h = pcg(bitcast<u32>(x) ^ pcg(bitcast<u32>(y) ^ pcg(bitcast<u32>(z) ^ pcg(channel))));
    return f32(h >> 8u) * (2.0 / 16777216.0) - 1.0;
}

// The gradient of the value noise; the value itself is never used.
fn value_noise_grad(p: vec3<f32>, channel: u32) -> vec3<f32> {
    let i = floor(p);
    let f = p - i;
    let ix = i32(i.x);
    let iy = i32(i.y);
    let iz = i32(i.z);

    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);

    let a = lattice(ix, iy, iz, channel);
    let b = lattice(ix + 1, iy, iz, channel);
    let c = lattice(ix, iy + 1, iz, channel);
    let d = lattice(ix + 1, iy + 1, iz, channel);
    let e = lattice(ix, iy, iz + 1, channel);
    let f1 = lattice(ix + 1, iy, iz + 1, channel);
    let g = lattice(ix, iy + 1, iz + 1, channel);
    let h = lattice(ix + 1, iy + 1, iz + 1, channel);

    let k1 = b - a;
    let k2 = c - a;
    let k3 = e - a;
    let k4 = a - b - c + d;
    let k5 = a - c - e + g;
    let k6 = a - b - e + f1;
    let k7 = -a + b + c - d + e - f1 - g + h;

    return du * vec3<f32>(
        k1 + k4 * u.y + k6 * u.z + k7 * u.y * u.z,
        k2 + k5 * u.z + k4 * u.x + k7 * u.z * u.x,
        k3 + k6 * u.x + k5 * u.y + k7 * u.x * u.y,
    );
}

fn curl(p: vec3<f32>) -> vec3<f32> {
    if !finite3(p) {
        return vec3<f32>(0.0);
    }
    let g1 = value_noise_grad(p, 1u);
    let g2 = value_noise_grad(p, 2u);
    let g3 = value_noise_grad(p, 3u);
    return vec3<f32>(g3.y - g2.z, g1.z - g3.x, g2.x - g1.y);
}

const MAX_OCTAVES: f32 = 8.0;
const OCTAVE_SHIFT: vec3<f32> = vec3<f32>(17.13, 5.71, 11.37);

fn curl_fbm(p: vec3<f32>, octaves_in: f32, lacunarity_in: f32, persistence_in: f32) -> vec3<f32> {
    if !finite3(p) || !finite(octaves_in) {
        return vec3<f32>(0.0);
    }
    let octaves = clamp(octaves_in, 0.0, MAX_OCTAVES);
    var lacunarity = 2.0;
    if finite(lacunarity_in) {
        lacunarity = lacunarity_in;
    }
    var persistence = 0.5;
    if finite(persistence_in) {
        persistence = persistence_in;
    }
    var sum = vec3<f32>(0.0);
    var freq = 1.0;
    var amp = 1.0;
    var i = 0.0;
    loop {
        if !(i < octaves) {
            break;
        }
        let weight = min(octaves - i, 1.0);
        sum = sum + curl(p * freq + OCTAVE_SHIFT * i) * (amp * weight);
        freq = freq * lacunarity;
        amp = amp * persistence;
        i = i + 1.0;
    }
    return sum;
}

// ── curve.rs, over layout.rs's packed tables ─────────────────────────────

fn ease(kind: f32, x_in: f32) -> f32 {
    let x = clamp(x_in, 0.0, 1.0);
    let k = u32(kind);
    if k == 1u {
        return 0.0;
    } else if k == 2u {
        return x * x * (3.0 - 2.0 * x);
    } else if k == 3u {
        return x * x;
    } else if k == 4u {
        return 1.0 - (1.0 - x) * (1.0 - x);
    } else if k == 5u {
        if x < 0.5 {
            return 4.0 * x * x * x;
        }
        let y = -2.0 * x + 2.0;
        return 1.0 - y * y * y * 0.5;
    }
    return x;
}

// partition_point(|k| k.t <= t) over `count` keys `stride` vec4s apart.
fn key_partition(offset: u32, count: u32, stride: u32, t: f32) -> u32 {
    var lo = 0u;
    var hi = count;
    loop {
        if lo >= hi {
            break;
        }
        let mid = (lo + hi) / 2u;
        if data[offset + mid * stride].x <= t {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    return lo;
}

// A key is (t, value, ease, 0). An empty curve is 1.0.
fn sample_curve(dir: u32, t: f32) -> f32 {
    let d = data[dir];
    let offset = u32(d.x);
    let count = u32(d.y);
    if count == 0u {
        return 1.0;
    }
    let first = data[offset];
    if count == 1u {
        return first.y;
    }
    if t != t || t <= first.x {
        return first.y;
    }
    let last = data[offset + count - 1u];
    if t >= last.x {
        return last.y;
    }
    let i = max(key_partition(offset, count, 1u, t), 1u) - 1u;
    let a = data[offset + i];
    let b = data[offset + i + 1u];
    let span = b.x - a.x;
    if span <= 1.1920929e-7 {
        return b.y;
    }
    let x = ease(a.z, (t - a.x) / span);
    return a.y + (b.y - a.y) * x;
}

// A stop is two vec4s, (t, 0, 0, 0) then the colour. An empty gradient is
// white.
fn sample_gradient(dir: u32, t: f32) -> vec4<f32> {
    let d = data[dir];
    let offset = u32(d.x);
    let count = u32(d.y);
    if count == 0u {
        return vec4<f32>(1.0);
    }
    if count == 1u {
        return data[offset + 1u];
    }
    let first_t = data[offset].x;
    if t != t || t <= first_t {
        return data[offset + 1u];
    }
    let last = offset + 2u * (count - 1u);
    if t >= data[last].x {
        return data[last + 1u];
    }
    let i = max(key_partition(offset, count, 2u, t), 1u) - 1u;
    let at = data[offset + 2u * i].x;
    let bt = data[offset + 2u * i + 2u].x;
    let a = data[offset + 2u * i + 1u];
    let b = data[offset + 2u * i + 3u];
    let span = bt - at;
    if span <= 1.1920929e-7 {
        return b;
    }
    // glam's Vec4::lerp, which the CPU gradient uses, in its own form.
    let x = (t - at) / span;
    return a * (1.0 - x) + b * x;
}

// ── sim/interp.rs ────────────────────────────────────────────────────────

const TAU: f32 = 6.2831855;
const PI: f32 = 3.1415927;

fn m1(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(v.x, 0.0, 0.0, 0.0);
}

fn m2(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(v.x, v.y, 0.0, 0.0);
}

fn m3(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(v.x, v.y, v.z, 0.0);
}

fn safe_div1(a: f32, b: f32) -> f32 {
    if abs(b) > 1e-20 {
        return a / b;
    }
    return 0.0;
}

fn safe_div(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(safe_div1(a.x, b.x), safe_div1(a.y, b.y), safe_div1(a.z, b.z), safe_div1(a.w, b.w));
}

// Equal edges are a hard step, which WGSL's own smoothstep leaves undefined.
fn smoothstep1(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 == e0 {
        if x < e0 {
            return 0.0;
        }
        return 1.0;
    }
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn smoothstep4(a: vec4<f32>, b: vec4<f32>, c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(smoothstep1(a.x, b.x, c.x), smoothstep1(a.y, b.y, c.y), smoothstep1(a.z, b.z, c.z), smoothstep1(a.w, b.w, c.w));
}

fn cbrt1(x: f32) -> f32 {
    if x == 0.0 || !finite(x) {
        return x;
    }
    return sign(x) * pow(abs(x), 1.0 / 3.0);
}

fn cbrt4(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(cbrt1(v.x), cbrt1(v.y), cbrt1(v.z), cbrt1(v.w));
}

fn normalize4(v: vec4<f32>) -> vec4<f32> {
    let len = length(v);
    if len > 1e-12 {
        return v / len;
    }
    return vec4<f32>(0.0);
}

fn unit_sphere(u: f32, v: f32) -> vec3<f32> {
    let z = 1.0 - 2.0 * u;
    let r = sqrt(max(1.0 - z * z, 0.0));
    let phi = TAU * v;
    return vec3<f32>(r * cos(phi), r * sin(phi), z);
}

fn cone_direction(angle: f32, u: f32, v: f32) -> vec3<f32> {
    let cos_max = cos(clamp(angle, 0.0, PI));
    let cos_t = 1.0 - u * (1.0 - cos_max);
    let sin_t = sqrt(max(1.0 - cos_t * cos_t, 0.0));
    let phi = TAU * v;
    return vec3<f32>(sin_t * cos(phi), cos_t, sin_t * sin(phi));
}

fn unit_circle(x: f32) -> vec4<f32> {
    let a = TAU * x;
    return vec4<f32>(cos(a), sin(a), 0.0, 0.0);
}

// glam's normalize_or_zero and try_normalize().unwrap_or(Y).
fn normalize_or_zero(v: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if len > 0.0 && finite(1.0 / len) {
        return v / len;
    }
    return vec3<f32>(0.0);
}

fn normalize_or_y(v: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if len > 0.0 && finite(1.0 / len) {
        return v / len;
    }
    return vec3<f32>(0.0, 1.0, 0.0);
}

// ── sim/collide.rs ───────────────────────────────────────────────────────

struct Contact {
    hit: bool,
    at: vec3<f32>,
    normal: vec3<f32>,
};

// 0 no hit, 1 bounced, 2 killed.
struct Collided {
    hit: u32,
    p1: vec3<f32>,
    v: vec3<f32>,
};

fn no_contact() -> Contact {
    return Contact(false, vec3<f32>(0.0), vec3<f32>(0.0));
}

fn axis_unit(axis: u32) -> vec3<f32> {
    if axis == 0u {
        return vec3<f32>(1.0, 0.0, 0.0);
    } else if axis == 1u {
        return vec3<f32>(0.0, 1.0, 0.0);
    }
    return vec3<f32>(0.0, 0.0, 1.0);
}

fn respond(v_in: vec3<f32>, n: vec3<f32>, bounce_in: f32, friction_in: f32, accel_dt: vec3<f32>) -> vec3<f32> {
    let vn = dot(v_in, n);
    if vn >= 0.0 {
        return v_in;
    }
    var bounce = 0.0;
    if finite(bounce_in) {
        bounce = clamp(bounce_in, 0.0, 1.0);
    }
    var friction = 0.0;
    if finite(friction_in) {
        friction = max(friction_in, 0.0);
    }
    var tangent = v_in - n * vn;
    let rest = abs(dot(accel_dt, n)) * 1.5;
    var out = -vn * bounce;
    if -vn <= rest {
        out = 0.0;
    }
    let removed = out - vn;
    let t_len = length(tangent);
    if t_len > 1e-9 {
        tangent = tangent * (max(t_len - friction * removed, 0.0) / t_len);
    }
    return tangent + n * out;
}

fn plane_contact(point: vec3<f32>, normal: vec3<f32>, p1: vec3<f32>) -> Contact {
    let n = normalize_or_zero(normal);
    if all(n == vec3<f32>(0.0)) {
        return no_contact();
    }
    let d = dot(p1 - point, n);
    if d < 0.0 {
        return Contact(true, p1 - n * d, n);
    }
    return no_contact();
}

fn sphere_solid(center: vec3<f32>, r: f32, p0: vec3<f32>, p1: vec3<f32>) -> Contact {
    if r != r || r <= 0.0 {
        return no_contact();
    }
    let rel0 = p0 - center;
    if dot(rel0, rel0) < r * r {
        let rel1 = p1 - center;
        if dot(rel1, rel1) >= r * r {
            return no_contact();
        }
        let n = normalize_or_y(rel1);
        return Contact(true, center + n * r, n);
    }
    let d = p1 - p0;
    let a = dot(d, d);
    if a < 1e-20 {
        return no_contact();
    }
    let b = dot(rel0, d);
    let c = dot(rel0, rel0) - r * r;
    let disc = b * b - a * c;
    if disc < 0.0 {
        return no_contact();
    }
    let t = (-b - sqrt(disc)) / a;
    if !(t >= 0.0 && t <= 1.0) {
        return no_contact();
    }
    let at = p0 + d * t;
    let n = normalize_or_y(at - center);
    return Contact(true, center + n * r, n);
}

fn sphere_inside(center: vec3<f32>, r: f32, p1: vec3<f32>) -> Contact {
    let rel = p1 - center;
    let len = length(rel);
    if r != r || r <= 0.0 || len <= r {
        return no_contact();
    }
    let out = rel / len;
    return Contact(true, center + out * r, -out);
}

fn box_solid(center: vec3<f32>, half: vec3<f32>, p0: vec3<f32>, p1: vec3<f32>) -> Contact {
    if min(half.x, min(half.y, half.z)) <= 0.0 || !finite3(half) {
        return no_contact();
    }
    let lo = center - half;
    let hi = center + half;
    let inside0 = all(p0 > lo) && all(p0 < hi);
    if inside0 {
        let inside1 = all(p1 > lo) && all(p1 < hi);
        if !inside1 {
            return no_contact();
        }
        let to_min = p1 - lo;
        let to_max = hi - p1;
        var best_d = 3.4028235e38;
        var best_n = vec3<f32>(0.0);
        var best_at = p1;
        for (var axis = 0u; axis < 3u; axis = axis + 1u) {
            let unit = axis_unit(axis);
            if to_min[axis] < best_d {
                var at = p1;
                at[axis] = lo[axis];
                best_d = to_min[axis];
                best_n = -unit;
                best_at = at;
            }
            if to_max[axis] < best_d {
                var at = p1;
                at[axis] = hi[axis];
                best_d = to_max[axis];
                best_n = unit;
                best_at = at;
            }
        }
        return Contact(true, best_at, best_n);
    }
    let d = p1 - p0;
    var t0 = 0.0;
    var t1 = 1.0;
    var normal = vec3<f32>(0.0);
    for (var axis = 0u; axis < 3u; axis = axis + 1u) {
        if abs(d[axis]) < 1e-12 {
            if p0[axis] < lo[axis] || p0[axis] > hi[axis] {
                return no_contact();
            }
            continue;
        }
        let inv = 1.0 / d[axis];
        var ta = (lo[axis] - p0[axis]) * inv;
        var tb = (hi[axis] - p0[axis]) * inv;
        if ta > tb {
            let swap = ta;
            ta = tb;
            tb = swap;
        }
        if ta > t0 {
            t0 = ta;
            normal = axis_unit(axis) * -sign(d[axis]);
        }
        t1 = min(t1, tb);
        if t0 > t1 {
            return no_contact();
        }
    }
    if all(normal == vec3<f32>(0.0)) {
        return no_contact();
    }
    return Contact(true, p0 + d * t0, normal);
}

fn box_inside(center: vec3<f32>, size: vec3<f32>, kill: bool, bounce: f32, friction: f32, p1_in: vec3<f32>, v_in: vec3<f32>, accel_dt: vec3<f32>) -> Collided {
    let half = abs(size) * 0.5;
    if !finite3(half) {
        return Collided(0u, p1_in, v_in);
    }
    let lo = center - half;
    let hi = center + half;
    var p1 = p1_in;
    var v = v_in;
    var hit = false;
    for (var axis = 0u; axis < 3u; axis = axis + 1u) {
        let unit = axis_unit(axis);
        var n = vec3<f32>(0.0);
        if p1[axis] > hi[axis] {
            p1[axis] = hi[axis];
            n = -unit;
        } else if p1[axis] < lo[axis] {
            p1[axis] = lo[axis];
            n = unit;
        } else {
            continue;
        }
        hit = true;
        if !kill {
            v = respond(v, n, bounce, friction, accel_dt);
        }
    }
    if !hit {
        return Collided(0u, p1, v);
    }
    if kill {
        return Collided(2u, p1, v);
    }
    return Collided(1u, p1, v);
}

// shape: 0 plane, 1 sphere, 2 box.
fn collide(shape: u32, contain: bool, kill: bool, a: vec4<f32>, b: vec4<f32>, bounce: f32, friction: f32, p0: vec3<f32>, p1: vec3<f32>, v: vec3<f32>, accel_dt: vec3<f32>) -> Collided {
    var contact = no_contact();
    if shape == 0u {
        contact = plane_contact(a.xyz, b.xyz, p1);
    } else if shape == 1u {
        if contain {
            contact = sphere_inside(a.xyz, abs(b.x), p1);
        } else {
            contact = sphere_solid(a.xyz, abs(b.x), p0, p1);
        }
    } else {
        if contain {
            return box_inside(a.xyz, b.xyz, kill, bounce, friction, p1, v, accel_dt);
        }
        contact = box_solid(a.xyz, abs(b.xyz) * 0.5, p0, p1);
    }
    if !contact.hit {
        return Collided(0u, p1, v);
    }
    if kill {
        return Collided(2u, contact.at, v);
    }
    return Collided(1u, contact.at, respond(v, contact.normal, bounce, friction, accel_dt));
}

// A slot whose particle has died: marked free and handed back to the pool.
fn free_slot(slot: u32) {
    particles[slot].lifetime = -1.0;
    let k = atomicAdd(&state.counters[C_DEAD], 1u);
    state.words[k] = slot;
}

// ── sim/mod.rs's trail ring (ParticleBuffer::record_trail) ───────────────
// Per slot: [head, len, 0, 0], then `points` entries of (x, y, z, 0).

fn trail_start(slot: u32, slot_words: u32, at: vec3<f32>) {
    let tb = trail_base() + slot * slot_words;
    state.words[tb] = 0u;
    state.words[tb + 1u] = 1u;
    state.words[tb + 4u] = bitcast<u32>(at.x);
    state.words[tb + 5u] = bitcast<u32>(at.y);
    state.words[tb + 6u] = bitcast<u32>(at.z);
}

fn trail_record(slot: u32, slot_words: u32, points: u32, at: vec3<f32>, min_distance: f32) {
    let tb = trail_base() + slot * slot_words;
    let head = state.words[tb];
    let len = state.words[tb + 1u];
    let h = tb + 4u + head * 4u;
    let newest = vec3<f32>(bitcast<f32>(state.words[h]), bitcast<f32>(state.words[h + 1u]), bitcast<f32>(state.words[h + 2u]));
    let d = newest - at;
    if len > 0u && dot(d, d) < min_distance * min_distance {
        return;
    }
    let next = (head + 1u) % points;
    let w = tb + 4u + next * 4u;
    state.words[w] = bitcast<u32>(at.x);
    state.words[w + 1u] = bitcast<u32>(at.y);
    state.words[w + 2u] = bitcast<u32>(at.z);
    state.words[tb] = next;
    state.words[tb + 1u] = min(len + 1u, points);
}

// ── sim/sub.rs ───────────────────────────────────────────────────────────

// interval_events: the k with k * every in (before, after], at most
// MAX_INTERVAL_EVENTS, as [x, y).
fn interval_range(before: f32, after: f32, every: f32) -> vec2<u32> {
    if !finite(every) || every <= 0.0 || !finite(before) || !finite(after) || after <= before {
        return vec2<u32>(0u);
    }
    let first = floor(before / every) + 1.0;
    var last = floor(after / every);
    if last < first || first < 1.0 {
        return vec2<u32>(0u);
    }
    last = min(last, first + f32(MAX_INTERVAL_EVENTS) - 1.0);
    return vec2<u32>(u32(min(first, 4.0e9)), u32(min(last + 1.0, 4.0e9)));
}

// A child's spawn index, from what spawned it (see codegen.rs's module docs).
fn child_id(parent: u32, key: u32, index: u32, k: u32) -> u32 {
    return pcg(pcg(pcg(pcg(parent ^ 0x5BD1E995u) ^ key) ^ index) ^ k);
}

fn event_vec3(e: u32) -> vec3<f32> {
    return vec3<f32>(bitcast<f32>(atomicLoad(&events[e])), bitcast<f32>(atomicLoad(&events[e + 1u])), bitcast<f32>(atomicLoad(&events[e + 2u])));
}

fn event_put3(e: u32, v: vec3<f32>) {
    atomicStore(&events[e], bitcast<u32>(v.x));
    atomicStore(&events[e + 1u], bitcast<u32>(v.y));
    atomicStore(&events[e + 2u], bitcast<u32>(v.z));
}

// One event into the inbox of the rule whose (base, capacity, count, key)
// is data entry `rule`, already in the target's simulation space. A full
// inbox takes nothing and counts the children it refused, for the target to
// add to its drops; the count keeps rising past the capacity, and `deliver`
// reads only the first `capacity` entries, so nothing is written past it.
fn raise_event(rule: u32, pos: vec3<f32>, vel: vec3<f32>, color: vec4<f32>, flags: u32, parent: u32, index: u32) {
    let m = bitcast<vec4<u32>>(data[rule]);
    let base = m.x;
    let count = m.z;
    if count == 0u {
        return;
    }
    let k = atomicAdd(&events[base], 1u);
    if k >= m.y {
        atomicAdd(&events[base + 1u], count);
        return;
    }
    let e = base + EVENT_HEADER_WORDS + k * EVENT_WORDS;
    event_put3(e, san3(pos));
    atomicStore(&events[e + 3u], count);
    event_put3(e + 4u, san3(vel));
    atomicStore(&events[e + 7u], flags);
    event_put3(e + 8u, color.xyz);
    atomicStore(&events[e + 11u], bitcast<u32>(color.w));
    atomicStore(&events[e + 12u], parent);
    atomicStore(&events[e + 13u], m.w);
    atomicStore(&events[e + 14u], index);
}

fn point3(m: mat4x4<f32>, p: vec3<f32>) -> vec3<f32> {
    return (m * vec4<f32>(p, 1.0)).xyz;
}

fn vector3(m: mat4x4<f32>, v: vec3<f32>) -> vec3<f32> {
    return (m * vec4<f32>(v, 0.0)).xyz;
}

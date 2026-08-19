// Instanced grass.
//
// One draw call per chunk per foliage type, `VERTS_PER_BLADE` vertices by
// `blade_count` instances. There is no vertex or index buffer for the blade
// itself: the strip is rebuilt here from `vertex_index`, so a blade costs only
// its 48-byte instance record. See `instance.rs` — `BLADE_SEGMENTS` below must
// match the constant there.

#import bevy_render::view::View

const BLADE_SEGMENTS: u32 = 4u;

@group(0) @binding(0) var<uniform> view: View;

struct GrassParams {
    // xyz = chunk origin in world space, w = this layer's wind strength
    origin_wind: vec4<f32>,
    // xy = world wind direction, z = time in seconds, w = world wind strength
    wind_dir_time: vec4<f32>,
    // x = gust depth, y = gusts/sec, z = turbulence, w unused
    wind_gust: vec4<f32>,
    color_base: vec4<f32>,
    color_tip: vec4<f32>,
};

@group(1) @binding(0) var<uniform> grass: GrassParams;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    // v along the blade: 0 at the base, 1 at the tip.
    @location(2) t: f32,
    @location(3) color_var: f32,
};

// Rebuild one vertex of the blade strip from its index.
//
// The strip is a non-indexed triangle list: per segment `s` the six vertices are
// the quad (2s, 2s+1, 2s+2, 2s+1, 2s+3, 2s+2), where an even template index is
// the blade's left edge and an odd one its right. Unpacked as a (row, side)
// pair, that is the table below.
fn strip_vertex(vertex_index: u32) -> vec2<u32> {
    let segment = vertex_index / 6u;
    let corner = vertex_index % 6u;
    var row_offset = 0u;
    var side = 0u;
    switch corner {
        case 0u: { row_offset = 0u; side = 0u; }
        case 1u: { row_offset = 0u; side = 1u; }
        case 2u: { row_offset = 1u; side = 0u; }
        case 3u: { row_offset = 0u; side = 1u; }
        case 4u: { row_offset = 1u; side = 1u; }
        default: { row_offset = 1u; side = 0u; }
    }
    return vec2<u32>(segment + row_offset, side);
}

const TAU: f32 = 6.2831853;

/// Blade height that gets a wind response of exactly 1.0.
///
/// The midpoint of `FoliageType::default().height_range`, picked so a scene that
/// never touches the height brushes keeps the motion it already had.
const WIND_REFERENCE_HEIGHT: f32 = 0.25;

/// Three decorrelated randoms in 0..1 from a point on the XZ plane.
///
/// Sine-free on purpose: the classic `fract(sin(x) * 43758.5)` returns visibly
/// different numbers on different vendors, because some drivers evaluate `sin`
/// at reduced precision once the argument gets large — and a field of grass that
/// is laid out differently per GPU is not something anyone can debug. This is
/// the integer-free bit-mixing hash instead, which is stable everywhere.
///
/// Three at once rather than three calls: the wind wants several independent
/// per-blade streams and this is barely more expensive than one.
fn hash23(p_in: vec2<f32>) -> vec3<f32> {
    var p = fract(vec3<f32>(p_in.x, p_in.y, p_in.x) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p = p + vec3<f32>(dot(p, p.yxz + vec3<f32>(33.33)));
    return fract((p.xxy + p.yzz) * p.zyx);
}

fn hash21(p_in: vec2<f32>) -> f32 {
    var p = fract(vec3<f32>(p_in.x, p_in.y, p_in.x) * 0.1031);
    p = p + vec3<f32>(dot(p, p.yzx + vec3<f32>(33.33)));
    return fract((p.x + p.y) * p.z);
}

/// Smoothed value noise on the XZ plane, 0..1 with a mean near 0.5.
///
/// Value rather than gradient noise: the gust field only has to be *blobby*, and
/// nothing here differentiates it, so the extra cost of proper band-limiting
/// buys nothing visible.
fn vnoise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@vertex
fn vertex(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position_height: vec4<f32>,
    @location(1) width_phase_bend_var: vec4<f32>,
    @location(2) lean_rotation: vec4<f32>,
) -> VertexOut {
    let blade_pos = position_height.xyz;
    let blade_height = position_height.w;
    let blade_width = width_phase_bend_var.x;
    let phase = width_phase_bend_var.y;
    let bend = width_phase_bend_var.z;
    let color_var = width_phase_bend_var.w;
    let lean_x = lean_rotation.x;
    let lean_z = lean_rotation.y;
    let rot_sin = lean_rotation.z;
    let rot_cos = lean_rotation.w;

    let strip = strip_vertex(vertex_index);
    let t = f32(strip.x) / f32(BLADE_SEGMENTS);

    // Tapers to 40% of the base rather than to a point — a blade that narrows to
    // nothing reads as a spike, and the taper is pure lost coverage.
    let half_width = 0.5 * (1.0 - t * 0.6) * blade_width;
    let x = select(-half_width, half_width, strip.y == 1u);
    let y = t * blade_height;
    let z = t * t * 0.02 * blade_height; // slight forward curve

    // Rotate around Y, then place in the chunk and in the world.
    let rx = x * rot_cos - z * rot_sin;
    let rz = x * rot_sin + z * rot_cos;
    let world_pos = grass.origin_wind.xyz + blade_pos + vec3<f32>(rx, y, rz);

    // ── Wind model ────────────────────────────────────────────────────────
    // Driven by the world wind (`renzora::WindState`), so a blade of grass and
    // the tree behind it lean the same way and gust at the same moment.
    //
    // `gust_wave` below is a copy of `wind_gust` in
    // `renzora_wind/src/wind_common.wgsl` and must stay in step with it. It is
    // copied rather than imported because that module declares its uniform in
    // the *material* bind group, which this hand-written pipeline does not have.
    // Everything layered on top of it here is grass-only and has no counterpart
    // there — a tree is one object swaying as a unit, where a field is a hundred
    // thousand of them and reads as a pattern the moment they agree too closely.
    let wind_dir = grass.wind_dir_time.xy;
    let time = grass.wind_dir_time.z;
    let world_strength = grass.wind_dir_time.w;
    let layer_strength = grass.origin_wind.w;
    let gust_depth = grass.wind_gust.x;
    let gust_freq = grass.wind_gust.y;
    let turbulence = grass.wind_gust.z;

    // Per-blade randoms, hashed from where the blade stands rather than taken
    // from `phase`: the CPU spends `phase` on the blade's yaw and its colour as
    // well, so reusing it would tie a blade's timing to which way it faces.
    //
    // `rate` is the important one. Phase offsets alone are not variety — a field
    // of oscillators that all run at ONE frequency and differ only in phase
    // returns to its starting arrangement every 2π/ω, and what you see is the
    // whole field breathing in and out on a fixed period. Give each blade its
    // own speed and the arrangement drifts apart and never lands back on itself.
    let blade_world = grass.origin_wind.xyz + blade_pos;
    let rnd = hash23(blade_world.xz * 1.7);
    let rate = 0.7 + 0.6 * rnd.x;
    let amp_var = 0.75 + 0.5 * rnd.y;

    // The coherent gust envelope: a front sweeping downwind, shared with trees.
    let gust_travel = dot(world_pos.xz, wind_dir) * 0.02;
    let gust_phase = time * gust_freq * TAU - gust_travel;
    let gust_wave = 0.5 + 0.5 * (sin(gust_phase) * 0.6 + sin(gust_phase * 0.37 + 1.7) * 0.4);

    // ...and the patchiness on top of it. On its own that envelope is a function
    // of distance along the wind and nothing else, so its fronts are perfectly
    // straight bands running to the horizon — in a large field that is the most
    // obvious "this is a shader" tell there is. Two octaves of noise, advected
    // downwind, break the band into travelling cells: the envelope still decides
    // WHEN a surge happens, the noise decides WHERE it lands. The cells average
    // 0.5, so the pair averages back to `gust_wave` and the overall strength is
    // unchanged.
    let flow = world_pos.xz - wind_dir * time * 5.0;
    let gust_cells = vnoise2(flow * 0.055) * 0.65
        + vnoise2(flow * 0.017 + vec2<f32>(19.0, 7.0)) * 0.35;
    let gust = gust_wave * (0.35 + 1.3 * gust_cells);

    // Instantaneous strength for this blade. `layer_strength` is the foliage
    // layer's own stiffness (authored per grass type); `world_strength` is how
    // hard it is blowing right now.
    let wind_strength = world_strength * layer_strength * (1.0 + gust * gust_depth);

    // Medium turbulence (per-blade). Scaled by the world turbulence knob so a
    // steady wind lays the whole field over cleanly and a turbulent one churns.
    // Each carries the blade's own `rate`, so neighbours beat against each other
    // instead of tracing one curve a few hundred milliseconds apart.
    let turb1 = sin(time * 1.8 * rate + phase + world_pos.x * 0.25 + world_pos.z * 0.15) * turbulence;
    let turb2 = sin(time * 2.3 * rate + phase * 1.3 + world_pos.z * 0.3) * turbulence;
    let turb3 = sin(time * 1.1 * rate + phase * 0.7 + world_pos.x * 0.18) * turbulence;

    // High-frequency flutter (tip only). Its rate is jittered per blade too —
    // this is the fastest term in the model, so a shared frequency here is the
    // one that shows up as a visible shimmer crawling across the field.
    let flutter = sin(time * (4.5 + 2.5 * rnd.y) + phase * 4.0 + rnd.z * TAU) * 0.02 * t;

    // Cubic falloff from base to tip, scaled by flexibility
    let bend_factor = bend * 0.7 + 0.3;
    let wind_pow = t * t * (3.0 - 2.0 * t); // smoothstep

    // How far the blade leans scales with how long it is. It did not used to:
    // the offset below is in absolute metres, so a blade twice as tall travelled
    // the same distance and read as twice as stiff. That was survivable while
    // every blade of a type sat inside one narrow authored range, and stopped
    // being survivable the moment the Grow / Trim brushes could stand a 3x blade
    // next to a neutral one — which is the whole point of those brushes.
    //
    // Clamped at both ends: a mown blade should still stir, and a 2 m reed
    // should sweep, not fold flat.
    let height_response = clamp(blade_height / WIND_REFERENCE_HEIGHT, 0.35, 4.0);

    // The steady push never reverses — wind blows one way — so the blade
    // oscillates around a laid-over pose rather than swinging back upright
    // through vertical, which is what a plain sine would do. Two incommensurate
    // sines rather than one, so the sum has no period short enough to read, and
    // `phase` spans the whole turn on both: it used to be halved on the single
    // term that mattered, which left every blade within half a cycle of its
    // neighbour and set the entire field pulsing in near-unison.
    let sway_t = time * 1.2 * rate;
    let s1 = sin(sway_t + phase);
    let s2 = sin(sway_t * 0.53 + phase * 1.7 + rnd.z * TAU);
    let push = (0.55 + 0.45 * (s1 * 0.65 + s2 * 0.35)) * amp_var;

    // Blades don't all lie down along exactly the same heading. The yaw spread
    // opens with the turbulence knob, so a steady wind still combs the field one
    // way while a gusty one scatters it.
    let yaw = (rnd.z - 0.5) * (0.25 + 0.55 * turbulence);
    let cy = cos(yaw);
    let sy = sin(yaw);
    let bend_dir = vec2<f32>(
        wind_dir.x * cy - wind_dir.y * sy,
        wind_dir.x * sy + wind_dir.y * cy,
    );

    let wind_x = (bend_dir.x * push
                + turb1 * 0.35 + turb3 * 0.18
                + flutter) * wind_pow * bend_factor * wind_strength * height_response;
    let wind_z = (bend_dir.y * push
                + turb2 * 0.25
                + flutter * 0.7) * wind_pow * bend_factor * wind_strength * height_response;
    let displaced = vec3<f32>(world_pos.x + wind_x, world_pos.y, world_pos.z + wind_z);

    // Normal: perpendicular to the blade, tilted by wind + lean.
    let total_x = wind_x + lean_x * t * blade_height * 3.0;
    let total_z = wind_z + lean_z * t * blade_height * 3.0;
    let blade_normal = normalize(vec3<f32>(-total_x * 0.5, 1.0, -total_z * 0.5));

    var out: VertexOut;
    out.clip_position = view.clip_from_world * vec4<f32>(displaced, 1.0);
    out.world_position = displaced;
    out.world_normal = blade_normal;
    out.t = t;
    out.color_var = color_var;
    return out;
}

@fragment
fn fragment(in: VertexOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let t = in.t;

    // Height-based color gradient
    let base_col = grass.color_base.rgb;
    let tip_col = grass.color_tip.rgb;
    let mid_col = mix(base_col, tip_col, 0.5);

    var albedo: vec3<f32>;
    if (t < 0.5) {
        albedo = mix(base_col, mid_col, t * 2.0);
    } else {
        albedo = mix(mid_col, tip_col, (t - 0.5) * 2.0);
    }

    // Per-blade hue variation
    albedo += vec3<f32>(in.color_var * 0.7, in.color_var, in.color_var * 0.3);

    // Ambient occlusion at base
    albedo *= 0.5 + t * 0.5;

    // Subsurface scattering: tips glow when backlit
    let view_dir = normalize(in.world_position - view.world_position);
    let back_light = max(dot(normalize(vec3<f32>(0.3, -0.8, 0.5)), view_dir), 0.0);
    albedo += vec3<f32>(0.06, 0.1, 0.02) * t * t * back_light;

    // Slight yellow at dry tips
    albedo += vec3<f32>(0.03, 0.02, -0.01) * t * t;

    // Blades are drawn double-sided (a one-sided blade simply vanishes from
    // behind, which is half of them at any moment), so the back face has to be
    // lit against the flipped normal or it reads as a black cut-out.
    let normal = select(-in.world_normal, in.world_normal, front_facing);

    let light_dir = normalize(vec3<f32>(0.3, -0.8, 0.5));
    let ndotl = max(dot(normal, -light_dir), 0.0);
    let ambient = 0.35;
    let lit = albedo * (ambient + (1.0 - ambient) * ndotl);

    return vec4<f32>(lit, 1.0);
}

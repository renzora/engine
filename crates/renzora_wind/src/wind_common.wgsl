#define_import_path renzora_wind::common

// The wind displacement model, shared by the forward and prepass vertex
// entry points so depth, shadows and the lit pass can never disagree about
// where a vertex is. They *must* agree: leaves are `AlphaMode::Mask`, so they
// write depth in the prepass, and a vertex that lands in a different place in
// the two passes z-fights against its own prepass sample and flickers.
//
// Time is a parameter rather than read from `globals` in here, because the
// globals uniform sits at a DIFFERENT BINDING in the two pipelines —
// `@group(0) @binding(11)` in the forward mesh-view layout, `@group(0)
// @binding(1)` in the prepass layout. Importing it into a shared module would
// bake one of those in and fail to bind in the other pass.

struct WindParams {
    // xy = unit travel direction on the XZ plane, z = sustained strength
    // (1.0 at renzora::wind::REFERENCE_WIND_SPEED), w = gust depth 0..1.
    dir_strength: vec4<f32>,
    // x = gusts per second, y = turbulence 0..1, z = this mesh's response
    // multiplier, w = this mesh's flutter multiplier.
    gust_turb: vec4<f32>,
    // x = sway amplitude in metres (how far the floppiest vertex travels at
    // reference wind), y = fallback pivot height in metres, zw unused.
    misc: vec4<f32>,
};

// `MATERIAL_BIND_GROUP` resolves to 3 in the forward pipeline and 2 in the
// prepass one. Writing the literal would work in exactly one of them.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> wind: WindParams;

const WIND_TAU: f32 = 6.2831853;

/// The gust envelope, 0..1. Two incommensurate sines rather than one, so the
/// swell never repeats on an audible period, plus a spatial term along the wind
/// so a gust reads as a front sweeping across the landscape instead of every
/// tree in the level surging in unison.
///
/// `renzora_wind::gust_envelope` mirrors this in Rust for the CPU consumers
/// (cloth, particles) with the spatial term at zero. Keep the two in step.
fn wind_gust(world_pos: vec3<f32>, t: f32) -> f32 {
    let travel = dot(world_pos.xz, wind.dir_strength.xy) * 0.02;
    let p = t * wind.gust_turb.x * WIND_TAU - travel;
    return 0.5 + 0.5 * (sin(p) * 0.6 + sin(p * 0.37 + 1.7) * 0.4);
}

/// World-space offset for one vertex.
///
/// * `object_pos` — the object's origin in world space. Every vertex of one
///   plant shares it, which is what makes a tree sway as one object while the
///   tree next to it is out of phase. Deriving the phase from the *vertex*
///   position instead would shear each plant apart internally.
/// * `sway` — 0 at the rigid root, 1 at the floppiest tip. Authored into
///   `UV_1.x` by the tree mesh generator; falls back to a height ramp.
/// * `flutter` — extra high-frequency motion, 0 on woody geometry and 1 at a
///   leaf's outer edge. `UV_1.y`.
fn wind_displace(
    object_pos: vec3<f32>,
    world_pos: vec3<f32>,
    sway: f32,
    flutter: f32,
    t: f32,
) -> vec3<f32> {
    let strength = wind.dir_strength.z * wind.gust_turb.z;
    if strength <= 0.0 {
        return vec3<f32>(0.0);
    }

    let dir = normalize(vec3<f32>(wind.dir_strength.x, 0.0, wind.dir_strength.y));
    // Horizontal perpendicular. Wind is always level, so this is exact — no
    // need for a cross product against an up vector that can degenerate.
    let side = vec3<f32>(-dir.z, 0.0, dir.x);

    let gust = wind_gust(world_pos, t);
    let gusting = strength * (1.0 + gust * wind.dir_strength.w);
    let amplitude = wind.misc.x;

    // Per-plant phase. Irrational-ish coefficients so a grid-planted forest
    // doesn't land on a repeating pattern.
    let phase = dot(object_pos.xz, vec2<f32>(0.713, 0.431));

    // Branch sway. Harder wind sways faster as well as further — a gale does
    // not move a tree slowly. The downwind term never goes negative: wind
    // pushes, it does not pull, so the plant oscillates *around* a bent pose
    // rather than swinging back through vertical.
    let sway_t = t * (0.9 + 0.6 * gusting);
    let a = sin(sway_t + phase);
    let b = sin(sway_t * 1.61 + phase * 1.7);
    let bend = dir * ((0.65 + 0.35 * a) * gusting * sway * amplitude);
    let wobble = side * (b * gusting * sway * amplitude * wind.gust_turb.y * 0.6);

    // Leaf flutter: fast, small, and mostly across the leaf's own plane. Phase
    // comes off the vertex here (not the object) because neighbouring leaves
    // on one branch should flutter independently — that incoherence is the
    // whole visual point.
    let fphase = phase * 3.1 + dot(world_pos.xz, vec2<f32>(3.7, 2.9));
    let ft = t * (7.0 + 5.0 * gusting);
    let f = sin(ft + fphase) * cos(ft * 0.63 + fphase * 1.3);
    let flut = (vec3<f32>(0.0, 1.0, 0.0) + side * 0.6)
        * (f * flutter * wind.gust_turb.w * gusting * amplitude * 0.35);

    // Bending a branch swings its tip along an arc, so lateral travel has to
    // buy a little drop. Without it a hard gust visibly *stretches* the plant.
    let lateral = bend + wobble;
    let droop = vec3<f32>(0.0, -dot(lateral, lateral) * 0.25, 0.0);

    return lateral + flut + droop;
}

/// Pick the sway/flutter weights for a vertex.
///
/// Authored weights live in `UV_1` (see the tree mesh generator). `UV_1` is
/// used rather than vertex colour because StandardMaterial multiplies base
/// colour by `VERTEX_COLORS` unconditionally — storing a mask there would tint
/// every leaf by its own stiffness. Nothing reads `UV_1` unless a texture slot
/// is explicitly set to `UvChannel::Uv1`, so it is free real estate.
///
/// Meshes with no `UV_1` (an imported bush, a hand-modelled palm) get a height
/// ramp off the local position instead: rigid at the object origin, fully
/// flexible at `pivot_height`. Squared, because a trunk should stay noticeably
/// stiffer than a linear ramp makes it.
fn wind_weights(local_pos: vec3<f32>, uv_b: vec2<f32>, has_uv_b: bool) -> vec2<f32> {
    if has_uv_b {
        return uv_b;
    }
    let h = clamp(local_pos.y / max(wind.misc.y, 0.001), 0.0, 1.0);
    return vec2<f32>(h * h, 0.0);
}

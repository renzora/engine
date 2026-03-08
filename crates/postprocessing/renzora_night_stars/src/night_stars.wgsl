// Night Stars — Procedural starfield on a sky dome
//
// Uses a 3D voxel grid for star placement, which is completely seam-free
// (no UV discontinuities). Each voxel on the sphere surface may contain one
// randomly-placed star, rendered as a soft glowing point with optional twinkling.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

// density, brightness, star_size, twinkle_speed
@group(3) @binding(0) var<uniform> params_a: vec4<f32>;
// twinkle_amount, horizon_fade, unused, unused
@group(3) @binding(1) var<uniform> params_b: vec4<f32>;
// r, g, b, unused
@group(3) @binding(2) var<uniform> star_color: vec4<f32>;

const TAU: f32 = 6.28318530718;

fn hash31(p: vec3<f32>) -> f32 {
    var h = fract(p * 0.1031);
    h += dot(h, h.yzx + 33.33);
    return fract((h.x + h.y) * h.z);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        hash31(p),
        hash31(p + vec3<f32>(13.71, 5.37, 9.11)),
        hash31(p + vec3<f32>(31.30, 17.10, 3.71)),
    );
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let density      = params_a.x;
    let brightness   = params_a.y;
    let star_size    = params_a.z;
    let twinkle_spd  = params_a.w;
    let twinkle_amt  = params_b.x;
    let horizon_fade = params_b.y;

    let dir = normalize(in.world_position.xyz);

    // Discard fragments below the horizon
    if dir.y < -0.02 {
        discard;
    }

    // Smooth fade near the horizon
    let horizon_mask = smoothstep(0.0, max(horizon_fade, 0.005), dir.y);

    // Grid resolution: controls how densely stars are packed
    // Higher density → more voxel cells → more stars
    let grid_res = 8.0 + density * 22.0;

    // Angular radius of each star in radians
    // star_size 1.0 → ~0.003 rad (~0.17°, roughly 2-3px diameter at 1080p 90° FOV)
    let angular_radius = star_size * 0.003;

    let base_cell = floor(dir * grid_res);
    var out_val: f32 = 0.0;

    // Check 3×3×3 cell neighborhood to catch stars in adjacent voxels
    for (var dx: i32 = -1; dx <= 1; dx += 1) {
        for (var dy: i32 = -1; dy <= 1; dy += 1) {
            for (var dz: i32 = -1; dz <= 1; dz += 1) {
                let cell = base_cell + vec3<f32>(f32(dx), f32(dy), f32(dz));

                // Per-cell decision: does this cell contain a star?
                let rng = hash31(cell + vec3<f32>(71.31, 11.71, 43.13));
                if rng > density {
                    continue;
                }

                // Random star direction: offset within voxel, projected to unit sphere
                let rnd = hash33(cell);
                let star_dir = normalize(cell + rnd);

                // Only upper-hemisphere stars
                if star_dir.y < 0.0 {
                    continue;
                }

                // Angular distance via cross-product magnitude (sin ≈ angle for small angles)
                let cross_len = length(cross(dir, star_dir));

                // Early discard: fragment is outside this star's radius
                if cross_len >= angular_radius {
                    continue;
                }

                // Soft quadratic falloff from center to edge
                let t = 1.0 - cross_len / angular_radius;
                let contrib = t * t;

                if contrib > 0.001 {
                    // Per-star brightness variation (makes some stars brighter than others)
                    let star_br = 0.2 + hash31(cell + vec3<f32>(2.13, 9.37, 5.71)) * 0.8;

                    // Twinkling: each star has a unique phase offset
                    let phase = hash31(cell) * TAU;
                    let twinkle = 1.0
                        - twinkle_amt * 0.5
                        + twinkle_amt * 0.5 * sin(globals.time * twinkle_spd + phase);

                    out_val = max(out_val, contrib * star_br * twinkle);
                }
            }
        }
    }

    if out_val < 0.001 {
        discard;
    }

    let final_alpha = out_val * horizon_mask;
    if final_alpha < 0.001 {
        discard;
    }

    return vec4<f32>(star_color.rgb * brightness * out_val, final_alpha);
}

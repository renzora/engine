// Procedural Clouds Shader
// Renders volumetric-looking clouds on a sky dome using FBM noise

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

// Cloud parameters (group 3 = material bind group in Bevy 0.18)
@group(3) @binding(0) var<uniform> params_a: vec4<f32>;  // coverage, density, scale, speed
@group(3) @binding(1) var<uniform> params_b: vec4<f32>;  // wind_dir_x, wind_dir_y, altitude, unused
@group(3) @binding(2) var<uniform> cloud_color: vec4<f32>;
@group(3) @binding(3) var<uniform> shadow_color: vec4<f32>;

const PI: f32 = 3.14159265359;

// Hash-based 2D value noise
fn hash2(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// 2x2 rotation matrix for inter-octave rotation (reduces axis-aligned artifacts)
fn rot2(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, s, -s, c);
}

// Fractional Brownian Motion — 4 octaves with rotation
fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    let r = rot2(0.8);

    for (var i = 0; i < 4; i = i + 1) {
        value = value + amplitude * noise2d(p);
        p = r * p * 2.0 + vec2<f32>(100.0);
        amplitude *= 0.5;
    }
    return value;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let coverage = params_a.x;
    let density = params_a.y;
    let scale = params_a.z;
    let speed = params_a.w;
    let wind_dir = vec2<f32>(params_b.x, params_b.y);
    let altitude = params_b.z;

    // Direction from origin to this fragment on the dome
    let dir = normalize(in.world_position.xyz - vec3<f32>(0.0));

    // Discard below horizon
    if dir.y < 0.01 {
        discard;
    }

    // Altitude masking: fade clouds around the altitude threshold
    let alt_fade = smoothstep(altitude * 0.3, altitude * 0.3 + 0.25, dir.y);
    let alt_top_fade = smoothstep(0.95, 0.85, dir.y); // fade out near zenith
    let altitude_mask = alt_fade * alt_top_fade;

    if altitude_mask < 0.001 {
        discard;
    }

    // Project dome direction to 2D UV for noise sampling
    // Divide xz by y for natural hemisphere mapping (stretches near horizon)
    let uv = dir.xz / dir.y * scale;

    // Animate with wind
    let time = globals.time;
    let wind_offset = wind_dir * time * speed;
    let animated_uv = uv + wind_offset;

    // Primary FBM for cloud shape
    let noise_val = fbm(animated_uv);

    // Coverage remap: smoothstep to control cloud/clear boundary
    let edge_low = 1.0 - coverage;
    let edge_high = edge_low + 0.3;
    let cloud_shape = smoothstep(edge_low, edge_high, noise_val);

    if cloud_shape < 0.001 {
        discard;
    }

    // Detail pass: second FBM at higher frequency for subtle variation
    let detail = fbm(animated_uv * 2.5 + vec2<f32>(50.0, 50.0));
    let detail_brightness = 0.85 + detail * 0.3;

    // Final alpha
    let alpha = cloud_shape * density * altitude_mask;

    if alpha < 0.001 {
        discard;
    }

    // Light scattering approximation: blend between shadow (bottom) and lit (top)
    // Uses the vertical component of the noise sampling direction
    let scatter_factor = smoothstep(0.0, 0.6, dir.y);
    let lit_color = mix(shadow_color.rgb, cloud_color.rgb, scatter_factor);

    // Apply detail brightness variation
    let final_color = lit_color * detail_brightness;

    return vec4<f32>(final_color, alpha);
}

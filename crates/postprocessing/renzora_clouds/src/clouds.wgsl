// Procedural Clouds Shader — Realistic volumetric-look with Beer/powder lighting
// Rendered on a sky dome mesh using layered FBM noise with domain warping

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

@group(3) @binding(0) var<uniform> params_a: vec4<f32>;  // coverage, density, scale, speed
@group(3) @binding(1) var<uniform> params_b: vec4<f32>;  // wind_dir_x, wind_dir_y, altitude, unused
@group(3) @binding(2) var<uniform> cloud_color: vec4<f32>;
@group(3) @binding(3) var<uniform> shadow_color: vec4<f32>;
@group(3) @binding(4) var<uniform> params_c: vec4<f32>;  // sun_dir.x, sun_dir.y, sun_dir.z, absorption
@group(3) @binding(5) var<uniform> params_d: vec4<f32>;  // silver_intensity, silver_spread, powder_strength, ambient_brightness
@group(3) @binding(6) var<uniform> horizon_color: vec4<f32>; // rgb = haze color, a = atmosphere_strength

const PI: f32 = 3.14159265359;

// ── Gradient noise (quintic interpolation) ──

fn hash_grad(p: vec2<f32>) -> vec2<f32> {
    let k = vec2<f32>(
        dot(p, vec2<f32>(127.1, 311.7)),
        dot(p, vec2<f32>(269.5, 183.3)),
    );
    return fract(sin(k) * 43758.5453) * 2.0 - 1.0;
}

fn grad_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    // Quintic interpolation: 6t^5 - 15t^4 + 10t^3
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);

    let g00 = dot(hash_grad(i + vec2<f32>(0.0, 0.0)), f - vec2<f32>(0.0, 0.0));
    let g10 = dot(hash_grad(i + vec2<f32>(1.0, 0.0)), f - vec2<f32>(1.0, 0.0));
    let g01 = dot(hash_grad(i + vec2<f32>(0.0, 1.0)), f - vec2<f32>(0.0, 1.0));
    let g11 = dot(hash_grad(i + vec2<f32>(1.0, 1.0)), f - vec2<f32>(1.0, 1.0));

    return mix(mix(g00, g10, u.x), mix(g01, g11, u.x), u.y) * 0.5 + 0.5;
}

// ── Rotation matrix for inter-octave rotation ──

fn rot2(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, s, -s, c);
}

// ── Standard FBM (6 octaves) ──

fn fbm6(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    let r = rot2(0.8);
    for (var i = 0; i < 6; i = i + 1) {
        value += amplitude * grad_noise(p);
        p = r * p * 2.0 + vec2<f32>(100.0);
        amplitude *= 0.5;
    }
    return value;
}

// ── Ridged FBM (4 octaves) — produces cumulus billow shapes ──

fn fbm_ridged(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    let r = rot2(1.2);
    for (var i = 0; i < 4; i = i + 1) {
        let n = grad_noise(p);
        value += amplitude * (1.0 - abs(n * 2.0 - 1.0));
        p = r * p * 2.0 + vec2<f32>(37.0);
        amplitude *= 0.5;
    }
    return value;
}

// ── Quick 3-octave FBM for domain warping ──

fn fbm3(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    let r = rot2(0.6);
    for (var i = 0; i < 3; i = i + 1) {
        value += amplitude * grad_noise(p);
        p = r * p * 2.0 + vec2<f32>(50.0);
        amplitude *= 0.5;
    }
    return value;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Unpack uniforms
    let coverage = params_a.x;
    let density = params_a.y;
    let scale = params_a.z;
    let speed = params_a.w;
    let wind_dir = vec2<f32>(params_b.x, params_b.y);
    let altitude = params_b.z;
    let sun_dir = normalize(vec3<f32>(params_c.x, params_c.y, params_c.z));
    let absorption = params_c.w;
    let silver_intensity = params_d.x;
    let silver_spread = params_d.y;
    let powder_strength = params_d.z;
    let ambient_brightness = params_d.w;
    let haze_color = horizon_color.rgb;
    let atmosphere_strength = horizon_color.a;

    // Direction from origin to this fragment on the dome
    let dir = normalize(in.world_position.xyz);

    // Discard below horizon
    if dir.y < 0.01 {
        discard;
    }

    // Altitude masking
    let alt_fade = smoothstep(altitude * 0.3, altitude * 0.3 + 0.25, dir.y);
    let alt_top_fade = smoothstep(0.95, 0.85, dir.y);
    let altitude_mask = alt_fade * alt_top_fade;

    if altitude_mask < 0.001 {
        discard;
    }

    // Project dome to 2D UV
    let uv = dir.xz / dir.y * scale;
    let time = globals.time;
    let wind_offset = wind_dir * time * speed;
    let animated_uv = uv + wind_offset;

    // ── Domain warping for organic shapes ──
    let warp = vec2<f32>(
        fbm3(animated_uv + vec2<f32>(0.0, 0.0)),
        fbm3(animated_uv + vec2<f32>(5.2, 1.3)),
    );
    let warped_uv = animated_uv + warp * 0.35;

    // ── Cloud shape composition ──
    let base = fbm6(warped_uv);
    let billows = fbm_ridged(warped_uv * 2.0);
    let shape = base * 0.6 + billows * 0.4;

    // Coverage remap
    let edge_low = 1.0 - coverage;
    let edge_high = edge_low + 0.25;
    let cloud_shape = smoothstep(edge_low, edge_high, shape);

    if cloud_shape < 0.001 {
        discard;
    }

    // Fine detail erosion
    let detail = fbm3(warped_uv * 6.0 + vec2<f32>(77.0, 33.0));
    let eroded = saturate(cloud_shape - detail * 0.15);

    if eroded < 0.001 {
        discard;
    }

    // Height fraction within cloud layer (0 = base, 1 = top)
    let height_fraction = smoothstep(altitude * 0.3, 0.85, dir.y);

    // ── Lighting model ──

    // Beer's law: thicker cloud absorbs more light
    let optical_depth = eroded * density * absorption;
    let beer = exp(-optical_depth);

    // Powder effect: thin cloud edges scatter less
    let powder = 1.0 - exp(-optical_depth * 2.0);
    let beer_powder = mix(beer, beer * powder, powder_strength);

    // Self-shadowing: bottom of cloud is darker
    let self_shadow = mix(0.3, 1.0, height_fraction);

    // Combined lighting
    let light = beer_powder * self_shadow;

    // Base lit color
    var lit = mix(shadow_color.rgb, cloud_color.rgb, light);

    // Silver lining (forward scattering toward sun)
    let sun_dot = dot(dir, sun_dir);
    let silver = pow(saturate(sun_dot), 1.0 / max(silver_spread, 0.01)) * silver_intensity;
    let edge_mask = 1.0 - smoothstep(0.0, 0.5, eroded); // stronger at cloud edges
    lit += cloud_color.rgb * 1.3 * silver * edge_mask;

    // Ambient floor
    lit = max(lit, cloud_color.rgb * ambient_brightness);

    // ── Atmospheric perspective ──
    let horizon_dist = 1.0 - pow(dir.y, 0.4);
    let atmo = horizon_dist * atmosphere_strength;
    lit = mix(lit, haze_color, atmo);

    // Day/night fade: sun elevation in radians (positive = above horizon)
    let sun_elevation = params_b.w;
    let day_factor = smoothstep(-0.05, 0.15, sun_elevation);

    // Final alpha
    let alpha = eroded * density * altitude_mask * day_factor;

    if alpha < 0.001 {
        discard;
    }

    return vec4<f32>(lit, alpha);
}

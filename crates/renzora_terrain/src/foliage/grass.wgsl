#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

// ── Uniform buffer ─────────────────────────────────────────────────────────

struct GrassUniforms {
    time: f32,
    wind_strength: f32,
    wind_direction: vec2<f32>,
    color_base: vec4<f32>,
    color_tip: vec4<f32>,
    chunk_world_x: f32,
    chunk_world_z: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(3) @binding(0) var<uniform> grass: GrassUniforms;

// ── Vertex Shader ──────────────────────────────────────────────────────────
// Per-blade data is baked into vertex attributes:
//   uv_0.y  = t (0 at base, 1 at tip)
//   uv_1.x  = phase (random per-blade)
//   uv_1.y  = blade_height
//   color.r  = bend (flexibility 0..1)
//   color.gb = lean (encoded: value * 10 + 0.5)
//   color.a  = color_variation (encoded: value + 0.5)

@vertex
fn vertex(
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv_0: vec2<f32>,
    @location(3) uv_1: vec2<f32>,
    @location(5) color: vec4<f32>,
) -> VertexOutput {
    let t = uv_0.y; // 0 at base, 1 at tip
    let phase = uv_1.x;
    let blade_height = uv_1.y;
    let bend = color.r;
    let lean_x = (color.g - 0.5) / 10.0;
    let lean_z = (color.b - 0.5) / 10.0;

    // World position of this vertex (already baked into chunk-local space)
    let world_from_local = mesh_functions::get_world_from_local(instance_index);
    let world_pos = (world_from_local * vec4<f32>(position, 1.0)).xyz;

    // ── Wind model ────────────────────────────────────────────────────────
    let wind_dir = grass.wind_direction;
    let base_wind = 0.08;

    // Large-scale gusts (travel spatially)
    let gust_phase = world_pos.x * 0.08 + world_pos.z * 0.06 + grass.time * 0.5;
    let gust = (sin(gust_phase) * 0.5 + 0.5) * sin(grass.time * 0.35 + world_pos.z * 0.04);
    let gust_strength = gust * 0.15;

    // Medium turbulence (per-blade)
    let turb1 = sin(grass.time * 1.8 + phase + world_pos.x * 0.25 + world_pos.z * 0.15);
    let turb2 = sin(grass.time * 2.3 + phase * 1.3 + world_pos.z * 0.3);
    let turb3 = sin(grass.time * 1.1 + phase * 0.7 + world_pos.x * 0.18);

    // High-frequency flutter (tip only)
    let flutter = sin(grass.time * 5.5 + phase * 4.0) * 0.02 * t;

    // Cubic falloff from base to tip, scaled by flexibility
    let bend_factor = bend * 0.7 + 0.3;
    let wind_pow = t * t * (3.0 - 2.0 * t); // smoothstep

    let wind_x = (wind_dir.x * base_wind
                + wind_dir.x * gust_strength
                + turb1 * 0.06 + turb3 * 0.03
                + flutter) * wind_pow * bend_factor * grass.wind_strength;
    let wind_z = (wind_dir.y * base_wind
                + wind_dir.y * gust_strength
                + turb2 * 0.04
                + flutter * 0.7) * wind_pow * bend_factor * grass.wind_strength;

    // Apply wind displacement
    let displaced = vec3<f32>(
        world_pos.x + wind_x,
        world_pos.y,
        world_pos.z + wind_z,
    );

    // Normal: perpendicular to blade, tilted by wind + lean
    let total_x = wind_x + lean_x * t * blade_height * 3.0;
    let total_z = wind_z + lean_z * t * blade_height * 3.0;
    let blade_normal = normalize(vec3<f32>(-total_x * 0.5, 1.0, -total_z * 0.5));

    var out: VertexOutput;
    out.position = position_world_to_clip(displaced);
    out.world_position = vec4<f32>(displaced, 1.0);
    out.world_normal = blade_normal;
    out.uv = uv_0;
    // Pass per-blade data through to fragment via unused fields
    out.color = color;
    return out;
}

// ── Fragment Shader ────────────────────────────────────────────────────────

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = in.uv.y; // 0 at base, 1 at tip
    let color_var = in.color.a - 0.5;

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
    albedo += vec3<f32>(color_var * 0.7, color_var, color_var * 0.3);

    // Ambient occlusion at base
    albedo *= 0.5 + t * 0.5;

    // Subsurface scattering: tips glow when backlit
    let view_dir = normalize(in.world_position.xyz - view.world_position.xyz);
    let back_light = max(dot(normalize(vec3<f32>(0.3, -0.8, 0.5)), view_dir), 0.0);
    albedo += vec3<f32>(0.06, 0.1, 0.02) * t * t * back_light;

    // Slight yellow at dry tips
    albedo += vec3<f32>(0.03, 0.02, -0.01) * t * t;

    // Simple diffuse lighting using the world normal
    let light_dir = normalize(vec3<f32>(0.3, -0.8, 0.5));
    let ndotl = max(dot(in.world_normal, -light_dir), 0.0);
    let ambient = 0.35;
    let lit = albedo * (ambient + (1.0 - ambient) * ndotl);

    return vec4<f32>(lit, 1.0);
}

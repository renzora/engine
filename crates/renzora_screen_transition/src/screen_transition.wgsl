// Screen transition: blends a frozen "outgoing" frame (extra_texture) into the
// live "incoming" frame (screen_texture) as `progress` goes 0 → 1.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

// Binding 0/1: the LIVE incoming frame (shot B).
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ScreenTransitionSettings {
    progress: f32,
    mode: f32,
    direction: f32,
    smoothness: f32,
    color_r: f32,
    color_g: f32,
    color_b: f32,
    _padding: f32,
};
@group(0) @binding(2) var<uniform> settings: ScreenTransitionSettings;

// Binding 3/4: the FROZEN outgoing frame (shot A). When no snapshot has been
// captured yet this aliases the live frame, so the effect is a no-op.
@group(0) @binding(3) var extra_texture: texture_2d<f32>;
@group(0) @binding(4) var extra_sampler: sampler;

// Axis coordinate that runs 0 → 1 in the wipe/slide direction.
fn axis_coord(uv: vec2<f32>, direction: f32) -> f32 {
    if direction < 0.5 {
        return uv.x;          // left → right
    } else if direction < 1.5 {
        return 1.0 - uv.x;    // right → left
    } else if direction < 2.5 {
        return uv.y;          // top → bottom
    }
    return 1.0 - uv.y;        // bottom → top
}

// Offset (in UV space) that slides a layer in along the direction axis.
fn axis_offset(direction: f32, amount: f32) -> vec2<f32> {
    if direction < 0.5 {
        return vec2(amount, 0.0);
    } else if direction < 1.5 {
        return vec2(-amount, 0.0);
    } else if direction < 2.5 {
        return vec2(0.0, amount);
    }
    return vec2(0.0, -amount);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let p = clamp(settings.progress, 0.0, 1.0);
    let a = textureSample(extra_texture, extra_sampler, uv);   // outgoing (frozen)
    let b = textureSample(screen_texture, texture_sampler, uv); // incoming (live)
    let edge_color = vec4(settings.color_r, settings.color_g, settings.color_b, 1.0);
    let sm = max(settings.smoothness, 0.0001);

    // Crossfade / dissolve.
    if settings.mode < 0.5 {
        return mix(a, b, smoothstep(0.0, 1.0, p));
    }

    // Wipe: B sweeps across, replacing A along the direction axis.
    if settings.mode < 1.5 {
        let c = axis_coord(uv, settings.direction);
        // B is revealed where the wipe front (at `p`) has passed.
        let reveal = smoothstep(p - sm, p + sm, c);
        var col = mix(b, a, reveal);
        // Thin colored border at the moving front.
        let border = (1.0 - smoothstep(0.0, sm, abs(c - p))) * step(0.001, p) * step(p, 0.999);
        col = mix(col, edge_color, border * step(0.0001, settings.color_r + settings.color_g + settings.color_b));
        return col;
    }

    // Slide: A slides out and B slides in from the opposite side.
    if settings.mode < 2.5 {
        let a_uv = uv + axis_offset(settings.direction, p);       // A leaves
        let b_uv = uv - axis_offset(settings.direction, 1.0 - p); // B arrives
        let in_a = f32(all(a_uv >= vec2(0.0)) && all(a_uv <= vec2(1.0)));
        let a_s = textureSample(extra_texture, extra_sampler, clamp(a_uv, vec2(0.0), vec2(1.0)));
        let b_s = textureSample(screen_texture, texture_sampler, clamp(b_uv, vec2(0.0), vec2(1.0)));
        // The incoming B occupies the region the outgoing A has vacated.
        return mix(b_s, a_s, in_a);
    }

    // Iris: a circle of B grows over A from the center.
    let aspect = vec2(1.0, 1.0);
    let d = distance(uv * aspect, vec2(0.5) * aspect);
    let radius = p * 0.75; // 0.75 ≈ corner distance, so B fully covers at p = 1
    let mask = 1.0 - smoothstep(radius - sm, radius + sm, d);
    var col = mix(a, b, mask);
    let ring = (1.0 - smoothstep(0.0, sm, abs(d - radius))) * step(0.001, p) * step(p, 0.999);
    col = mix(col, edge_color, ring * step(0.0001, settings.color_r + settings.color_g + settings.color_b));
    return col;
}

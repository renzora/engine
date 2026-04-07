#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct CrossHatchSettings {
    density: f32,
    thickness: f32,
    angle: f32,
    brightness: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: CrossHatchSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let lum = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let pixel = in.uv * dims;

    let s = sin(settings.angle);
    let c = cos(settings.angle);

    // Rotated coordinates for two hatch directions
    let p1 = pixel.x * c + pixel.y * s;
    let p2 = pixel.x * c - pixel.y * s;

    let line1 = abs(sin(p1 * settings.density * 0.01));
    let line2 = abs(sin(p2 * settings.density * 0.01));

    var hatch = settings.brightness;
    // Darker areas get more hatch lines
    if lum < 0.75 {
        hatch = min(hatch, smoothstep(0.0, settings.thickness, line1));
    }
    if lum < 0.5 {
        hatch = min(hatch, smoothstep(0.0, settings.thickness, line2));
    }
    if lum < 0.25 {
        hatch *= 0.5;
    }

    return vec4(vec3(hatch), color.a);
}

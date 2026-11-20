// @effect(name = "Cross Hatch", icon = "scribble")
//
// Cross-hatch shading, as a pen-and-ink drawing shades with overlapping
// strokes: the darker the pixel, the more layers of hatching it gets.
//
// `angle` is in radians and tops out at a quarter turn, because past that the
// hatching is the same set of lines again, mirrored.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    density: f32,     // @slider(2, 100, default = 30, step = 0.5)
    thickness: f32,   // @slider(0.01, 0.5, default = 0.1, step = 0.01)
    angle: f32,       // @slider(0, 1.57, default = 0.785, step = 0.01)
    brightness: f32,  // @slider(0, 1, default = 0.9, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let lum = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let pixel = in_uv * dims;

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

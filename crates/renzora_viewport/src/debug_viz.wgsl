// Post-tonemap debug visualization.
//
// Reads the normal + depth prepass and writes display-ready colors
// directly to the view target, *after* Tonemapping has run. That means
// the output bypasses ACES tonemap and auto-exposure, so values come
// out as authored — same approach the Solari debug views use.
//
// Mode:
//   0 = None (passthrough — copy view target source to destination)
//   1 = Normals (world-space normal → RGB via *0.5 + 0.5)
//   2 = Depth (linear view-space depth → turbo-style rainbow)

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

struct DebugVizConfig {
    mode: u32,
    near: f32,
    far: f32,
    _pad: f32,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var normal_tex: texture_2d<f32>;
@group(0) @binding(4) var<uniform> view: View;
@group(0) @binding(5) var<uniform> config: DebugVizConfig;

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

fn linear_depth(uv: vec2<f32>, depth: f32) -> f32 {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let view_h = view.view_from_clip * vec4<f32>(ndc, 1.0);
    return -(view_h.z / view_h.w); // view-space z is negative going away
}

// Approximate "turbo" colormap — 5-stop piecewise linear gradient that
// covers the same range as matplotlib's turbo without needing a LUT.
fn turbo(t: f32) -> vec3<f32> {
    let s = clamp(t, 0.0, 1.0);
    let stops = array<vec3<f32>, 5>(
        vec3<f32>(0.18, 0.07, 0.34), // dark purple
        vec3<f32>(0.20, 0.55, 0.93), // blue
        vec3<f32>(0.30, 0.85, 0.40), // green
        vec3<f32>(0.95, 0.75, 0.10), // yellow
        vec3<f32>(0.85, 0.10, 0.10), // red
    );
    let scaled = s * 4.0;
    let i = u32(scaled);
    let f = scaled - f32(i);
    if (i >= 4u) { return stops[4]; }
    return mix(stops[i], stops[i + 1u], f);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);

    if (config.mode == 1u) {
        // Normals — pure prepass output, no shading. Sky gets a dim
        // neutral so it doesn't confuse the surface colors.
        let depth = textureLoad(depth_tex, pixel, 0);
        if (depth <= 0.0) {
            return vec4<f32>(0.05, 0.05, 0.07, 1.0);
        }
        let n = textureLoad(normal_tex, pixel, 0).xyz;
        return vec4<f32>(n, 1.0);
    }

    if (config.mode == 2u) {
        // Depth — linear view-space distance mapped to turbo. Sky is
        // the far end of the gradient.
        let depth = textureLoad(depth_tex, pixel, 0);
        if (depth <= 0.0) {
            return vec4<f32>(turbo(1.0), 1.0);
        }
        let lin = linear_depth(in.uv, depth);
        let t = clamp((lin - config.near) / max(config.far - config.near, 1e-3), 0.0, 1.0);
        return vec4<f32>(turbo(t), 1.0);
    }

    // Passthrough.
    return textureSample(scene_tex, scene_sampler, in.uv);
}

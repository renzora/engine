use renzora_plugin::prelude::*;
use renzora_plugin::sys::RenderPhase;

/// `#[repr(C)]` and 16-byte padded because this is a GPU uniform. The padding
/// must be scalar `f32`s on both sides — WGSL aligns `vec3<f32>` to 16 and Rust's
/// `[f32; 3]` to 4, so the "same" struct would be 32 bytes in the shader and 16
/// here.
#[derive(Component)]
#[repr(C)]
pub struct Tint {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub strength: f32,
}

impl Default for Tint {
    fn default() -> Self {
        Self {
            red: 1.3,
            green: 0.6,
            blue: 1.3,
            strength: 0.5,
        }
    }
}

const WGSL: &str = r#"
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Tint {
    red: f32,
    green: f32,
    blue: f32,
    strength: f32,
};
@group(0) @binding(2) var<uniform> settings: Tint;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let c = textureSample(screen_texture, texture_sampler, uv);
    let tinted = c.rgb * vec3<f32>(settings.red, settings.green, settings.blue);
    return vec4<f32>(mix(c.rgb, tinted, settings.strength), c.a);
}
"#;

pub struct TintPlugin;

impl Plugin for TintPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Tint>("tint", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(TintPlugin);

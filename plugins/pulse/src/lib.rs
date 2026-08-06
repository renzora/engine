#![no_std]
extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;
use renzora_plugin::sys::RenderPhase;

#[derive(Component)]
#[repr(C)]
pub struct Pulse {
    pub strength: f32,
    pub speed: f32,
    /// Advanced by `tick` each frame — a system driving an effect's uniform.
    pub time: f32,
    _pad: f32,
}

impl Default for Pulse {
    fn default() -> Self {
        Self {
            strength: 0.6,
            speed: 2.0,
            time: 0.0,
            _pad: 0.0,
        }
    }
}

const WGSL: &str = r#"
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Pulse {
    strength: f32,
    speed: f32,
    time: f32,
    _pad: f32,
};
@group(0) @binding(2) var<uniform> settings: Pulse;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let c = textureSample(screen_texture, texture_sampler, uv);
    let d = distance(uv, vec2<f32>(0.5, 0.5));
    let wave = sin(settings.time) * 0.5 + 0.5;
    let vignette = 1.0 - d * settings.strength * wave * 2.0;
    return vec4<f32>(c.rgb * clamp(vignette, 0.0, 1.0), c.a);
}
"#;

fn tick(mut q: Query<&mut Pulse>, time: Res<Time>) {
    for p in &mut q {
        p.time += p.speed * time.delta_secs();
    }
}

pub struct PulsePlugin;

impl Plugin for PulsePlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Pulse>("pulse", WGSL, RenderPhase::LdrPost, 1.0)
            .add_systems(Update, tick);
    }
}

renzora_plugin::add!(PulsePlugin);

//! Reference Renzora plugin.
//!
//! Every line below is ordinary Bevy source. `Query`, `Res`, `Transform`,
//! `With`, `App` and `Plugin` are `renzora_plugin`'s shims over a C function
//! table, but nothing about writing against them differs — which is the point.
//!
//! Lives in `plugins/`, outside the workspace, like every first-party
//! `renzora_plugin`. As a workspace member it would inherit the engine's cargo
//! feature unification and quietly link Bevy, hiding the property that matters:
//! `cargo build` here resolves exactly one dependency and finishes in about a
//! second. `cargo renzora` builds and stages it; `cd plugins/spinner && cargo
//! build` works standalone.

use renzora_plugin::prelude::*;
use renzora_plugin::sys::RenderPhase;

/// A plugin-owned component. The engine has no Rust type for this — it learns
/// the layout, the field schema and the default from `#[derive(Component)]` at
/// load time.
///
/// `Default` is required by the derive: the editor has to put *something* on the
/// entity when you add the component, and zeroed memory would mean `speed: 0.0`
/// — present, correct, and doing nothing, which reads as a broken plugin.
#[derive(Component)]
pub struct Spinner {
    pub speed: f32,
}

/// Hand-written rather than derived, and that difference matters: a derived
/// `Default` would give `speed: 0.0`, so adding a Spinner in the inspector would
/// produce a component that is present, correct, and visibly doing nothing.
impl Default for Spinner {
    fn default() -> Self {
        Self { speed: 1.0 }
    }
}

/// Querying `&Spinner` scopes this to entities that actually have one, so it
/// needs no `Mesh3d` filter — adding the component in the inspector is what
/// opts an object in.
fn spin(mut q: Query<(&mut Transform, &Spinner)>, time: Res<Time>) {
    for (t, s) in &mut q {
        t.rotate_y(s.speed * time.delta_secs());
    }
}

/// A full-screen pass, to prove plugin code can execute inside the render graph.
///
/// Deliberately garish — a subtle effect would be indistinguishable from the
/// pass silently not running, which is the failure this is meant to catch.
const TINT_WGSL: &str = r#"
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let c = textureSample(screen_texture, texture_sampler, uv);
    // Push toward magenta so it is unmistakable.
    return vec4<f32>(c.r * 1.3, c.g * 0.6, c.b * 1.3, c.a);
}
"#;

pub struct SpinPlugin;

impl Plugin for SpinPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<Spinner>()
            .add_systems(Update, spin)
            .add_render_pass(
                "spinner.tint",
                TINT_WGSL,
                RenderPhase::LdrPost,
                0.0,
                |pass: &mut renzora_plugin::ecs::RenderPass| {
                    pass.set_pipeline();
                    pass.draw(0..3, 0..1);
                },
            );
    }
}

renzora_plugin::add!(SpinPlugin);

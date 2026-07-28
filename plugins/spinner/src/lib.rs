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

/// Settings for [`TINT_WGSL`], and the thing that turns "a plugin can draw" into
/// "a plugin can ship an effect".
///
/// One declaration does three jobs: it is the shader's uniform, it is the
/// inspector's controls (via the derived field schema), and putting it on an
/// entity is what switches the effect on.
///
/// `#[repr(C)]` and padded to 16 bytes because a uniform binding is std140.
///
/// The padding must lay out identically on BOTH sides. `[f32; 3]` here pairs with
/// three scalar `f32`s in the shader, not `vec3<f32>` — see the note on
/// [`TINT_WGSL`]. Getting this wrong is not a soft failure: wgpu rejects the
/// pipeline and the engine escalates it to an unrecoverable GPU panic.
#[derive(Component)]
#[repr(C)]
pub struct Tint {
    pub strength: f32,
    _pad: [f32; 3],
}

impl Default for Tint {
    fn default() -> Self {
        Self { strength: 0.5, _pad: [0.0; 3] }
    }
}

/// Bindings 0 and 1 are the screen texture and sampler; binding 2 is whatever
/// component was named in `add_post_process`.
const TINT_WGSL: &str = r#"
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

// Scalar pads, NOT `vec3<f32>`: WGSL aligns a vec3 to 16 bytes, so
// `strength: f32` followed by `_pad: vec3<f32>` puts the vec3 at offset 16 and
// makes the struct 32 bytes — while the Rust `[f32; 3]` has no such alignment
// and keeps it at 16. wgpu rejects the mismatch and this engine turns that into
// an unrecoverable GPU panic. Three scalars lay out identically on both sides.
struct Tint {
    strength: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(0) @binding(2) var<uniform> settings: Tint;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let c = textureSample(screen_texture, texture_sampler, uv);
    let tinted = vec3<f32>(c.r * 1.3, c.g * 0.6, c.b * 1.3);
    return vec4<f32>(mix(c.rgb, tinted, settings.strength), c.a);
}
"#;

pub struct SpinPlugin;

impl Plugin for SpinPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<Spinner>()
            .add_systems(Update, spin)
            // No render code at all: the host does extraction, the uniform
            // upload, the bind group and the draw. Add a `Tint` to any entity to
            // switch it on.
            .add_post_process::<Tint>("spinner.tint", TINT_WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(SpinPlugin);

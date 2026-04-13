//! Renzora Marketplace Preview Harness
//!
//! A minimal Bevy app compiled to WASM that provides live previews for
//! marketplace assets: shaders, materials, 3D models, animations,
//! particle effects, textures, and HDRIs.
//!
//! The Leptos frontend loads this as a single static WASM binary and
//! communicates via wasm-bindgen function calls. Assets are fetched
//! dynamically at runtime — no per-asset WASM builds needed.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────┐
//! │  Leptos Frontend (website)      │
//! │  - Asset detail page            │
//! │  - Param sliders / UI controls  │
//! │  - Calls preview_*() functions  │
//! └────────────┬────────────────────┘
//!              │ wasm-bindgen
//! ┌────────────▼────────────────────┐
//! │  renzora_preview (this crate)   │
//! │  - bridge.rs: JS ↔ Bevy events │
//! │  - scene.rs: camera, lighting   │
//! │  - modes/: per-asset-type logic │
//! │    ├── shader.rs                │
//! │    ├── model.rs                 │
//! │    ├── animation.rs             │
//! │    ├── particle.rs              │
//! │    └── texture.rs               │
//! └────────────┬────────────────────┘
//!              │ uses
//! ┌────────────▼────────────────────┐
//! │  Engine crates                  │
//! │  - renzora_shader (compiler)    │
//! │  - renzora_hanabi (particles)   │
//! │  - renzora_animation            │
//! │  - renzora_lighting             │
//! │  - Bevy 0.18 (rendering)       │
//! └─────────────────────────────────┘
//! ```

pub mod bridge;
pub mod scene;
pub mod modes;

use bevy::prelude::*;
use wasm_bindgen::prelude::*;

/// Initialize and run the preview harness.
///
/// Called once from JavaScript when the asset detail page mounts the preview canvas.
/// The canvas element must exist in the DOM before calling this.
#[wasm_bindgen]
pub fn preview_init() {
    // Prevent double-init panics
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    App::new()
        .add_plugins(DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    canvas: Some("#preview-canvas".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: false,
                    composite_alpha_mode: bevy::window::CompositeAlphaMode::Opaque,
                    ..default()
                }),
                ..default()
            })
            .set(bevy::log::LogPlugin {
                level: bevy::log::Level::WARN,
                ..default()
            })
        )
        // Engine plugins (runtime only, no editor features)
        .add_plugins(renzora_shader::ShaderPlugin)
        .add_plugins(renzora_hanabi::HanabiParticlePlugin)
        // Preview harness plugins
        .add_plugins((
            bridge::BridgePlugin,
            scene::ScenePlugin,
            modes::ModesPlugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.06, 0.06, 0.08)))
        .add_systems(Startup, set_tonemapping)
        .run();
}

fn set_tonemapping(
    mut camera_q: Query<&mut bevy::core_pipeline::tonemapping::Tonemapping, With<Camera3d>>,
) {
    for mut tm in camera_q.iter_mut() {
        *tm = bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted;
    }
}

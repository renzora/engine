//! 2D lighting distribution plugin.
//!
//! Wraps the vendored [`bevy_firefly`] crate — 2D point lights, polygonal /
//! round occluders, soft shadows, occlusion z-sorting, light banding, and
//! normal maps — as a single dlopen'd cdylib.
//!
//! Runtime scope on purpose: the same plugin lights the editor's 2D viewport,
//! in-editor play, and the shipped game. Lighting is authored as ordinary
//! scene data — a [`FireflyConfig`] on the game's `Camera2d` plus
//! [`PointLight2d`] / [`Occluder2d`] entities — and persists through the
//! reflection-driven scene serializer because the vendored crate's components
//! carry `#[reflect(Component, Default)]` (a local patch, see
//! `crates/bevy_firefly/Cargo.toml`).
//!
//! Editor-only wiring (inspectors, Add-Entity presets, selection gizmos, and
//! the editor-camera lightmap preview) is compile-gated behind the `editor`
//! feature; in a shipped game those registrations are harmless no-ops because
//! the editor framework resources / marker entities they target don't exist.

use bevy::prelude::*;
use bevy_firefly::prelude::*;

#[cfg(feature = "editor")]
mod editor;

#[derive(Default)]
pub struct Light2dPlugin;

impl Plugin for Light2dPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] Light2dPlugin (bevy_firefly 2d lighting)");
        app.add_plugins(FireflyPlugin);

        // Scene persistence + script reflection access. Registering the
        // components pulls their field types (Falloff, LightCore, NormalMode,
        // Occluder2dShape, ...) into the registry with them.
        app.register_type::<FireflyConfig>()
            .register_type::<PointLight2d>()
            .register_type::<Occluder2d>()
            .register_type::<Occluder2dEnabled>()
            .register_type::<LightHeight>()
            .register_type::<SpriteHeight>();

        #[cfg(feature = "editor")]
        editor::register(app);
    }
}

renzora::add!(Light2dPlugin);

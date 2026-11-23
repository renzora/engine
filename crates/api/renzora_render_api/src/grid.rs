//! The infinite ground grid — the component vocabulary, not the renderer.
//!
//! Spawn an [`InfiniteGrid`] and something draws a ground plane under your
//! scene, fading out with distance so there is no hard edge. [`InfiniteGridSettings`]
//! tunes the colours, spacing and fade, and can sit either on the grid entity or
//! on a camera that sees it.
//!
//! # Why these two types are here and the renderer is not
//!
//! This is the shape every contract in this crate takes, and the grid is the
//! simplest example of it: the **vocabulary** lives here, the **implementation**
//! lives in the crate that owns the render graph (`renzora_grid`), and the two
//! agree because there is exactly one definition of the component.
//!
//! The reason it matters is plugins. A native plugin is compiled against `bevy`,
//! `renzora` and `renzora_ember` and nothing else, so before this move a plugin
//! could not put a grid under anything — not a model preview, not a turntable,
//! not its own editing surface — because the type it needed lived in a crate it
//! could not reach. Moving two plain-data components costs this crate no
//! dependencies at all (they are `Color`s and `f32`s) and hands every plugin a
//! ground plane.
//!
//! `renzora_grid` re-exports both, so `renzora_grid::InfiniteGrid` still
//! resolves for everything that already used it.

use bevy::camera::visibility::{self, NoFrustumCulling, VisibilityClass};
use bevy::color::Color;
use bevy::ecs::prelude::*;
use bevy::prelude::{Transform, Visibility};
use bevy::reflect::{std_traits::ReflectDefault, Reflect};
use bevy::render::sync_world::SyncToRenderWorld;

/// The component used to represent an infinite grid.
///
/// This is intended for use as a ground plane in editor-like tools.
#[derive(Component, Default, Reflect, Copy, Clone)]
#[reflect(Component, Default)]
#[require(
    InfiniteGridSettings,
    Transform,
    Visibility,
    VisibilityClass,
    NoFrustumCulling,
    SyncToRenderWorld
)]
#[component(on_add = visibility::add_visibility_class::<InfiniteGrid>)]
pub struct InfiniteGrid;

/// Component to configure the infinite grid.
///
/// This component can be applied directly on the grid entity or on a camera that
/// can see the grid.
#[derive(Component, Copy, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct InfiniteGridSettings {
    /// The color of the X axis
    pub x_axis_color: Color,
    /// The color of the Z axis
    pub z_axis_color: Color,
    /// The color of the minor lines of the grid
    pub minor_line_color: Color,
    /// The color of the major lines of the grid. Every 10th line is considered major
    pub major_line_color: Color,
    /// How far the grid will be visible relative to the camera
    pub fadeout_distance: f32,
    /// How quickly the grid will fadeout
    pub dot_fadeout_strength: f32,
    /// The scale of the distance between the lines. A smaller value increases the
    /// distance between the lines
    pub scale: f32,
}

impl Default for InfiniteGridSettings {
    fn default() -> Self {
        Self {
            // These colors are copied from bevy_feathers but we don't need to
            // depend on it just for that
            x_axis_color: Color::oklcha(0.5232, 0.1404, 13.84, 1.0),
            z_axis_color: Color::oklcha(0.4847, 0.1249, 253.08, 1.0),
            minor_line_color: Color::srgb(0.2, 0.2, 0.2),
            major_line_color: Color::srgb(0.25, 0.25, 0.25),
            fadeout_distance: 100.,
            dot_fadeout_strength: 0.25,
            scale: 1.0,
        }
    }
}

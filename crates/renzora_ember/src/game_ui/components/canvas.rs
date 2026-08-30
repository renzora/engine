//! Canvas root component.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Marker component for a UI canvas root entity.
///
/// A canvas is a top-level container that groups UI widgets. Multiple canvases
/// can exist per scene (e.g. HUD, pause menu, inventory).
///
/// A canvas renders either to the **screen** (the normal fullscreen UI) or into
/// **world space** — the same template projected onto a plane in the 3D scene (a
/// monitor on a wall, a floating menu in VR). World space is the unified home of
/// what used to be a separate `WorldUiPanel`: because the template, the script,
/// and the world plane all live on the *one* canvas entity, `{{ }}` bindings
/// resolve against the canvas's own components exactly like a screen canvas — no
/// separate host entity, so scripts "just work".
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiCanvas {
    /// Render order — higher values draw on top.
    pub sort_order: i32,
    /// When to show: "always", "play_only", "editor_only".
    pub visibility_mode: String,
    /// Reference resolution width for UI scaling (design-time canvas width). In
    /// world space this is also the offscreen layout resolution.
    pub reference_width: f32,
    /// Reference resolution height for UI scaling (design-time canvas height). In
    /// world space this is also the offscreen layout resolution.
    pub reference_height: f32,
    /// `"screen"` = normal fullscreen UI; `"world"` = projected onto a plane in
    /// the 3D scene, placed/scaled by the entity's `Transform`.
    #[serde(default = "default_render_space")]
    #[reflect(default = "default_render_space")]
    pub render_space: String,
    /// World space only: `"texture"` = render the UI to an offscreen image shown
    /// on the plane (RTT); `"mesh"` = emit the laid-out UI as batched 3D geometry
    /// directly in the scene (the Unity way). Ignored in screen space.
    #[serde(default = "default_render_mode")]
    #[reflect(default = "default_render_mode")]
    pub render_mode: String,
}

fn default_render_space() -> String {
    "screen".into()
}
fn default_render_mode() -> String {
    "texture".into()
}

impl UiCanvas {
    /// Whether this canvas projects into world space (vs the normal screen UI).
    pub fn is_world(&self) -> bool {
        self.render_space.trim().eq_ignore_ascii_case("world")
    }

    /// World space only: whether the UI is emitted as 3D geometry (vs RTT texture).
    pub fn is_mesh_mode(&self) -> bool {
        self.render_mode.trim().eq_ignore_ascii_case("mesh")
    }
}

impl Default for UiCanvas {
    fn default() -> Self {
        Self {
            sort_order: 0,
            visibility_mode: "always".into(),
            reference_width: 1280.0,
            reference_height: 720.0,
            render_space: default_render_space(),
            render_mode: default_render_mode(),
        }
    }
}

/// The `Node` every canvas root must have: the full render target, pinned to its
/// top-left.
///
/// A canvas is the *surface* a template is laid out on, not a box on a surface —
/// there is no outer frame for it to be positioned within, so its rect is not a
/// property anyone gets to author. Where the UI sits on screen is decided by the
/// template's own layout; the canvas only says how big "the screen" is, and that
/// is `reference_width`/`reference_height`, not this.
///
/// It lives here, next to the component, because three separate places need the
/// same answer: the editor's spawn, the scene-load healer, and anything that
/// rebuilds a canvas. When it was written out longhand at the spawn site only,
/// nothing else knew what a correct canvas root looked like.
pub fn canvas_root_node() -> bevy::ui::Node {
    bevy::ui::Node {
        position_type: bevy::ui::PositionType::Absolute,
        left: bevy::ui::Val::Px(0.0),
        top: bevy::ui::Val::Px(0.0),
        width: bevy::ui::Val::Percent(100.0),
        height: bevy::ui::Val::Percent(100.0),
        ..default()
    }
}

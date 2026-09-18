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
    /// Screen space only: how the reference resolution maps onto the real
    /// window. See [`CanvasScaleMode`]. Ignored in world space, where the
    /// offscreen target *is* the reference resolution.
    #[serde(default = "default_scale_mode")]
    #[reflect(default = "default_scale_mode")]
    pub scale_mode: String,
}

fn default_render_space() -> String {
    "screen".into()
}
fn default_render_mode() -> String {
    "texture".into()
}
fn default_scale_mode() -> String {
    "fit".into()
}

/// How a screen canvas maps its reference resolution onto the real render
/// target when the two don't match — which, outside the UI editor, is almost
/// always.
///
/// The distinction only shows up once a canvas holds more than one thing. A
/// single centred panel looks the same either way; a centred panel *and* a
/// bottom-left dialogue box do not. Under [`Expand`](Self::Expand) each is
/// resolved against the live window, so widening it walks them apart. Under
/// [`Fit`](Self::Fit) the design box moves as one piece and the arrangement is
/// the one the editor showed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CanvasScaleMode {
    /// Lay the canvas out at exactly the reference resolution and scale that
    /// whole box uniformly to fit the window, centred, with bars on the
    /// leftover axis. Composition is preserved exactly — this is what the UI
    /// editor shows you, so it is the default.
    #[default]
    Fit,
    /// Scale text and padding uniformly, but let the canvas fill the window,
    /// so layout re-flows to the real aspect ratio. Right for a HUD whose
    /// pieces are meant to hug the screen edges however wide the screen gets;
    /// wrong for anything whose parts are positioned relative to each other.
    Expand,
    /// No scaling at all. One authored pixel is one target pixel, and the
    /// canvas fills the window. For pixel-art UI that must not resample.
    Constant,
}

impl CanvasScaleMode {
    /// Parse the stored string. Unknown values fall back to the default rather
    /// than erroring: this field arrives from a hand-editable scene file, and a
    /// typo should cost you the mode you asked for, not the UI.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "expand" => Self::Expand,
            "constant" | "none" => Self::Constant,
            _ => Self::Fit,
        }
    }

    /// The uniform factor to render this canvas's authored pixels at, given the
    /// real target size.
    pub fn scale_for(self, ref_w: f32, ref_h: f32, target_w: f32, target_h: f32) -> f32 {
        match self {
            Self::Constant => 1.0,
            Self::Fit | Self::Expand => (target_w / ref_w).min(target_h / ref_h),
        }
    }
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

    /// How this canvas maps its reference resolution onto the window.
    ///
    /// A world canvas is always [`CanvasScaleMode::Constant`] whatever the
    /// field says: its render target is *created* at the reference resolution,
    /// so there is nothing to reconcile.
    pub fn scale_mode(&self) -> CanvasScaleMode {
        if self.is_world() {
            return CanvasScaleMode::Constant;
        }
        CanvasScaleMode::parse(&self.scale_mode)
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
            scale_mode: default_scale_mode(),
        }
    }
}

/// The `Node` a canvas root must have.
///
/// A canvas is the *surface* a template is laid out on, not a box on a surface —
/// there is no outer frame for it to be positioned within, so its rect is not a
/// property anyone gets to author. Where the UI sits on screen is decided by the
/// template's own layout; the canvas only says how big "the screen" is, and that
/// is `reference_width`/`reference_height`, not this.
///
/// What that means geometrically depends on [`CanvasScaleMode`]:
///
/// * `Fit` — the canvas is *literally* the reference box, in authored pixels,
///   centred in the target. The uniform shrink onto the real window is
///   [`UiScale`](bevy::ui::UiScale)'s job, so text re-rasterizes at its true
///   size instead of being resampled, and the layout inside the box never has
///   to know the window changed. Centring is `50%` minus half the box: the
///   scale factor isn't knowable from a `Val`, but "half of what's left" is.
/// * `Expand` / `Constant` — the canvas fills the target, and layout re-flows.
///
/// This lives next to the component because several places need the same
/// answer: the editor's spawn, the runtime spawn, the scene-load healer, and
/// the world→screen switch. When it was written out longhand at one spawn site
/// only, nothing else knew what a correct canvas root looked like — and the
/// copy that stayed behind in `spawn_root_canvas` is exactly the drift this
/// warns about.
pub fn canvas_root_node(canvas: &UiCanvas) -> bevy::ui::Node {
    use bevy::ui::{PositionType, UiRect, Val};

    if canvas.scale_mode() != CanvasScaleMode::Fit {
        return bevy::ui::Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        };
    }

    // `max(1.0)`: a zero reference collapses the canvas to nothing, and a
    // canvas at zero size is not a visible mistake — see the healer.
    let w = canvas.reference_width.max(1.0);
    let h = canvas.reference_height.max(1.0);
    bevy::ui::Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Percent(50.0),
        width: Val::Px(w),
        height: Val::Px(h),
        margin: UiRect {
            left: Val::Px(-w / 2.0),
            top: Val::Px(-h / 2.0),
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
        },
        ..default()
    }
}

/// The editor's viewport preview owns this canvas root's rect for now.
///
/// The root rect is normally an invariant re-established every frame by
/// `heal_canvas_root_geometry`, because nothing should be able to author it.
/// The one legitimate exception is the editor previewing a canvas over a
/// viewport that is not the reference resolution: reproducing the runtime's
/// scaling there means laying the canvas out at `target / scale` and scaling
/// the result down, and the layout half of that is a rect the healer would
/// otherwise revert on the same frame.
///
/// Deliberately **not** `Reflect`, so it cannot reach a saved scene. A scene
/// that came back carrying it would keep the healer off a root whose preview
/// rect is meaningless in the new session, and in a shipped game there is no
/// editor to ever take it off again.
#[derive(Component)]
pub struct CanvasPreviewSized;

/// The offset that keeps a centre-scaled box's top-left corner at the origin.
///
/// `UiTransform` scales a node about its own centre, so a box of logical length
/// `logical` scaled by `scale` leaves its top-left at `logical / 2 * (1 - scale)`
/// instead of at `0`. The preview needs the scaled result flush with the
/// target's corner, not floating in the middle of it, so this is subtracted
/// back off as a translation.
pub fn preview_centre_scale_offset(logical: f32, scale: f32) -> f32 {
    -(logical / 2.0) * (1.0 - scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ui::Val;

    #[test]
    fn unknown_scale_mode_falls_back_to_fit() {
        assert_eq!(CanvasScaleMode::parse("fit"), CanvasScaleMode::Fit);
        assert_eq!(CanvasScaleMode::parse("  EXPAND "), CanvasScaleMode::Expand);
        assert_eq!(CanvasScaleMode::parse("constant"), CanvasScaleMode::Constant);
        assert_eq!(CanvasScaleMode::parse("wibble"), CanvasScaleMode::Fit);
    }

    /// A world canvas renders into a target it sized itself, so scaling it
    /// again would double-apply.
    #[test]
    fn world_canvas_never_scales() {
        let canvas = UiCanvas {
            render_space: "world".into(),
            scale_mode: "fit".into(),
            ..default()
        };
        assert_eq!(canvas.scale_mode(), CanvasScaleMode::Constant);
    }

    #[test]
    fn fit_takes_the_smaller_axis() {
        // 16:9 design in a 4:3 window: height is the binding constraint.
        let s = CanvasScaleMode::Fit.scale_for(1280.0, 720.0, 1600.0, 720.0);
        assert!((s - 1.0).abs() < 1e-6);
        let s = CanvasScaleMode::Fit.scale_for(1280.0, 720.0, 2560.0, 720.0);
        assert!((s - 1.0).abs() < 1e-6, "width alone must not stretch it");
        let s = CanvasScaleMode::Fit.scale_for(1280.0, 720.0, 2560.0, 1440.0);
        assert!((s - 2.0).abs() < 1e-6);
    }

    #[test]
    fn constant_ignores_the_target() {
        assert_eq!(
            CanvasScaleMode::Constant.scale_for(1280.0, 720.0, 3840.0, 2160.0),
            1.0
        );
    }

    /// The centring offset has to be expressible without knowing the scale
    /// factor, because a `Val` can't see it — half the parent, back half the box.
    #[test]
    fn fit_root_is_the_reference_box_centred() {
        let node = canvas_root_node(&UiCanvas::default());
        assert_eq!(node.width, Val::Px(1280.0));
        assert_eq!(node.height, Val::Px(720.0));
        assert_eq!(node.left, Val::Percent(50.0));
        assert_eq!(node.margin.left, Val::Px(-640.0));
        assert_eq!(node.margin.top, Val::Px(-360.0));
    }

    /// The worked example from issue #110: a `1280x720` reference previewed in
    /// a `1225x551` viewport. Laying out at `target / scale` and scaling back by
    /// `scale` has to land exactly on the target, or an edge-anchored child
    /// misses the edge it was pinned to.
    #[test]
    fn expand_preview_box_scales_back_onto_the_target() {
        let (tw, th) = (1225.0_f32, 551.0_f32);
        let scale = CanvasScaleMode::Expand.scale_for(1280.0, 720.0, tw, th);
        assert!((scale - 551.0 / 720.0).abs() < 1e-6);

        let (lw, lh) = (tw / scale, th / scale);
        assert!((lw - 1600.726).abs() < 1e-3);
        assert!((lh - 720.0).abs() < 1e-3);
        assert!((lw * scale - tw).abs() < 1e-3);
        assert!((lh * scale - th).abs() < 1e-3);

        // A 64px bar authored against the reference renders at ~49px.
        assert!((64.0 * scale - 48.97778).abs() < 1e-4);
    }

    /// Centre-scaling moves a box's top-left corner; the preview needs it flush
    /// with the target's corner, so the offset has to cancel that exactly.
    #[test]
    fn centre_scale_offset_pins_the_top_left() {
        let logical = 1600.726_f32;
        let scale = 1225.0 / logical;
        let offset = preview_centre_scale_offset(logical, scale);

        // Where the corner lands: centre + (0 - centre) * scale, then shifted.
        let centre = logical / 2.0;
        assert!((centre - centre * scale + offset).abs() < 1e-3);
        // An unscaled box needs no correction at all.
        assert_eq!(preview_centre_scale_offset(1280.0, 1.0), 0.0);
    }

    #[test]
    fn expand_root_fills_the_target() {
        let canvas = UiCanvas {
            scale_mode: "expand".into(),
            ..default()
        };
        let node = canvas_root_node(&canvas);
        assert_eq!(node.width, Val::Percent(100.0));
        assert_eq!(node.left, Val::Px(0.0));
    }
}

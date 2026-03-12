//! Universal widget styling — fill, stroke, border radius, shadow, opacity, cursor.
//!
//! `UiWidgetStyle` is the single source of truth for a widget's visual appearance.
//! It maps to bevy_ui components at runtime via `apply_widget_style_system`.
//! The inspector exposes these fields in Figma-like Fill / Stroke / Effects sections.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ── Fill ────────────────────────────────────────────────────────────────────

/// A color stop in a gradient.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct GradientStop {
    /// Position along the gradient axis (0.0 = start, 1.0 = end).
    pub position: f32,
    pub color: Color,
}

/// How a widget's background is filled.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub enum UiFill {
    /// No fill (fully transparent).
    None,
    /// A single solid color.
    Solid(Color),
    /// Linear gradient defined by an angle (degrees, 0 = left→right) and color stops.
    LinearGradient {
        angle: f32,
        stops: Vec<GradientStop>,
    },
    /// Radial gradient from a center point (0..1 normalized) outward.
    RadialGradient {
        /// Center in normalized coordinates (0.5, 0.5 = center).
        center: [f32; 2],
        stops: Vec<GradientStop>,
    },
}

impl Default for UiFill {
    fn default() -> Self {
        Self::None
    }
}

impl UiFill {
    /// Convenience: solid color fill.
    pub fn solid(color: Color) -> Self {
        Self::Solid(color)
    }

    /// Convenience: two-stop linear gradient.
    pub fn linear(angle: f32, from: Color, to: Color) -> Self {
        Self::LinearGradient {
            angle,
            stops: vec![
                GradientStop {
                    position: 0.0,
                    color: from,
                },
                GradientStop {
                    position: 1.0,
                    color: to,
                },
            ],
        }
    }

    /// Returns the primary color (for fallback / bevy_ui BackgroundColor).
    /// For gradients this is the first stop color; for None it's transparent.
    pub fn primary_color(&self) -> Color {
        match self {
            Self::None => Color::NONE,
            Self::Solid(c) => *c,
            Self::LinearGradient { stops, .. } | Self::RadialGradient { stops, .. } => {
                stops.first().map(|s| s.color).unwrap_or(Color::NONE)
            }
        }
    }
}

// ── Stroke ──────────────────────────────────────────────────────────────────

/// Which sides of the border are drawn.
#[derive(Clone, Copy, Debug, Reflect, Serialize, Deserialize)]
pub struct UiSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl Default for UiSides {
    fn default() -> Self {
        Self {
            top: true,
            right: true,
            bottom: true,
            left: true,
        }
    }
}

impl UiSides {
    pub fn all() -> Self {
        Self::default()
    }

    pub fn none() -> Self {
        Self {
            top: false,
            right: false,
            bottom: false,
            left: false,
        }
    }
}

/// Border / outline stroke around the widget.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct UiStroke {
    pub color: Color,
    pub width: f32,
    pub sides: UiSides,
}

impl Default for UiStroke {
    fn default() -> Self {
        Self {
            color: Color::NONE,
            width: 0.0,
            sides: UiSides::all(),
        }
    }
}

impl UiStroke {
    pub fn new(color: Color, width: f32) -> Self {
        Self {
            color,
            width,
            sides: UiSides::all(),
        }
    }

    /// True if the stroke would be invisible.
    pub fn is_none(&self) -> bool {
        self.width <= 0.0 || self.color == Color::NONE
    }
}

// ── Border Radius ───────────────────────────────────────────────────────────

/// Per-corner border radius in logical pixels.
#[derive(Clone, Copy, Debug, Reflect, Serialize, Deserialize)]
pub struct UiBorderRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Default for UiBorderRadius {
    fn default() -> Self {
        Self {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 0.0,
            bottom_left: 0.0,
        }
    }
}

impl UiBorderRadius {
    /// All corners the same radius.
    pub fn all(r: f32) -> Self {
        Self {
            top_left: r,
            top_right: r,
            bottom_right: r,
            bottom_left: r,
        }
    }

    /// Convert to bevy's BorderRadius component.
    pub fn to_bevy(&self) -> bevy::ui::BorderRadius {
        bevy::ui::BorderRadius {
            top_left: bevy::ui::Val::Px(self.top_left),
            top_right: bevy::ui::Val::Px(self.top_right),
            bottom_right: bevy::ui::Val::Px(self.bottom_right),
            bottom_left: bevy::ui::Val::Px(self.bottom_left),
        }
    }
}

// ── Box Shadow ──────────────────────────────────────────────────────────────

/// Drop shadow effect.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct UiBoxShadow {
    pub color: Color,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
}

impl Default for UiBoxShadow {
    fn default() -> Self {
        Self {
            color: Color::srgba(0.0, 0.0, 0.0, 0.25),
            offset_x: 0.0,
            offset_y: 2.0,
            blur: 8.0,
            spread: 0.0,
        }
    }
}

// ── Cursor ──────────────────────────────────────────────────────────────────

/// Cursor icon to show when hovering this widget.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum UiCursor {
    #[default]
    Default,
    Pointer,
    Text,
    Grab,
    Grabbing,
    NotAllowed,
    Crosshair,
    EwResize,
    NsResize,
    Move,
}

// ── Text Style ──────────────────────────────────────────────────────────────

/// Text appearance properties for widgets that display text.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct UiTextStyle {
    pub color: Color,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    /// Horizontal alignment within the widget.
    pub align: UiTextAlign,
}

impl Default for UiTextStyle {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            size: 14.0,
            bold: false,
            italic: false,
            align: UiTextAlign::Center,
        }
    }
}

/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum UiTextAlign {
    Left,
    #[default]
    Center,
    Right,
}

// ── Padding ─────────────────────────────────────────────────────────────────

/// Padding in logical pixels (inner spacing between border and content).
#[derive(Clone, Copy, Debug, Default, Reflect, Serialize, Deserialize)]
pub struct UiPadding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl UiPadding {
    pub fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

// ── Widget Style (main component) ───────────────────────────────────────────

/// Universal visual style for any UI widget. This is the Figma-like styling layer.
///
/// Attached to every `UiWidget` entity. The runtime `apply_widget_style_system`
/// converts these fields into actual bevy_ui components (`BackgroundColor`,
/// `BorderRadius`, `BoxShadow`, etc.).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiWidgetStyle {
    /// Background fill (solid, gradient, or none).
    pub fill: UiFill,
    /// Border / outline.
    pub stroke: UiStroke,
    /// Per-corner border radius.
    pub border_radius: UiBorderRadius,
    /// Drop shadow (None = no shadow).
    pub shadow: Option<UiBoxShadow>,
    /// Overall opacity (0.0 = invisible, 1.0 = fully opaque).
    pub opacity: f32,
    /// Cursor to show on hover.
    pub cursor: UiCursor,
    /// Whether child content is clipped to the widget bounds.
    pub clip_content: bool,
    /// Text style (for widgets that display text).
    pub text: UiTextStyle,
    /// Inner padding.
    pub padding: UiPadding,
}

impl Default for UiWidgetStyle {
    fn default() -> Self {
        Self {
            fill: UiFill::None,
            stroke: UiStroke::default(),
            border_radius: UiBorderRadius::default(),
            shadow: None,
            opacity: 1.0,
            cursor: UiCursor::Default,
            clip_content: false,
            text: UiTextStyle::default(),
            padding: UiPadding::default(),
        }
    }
}

impl UiWidgetStyle {
    /// Merge a state override on top of this base style, returning the result.
    pub fn with_overrides(&self, overrides: &UiStateStyle) -> Self {
        let mut result = self.clone();
        if let Some(ref fill) = overrides.fill {
            result.fill = fill.clone();
        }
        if let Some(ref stroke) = overrides.stroke {
            result.stroke = stroke.clone();
        }
        if let Some(radius) = overrides.border_radius {
            result.border_radius = radius;
        }
        if let Some(ref shadow) = overrides.shadow {
            result.shadow = shadow.clone();
        }
        if let Some(opacity) = overrides.opacity {
            result.opacity = opacity;
        }
        if let Some(cursor) = overrides.cursor {
            result.cursor = cursor;
        }
        if let Some(color) = overrides.text_color {
            result.text.color = color;
        }
        if let Some(size) = overrides.text_size {
            result.text.size = size;
        }
        if let Some(padding) = overrides.padding {
            result.padding = padding;
        }
        result
    }
}

// ── State Style (per-interaction-state overrides) ───────────────────────────

/// Optional overrides applied per interaction state (hover, pressed, disabled).
///
/// Only `Some` fields override the base `UiWidgetStyle`; `None` fields inherit.
#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize)]
pub struct UiStateStyle {
    pub fill: Option<UiFill>,
    pub stroke: Option<UiStroke>,
    pub border_radius: Option<UiBorderRadius>,
    pub shadow: Option<Option<UiBoxShadow>>,
    pub opacity: Option<f32>,
    pub cursor: Option<UiCursor>,
    pub text_color: Option<Color>,
    pub text_size: Option<f32>,
    pub padding: Option<UiPadding>,
    /// Scale multiplier (1.0 = no change).
    pub scale: Option<f32>,
}

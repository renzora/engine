//! Interaction styling and UI animation components.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::style::{UiFill, UiStateStyle};

// ── Interaction Styles ──────────────────────────────────────────────────────

/// Per-state style overrides for interactive widgets.
///
/// Each state can override any field from `UiWidgetStyle` via `UiStateStyle`.
/// At runtime, the active state's overrides are merged on top of the base style.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiInteractionStyle {
    pub normal: UiStateStyle,
    pub hovered: UiStateStyle,
    pub pressed: UiStateStyle,
    pub disabled: UiStateStyle,
}

impl Default for UiInteractionStyle {
    fn default() -> Self {
        Self {
            normal: UiStateStyle::default(),
            hovered: UiStateStyle {
                fill: Some(UiFill::Solid(Color::srgba(0.35, 0.35, 0.9, 1.0))),
                ..default()
            },
            pressed: UiStateStyle {
                fill: Some(UiFill::Solid(Color::srgba(0.2, 0.2, 0.7, 1.0))),
                ..default()
            },
            disabled: UiStateStyle {
                opacity: Some(0.5),
                ..default()
            },
        }
    }
}

/// Legacy alias — use `UiStateStyle` for new code.
pub type UiStyleOverrides = UiStateStyle;

/// Duration for interpolating between interaction states.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiTransition {
    pub duration: f32,
}

impl Default for UiTransition {
    fn default() -> Self {
        Self { duration: 0.15 }
    }
}

// ── Tweening ────────────────────────────────────────────────────────────────

/// Easing function for UI animations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum EasingFunction {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseOutBack,
    EaseOutBounce,
}

impl EasingFunction {
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t * t,
            Self::EaseOut => 1.0 - (1.0 - t).powi(3),
            Self::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Self::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Self::EaseOutBounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                let mut t = t;
                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    t -= 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    t -= 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    t -= 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }
        }
    }
}

/// What property a tween animates.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub enum UiTweenProperty {
    Opacity,
    PositionX,
    PositionY,
    Width,
    Height,
    BgColorR,
    BgColorG,
    BgColorB,
    BgColorA,
    Scale,
    Rotation,
}

/// What to do when a tween completes.
#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize)]
pub enum TweenComplete {
    #[default]
    Remove,
    Loop,
    PingPong,
}

/// Active tween on a UI entity.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiTween {
    pub property: UiTweenProperty,
    pub start: f32,
    pub target: f32,
    pub duration: f32,
    pub elapsed: f32,
    pub easing: EasingFunction,
    pub on_complete: TweenComplete,
    /// True once — set start value from current on first tick.
    pub needs_init: bool,
}

impl Default for UiTween {
    fn default() -> Self {
        Self {
            property: UiTweenProperty::Opacity,
            start: 0.0,
            target: 1.0,
            duration: 0.3,
            elapsed: 0.0,
            easing: EasingFunction::EaseOut,
            on_complete: TweenComplete::Remove,
            needs_init: true,
        }
    }
}

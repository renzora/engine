//! Procedural tweens — animate properties over time via easing functions.
//!
//! Tweens are created by script commands (TweenPosition, TweenRotation, TweenScale)
//! and are consumed by the `update_procedural_tweens` system.

use bevy::prelude::*;
use std::f32::consts::PI;

/// Easing functions for tweens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    EaseInElastic,
    EaseOutElastic,
    EaseInBounce,
    EaseOutBounce,
}

impl EasingFunction {
    /// Parse an easing function from a string name.
    pub fn from_str(s: &str) -> Self {
        match s {
            "linear" => Self::Linear,
            "ease_in" => Self::EaseIn,
            "ease_out" => Self::EaseOut,
            "ease_in_out" => Self::EaseInOut,
            "ease_in_quad" => Self::EaseInQuad,
            "ease_out_quad" => Self::EaseOutQuad,
            "ease_in_out_quad" => Self::EaseInOutQuad,
            "ease_in_cubic" => Self::EaseInCubic,
            "ease_out_cubic" => Self::EaseOutCubic,
            "ease_in_out_cubic" => Self::EaseInOutCubic,
            "ease_in_back" => Self::EaseInBack,
            "ease_out_back" => Self::EaseOutBack,
            "ease_in_out_back" => Self::EaseInOutBack,
            "ease_in_elastic" => Self::EaseInElastic,
            "ease_out_elastic" => Self::EaseOutElastic,
            "ease_in_bounce" => Self::EaseInBounce,
            "ease_out_bounce" => Self::EaseOutBounce,
            _ => Self::EaseInOut,
        }
    }

    /// Evaluate the easing function at time t (0.0–1.0).
    pub fn evaluate(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
            }
            Self::EaseInQuad => t * t,
            Self::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOutQuad => {
                if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
            }
            Self::EaseInCubic => t * t * t,
            Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Self::EaseInOutCubic => {
                if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
            }
            Self::EaseInBack => {
                let c = 1.70158;
                (c + 1.0) * t * t * t - c * t * t
            }
            Self::EaseOutBack => {
                let c = 1.70158;
                1.0 + (c + 1.0) * (t - 1.0).powi(3) + c * (t - 1.0).powi(2)
            }
            Self::EaseInOutBack => {
                let c = 1.70158 * 1.525;
                if t < 0.5 {
                    ((2.0 * t).powi(2) * ((c + 1.0) * 2.0 * t - c)) / 2.0
                } else {
                    ((2.0 * t - 2.0).powi(2) * ((c + 1.0) * (t * 2.0 - 2.0) + c) + 2.0) / 2.0
                }
            }
            Self::EaseInElastic => {
                if t == 0.0 || t == 1.0 { return t; }
                let c = (2.0 * PI) / 3.0;
                -(2.0f32.powf(10.0 * t - 10.0)) * ((t * 10.0 - 10.75) * c).sin()
            }
            Self::EaseOutElastic => {
                if t == 0.0 || t == 1.0 { return t; }
                let c = (2.0 * PI) / 3.0;
                2.0f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c).sin() + 1.0
            }
            Self::EaseInBounce => 1.0 - Self::EaseOutBounce.evaluate(1.0 - t),
            Self::EaseOutBounce => {
                let n = 7.5625;
                let d = 2.75;
                if t < 1.0 / d {
                    n * t * t
                } else if t < 2.0 / d {
                    let t = t - 1.5 / d;
                    n * t * t + 0.75
                } else if t < 2.5 / d {
                    let t = t - 2.25 / d;
                    n * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d;
                    n * t * t + 0.984375
                }
            }
        }
    }
}

/// Which property a tween animates.
#[derive(Debug, Clone)]
pub enum TweenProperty {
    Position(Vec3),
    Rotation(Vec3), // Euler angles in degrees
    Scale(Vec3),
}

/// A running procedural tween on an entity.
#[derive(Component)]
pub struct ProceduralTween {
    pub property: TweenProperty,
    pub start_value: Option<Vec3>,
    pub easing: EasingFunction,
    pub duration: f32,
    pub elapsed: f32,
}

/// System that advances procedural tweens and applies them.
pub fn update_procedural_tweens(
    mut commands: Commands,
    time: Res<Time>,
    mut tweens: Query<(Entity, &mut ProceduralTween, &mut Transform)>,
) {
    for (entity, mut tween, mut transform) in tweens.iter_mut() {
        // Capture start value on first frame
        if tween.start_value.is_none() {
            tween.start_value = Some(match &tween.property {
                TweenProperty::Position(_) => transform.translation,
                TweenProperty::Rotation(_) => {
                    let (x, y, z) = transform.rotation.to_euler(EulerRot::XYZ);
                    Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
                }
                TweenProperty::Scale(_) => transform.scale,
            });
        }

        tween.elapsed += time.delta_secs();
        let t = (tween.elapsed / tween.duration).clamp(0.0, 1.0);
        let eased = tween.easing.evaluate(t);

        let start = tween.start_value.unwrap();
        let target = match &tween.property {
            TweenProperty::Position(v) => *v,
            TweenProperty::Rotation(v) => *v,
            TweenProperty::Scale(v) => *v,
        };

        let current = start.lerp(target, eased);

        match &tween.property {
            TweenProperty::Position(_) => {
                transform.translation = current;
            }
            TweenProperty::Rotation(_) => {
                transform.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    current.x.to_radians(),
                    current.y.to_radians(),
                    current.z.to_radians(),
                );
            }
            TweenProperty::Scale(_) => {
                transform.scale = current;
            }
        }

        // Remove tween when complete
        if t >= 1.0 {
            commands.entity(entity).remove::<ProceduralTween>();
        }
    }
}

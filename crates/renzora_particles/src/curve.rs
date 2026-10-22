//! Curves and gradients: the two node properties that are tables rather than
//! values.
//!
//! Both are authored as a short list of keys and sampled at a `t` the graph
//! supplies (usually `input/life`). The compiler copies each table into the
//! emitter's IR untouched, so the CPU backend samples it here and a later GPU
//! backend can bake the very same table into a texture row: the IR never holds
//! a pre-sampled approximation, which is what keeps the two backends agreeing.
//!
//! Keys are sorted on construction rather than trusted. A hand-written file
//! lists them in whatever order the author typed, and a binary search over an
//! unsorted table returns plausible nonsense rather than an error.

use bevy::math::Vec4;
use serde::{Deserialize, Serialize};

/// How a curve moves from one key to the next. Stored on the key the segment
/// *starts* at, so the last key's easing is never used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ease {
    #[default]
    Linear,
    /// Hold this key's value until the next key.
    Step,
    /// Hermite smoothstep: zero slope at both ends.
    Smooth,
    /// Quadratic ease in (slow start).
    EaseIn,
    /// Quadratic ease out (slow end).
    EaseOut,
    /// Cubic ease in and out.
    EaseInOut,
}

impl Ease {
    /// Remap a segment-local `x` in 0..1.
    pub fn apply(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            Ease::Linear => x,
            Ease::Step => 0.0,
            Ease::Smooth => x * x * (3.0 - 2.0 * x),
            Ease::EaseIn => x * x,
            Ease::EaseOut => 1.0 - (1.0 - x) * (1.0 - x),
            Ease::EaseInOut => {
                if x < 0.5 {
                    4.0 * x * x * x
                } else {
                    let y = -2.0 * x + 2.0;
                    1.0 - y * y * y * 0.5
                }
            }
        }
    }
}

/// One key of a float curve.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub t: f32,
    pub value: f32,
    #[serde(default)]
    pub ease: Ease,
}

/// One stop of a colour gradient, linear RGBA.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub t: f32,
    pub color: [f32; 4],
}

/// A curve as the IR carries it: keys sorted by `t`, non-finite keys dropped.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveTable {
    pub points: Vec<CurvePoint>,
}

impl CurveTable {
    pub fn new(points: &[CurvePoint]) -> Self {
        let mut points: Vec<CurvePoint> = points
            .iter()
            .copied()
            .filter(|p| p.t.is_finite() && p.value.is_finite())
            .collect();
        points.sort_by(|a, b| a.t.total_cmp(&b.t));
        Self { points }
    }

    /// Value at `t`. Clamps outside the keyed range; an empty curve is 1.0,
    /// because a curve is nearly always a multiplier and an empty one reading
    /// as zero made particles vanish the moment someone cleared it.
    pub fn sample(&self, t: f32) -> f32 {
        let pts = &self.points;
        match pts.len() {
            0 => 1.0,
            1 => pts[0].value,
            _ => {
                if t.is_nan() || t <= pts[0].t {
                    return pts[0].value;
                }
                let last = pts[pts.len() - 1];
                if t >= last.t {
                    return last.value;
                }
                let i = pts.partition_point(|p| p.t <= t).saturating_sub(1);
                let a = pts[i];
                let b = pts[i + 1];
                let span = b.t - a.t;
                if span <= f32::EPSILON {
                    return b.value;
                }
                let x = a.ease.apply((t - a.t) / span);
                a.value + (b.value - a.value) * x
            }
        }
    }
}

/// A gradient as the IR carries it: stops sorted by `t`.
#[derive(Clone, Debug, PartialEq)]
pub struct GradientTable {
    pub stops: Vec<GradientStop>,
}

impl GradientTable {
    pub fn new(stops: &[GradientStop]) -> Self {
        let mut stops: Vec<GradientStop> = stops
            .iter()
            .copied()
            .filter(|s| s.t.is_finite() && s.color.iter().all(|c| c.is_finite()))
            .collect();
        stops.sort_by(|a, b| a.t.total_cmp(&b.t));
        Self { stops }
    }

    /// Colour at `t`, linearly interpolated per channel. Empty is white, for
    /// the same reason an empty curve is 1.0.
    pub fn sample(&self, t: f32) -> Vec4 {
        let s = &self.stops;
        match s.len() {
            0 => Vec4::ONE,
            1 => Vec4::from_array(s[0].color),
            _ => {
                if t.is_nan() || t <= s[0].t {
                    return Vec4::from_array(s[0].color);
                }
                let last = s[s.len() - 1];
                if t >= last.t {
                    return Vec4::from_array(last.color);
                }
                let i = s.partition_point(|p| p.t <= t).saturating_sub(1);
                let a = s[i];
                let b = s[i + 1];
                let span = b.t - a.t;
                if span <= f32::EPSILON {
                    return Vec4::from_array(b.color);
                }
                let x = (t - a.t) / span;
                Vec4::from_array(a.color).lerp(Vec4::from_array(b.color), x)
            }
        }
    }
}

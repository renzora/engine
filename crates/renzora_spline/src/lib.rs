//! Spline primitive — control-point path with Catmull-Rom interpolation.
//!
//! A [`SplinePath`] is an authored set of world-local control points. The
//! sampling methods produce a smooth curve through every control point using
//! the centripetal Catmull-Rom formulation (chord-length parameterisation),
//! which avoids cusps and self-intersections around sharp turns.
//!
//! Splines are stored on regular entities alongside a `Transform`, so the
//! whole path can be translated/rotated/scaled as one unit. Other systems
//! (brush, road mesh, water strip) read the component and sample.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// A control-point path. Points are in the entity's local space.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SplinePath {
    /// Authored control points. Local space (transform them via the entity's
    /// `GlobalTransform` to get world positions).
    pub control_points: Vec<Vec3>,
    /// If true, the curve closes back on itself (last point connects to first).
    #[serde(default)]
    pub closed: bool,
}

impl SplinePath {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_points(points: impl IntoIterator<Item = Vec3>) -> Self {
        Self {
            control_points: points.into_iter().collect(),
            closed: false,
        }
    }

    /// Number of segments the curve spans. `points - 1` for open, `points` for closed.
    pub fn segment_count(&self) -> usize {
        let n = self.control_points.len();
        if n < 2 {
            0
        } else if self.closed {
            n
        } else {
            n - 1
        }
    }

    /// Sample the curve at global parameter `t` in `[0, segment_count()]`.
    /// Integer part = segment index; fractional part = interpolation within segment.
    /// Clamps to valid range.
    pub fn sample(&self, t: f32) -> Vec3 {
        let n = self.control_points.len();
        if n == 0 {
            return Vec3::ZERO;
        }
        if n == 1 {
            return self.control_points[0];
        }
        let segs = self.segment_count() as f32;
        if segs <= 0.0 {
            return self.control_points[0];
        }

        let t = t.clamp(0.0, segs - f32::EPSILON);
        let seg = t.floor() as usize;
        let local = t - seg as f32;

        let idx = |i: isize| -> Vec3 {
            let len = n as isize;
            let wrapped = if self.closed {
                ((i % len) + len) % len
            } else {
                i.clamp(0, len - 1)
            };
            self.control_points[wrapped as usize]
        };

        let p0 = idx(seg as isize - 1);
        let p1 = idx(seg as isize);
        let p2 = idx(seg as isize + 1);
        let p3 = idx(seg as isize + 2);

        catmull_rom(p0, p1, p2, p3, local)
    }

    /// Sample `count` evenly-spaced points along the full curve (in param space).
    /// For uniform arc-length spacing, use [`sample_by_arc_length`].
    pub fn sample_uniform(&self, count: usize) -> Vec<Vec3> {
        if count == 0 || self.control_points.is_empty() {
            return Vec::new();
        }
        let segs = self.segment_count() as f32;
        if segs <= 0.0 {
            return vec![self.control_points[0]; count];
        }
        (0..count)
            .map(|i| {
                let t = if count == 1 {
                    0.0
                } else {
                    i as f32 / (count - 1) as f32 * segs
                };
                self.sample(t)
            })
            .collect()
    }

    /// Sample the curve at approximately uniform arc-length intervals of
    /// `spacing` world-units. Returns at least the first control point.
    ///
    /// Uses a fine resampling of the curve (32 samples/segment) to estimate
    /// cumulative arc length, then marches along picking samples at each
    /// multiple of `spacing`.
    pub fn sample_by_arc_length(&self, spacing: f32) -> Vec<Vec3> {
        if self.control_points.is_empty() || spacing <= 0.0 {
            return self.control_points.clone();
        }
        let segs = self.segment_count();
        if segs == 0 {
            return self.control_points.clone();
        }

        const STEPS_PER_SEGMENT: usize = 32;
        let total_steps = segs * STEPS_PER_SEGMENT;
        let mut fine: Vec<(f32, Vec3)> = Vec::with_capacity(total_steps + 1);

        let mut acc = 0.0f32;
        let mut prev = self.sample(0.0);
        fine.push((0.0, prev));

        for i in 1..=total_steps {
            let t = i as f32 / STEPS_PER_SEGMENT as f32;
            let p = self.sample(t);
            acc += prev.distance(p);
            fine.push((acc, p));
            prev = p;
        }

        let total_len = acc;
        if total_len <= 0.0 {
            return vec![self.control_points[0]];
        }

        let count = (total_len / spacing).floor() as usize + 1;
        let mut out = Vec::with_capacity(count.max(2));
        let mut fine_idx = 0usize;
        for i in 0..count {
            let target = i as f32 * spacing;
            while fine_idx + 1 < fine.len() && fine[fine_idx + 1].0 < target {
                fine_idx += 1;
            }
            if fine_idx + 1 >= fine.len() {
                out.push(fine.last().unwrap().1);
                break;
            }
            let (d0, p0) = fine[fine_idx];
            let (d1, p1) = fine[fine_idx + 1];
            let span = (d1 - d0).max(1e-6);
            let local = ((target - d0) / span).clamp(0.0, 1.0);
            out.push(p0.lerp(p1, local));
        }
        if out.last().copied() != fine.last().map(|(_, p)| *p) {
            out.push(fine.last().unwrap().1);
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.control_points.is_empty()
    }

    pub fn push(&mut self, point: Vec3) {
        self.control_points.push(point);
    }
}

/// Centripetal Catmull-Rom interpolation between `p1` and `p2`, with `p0` and
/// `p3` as the surrounding control points. `t` in `[0, 1]`.
fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    // Chord-length (centripetal) parameterization with alpha = 0.5.
    const ALPHA: f32 = 0.5;
    let d01 = p0.distance(p1).max(1e-5);
    let d12 = p1.distance(p2).max(1e-5);
    let d23 = p2.distance(p3).max(1e-5);

    let t0 = 0.0;
    let t1 = t0 + d01.powf(ALPHA);
    let t2 = t1 + d12.powf(ALPHA);
    let t3 = t2 + d23.powf(ALPHA);

    let t = t1 + (t2 - t1) * t;

    let a1 = p0 * ((t1 - t) / (t1 - t0).max(1e-6)) + p1 * ((t - t0) / (t1 - t0).max(1e-6));
    let a2 = p1 * ((t2 - t) / (t2 - t1).max(1e-6)) + p2 * ((t - t1) / (t2 - t1).max(1e-6));
    let a3 = p2 * ((t3 - t) / (t3 - t2).max(1e-6)) + p3 * ((t - t2) / (t3 - t2).max(1e-6));

    let b1 = a1 * ((t2 - t) / (t2 - t0).max(1e-6)) + a2 * ((t - t0) / (t2 - t0).max(1e-6));
    let b2 = a2 * ((t3 - t) / (t3 - t1).max(1e-6)) + a3 * ((t - t1) / (t3 - t1).max(1e-6));

    b1 * ((t2 - t) / (t2 - t1).max(1e-6)) + b2 * ((t - t1) / (t2 - t1).max(1e-6))
}

#[derive(Default)]
pub struct SplinePlugin;

impl Plugin for SplinePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SplinePlugin");
        app.register_type::<SplinePath>();
    }
}

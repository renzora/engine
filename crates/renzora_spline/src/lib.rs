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

renzora::add!(SplinePlugin);

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;

    fn assert_vec_eq(a: Vec3, b: Vec3) {
        assert!(
            (a - b).length() < EPS,
            "vectors differ: {a:?} vs {b:?} (dist {})",
            (a - b).length()
        );
    }

    #[test]
    fn segment_count_open_and_closed() {
        // < 2 points => 0 segments
        assert_eq!(SplinePath::default().segment_count(), 0);
        assert_eq!(SplinePath::with_points([Vec3::ZERO]).segment_count(), 0);

        // open: points - 1
        let open = SplinePath::with_points([Vec3::ZERO, Vec3::X, Vec3::Y]);
        assert_eq!(open.segment_count(), 2);

        // closed: points
        let mut closed = open.clone();
        closed.closed = true;
        assert_eq!(closed.segment_count(), 3);
    }

    #[test]
    fn empty_spline_samples_to_zero() {
        let s = SplinePath::default();
        assert_vec_eq(s.sample(0.0), Vec3::ZERO);
        assert_vec_eq(s.sample(5.0), Vec3::ZERO);
        assert!(s.is_empty());
    }

    #[test]
    fn single_point_samples_to_that_point() {
        let p = Vec3::new(3.0, -2.0, 7.0);
        let s = SplinePath::with_points([p]);
        assert_vec_eq(s.sample(0.0), p);
        assert_vec_eq(s.sample(1.0), p);
        assert_vec_eq(s.sample(-10.0), p);
    }

    #[test]
    fn endpoints_match_control_points() {
        // Catmull-Rom passes through every control point. Sampling at integer
        // params should land exactly on the corresponding control point.
        let pts = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(3.0, 2.0, 1.0),
            Vec3::new(5.0, 0.0, 0.0),
        ];
        let s = SplinePath::with_points(pts);
        // Start endpoint.
        assert_vec_eq(s.sample(0.0), pts[0]);
        // Interior control points at integer params.
        assert_vec_eq(s.sample(1.0), pts[1]);
        assert_vec_eq(s.sample(2.0), pts[2]);
        // End endpoint: clamps to just below segment_count, but should be
        // essentially the last point.
        assert_vec_eq(s.sample(3.0), pts[3]);
    }

    #[test]
    fn straight_two_point_midpoint() {
        // A straight 2-point spline sampled at t=0.5 must be the midpoint.
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(4.0, 0.0, 0.0);
        let s = SplinePath::with_points([a, b]);
        assert_vec_eq(s.sample(0.0), a);
        assert_vec_eq(s.sample(0.5), Vec3::new(2.0, 0.0, 0.0));
        // t=1.0 clamps to just under 1; should be at the far end.
        assert_vec_eq(s.sample(1.0), b);
    }

    #[test]
    fn collinear_points_stay_on_the_line() {
        // Four evenly-spaced collinear points: the curve is the straight line,
        // so the midpoint of the middle segment is the average of its ends.
        let pts = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
        ];
        let s = SplinePath::with_points(pts);
        // Middle of segment 1 (between pts[1] and pts[2]).
        let mid = s.sample(1.5);
        assert!(mid.y.abs() < EPS && mid.z.abs() < EPS, "left the line: {mid:?}");
        assert!((mid.x - 1.5).abs() < EPS, "expected x=1.5, got {}", mid.x);
    }

    #[test]
    fn sample_clamps_out_of_range() {
        let a = Vec3::ZERO;
        let b = Vec3::new(2.0, 0.0, 0.0);
        let s = SplinePath::with_points([a, b]);
        // Negative clamps to start.
        assert_vec_eq(s.sample(-5.0), a);
        // Beyond segment_count clamps to the end.
        assert_vec_eq(s.sample(100.0), b);
    }

    #[test]
    fn sample_uniform_counts_and_endpoints() {
        let a = Vec3::ZERO;
        let b = Vec3::new(4.0, 0.0, 0.0);
        let s = SplinePath::with_points([a, b]);

        // count 0 => empty
        assert!(s.sample_uniform(0).is_empty());

        // count 1 => single sample at t=0 (start)
        let one = s.sample_uniform(1);
        assert_eq!(one.len(), 1);
        assert_vec_eq(one[0], a);

        // count 5 evenly spaced; first==start, last==end, middle==midpoint.
        let five = s.sample_uniform(5);
        assert_eq!(five.len(), 5);
        assert_vec_eq(five[0], a);
        assert_vec_eq(five[4], b);
        assert_vec_eq(five[2], Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn sample_uniform_empty_spline() {
        let s = SplinePath::default();
        assert!(s.sample_uniform(3).is_empty());
    }

    #[test]
    fn sample_by_arc_length_straight_line_spacing() {
        // Straight line of length 4. Spacing 1.0 should give points at
        // x = 0,1,2,3 plus the final endpoint at 4 => 5 points total.
        let a = Vec3::ZERO;
        let b = Vec3::new(4.0, 0.0, 0.0);
        let s = SplinePath::with_points([a, b]);
        let pts = s.sample_by_arc_length(1.0);
        assert_eq!(pts.len(), 5, "got {pts:?}");
        for (i, p) in pts.iter().enumerate() {
            assert!(
                (p.x - i as f32).abs() < 1e-3,
                "point {i} expected x≈{i}, got {p:?}"
            );
            assert!(p.y.abs() < 1e-3 && p.z.abs() < 1e-3);
        }
    }

    #[test]
    fn sample_by_arc_length_invalid_spacing_returns_points() {
        let pts = [Vec3::ZERO, Vec3::X, Vec3::Y];
        let s = SplinePath::with_points(pts);
        // Non-positive spacing returns the raw control points unchanged.
        assert_eq!(s.sample_by_arc_length(0.0), pts.to_vec());
        assert_eq!(s.sample_by_arc_length(-1.0), pts.to_vec());
    }

    #[test]
    fn push_and_with_points_build_same_path() {
        let mut a = SplinePath::new();
        a.push(Vec3::ZERO);
        a.push(Vec3::X);
        let b = SplinePath::with_points([Vec3::ZERO, Vec3::X]);
        assert_eq!(a.control_points, b.control_points);
        assert!(!a.closed);
    }

    #[test]
    fn closed_loop_wraps_around() {
        // A closed square has one more segment than open; sampling at the
        // final segment's start should land on the last control point, and
        // the curve should pass through each control point at integer params.
        let pts = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 2.0),
            Vec3::new(0.0, 0.0, 2.0),
        ];
        let mut s = SplinePath::with_points(pts);
        s.closed = true;
        assert_eq!(s.segment_count(), 4);
        assert_vec_eq(s.sample(0.0), pts[0]);
        assert_vec_eq(s.sample(1.0), pts[1]);
        assert_vec_eq(s.sample(2.0), pts[2]);
        assert_vec_eq(s.sample(3.0), pts[3]);
    }

    #[test]
    fn catmull_rom_endpoints_are_exact() {
        // The interpolation must return p1 at t=0 and p2 at t=1 regardless of
        // the surrounding points.
        let p0 = Vec3::new(-1.0, 0.0, 0.0);
        let p1 = Vec3::new(0.0, 1.0, 0.0);
        let p2 = Vec3::new(1.0, 1.0, 0.0);
        let p3 = Vec3::new(2.0, 0.0, 0.0);
        assert_vec_eq(catmull_rom(p0, p1, p2, p3, 0.0), p1);
        assert_vec_eq(catmull_rom(p0, p1, p2, p3, 1.0), p2);
    }

    #[test]
    fn serde_round_trip() {
        let mut s = SplinePath::with_points([Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0)]);
        s.closed = true;
        let json = serde_json::to_string(&s).expect("serialize");
        let back: SplinePath = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(s.control_points, back.control_points);
        assert_eq!(s.closed, back.closed);
    }
}

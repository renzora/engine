//! Spring (jiggle) chains: hair, tails, ears, antennae.
//!
//! Each joint of the chain is a particle pulled by a damped spring toward where
//! the animation puts it *relative to the particle before it*, so the chain
//! keeps the animated shape as its rest shape and only the motion of the
//! character (and gravity) bends it. Segment lengths are then enforced exactly
//! and sphere colliders push particles out, position-based-dynamics style,
//! with the velocity recovered from the corrected positions; that is what keeps
//! a stiff chain from exploding where a pure force model would need a tiny
//! step.
//!
//! The simulation runs at a fixed [`STEP`] regardless of the frame rate, and
//! the animated goals are interpolated across the frame for the steps inside
//! it. So a game at 30 fps and one at 144 fps take the same steps against the
//! same goals and end up with the same hair; only how often it is drawn
//! differs. Between steps the chain is shown interpolated (by segment, not by
//! absolute position, so a fast-moving anchor does not drag a lagging copy of
//! the hair behind it).

use bevy::math::Vec3;

/// The fixed simulation step, seconds.
pub const STEP: f32 = 1.0 / 120.0;
/// Most steps one frame may take. A hitch longer than this drops the rest of
/// its time instead of spending the next frame catching up, which would make
/// the next frame slower still.
pub const MAX_STEPS: u32 = 12;

/// How a chain moves.
#[derive(Clone, Copy, Debug)]
pub struct SpringParams {
    /// Pull toward the animated shape, per second squared. Higher is stiffer.
    pub stiffness: f32,
    /// Damping of motion *relative to the animation*, per second. Higher
    /// settles faster with less wobble.
    pub damping: f32,
    /// Air drag on absolute motion, per second: velocity lost to the air.
    pub drag: f32,
    /// Acceleration applied to every particle, world space.
    pub gravity: Vec3,
    /// Particle radius, for colliders.
    pub radius: f32,
}

impl Default for SpringParams {
    fn default() -> Self {
        Self { stiffness: 150.0, damping: 8.0, drag: 0.5, gravity: Vec3::new(0.0, -3.0, 0.0), radius: 0.02 }
    }
}

/// A sphere the chain cannot enter.
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

/// One chain's state. Index 0 is the anchor, pinned to the animation; the rest
/// are simulated.
#[derive(Clone, Debug, Default)]
pub struct SpringChain {
    pos: Vec<Vec3>,
    prev: Vec<Vec3>,
    vel: Vec<Vec3>,
    /// The goals at the end of the last frame, so this frame's steps can
    /// interpolate from them.
    goals_last: Vec<Vec3>,
    accumulator: f32,
}

impl SpringChain {
    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    /// Snap to the animated pose at rest. For the first frame, a teleport, or
    /// a chain that changed shape.
    pub fn reset(&mut self, animated: &[Vec3]) {
        self.pos = animated.to_vec();
        self.prev = animated.to_vec();
        self.goals_last = animated.to_vec();
        self.vel = vec![Vec3::ZERO; animated.len()];
        self.accumulator = 0.0;
    }

    /// Advance by `dt` toward this frame's `animated` joint positions and
    /// return the positions to pose the bones from.
    pub fn advance(&mut self, dt: f32, animated: &[Vec3], params: &SpringParams, colliders: &[Sphere]) -> Vec<Vec3> {
        if self.pos.len() != animated.len() || animated.is_empty() {
            self.reset(animated);
        }
        let dt = dt.max(0.0);
        let start = std::mem::take(&mut self.goals_last);
        let goal_vel: Vec<Vec3> = if dt > 0.0 {
            start.iter().zip(animated).map(|(s, a)| (*a - *s) / dt).collect()
        } else {
            vec![Vec3::ZERO; animated.len()]
        };

        let mut goals = animated.to_vec();
        let mut t = -self.accumulator;
        self.accumulator += dt;
        let mut steps = 0;
        while self.accumulator >= STEP {
            t += STEP;
            let s = if dt > 0.0 { (t / dt).clamp(0.0, 1.0) } else { 1.0 };
            for (g, (a, b)) in goals.iter_mut().zip(start.iter().zip(animated)) {
                *g = a.lerp(*b, s);
            }
            self.prev.clone_from(&self.pos);
            self.step(STEP, &goals, &goal_vel, params, colliders);
            self.accumulator -= STEP;
            steps += 1;
            if steps >= MAX_STEPS {
                self.accumulator = 0.0;
                break;
            }
        }
        self.goals_last = animated.to_vec();

        // Interpolate each segment between the last two steps, hung from the
        // anchor where the animation has it *now*.
        let alpha = (self.accumulator / STEP).clamp(0.0, 1.0);
        let mut out = Vec::with_capacity(animated.len());
        out.push(animated[0]);
        for i in 1..animated.len() {
            let before = self.prev[i] - self.prev[i - 1];
            let after = self.pos[i] - self.pos[i - 1];
            let seg = before.lerp(after, alpha);
            let len = (animated[i] - animated[i - 1]).length();
            let seg = seg.try_normalize().map_or(animated[i] - animated[i - 1], |d| d * len);
            out.push(out[i - 1] + seg);
        }
        out
    }

    fn step(&mut self, h: f32, goals: &[Vec3], goal_vel: &[Vec3], p: &SpringParams, colliders: &[Sphere]) {
        self.pos[0] = goals[0];
        self.vel[0] = goal_vel[0];
        let air = (-p.drag.max(0.0) * h).exp();
        for i in 1..self.pos.len() {
            let rest = goals[i] - goals[i - 1];
            let len = rest.length();
            let target = self.pos[i - 1] + rest;
            let relative = self.vel[i] - goal_vel[i];
            let accel = (target - self.pos[i]) * p.stiffness.max(0.0) - relative * p.damping.max(0.0) + p.gravity;
            let v = (self.vel[i] + accel * h) * air;
            let old = self.pos[i];
            let mut x = old + v * h;

            let dir = (x - self.pos[i - 1])
                .try_normalize()
                .or_else(|| rest.try_normalize())
                .unwrap_or(Vec3::Y);
            x = self.pos[i - 1] + dir * len;
            for s in colliders {
                let d = x - s.center;
                let r = s.radius + p.radius;
                if d.length_squared() < r * r {
                    x = s.center + d.try_normalize().unwrap_or(Vec3::Y) * r;
                }
            }
            if !x.is_finite() {
                // Never let one bad frame poison the chain for good.
                x = self.pos[i - 1] + rest;
            }
            self.vel[i] = (x - old) / h;
            self.pos[i] = x;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A three-segment chain standing up from the origin.
    fn upright() -> Vec<Vec3> {
        (0..4).map(|i| Vec3::new(0.0, i as f32 * 0.1, 0.0)).collect()
    }

    fn displaced(chain: &mut SpringChain, rest: &[Vec3]) {
        chain.reset(rest);
        for i in 1..rest.len() {
            chain.pos[i] = Vec3::new(i as f32 * 0.07, i as f32 * 0.07, 0.0);
            chain.prev[i] = chain.pos[i];
        }
    }

    fn run(chain: &mut SpringChain, fps: f32, seconds: f32, anim: impl Fn(f32) -> Vec<Vec3>, p: &SpringParams) -> Vec<Vec3> {
        let dt = 1.0 / fps;
        let frames = (seconds * fps).round() as usize;
        let mut out = Vec::new();
        for f in 1..=frames {
            out = chain.advance(dt, &anim(f as f32 * dt), p, &[]);
        }
        out
    }

    #[test]
    fn settles_back_to_the_animated_shape() {
        let rest = upright();
        let mut chain = SpringChain::default();
        displaced(&mut chain, &rest);
        let p = SpringParams { gravity: Vec3::ZERO, ..Default::default() };
        let out = run(&mut chain, 60.0, 3.0, |_| upright(), &p);
        for (o, r) in out.iter().zip(&rest) {
            assert!(o.distance(*r) < 1e-2, "{o:?} vs {r:?}");
        }
    }

    #[test]
    fn a_stiff_undamped_chain_stays_bounded_and_keeps_its_lengths() {
        let rest = upright();
        let mut chain = SpringChain::default();
        displaced(&mut chain, &rest);
        let p = SpringParams { stiffness: 5000.0, damping: 0.0, drag: 0.0, gravity: Vec3::ZERO, radius: 0.0 };
        let out = run(&mut chain, 60.0, 10.0, |_| upright(), &p);
        for w in out.windows(2) {
            assert!(w[0].is_finite() && w[1].is_finite());
            assert!(((w[1] - w[0]).length() - 0.1).abs() < 1e-3);
        }
        for o in &out {
            assert!(o.length() <= 0.3 + 1e-3);
        }
    }

    #[test]
    fn the_result_does_not_depend_on_the_frame_rate() {
        let rest = upright();
        let p = SpringParams::default();
        let mut slow = SpringChain::default();
        let mut fast = SpringChain::default();
        displaced(&mut slow, &rest);
        displaced(&mut fast, &rest);
        let a = run(&mut slow, 30.0, 2.0, |_| upright(), &p);
        let b = run(&mut fast, 144.0, 2.0, |_| upright(), &p);
        for (x, y) in a.iter().zip(&b) {
            assert!(x.distance(*y) < 2e-3, "{x:?} vs {y:?}");
        }

        // A moving anchor too: the goals are interpolated inside each frame.
        let moving = |t: f32| -> Vec<Vec3> {
            let o = Vec3::new((t * 3.0).sin() * 0.5, 0.0, 0.0);
            upright().into_iter().map(|v| v + o).collect()
        };
        let mut slow = SpringChain::default();
        let mut fast = SpringChain::default();
        slow.reset(&moving(0.0));
        fast.reset(&moving(0.0));
        let a = run(&mut slow, 30.0, 2.0, moving, &p);
        let b = run(&mut fast, 144.0, 2.0, moving, &p);
        for (x, y) in a.iter().zip(&b) {
            assert!(x.distance(*y) < 2e-2, "{x:?} vs {y:?}");
        }
    }

    #[test]
    fn gravity_bends_a_slack_chain_down() {
        let rest: Vec<Vec3> = (0..4).map(|i| Vec3::new(i as f32 * 0.1, 0.0, 0.0)).collect();
        let mut chain = SpringChain::default();
        chain.reset(&rest);
        let p = SpringParams { stiffness: 5.0, gravity: Vec3::new(0.0, -9.81, 0.0), ..Default::default() };
        let out = run(&mut chain, 60.0, 2.0, |_| rest.clone(), &p);
        assert!(out[3].y < -0.1, "tip at {:?}", out[3]);
    }

    #[test]
    fn colliders_push_particles_out() {
        let rest = upright();
        let mut chain = SpringChain::default();
        chain.reset(&rest);
        // Just off the tip's rest spot, so the tip is pushed aside (toward -X)
        // instead of balancing on a perfectly symmetric contact.
        let ball = Sphere { center: Vec3::new(0.03, 0.3, 0.0), radius: 0.05 };
        let p = SpringParams { gravity: Vec3::ZERO, ..Default::default() };
        let mut out = Vec::new();
        for _ in 0..120 {
            out = chain.advance(1.0 / 60.0, &rest, &p, &[ball]);
        }
        assert!(out[3].x < -0.01, "tip {:?}", out[3]);
        assert!(out[3].distance(ball.center) > 0.05, "tip {:?}", out[3]);
    }
}

//! Smoothing a remotely moved entity between the positions that arrive for it.
//!
//! Updates arrive 20 to 60 times a second, at uneven intervals, and some never
//! arrive. Drawing each as it lands makes motion stutter. Instead every remote
//! entity keeps a short history of `(time, transform)` and is drawn at a point
//! [`INTERPOLATION_DELAY`] in the past, between the two samples either side of
//! it. The delay is what a lost packet has to hide behind: as long as a later
//! sample has arrived, the gap is bridged.
//!
//! Times are always **server time** in seconds: the server stamps snapshots
//! with its own clock, and a client stamps what it sends with its estimate of
//! that clock ([`NetClock`]), so both directions share one timeline.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::protocol::WireTransform;

/// How far behind the newest sample remote entities are drawn. Covers two
/// snapshot intervals at the default rate plus some jitter.
pub const INTERPOLATION_DELAY: f64 = 0.1;
/// History older than this is dropped.
const HISTORY: f64 = 1.0;

/// Recent transforms of a remotely moved entity.
#[derive(Component, Default, Debug)]
pub struct NetInterpolation {
    samples: VecDeque<(f64, WireTransform)>,
    /// Off: snap to the newest sample (`NetworkTransform::interpolate` false).
    pub smooth: bool,
}

impl NetInterpolation {
    pub fn new(smooth: bool) -> Self {
        Self { samples: VecDeque::new(), smooth }
    }

    pub fn push(&mut self, time: f64, t: WireTransform) {
        // Out of order (an unreliable message overtaken by a later one):
        // slot it in, rather than let time run backwards.
        let at = self.samples.iter().rposition(|(s, _)| *s <= time).map_or(0, |i| i + 1);
        if at > 0 && self.samples[at - 1].0 == time {
            return;
        }
        self.samples.insert(at, (time, t));
        let newest = self.samples.back().map(|(s, _)| *s).unwrap_or(time);
        while self.samples.len() > 2 && self.samples.front().is_some_and(|(s, _)| newest - s > HISTORY) {
            self.samples.pop_front();
        }
    }

    /// The transform to draw at server time `render_time`.
    pub fn sample(&self, render_time: f64) -> Option<WireTransform> {
        if !self.smooth {
            return self.samples.back().map(|(_, t)| *t);
        }
        let (first_t, first) = self.samples.front()?;
        if render_time <= *first_t {
            return Some(*first);
        }
        for pair in self.samples.iter().collect::<Vec<_>>().windows(2) {
            let ((a_t, a), (b_t, b)) = (pair[0], pair[1]);
            if render_time >= *a_t && render_time <= *b_t {
                let span = (b_t - a_t).max(1e-6);
                return Some(a.lerp(b, ((render_time - a_t) / span) as f32));
            }
        }
        // Past the newest: hold it. Extrapolating guesses, and a wrong guess
        // snapping back reads worse than a brief pause.
        self.samples.back().map(|(_, t)| *t)
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// This peer's view of the server's clock.
///
/// On the server it is the server's own time. On a client it starts from the
/// first snapshot's stamp plus half a round trip, advances with the local
/// frame clock, and is nudged toward each new stamp so drift and changing
/// latency are absorbed gradually instead of as jumps.
#[derive(Resource, Debug, Default)]
pub struct NetClock {
    pub now: f64,
    pub synced: bool,
}

impl NetClock {
    pub fn advance(&mut self, dt: f64) {
        self.now += dt;
    }

    /// A snapshot stamped `server_time` arrived with `rtt` seconds round trip.
    pub fn observe(&mut self, server_time: f64, rtt: f64) {
        let target = server_time + rtt * 0.5;
        if !self.synced || (target - self.now).abs() > 0.25 {
            self.now = target;
            self.synced = true;
        } else {
            self.now += (target - self.now) * 0.05;
        }
    }

    pub fn render_time(&self) -> f64 {
        self.now - INTERPOLATION_DELAY
    }
}

/// Draw every interpolated entity at the clock's render time.
pub fn apply_interpolation(clock: Res<NetClock>, mut q: Query<(&NetInterpolation, &mut Transform)>) {
    if !clock.synced {
        return;
    }
    let at = clock.render_time();
    for (interp, mut transform) in &mut q {
        if let Some(t) = interp.sample(at) {
            let mut next = *transform;
            t.apply(&mut next);
            // Only write a real change, so change detection downstream (and
            // the replication diff on a host) means something.
            transform.set_if_neq(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> WireTransform {
        WireTransform { translation: [x, 0.0, 0.0], rotation: None, scale: None }
    }

    #[test]
    fn draws_between_the_samples_around_it() {
        let mut i = NetInterpolation::new(true);
        i.push(1.0, at(0.0));
        i.push(1.1, at(10.0));
        let x = i.sample(1.05).unwrap().translation[0];
        assert!((x - 5.0).abs() < 1e-3);
        assert_eq!(i.sample(0.5).unwrap().translation[0], 0.0);
        assert_eq!(i.sample(9.0).unwrap().translation[0], 10.0);
    }

    #[test]
    fn a_late_sample_is_slotted_in_order() {
        let mut i = NetInterpolation::new(true);
        i.push(1.0, at(0.0));
        i.push(1.2, at(20.0));
        i.push(1.1, at(10.0));
        let x = i.sample(1.15).unwrap().translation[0];
        assert!((x - 15.0).abs() < 1e-3);
    }

    #[test]
    fn unsmoothed_snaps_to_newest() {
        let mut i = NetInterpolation::new(false);
        i.push(1.0, at(0.0));
        i.push(2.0, at(4.0));
        assert_eq!(i.sample(0.0).unwrap().translation[0], 4.0);
    }

    #[test]
    fn clock_jumps_once_then_drifts() {
        let mut c = NetClock::default();
        c.observe(100.0, 0.1);
        assert!((c.now - 100.05).abs() < 1e-9);
        c.advance(0.016);
        c.observe(100.02, 0.1);
        assert!((c.now - 100.066).abs() < 0.01);
    }
}

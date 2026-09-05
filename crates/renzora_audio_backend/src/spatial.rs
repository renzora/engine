//! Turning "where is this sound" into a gain and a pan.
//!
//! Deliberately the cheap model rather than HRTF: distance attenuation plus
//! stereo positioning, which is what the engine's `AudioPlayer` has always
//! exposed (`min_distance`, `max_distance`, a rolloff curve) and what a game
//! mix is actually built on. Anything better belongs behind the same call, so
//! swapping it later moves no call sites.

/// How loudness falls off between the min and max distance.
///
/// Named to match the engine's `RolloffType`, because these two enums are the
/// same choice seen from either side of the boundary and a second vocabulary
/// for it would only ever be a translation table to get wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rolloff {
    /// Fast near the source, gentle further out. The default, and the one that
    /// sounds like distance.
    #[default]
    Logarithmic,
    /// Straight line from full to silent. Predictable, useful for gameplay
    /// audibility rules where "can they hear me at 20 m" needs a flat answer.
    Linear,
}

/// The ears: a world position and the direction they face.
///
/// `right` rather than `forward` because the only thing the pan calculation
/// needs is which side a source is on, and deriving that from a forward vector
/// means reconstructing the right vector on every emitter every frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Listener {
    pub position: [f32; 3],
    /// Unit vector pointing out of the listener's right ear.
    pub right: [f32; 3],
}

impl Default for Listener {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            right: [1.0, 0.0, 0.0],
        }
    }
}

/// A positioned sound source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emitter {
    pub position: [f32; 3],
    /// Distance at which attenuation *starts*. Inside this radius the source
    /// plays at full gain — without it, a sound sitting on the listener would
    /// divide by a vanishing distance.
    pub min_distance: f32,
    /// Distance at which the source reaches silence.
    pub max_distance: f32,
    pub rolloff: Rolloff,
}

impl Default for Emitter {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            min_distance: 1.0,
            max_distance: 100.0,
            rolloff: Rolloff::Logarithmic,
        }
    }
}

impl Emitter {
    /// `(gain, pan)` for this emitter heard from `listener`.
    ///
    /// Pan collapses toward centre as the source approaches the listener. That
    /// is not a nicety: at very short range the direction to a source swings
    /// wildly for tiny movements, and a hard-panned sound would flick between
    /// ears as the player turns. Blending it out below `min_distance` is what
    /// stops a footstep at your feet from strobing across the stereo field.
    pub fn gain_and_pan(&self, listener: &Listener) -> (f32, f32) {
        let to = [
            self.position[0] - listener.position[0],
            self.position[1] - listener.position[1],
            self.position[2] - listener.position[2],
        ];
        let distance = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
        let gain = self.attenuation(distance);

        if distance <= f32::EPSILON {
            return (gain, 0.0);
        }
        let side = (to[0] * listener.right[0]
            + to[1] * listener.right[1]
            + to[2] * listener.right[2])
            / distance;
        // `min_distance` doubles as the radius inside which direction stops
        // being trustworthy — the same number, because the point at which a
        // source is "on top of you" is the point at which it stops attenuating.
        let proximity = (distance / self.min_distance.max(f32::EPSILON)).clamp(0.0, 1.0);
        (gain, (side * proximity).clamp(-1.0, 1.0))
    }

    /// Gain from distance alone, 1.0 inside `min_distance` and 0.0 beyond
    /// `max_distance`.
    fn attenuation(&self, distance: f32) -> f32 {
        let min = self.min_distance.max(0.0);
        let max = self.max_distance;
        if distance <= min {
            return 1.0;
        }
        // A max at or inside min is a degenerate emitter — treat the boundary
        // as a cliff rather than dividing by zero.
        if max <= min {
            return 0.0;
        }
        let t = ((distance - min) / (max - min)).clamp(0.0, 1.0);
        match self.rolloff {
            Rolloff::Linear => 1.0 - t,
            // Squared falloff: -6 dB per doubling in the near field, which is
            // where it matters, and a long quiet tail rather than an audible
            // cut-off at max_distance.
            Rolloff::Logarithmic => (1.0 - t) * (1.0 - t),
        }
    }
}

/// Constant-power stereo pan for a **source**: -1 hard left, 0 centre, 1 hard
/// right.
///
/// Constant *power* rather than the obvious linear crossfade, because a linear
/// pan drops about 3 dB in the middle — a sound swept across the field audibly
/// dips as it passes centre, which is exactly where most sounds sit.
///
/// Note this is 0.707 per channel at centre, not 1.0. That is correct for
/// *placing* a source in the field, and wrong for a bus — see [`balance_gains`].
pub fn pan_gains(pan: f32) -> [f32; 2] {
    // -1..1 → 0..π/2, so the two gains are cos/sin of the same angle and their
    // squares always sum to 1.
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * (core::f32::consts::FRAC_PI_4);
    [angle.cos(), angle.sin()]
}

/// Stereo **balance** for a bus: attenuate one side, leave centre alone.
///
/// A bus carries an already-stereo mix, so its pan knob is a balance control,
/// not a placement control. Running [`pan_gains`] here instead would drop every
/// centred bus by 3 dB — and with a bus and a master in the chain, a plain
/// unity board would arrive at half amplitude for no reason a user could see.
/// Centre is exactly unity here, which is what "I didn't touch the pan knob"
/// has to mean.
pub fn balance_gains(pan: f32) -> [f32; 2] {
    let pan = pan.clamp(-1.0, 1.0);
    [(1.0 - pan).min(1.0), (1.0 + pan).min(1.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "{a} != {b}");
    }

    #[test]
    fn centre_pan_is_equal_power_not_equal_amplitude() {
        let [l, r] = pan_gains(0.0);
        approx(l, r);
        // The whole point: powers sum to 1, so amplitudes are ~0.707, not 0.5.
        approx(l * l + r * r, 1.0);
        approx(l, core::f32::consts::FRAC_1_SQRT_2);
    }

    /// A bus carries an already-stereo mix: leaving its pan alone must not
    /// attenuate it. This is the bug a shared pan law would hide.
    #[test]
    fn a_centred_balance_is_exactly_unity_on_both_channels() {
        let [l, r] = balance_gains(0.0);
        approx(l, 1.0);
        approx(r, 1.0);
    }

    #[test]
    fn balance_attenuates_only_the_side_it_moves_away_from() {
        let [l, r] = balance_gains(1.0);
        approx(l, 0.0);
        approx(r, 1.0);
        let [l, r] = balance_gains(-0.5);
        approx(l, 1.0);
        approx(r, 0.5);
    }

    #[test]
    fn hard_pans_silence_the_far_channel() {
        approx(pan_gains(-1.0)[1], 0.0);
        approx(pan_gains(1.0)[0], 0.0);
    }

    #[test]
    fn power_is_constant_across_the_whole_sweep() {
        for i in 0..=20 {
            let pan = -1.0 + i as f32 / 10.0;
            let [l, r] = pan_gains(pan);
            approx(l * l + r * r, 1.0);
        }
    }

    #[test]
    fn inside_min_distance_a_source_is_at_full_gain() {
        let e = Emitter::default();
        let l = Listener::default();
        let (gain, _) = e.gain_and_pan(&l);
        approx(gain, 1.0);
    }

    #[test]
    fn beyond_max_distance_a_source_is_silent() {
        let e = Emitter {
            position: [500.0, 0.0, 0.0],
            ..Default::default()
        };
        let (gain, _) = e.gain_and_pan(&Listener::default());
        approx(gain, 0.0);
    }

    #[test]
    fn linear_rolloff_is_halfway_down_at_the_midpoint() {
        let e = Emitter {
            position: [50.5, 0.0, 0.0],
            min_distance: 1.0,
            max_distance: 100.0,
            rolloff: Rolloff::Linear,
        };
        let (gain, _) = e.gain_and_pan(&Listener::default());
        approx(gain, 0.5);
    }

    #[test]
    fn logarithmic_rolloff_falls_faster_than_linear_everywhere_between() {
        let at = |rolloff| {
            Emitter {
                position: [25.0, 0.0, 0.0],
                min_distance: 1.0,
                max_distance: 100.0,
                rolloff,
            }
            .gain_and_pan(&Listener::default())
            .0
        };
        assert!(at(Rolloff::Logarithmic) < at(Rolloff::Linear));
    }

    #[test]
    fn a_source_to_the_right_pans_right() {
        let e = Emitter {
            position: [10.0, 0.0, 0.0],
            ..Default::default()
        };
        let (_, pan) = e.gain_and_pan(&Listener::default());
        approx(pan, 1.0);
    }

    /// Direction is meaningless at zero distance and jittery near it; a source
    /// on top of the listener must not strobe between ears.
    #[test]
    fn pan_collapses_to_centre_as_a_source_reaches_the_listener() {
        let near = Emitter {
            position: [0.25, 0.0, 0.0],
            min_distance: 1.0,
            ..Default::default()
        };
        let (_, pan) = near.gain_and_pan(&Listener::default());
        approx(pan, 0.25);

        let on_top = Emitter {
            position: [0.0, 0.0, 0.0],
            ..Default::default()
        };
        approx(on_top.gain_and_pan(&Listener::default()).1, 0.0);
    }

    /// A `max_distance` at or below `min_distance` is a degenerate emitter, and
    /// the naive formula divides by zero there.
    #[test]
    fn a_degenerate_distance_range_does_not_produce_nan() {
        let e = Emitter {
            position: [5.0, 0.0, 0.0],
            min_distance: 10.0,
            max_distance: 10.0,
            ..Default::default()
        };
        let (gain, _) = e.gain_and_pan(&Listener::default());
        assert!(gain.is_finite());
        approx(gain, 1.0);

        let past = Emitter {
            position: [50.0, 0.0, 0.0],
            min_distance: 10.0,
            max_distance: 10.0,
            ..Default::default()
        };
        assert!(past.gain_and_pan(&Listener::default()).0.is_finite());
    }
}

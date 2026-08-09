//! Reverb and delay, as send effects.
//!
//! These exist because `AudioPlayer` already has `reverb_send` and `delay_send`
//! fields and they are scene-serialized — projects may already carry non-zero
//! values, so dropping the feature would silently change existing mixes rather
//! than merely removing something unused.
//!
//! ## Sends, not inserts
//!
//! Each voice contributes a *proportion* of itself to one shared reverb and one
//! shared delay, and those two sum into master alongside the dry signal. That is
//! how a desk does it and it is also the only affordable arrangement: one reverb
//! running for the whole mix instead of one per voice, which for a scene with
//! forty emitters is the difference between a reverb and forty of them.
//!
//! It also matches what the engine's components already describe — a per-emitter
//! *amount*, not a per-emitter effect — so the port needs no change to any
//! authored scene.
//!
//! ## Why these algorithms
//!
//! The reverb is a Schroeder/Freeverb topology: parallel damped comb filters
//! into series allpass sections. It is fifty years old, it is what most game
//! engines still ship, and it costs a handful of adds per sample. Anything
//! better — convolution, an FDN with a real modal response — is a different
//! product, and this one has to run inside a device callback beside everything
//! else.

use alloc::vec;
use alloc::vec::Vec;

/// Freeverb's comb delay lengths, in samples at 44.1 kHz.
///
/// Mutually prime on purpose: shared factors make the combs reinforce each other
/// at the same frequencies, which is heard as a metallic ring rather than as a
/// room. Scaled to the running rate in [`Reverb::new`].
const COMB_LENGTHS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];

/// Allpass lengths, same units and same reasoning.
const ALLPASS_LENGTHS: [usize; 4] = [556, 441, 341, 225];

/// Samples the right channel's delay lines are offset by, decorrelating the two
/// so the tail is a stereo field rather than a single point between the speakers.
const STEREO_SPREAD: usize = 23;

/// Allpass feedback. Fixed at Freeverb's value — it sets diffusion, and there is
/// no useful range to expose.
const ALLPASS_FEEDBACK: f32 = 0.5;

/// A damped comb filter: a delay line whose feedback is lowpassed.
///
/// The damping is what makes it sound like a room instead of a pipe. Real rooms
/// absorb high frequencies faster than low ones, so a tail that keeps its treble
/// reads as artificial however long it is.
struct Comb {
    buffer: Vec<f32>,
    index: usize,
    /// One-pole lowpass state in the feedback path.
    filtered: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            index: 0,
            filtered: 0.0,
        }
    }

    fn process(&mut self, input: f32, feedback: f32, damping: f32) -> f32 {
        let out = self.buffer[self.index];
        self.filtered = out * (1.0 - damping) + self.filtered * damping;
        self.buffer[self.index] = input + self.filtered * feedback;
        self.index = (self.index + 1) % self.buffer.len();
        out
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.filtered = 0.0;
    }
}

/// An allpass section: passes every frequency at equal level but smears them in
/// time. Where the density of the tail comes from.
struct Allpass {
    buffer: Vec<f32>,
    index: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            index: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        let out = -input + buffered;
        self.buffer[self.index] = input + buffered * ALLPASS_FEEDBACK;
        self.index = (self.index + 1) % self.buffer.len();
        out
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// How a reverb sounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReverbSettings {
    /// Tail length, 0..1. Clamped below 1.0 in [`Reverb::process`]: at exactly
    /// 1.0 a comb filter's feedback loop has unity gain and never decays, which
    /// is not a long reverb but an oscillator.
    pub feedback: f32,
    /// How fast the tail loses its high frequencies, 0..1.
    pub damping: f32,
}

impl Default for ReverbSettings {
    /// Matches what the engine shipped under kira — `feedback(0.85)` and
    /// `damping(0.3)` — so a project's existing reverb sends sound the same
    /// after the port rather than merely still working.
    fn default() -> Self {
        Self {
            feedback: 0.85,
            damping: 0.3,
        }
    }
}

/// A stereo Schroeder reverb.
pub struct Reverb {
    combs: [(Comb, Comb); 8],
    allpasses: [(Allpass, Allpass); 4],
    pub settings: ReverbSettings,
}

impl Reverb {
    /// Build for a given output rate. Lengths scale with the rate so the room
    /// is the same size at 44.1 and 48 kHz — using the raw sample counts at
    /// 48 kHz would make every room ~9% smaller and brighter.
    pub fn new(sample_rate: u32) -> Self {
        let scale = |n: usize| ((n as f64) * sample_rate as f64 / 44_100.0) as usize;
        Self {
            combs: core::array::from_fn(|i| {
                (
                    Comb::new(scale(COMB_LENGTHS[i])),
                    Comb::new(scale(COMB_LENGTHS[i] + STEREO_SPREAD)),
                )
            }),
            allpasses: core::array::from_fn(|i| {
                (
                    Allpass::new(scale(ALLPASS_LENGTHS[i])),
                    Allpass::new(scale(ALLPASS_LENGTHS[i] + STEREO_SPREAD)),
                )
            }),
            settings: ReverbSettings::default(),
        }
    }

    /// Process one stereo frame; returns the wet signal only.
    ///
    /// Wet-only because this is a send — the dry path never came through here,
    /// so mixing it back in would double it.
    pub fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        // Strictly below 1.0: unity feedback is an oscillator, and a value
        // arriving from a host slider must not be able to make one.
        let feedback = self.settings.feedback.clamp(0.0, 0.98);
        let damping = self.settings.damping.clamp(0.0, 1.0);

        let mut out = [0.0f32; 2];
        for (l, r) in &mut self.combs {
            out[0] += l.process(input[0], feedback, damping);
            out[1] += r.process(input[1], feedback, damping);
        }
        // The combs sum eight copies, so normalise before the allpasses or the
        // level would depend on how many combs the topology happens to use.
        out[0] /= COMB_LENGTHS.len() as f32;
        out[1] /= COMB_LENGTHS.len() as f32;

        for (l, r) in &mut self.allpasses {
            out[0] = l.process(out[0]);
            out[1] = r.process(out[1]);
        }
        out
    }

    /// Drop the tail. For a scene change — carrying the previous level's
    /// reverb into the next one is a very audible mistake.
    pub fn clear(&mut self) {
        for (l, r) in &mut self.combs {
            l.clear();
            r.clear();
        }
        for (l, r) in &mut self.allpasses {
            l.clear();
            r.clear();
        }
    }
}

/// A stereo feedback delay.
pub struct Delay {
    buffer: Vec<f32>,
    index: usize,
    /// Delay length in frames. Never zero — a zero-length line would feed back
    /// into itself within one sample.
    frames: usize,
    /// How much of the output returns to the input, 0..1.
    pub feedback: f32,
}

impl Delay {
    /// Build for a given output rate.
    ///
    /// The defaults match what the engine shipped under kira: 375 ms at −6 dB
    /// feedback, which is 0.5 in amplitude.
    pub fn new(sample_rate: u32) -> Self {
        Self::with_time(sample_rate, 0.375, 0.5)
    }

    pub fn with_time(sample_rate: u32, seconds: f32, feedback: f32) -> Self {
        // Capacity for the maximum time the setter allows, so retiming never
        // reallocates — this runs on the audio thread.
        let max_frames = (sample_rate as f32 * MAX_DELAY_SECONDS) as usize + 1;
        let frames = ((sample_rate as f32 * seconds) as usize).clamp(1, max_frames);
        Self {
            buffer: vec![0.0; max_frames * 2],
            index: 0,
            frames,
            feedback: feedback.clamp(0.0, 0.95),
        }
    }

    /// Retune the delay time without reallocating. Clamped to the capacity the
    /// buffer was built with.
    pub fn set_time(&mut self, sample_rate: u32, seconds: f32) {
        let max = self.buffer.len() / 2;
        self.frames = ((sample_rate as f32 * seconds) as usize).clamp(1, max);
    }

    /// Process one stereo frame; returns the wet signal only. See
    /// [`Reverb::process`] for why wet-only.
    pub fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        let i = self.index % self.frames;
        let out = [self.buffer[i * 2], self.buffer[i * 2 + 1]];
        // Clamped strictly below 1.0 for the same reason as the reverb: unity
        // feedback on a delay line grows without bound.
        let fb = self.feedback.clamp(0.0, 0.95);
        self.buffer[i * 2] = input[0] + out[0] * fb;
        self.buffer[i * 2 + 1] = input[1] + out[1] * fb;
        self.index = (self.index + 1) % self.frames;
        out
    }

    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
    }
}

/// Longest delay the line can be retuned to. Fixes the allocation up front so
/// [`Delay::set_time`] never allocates in the audio callback.
const MAX_DELAY_SECONDS: f32 = 2.0;

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// Feed one impulse, then silence, and return the peak output over the
    /// first `early` frames and over the `late` frames that follow.
    ///
    /// One impulse, injected once. An earlier version of this helper injected on
    /// every call and so measured accumulation rather than decay — it reported a
    /// stable delay as growing, which is the opposite of the thing being tested.
    fn decay_profile(
        effect: &mut impl FnMut([f32; 2]) -> [f32; 2],
        early: usize,
        late: usize,
    ) -> (f32, f32) {
        let mut first = 0.0f32;
        for f in 0..early {
            let input = if f == 0 { [1.0, 1.0] } else { [0.0, 0.0] };
            let out = effect(input);
            first = first.max(out[0].abs()).max(out[1].abs());
        }
        let mut second = 0.0f32;
        for _ in 0..late {
            let out = effect([0.0, 0.0]);
            second = second.max(out[0].abs()).max(out[1].abs());
        }
        (first, second)
    }

    #[test]
    fn a_reverb_produces_a_tail_after_the_input_stops() {
        let mut r = Reverb::new(RATE);
        // Past the longest comb, so anything here is feedback rather than the
        // original impulse coming through the line for the first time.
        for _ in 0..2000 {
            r.process([1.0, 1.0]);
        }
        let mut later = 0.0f32;
        for _ in 0..4000 {
            let out = r.process([0.0, 0.0]);
            later = later.max(out[0].abs());
        }
        assert!(later > 0.0, "reverb produced no tail");
    }

    /// The failure mode that matters: feedback at or above unity turns a reverb
    /// into an oscillator that grows until the output clips permanently.
    #[test]
    fn a_reverb_decays_even_when_asked_for_infinite_feedback() {
        let mut r = Reverb::new(RATE);
        r.settings.feedback = 1.0;
        let (early, late) = decay_profile(&mut |i| r.process(i), 8_000, 200_000);
        assert!(late < early, "reverb is not decaying: {early} -> {late}");
        assert!(late.is_finite());
    }

    #[test]
    fn a_reverb_never_produces_a_non_finite_sample() {
        let mut r = Reverb::new(RATE);
        r.settings.feedback = 2.0;
        r.settings.damping = -1.0;
        for f in 0..20_000 {
            let input = if f % 100 == 0 { [1.0, -1.0] } else { [0.0, 0.0] };
            let out = r.process(input);
            assert!(out[0].is_finite() && out[1].is_finite(), "frame {f}");
        }
    }

    #[test]
    fn clearing_a_reverb_silences_its_tail() {
        let mut r = Reverb::new(RATE);
        for _ in 0..2000 {
            r.process([1.0, 1.0]);
        }
        r.clear();
        let out = r.process([0.0, 0.0]);
        assert_eq!(out, [0.0, 0.0]);
    }

    /// The two channels use different delay lengths so the tail is a field
    /// rather than a point between the speakers.
    #[test]
    fn a_reverb_tail_is_decorrelated_between_channels() {
        let mut r = Reverb::new(RATE);
        let mut differed = false;
        for f in 0..8000 {
            let input = if f == 0 { [1.0, 1.0] } else { [0.0, 0.0] };
            let out = r.process(input);
            if (out[0] - out[1]).abs() > 1e-6 {
                differed = true;
            }
        }
        assert!(differed, "both channels produced the same signal");
    }

    #[test]
    fn a_delay_repeats_its_input_after_the_delay_time() {
        let mut d = Delay::with_time(RATE, 0.01, 0.0);
        let frames = (RATE as f32 * 0.01) as usize;
        let mut heard_at = None;
        for f in 0..frames * 3 {
            let input = if f == 0 { [1.0, 1.0] } else { [0.0, 0.0] };
            let out = d.process(input);
            if out[0].abs() > 0.5 {
                heard_at = Some(f);
                break;
            }
        }
        assert_eq!(heard_at, Some(frames));
    }

    #[test]
    fn a_delay_decays_even_when_asked_for_infinite_feedback() {
        let mut d = Delay::with_time(RATE, 0.001, 1.0);
        let (early, late) = decay_profile(&mut |i| d.process(i), 480, 48_000);
        assert!(late < early, "delay is not decaying: {early} -> {late}");
        assert!(late.is_finite());
    }

    /// Retiming happens from a host slider, on the audio thread. It must not
    /// reallocate and must not read outside the buffer.
    #[test]
    fn retiming_a_delay_stays_inside_its_buffer() {
        let mut d = Delay::new(RATE);
        let capacity = d.buffer.len();
        for seconds in [0.0, 0.001, 0.5, 2.0, 60.0, -1.0] {
            d.set_time(RATE, seconds);
            for _ in 0..256 {
                let out = d.process([0.5, -0.5]);
                assert!(out[0].is_finite() && out[1].is_finite());
            }
        }
        assert_eq!(d.buffer.len(), capacity, "retiming reallocated");
    }

    /// A zero-length line would feed back into itself within one sample.
    #[test]
    fn a_delay_time_of_zero_is_clamped_to_one_frame() {
        let d = Delay::with_time(RATE, 0.0, 0.5);
        assert!(d.frames >= 1);
    }

    /// Room size should not depend on the device's rate.
    #[test]
    fn reverb_delay_lines_scale_with_the_sample_rate() {
        let a = Reverb::new(44_100);
        let b = Reverb::new(88_200);
        assert!(
            b.combs[0].0.buffer.len() > a.combs[0].0.buffer.len(),
            "lengths did not scale with the rate"
        );
    }

    #[test]
    fn silence_in_gives_silence_out() {
        let mut r = Reverb::new(RATE);
        let mut d = Delay::new(RATE);
        for _ in 0..1000 {
            assert_eq!(r.process([0.0, 0.0]), [0.0, 0.0]);
            assert_eq!(d.process([0.0, 0.0]), [0.0, 0.0]);
        }
    }
}

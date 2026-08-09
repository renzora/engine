//! Decoded audio, and how a voice reads it back at an arbitrary rate.
//!
//! Everything downstream of the decoder speaks in [`Pcm`]: interleaved stereo
//! `f32` at a known sample rate. Mono sources are widened at decode time rather
//! than at playback, because the alternative is a channel-count branch inside
//! the per-frame render loop — the one place in the engine where a branch is
//! actually worth avoiding.

use alloc::sync::Arc;
use alloc::vec::Vec;

/// A decoded clip: interleaved stereo `f32` frames plus the rate they were
/// decoded at.
///
/// Shared behind an [`Arc`] because playing the same footstep forty times must
/// not decode or copy it forty times — a voice holds a handle and its own
/// playhead, nothing more.
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    /// `[left, right, left, right, …]`. Always stereo; see the module doc.
    samples: Vec<f32>,
    sample_rate: u32,
}

impl Pcm {
    /// Wrap already-interleaved stereo samples.
    ///
    /// A trailing half-frame (an odd sample count) is dropped rather than
    /// accepted: it would make `frames()` disagree with `samples.len() / 2` and
    /// send the interpolator one sample past the end on the final frame.
    pub fn stereo(mut samples: Vec<f32>, sample_rate: u32) -> Self {
        if samples.len() % 2 != 0 {
            samples.pop();
        }
        Self {
            samples,
            sample_rate: sample_rate.max(1),
        }
    }

    /// Widen a mono source, duplicating each sample across both channels.
    pub fn mono(samples: &[f32], sample_rate: u32) -> Self {
        let mut out = Vec::with_capacity(samples.len() * 2);
        for &s in samples {
            out.push(s);
            out.push(s);
        }
        Self::stereo(out, sample_rate)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Number of stereo frames.
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Duration in seconds.
    pub fn duration(&self) -> f64 {
        self.frames() as f64 / self.sample_rate as f64
    }

    /// The frame at `index`, or silence past the end.
    ///
    /// Returning silence rather than panicking is load-bearing: the interpolator
    /// below reads `index + 1`, so the final frame of every clip in the project
    /// would otherwise be an out-of-bounds read.
    fn frame(&self, index: usize) -> [f32; 2] {
        match self.samples.get(index * 2..index * 2 + 2) {
            Some([l, r]) => [*l, *r],
            _ => [0.0, 0.0],
        }
    }

    /// The frame at a *fractional* position, linearly interpolated.
    ///
    /// Linear rather than anything fancier because this runs per output frame
    /// per voice. The artefact it trades for that — a little high-frequency
    /// aliasing on heavily pitched-up material — is inaudible at the ±2 octaves
    /// games actually use, and a windowed-sinc resampler here would cost more
    /// than the rest of the mixer put together.
    pub fn sample(&self, position: f64) -> [f32; 2] {
        if position < 0.0 {
            return [0.0, 0.0];
        }
        let index = position as usize;
        let t = (position - index as f64) as f32;
        let a = self.frame(index);
        let b = self.frame(index + 1);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }
}

/// A shared handle to decoded audio.
pub type PcmRef = Arc<Pcm>;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn a_mono_source_is_widened_to_both_channels() {
        let pcm = Pcm::mono(&[0.5, -0.25], 48_000);
        assert_eq!(pcm.frames(), 2);
        assert_eq!(pcm.sample(0.0), [0.5, 0.5]);
        assert_eq!(pcm.sample(1.0), [-0.25, -0.25]);
    }

    /// An odd sample count would make `frames()` lie and push the interpolator
    /// one sample past the buffer on the last frame.
    #[test]
    fn a_trailing_half_frame_is_dropped() {
        let pcm = Pcm::stereo(vec![1.0, 1.0, 1.0], 48_000);
        assert_eq!(pcm.frames(), 1);
    }

    #[test]
    fn fractional_positions_interpolate_between_frames() {
        let pcm = Pcm::stereo(vec![0.0, 0.0, 1.0, -1.0], 48_000);
        assert_eq!(pcm.sample(0.5), [0.5, -0.5]);
        assert_eq!(pcm.sample(0.25), [0.25, -0.25]);
    }

    /// The interpolator reads `index + 1`, so the last frame of every clip is a
    /// potential out-of-bounds read.
    #[test]
    fn reading_past_the_end_gives_silence_rather_than_panicking() {
        let pcm = Pcm::stereo(vec![1.0, 1.0], 48_000);
        assert_eq!(pcm.sample(1.0), [0.0, 0.0]);
        assert_eq!(pcm.sample(9999.0), [0.0, 0.0]);
        assert_eq!(pcm.sample(-1.0), [0.0, 0.0]);
    }

    #[test]
    fn duration_follows_frame_count_and_rate() {
        let pcm = Pcm::mono(&[0.0; 24_000], 48_000);
        assert_eq!(pcm.duration(), 0.5);
    }
}

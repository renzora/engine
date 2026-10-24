//! Mouth movement from a voice recording.
//!
//! The loudness of the voice, frame by frame, drives how open the jaw is:
//! the simplest lip sync that reads as talking, and one that needs nothing
//! but the recording. It is measured as RMS over each animation frame,
//! smoothed so the jaw opens quickly and closes a little slower (as a mouth
//! does between syllables), then scaled so the loud parts of this recording
//! open the mouth fully whatever its overall level.
//!
//! Matching mouth shapes to individual sounds (visemes) would need speech
//! recognition, which this deliberately does not attempt.

/// The jaw's openness, `0..1`, for each frame at `fps`.
pub(crate) fn envelope(samples: &[f32], sample_rate: u32, channels: usize, fps: f32) -> Vec<f32> {
    let channels = channels.max(1);
    let frames = samples.len() / channels;
    let per_frame = ((sample_rate as f32) / fps).max(1.0) as usize;
    let mut raw = Vec::with_capacity(frames / per_frame + 1);
    let mut i = 0;
    while i < frames {
        let end = (i + per_frame).min(frames);
        let mut sum = 0.0f64;
        for f in i..end {
            let mut v = 0.0f32;
            for c in 0..channels {
                v += samples[f * channels + c];
            }
            let v = v / channels as f32;
            sum += (v * v) as f64;
        }
        raw.push(((sum / (end - i).max(1) as f64).sqrt()) as f32);
        i = end;
    }
    // The level that counts as fully open: near the top, not the single
    // loudest peak, so one plosive does not shrink everything else.
    let mut sorted = raw.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let top = sorted.get(sorted.len().saturating_sub(1) * 95 / 100).copied().unwrap_or(0.0).max(1e-4);
    // Below this the mouth is shut: breath and room noise.
    let floor = top * 0.12;
    let mut out = Vec::with_capacity(raw.len());
    let mut level = 0.0f32;
    for r in raw {
        let want = ((r - floor) / (top - floor)).clamp(0.0, 1.0);
        let rate = if want > level { 0.6 } else { 0.3 };
        level += (want - level) * rate;
        out.push(level);
    }
    out
}

/// Keep only the frames that matter: where the curve turns or changes by
/// more than `tolerance`, plus the first and last. Lip sync at 30 keys a
/// second would bury the timeline.
pub(crate) fn thin(values: &[f32], tolerance: f32) -> Vec<usize> {
    if values.len() <= 2 {
        return (0..values.len()).collect();
    }
    let mut keep = vec![0];
    for i in 1..values.len() - 1 {
        let (a, b, c) = (values[i - 1], values[i], values[i + 1]);
        let turn = (b - a) * (c - b) < 0.0;
        let last = values[*keep.last().expect("starts with 0")];
        if turn || (b - last).abs() > tolerance {
            keep.push(i);
        }
    }
    keep.push(values.len() - 1);
    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_keeps_the_mouth_shut_and_speech_opens_it() {
        let rate = 8000;
        let mut samples = vec![0.0f32; rate as usize];
        for (i, s) in samples.iter_mut().enumerate().skip(rate as usize / 2) {
            *s = (i as f32 * 0.3).sin() * 0.5;
        }
        let env = envelope(&samples, rate, 1, 30.0);
        assert!(env[2] < 0.01, "silent start {}", env[2]);
        assert!(*env.last().unwrap() > 0.8, "loud end {}", env.last().unwrap());
    }

    #[test]
    fn thinning_keeps_the_turns() {
        let v = [0.0, 0.5, 1.0, 0.5, 0.0, 0.0, 0.0];
        let k = thin(&v, 0.05);
        assert!(k.contains(&2), "the peak");
        assert_eq!(k.first(), Some(&0));
        assert_eq!(k.last(), Some(&6));
    }
}

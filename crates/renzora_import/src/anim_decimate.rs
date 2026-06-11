//! Keyframe decimation — drop baked keys that linear interpolation between
//! their neighbours already reproduces.
//!
//! FBX/USD/BVH extraction samples every bone at a fixed rate (30 Hz), so a
//! 2-second clip with 65 bones lands at ~12k keys even when most channels are
//! constant or smoothly varying. Runtime playback lerps/slerps between keys,
//! so any key on (or within tolerance of) the segment between its kept
//! neighbours carries no information. A Ramer–Douglas–Peucker pass per channel
//! keeps the keys that actually shape the curve.

use renzora::AnimClip;

/// Positional tolerance in scene units (metres) — half a millimetre.
const TRANSLATION_TOLERANCE: f32 = 0.0005;
/// Rotation tolerance as quaternion component distance. ~0.0015 corresponds
/// to roughly a tenth of a degree — far below visible jitter.
const ROTATION_TOLERANCE: f32 = 0.0015;
/// Relative scale tolerance.
const SCALE_TOLERANCE: f32 = 0.0005;

/// Decimate every channel of every track in place. Returns the number of keys
/// removed (for logging).
pub fn decimate_clip(clip: &mut AnimClip) -> usize {
    let mut removed = 0;
    for track in &mut clip.tracks {
        removed += decimate_channel(&mut track.translations, TRANSLATION_TOLERANCE);
        removed += decimate_channel(&mut track.rotations, ROTATION_TOLERANCE);
        removed += decimate_channel(&mut track.scales, SCALE_TOLERANCE);
    }
    removed
}

/// RDP over `(time, value)` keys with component-wise linear interpolation.
/// Works for both `[f32; 3]` and `[f32; 4]` values; quaternion keys sampled
/// densely from a continuous animation stay sign-consistent, so component
/// lerp is a faithful stand-in for slerp at these tolerances.
fn decimate_channel<const N: usize>(keys: &mut Vec<(f32, [f32; N])>, tolerance: f32) -> usize {
    if keys.len() <= 2 {
        // Even a constant channel keeps first + last so duration is anchored.
        return 0;
    }
    let before = keys.len();
    let mut keep = vec![false; keys.len()];
    keep[0] = true;
    keep[before - 1] = true;
    rdp_mark(keys, 0, before - 1, tolerance, &mut keep);

    let mut i = 0;
    keys.retain(|_| {
        let k = keep[i];
        i += 1;
        k
    });
    before - keys.len()
}

/// Mark the keys between `lo` and `hi` (exclusive) that deviate from the
/// straight segment by more than `tolerance`, recursing around the worst one.
fn rdp_mark<const N: usize>(
    keys: &[(f32, [f32; N])],
    lo: usize,
    hi: usize,
    tolerance: f32,
    keep: &mut [bool],
) {
    if hi <= lo + 1 {
        return;
    }
    let (t0, v0) = keys[lo];
    let (t1, v1) = keys[hi];
    let span = (t1 - t0).max(f32::EPSILON);

    let mut worst = 0.0f32;
    let mut worst_idx = lo;
    for (i, &(t, v)) in keys.iter().enumerate().take(hi).skip(lo + 1) {
        let s = (t - t0) / span;
        let mut err = 0.0f32;
        for c in 0..N {
            let interp = v0[c] + (v1[c] - v0[c]) * s;
            err = err.max((v[c] - interp).abs());
        }
        if err > worst {
            worst = err;
            worst_idx = i;
        }
    }

    if worst > tolerance {
        keep[worst_idx] = true;
        rdp_mark(keys, lo, worst_idx, tolerance, keep);
        rdp_mark(keys, worst_idx, hi, tolerance, keep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_channel_collapses_to_endpoints() {
        let mut keys: Vec<(f32, [f32; 3])> =
            (0..60).map(|i| (i as f32 / 30.0, [1.0, 2.0, 3.0])).collect();
        decimate_channel(&mut keys, 0.0005);
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn linear_ramp_collapses_to_endpoints() {
        let mut keys: Vec<(f32, [f32; 3])> =
            (0..60).map(|i| (i as f32 / 30.0, [i as f32 * 0.1, 0.0, 0.0])).collect();
        decimate_channel(&mut keys, 0.0005);
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn corner_is_preserved() {
        // Ramp up then back down — the apex must survive.
        let mut keys: Vec<(f32, [f32; 3])> = (0..61)
            .map(|i| {
                let t = i as f32 / 30.0;
                let v = if i <= 30 { i as f32 } else { (60 - i) as f32 };
                (t, [v, 0.0, 0.0])
            })
            .collect();
        decimate_channel(&mut keys, 0.0005);
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[1].1[0], 30.0);
    }

    #[test]
    fn short_channels_untouched() {
        let mut keys: Vec<(f32, [f32; 3])> = vec![(0.0, [0.0; 3]), (1.0, [1.0; 3])];
        assert_eq!(decimate_channel(&mut keys, 0.0005), 0);
        assert_eq!(keys.len(), 2);
    }
}

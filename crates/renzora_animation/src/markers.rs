//! Playback steps: what a playing clip covered this frame, for event markers
//! and root motion.
//!
//! Both need the same thing from a clip each frame: the stretch of its
//! timeline it played through, including any loop wraps, and nothing it was
//! merely seeked across. Bevy's `ActiveAnimation` exposes the pieces (seek
//! time, the seek time before this frame's advance, completions, elapsed) but
//! not the stretch, and reading them naively double-fires a marker on a paused
//! clip (its last seek time and seek time are a frame apart forever) and fires
//! everything between the old and new time on a seek. So each animator keeps a
//! [`NodeCursor`] per playing clip node, and [`playback_step`] compares it with
//! the clip's state now.
//!
//! Markers fire for every clip that is playing, not only the property-track
//! sampler's clip, which skipped clips with no property tracks and so never
//! fired a marker on a skeletal clip at all.

use bevy::animation::ActiveAnimation;

/// What a clip node looked like at the end of the last frame it was seen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeCursor {
    pub seek: f32,
    pub completions: u32,
    pub elapsed: f32,
}

impl NodeCursor {
    pub fn of(anim: &ActiveAnimation) -> Self {
        NodeCursor { seek: anim.seek_time(), completions: anim.completions(), elapsed: anim.elapsed() }
    }
}

/// The stretch of a clip's timeline played through this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackStep {
    /// Where playback came from. `None` means it (re)started this frame, so
    /// the clip's first instant counts as crossed: a marker at time 0 fires
    /// when the clip starts, as it does when the clip loops back to 0.
    pub from: Option<f32>,
    /// Where playback is now.
    pub to: f32,
    /// Times playback wrapped past the end (or, in reverse, the start).
    pub wraps: u32,
    /// Playing forwards (speed zero or above).
    pub forward: bool,
}

/// How many whole loops a single step may report. A very short clip under a
/// very long frame could otherwise fire one marker hundreds of times.
const MAX_FULL_LOOPS: u32 = 4;

/// Below this two times are the same time.
const TIME_EPS: f32 = 1e-5;

/// The step a clip node took since `prev` was recorded.
///
/// `last_seek` is Bevy's seek time from before this frame's advance: when it
/// does not match where the clip was left, something seeked it in between,
/// and playback is taken to start from the seek target instead.
pub fn playback_step(
    prev: Option<NodeCursor>,
    now: NodeCursor,
    last_seek: Option<f32>,
    duration: f32,
    speed: f32,
    finished: bool,
) -> Option<PlaybackStep> {
    let forward = speed >= 0.0;
    let duration = duration.max(0.0);
    let to = if finished {
        if forward { duration } else { 0.0 }
    } else {
        now.seek.clamp(0.0, duration)
    };
    let start = if forward { 0.0 } else { duration };
    let fresh = |wraps: u32| -> Option<PlaybackStep> {
        let last = last_seek?;
        let from = if (last - start).abs() <= TIME_EPS { None } else { Some(last) };
        Some(PlaybackStep { from, to, wraps: if finished { 0 } else { wraps }, forward })
    };
    let Some(prev) = prev else {
        if now.elapsed <= 0.0 {
            return None;
        }
        return fresh(now.completions);
    };
    // Restarted (`replay` zeroes elapsed and completions).
    if now.elapsed + TIME_EPS < prev.elapsed || now.completions < prev.completions {
        if now.elapsed <= 0.0 {
            return None;
        }
        return fresh(now.completions);
    }
    // Paused, finished, or seeked without advancing: nothing was played.
    if (now.elapsed - prev.elapsed).abs() <= f32::EPSILON {
        return None;
    }
    let from = match last_seek {
        Some(last) if (last - prev.seek).abs() > 1e-4 => last,
        _ => prev.seek,
    };
    let wraps = if finished { 0 } else { now.completions - prev.completions };
    Some(PlaybackStep { from: Some(from.clamp(0.0, duration)), to, wraps, forward })
}

/// Indices of the `markers` (times in seconds) a step crossed, in the order
/// playback met them. A marker crossed on several loops is listed once per
/// crossing.
pub fn markers_crossed(markers: &[f32], duration: f32, step: &PlaybackStep) -> Vec<usize> {
    let mut out = Vec::new();
    let mut push_range = |lo: f32, lo_inclusive: bool, hi: f32, hi_inclusive: bool, reverse: bool| {
        let mut hits: Vec<(usize, f32)> = markers
            .iter()
            .enumerate()
            .filter(|(_, &m)| {
                let above = if lo_inclusive { m >= lo } else { m > lo };
                let below = if hi_inclusive { m <= hi } else { m < hi };
                above && below
            })
            .map(|(i, &m)| (i, m))
            .collect();
        hits.sort_by(|a, b| a.1.total_cmp(&b.1));
        if reverse {
            hits.reverse();
        }
        out.extend(hits.into_iter().map(|(i, _)| i));
    };
    let loops = step.wraps.saturating_sub(1).min(MAX_FULL_LOOPS);
    if step.forward {
        let (lo, lo_inc) = match step.from {
            Some(f) => (f, false),
            None => (0.0, true),
        };
        if step.wraps == 0 {
            push_range(lo, lo_inc, step.to, true, false);
        } else {
            push_range(lo, lo_inc, duration, true, false);
            for _ in 0..loops {
                push_range(0.0, true, duration, true, false);
            }
            push_range(0.0, true, step.to, true, false);
        }
    } else {
        let (hi, hi_inc) = match step.from {
            Some(f) => (f, false),
            None => (duration, true),
        };
        if step.wraps == 0 {
            push_range(step.to, true, hi, hi_inc, true);
        } else {
            push_range(0.0, true, hi, hi_inc, true);
            for _ in 0..loops {
                push_range(0.0, true, duration, true, true);
            }
            push_range(step.to, true, duration, true, true);
        }
    }
    out
}

/// The step the property-track sampler took, which keeps its own clock
/// rather than a Bevy node: from `prev` by `advance` seconds of clip time.
pub fn clock_step(prev: f32, advance: f32, duration: f32, looping: bool, fresh: bool) -> PlaybackStep {
    let forward = advance >= 0.0;
    let raw = prev + advance;
    let (to, wraps) = if duration > 0.0 && looping {
        let wraps = if forward { (raw / duration).floor().max(0.0) } else { (-raw / duration).ceil().max(0.0) };
        (raw.rem_euclid(duration), wraps as u32)
    } else if duration > 0.0 {
        (raw.clamp(0.0, duration), 0)
    } else {
        (raw.max(0.0), 0)
    };
    PlaybackStep { from: if fresh { None } else { Some(prev) }, to, wraps, forward }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cur(seek: f32, completions: u32, elapsed: f32) -> NodeCursor {
        NodeCursor { seek, completions, elapsed }
    }

    fn crossed(markers: &[f32], dur: f32, step: Option<PlaybackStep>) -> Vec<usize> {
        step.map(|s| markers_crossed(markers, dur, &s)).unwrap_or_default()
    }

    /// The plain case: a marker between last frame's time and this frame's
    /// fires, one exactly at last frame's time does not (it already fired).
    #[test]
    fn a_marker_fires_once_as_playback_passes_it() {
        let m = [0.5, 0.3];
        let s = playback_step(Some(cur(0.3, 0, 0.3)), cur(0.55, 0, 0.55), Some(0.3), 1.0, 1.0, false);
        assert_eq!(crossed(&m, 1.0, s), vec![0]);
        // Next frame does not fire it again.
        let s = playback_step(Some(cur(0.55, 0, 0.55)), cur(0.6, 0, 0.6), Some(0.55), 1.0, 1.0, false);
        assert!(crossed(&m, 1.0, s).is_empty());
    }

    /// Crossing the loop point fires what is left of the old loop and the
    /// start of the new one, including a marker at time 0.
    #[test]
    fn a_loop_wrap_fires_both_sides() {
        let m = [0.0, 0.95, 0.05, 0.5];
        let s = playback_step(Some(cur(0.9, 2, 2.9)), cur(0.1, 3, 3.1), Some(0.9), 1.0, 1.0, false);
        assert_eq!(crossed(&m, 1.0, s), vec![1, 0, 2]);
    }

    /// A paused clip keeps last frame's seek pair, which naive reading turns
    /// into the same crossing every frame.
    #[test]
    fn a_paused_clip_fires_nothing() {
        let s = playback_step(Some(cur(0.6, 0, 0.6)), cur(0.6, 0, 0.6), Some(0.5), 1.0, 1.0, false);
        assert_eq!(s, None);
    }

    /// A seek jumps; markers between the old time and the target were never
    /// played, so they do not fire. Playing on from the target does.
    #[test]
    fn a_seek_skips_the_markers_it_jumps_over() {
        let m = [0.2, 0.75];
        let s = playback_step(Some(cur(0.1, 0, 0.1)), cur(0.72, 0, 0.12), Some(0.7), 1.0, 1.0, false);
        assert!(crossed(&m, 1.0, s).is_empty());
        let s = playback_step(Some(cur(0.72, 0, 0.12)), cur(0.8, 0, 0.2), Some(0.72), 1.0, 1.0, false);
        assert_eq!(crossed(&m, 1.0, s), vec![1]);
    }

    /// A clip that just started counts its first instant, so a marker at 0
    /// fires on the first frame rather than only after the first loop.
    #[test]
    fn a_fresh_clip_fires_its_start_marker() {
        let s = playback_step(None, cur(0.02, 0, 0.02), Some(0.0), 1.0, 1.0, false);
        assert_eq!(crossed(&[0.0, 0.5], 1.0, s), vec![0]);
        // A restart (replay) is the same as a fresh start.
        let s = playback_step(Some(cur(0.9, 4, 4.9)), cur(0.02, 0, 0.02), Some(0.0), 1.0, 1.0, false);
        assert_eq!(crossed(&[0.0, 0.5], 1.0, s), vec![0]);
    }

    /// Speed scales how far playback moves, not which markers are crossed;
    /// reverse playback crosses them in reverse order and wraps at the start.
    #[test]
    fn reverse_playback_crosses_backwards() {
        let m = [0.2, 0.6, 0.9];
        let s = playback_step(Some(cur(0.7, 0, 1.0)), cur(0.1, 0, 1.2), Some(0.7), 1.0, -3.0, false);
        assert_eq!(crossed(&m, 1.0, s), vec![1, 0]);
        let s = playback_step(Some(cur(0.1, 0, 1.2)), cur(0.85, 1, 1.3), Some(0.1), 1.0, -2.0, false);
        assert_eq!(crossed(&m, 1.0, s), vec![2]);
    }

    /// A clip that finishes (no loop) runs to its end and stops: the last
    /// marker fires, and nothing after it.
    #[test]
    fn a_finished_clip_fires_up_to_its_end_once() {
        let m = [1.0];
        let s = playback_step(Some(cur(0.95, 0, 0.95)), cur(1.02, 1, 1.02), Some(0.95), 1.0, 1.0, true);
        assert_eq!(crossed(&m, 1.0, s), vec![0]);
        let s = playback_step(Some(cur(1.02, 1, 1.02)), cur(1.02, 1, 1.02), Some(1.02), 1.0, 1.0, true);
        assert!(crossed(&m, 1.0, s).is_empty());
    }

    #[test]
    fn the_sampler_clock_wraps_like_a_node() {
        let s = clock_step(0.9, 0.2, 1.0, true, false);
        assert_eq!(s.wraps, 1);
        assert!((s.to - 0.1).abs() < 1e-5);
        assert_eq!(markers_crossed(&[0.0, 0.95], 1.0, &s), vec![1, 0]);
        let s = clock_step(0.9, 0.2, 1.0, false, false);
        assert_eq!((s.wraps, s.to), (0, 1.0));
    }
}

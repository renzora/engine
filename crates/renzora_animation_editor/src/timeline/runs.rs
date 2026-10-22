//! Key runs: each lane's keys grouped into stretches of (nearly) every-frame
//! keys, computed once per clip edit rather than per draw.
//!
//! A clip baked from an imported model has a key on every frame of every
//! channel, and drawn one diamond per key it is a solid wall nobody can read.
//! A run is drawn as one rounded bar with diamonds only where it starts and
//! ends, so the dopesheet shows *where* a bone is animated at a glance. Runs
//! are found by a frame-sized gap, not by pixels, so they mean the same thing
//! at every zoom; whether a run's inner keys are also drawn (and pickable) is
//! decided at draw time from how far apart they land on screen.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use super::clip::TimelineClip;

/// Keys closer than this belong to one run: two frames at 30 fps, plus slack
/// for float drift in baked times.
pub(crate) const RUN_GAP: f32 = 2.5 / 30.0;

/// One run of keys in a lane: key indices `i0..=i1`, times `t0..=t1`.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Run {
    pub(crate) i0: usize,
    pub(crate) i1: usize,
    pub(crate) t0: f32,
    pub(crate) t1: f32,
}

impl Run {
    pub(crate) fn len(&self) -> usize {
        self.i1 - self.i0 + 1
    }
    /// Average spacing between the run's keys, seconds.
    pub(crate) fn spacing(&self) -> f32 {
        if self.i1 > self.i0 {
            (self.t1 - self.t0) / (self.i1 - self.i0) as f32
        } else {
            f32::INFINITY
        }
    }
}

/// Every lane's runs, the merged summary, and all key times in order.
#[derive(Resource, Default)]
pub(crate) struct KeyRuns {
    /// Per bone track, per channel (T, R, S).
    pub(crate) bone: Vec<[Vec<Run>; 3]>,
    /// Per property track.
    pub(crate) prop: Vec<Vec<Run>>,
    /// Per track of the shown effect (`VfxAsset::tracks`), the Particles group.
    pub(crate) fx: Vec<Vec<Run>>,
    /// The summary row's bars: every multi-key run, merged across lanes.
    pub(crate) summary: Vec<(f32, f32)>,
    /// Every distinct key time, sorted: previous / next key, and the summary
    /// row's diamonds (`snapshots::summary_diamonds`).
    pub(crate) times: Vec<f32>,
    pub(crate) gen: u64,
    sig: u64,
}

fn runs_of(times: impl Iterator<Item = f32>) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for (i, t) in times.enumerate() {
        match out.last_mut() {
            Some(r) if t - r.t1 <= RUN_GAP && t >= r.t1 => {
                r.i1 = i;
                r.t1 = t;
            }
            _ => out.push(Run { i0: i, i1: i, t0: t, t1: t }),
        }
    }
    out
}

/// Cheap fingerprint of every key time: a fold over the bits, O(keys). This
/// runs every frame, which is what lets the run table itself be rebuilt only
/// when a key actually moved, was added, or was removed.
fn fingerprint(cache: &TimelineClip) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    cache.key.hash(&mut h);
    let Some(clip) = cache.clip.as_ref() else { return h.finish() };
    let mut acc: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |lane: u64, t: f32| {
        acc = (acc ^ (t.to_bits() as u64).wrapping_add(lane.wrapping_mul(0x9e37_79b9))).wrapping_mul(0x0100_0000_01b3);
    };
    for (ti, tr) in clip.tracks.iter().enumerate() {
        let base = ti as u64 * 4;
        tr.translations.iter().for_each(|k| mix(base, k.0));
        tr.rotations.iter().for_each(|k| mix(base + 1, k.0));
        tr.scales.iter().for_each(|k| mix(base + 2, k.0));
    }
    for (pi, pt) in clip.property_tracks.iter().enumerate() {
        pt.keys.iter().for_each(|k| mix(1_000_000 + pi as u64, k.time));
    }
    if let Some(fx) = cache.fx.as_ref() {
        super::fx::mix_keys(fx, &mut mix);
        super::fx::structure_hash(fx, &mut h);
    }
    (clip.tracks.len(), clip.property_tracks.len()).hash(&mut h);
    acc.hash(&mut h);
    h.finish()
}

pub(super) fn rebuild_runs(cache: Res<TimelineClip>, mut runs: ResMut<KeyRuns>) {
    let sig = fingerprint(&cache);
    if sig == runs.sig {
        return;
    }
    runs.sig = sig;
    runs.gen = runs.gen.wrapping_add(1);
    let Some(clip) = cache.clip.as_ref() else {
        runs.bone.clear();
        runs.prop.clear();
        runs.fx.clear();
        runs.summary.clear();
        runs.times.clear();
        return;
    };
    runs.bone = clip
        .tracks
        .iter()
        .map(|t| {
            [
                runs_of(t.translations.iter().map(|k| k.0)),
                runs_of(t.rotations.iter().map(|k| k.0)),
                runs_of(t.scales.iter().map(|k| k.0)),
            ]
        })
        .collect();
    runs.prop = clip.property_tracks.iter().map(|p| runs_of(p.keys.iter().map(|k| k.time))).collect();
    let fx_tracks = cache.fx.as_ref().map(|f| f.doc.tracks.as_slice()).unwrap_or(&[]);
    runs.fx = fx_tracks.iter().map(|t| runs_of(t.keys.iter().map(|k| k.time))).collect();

    let mut spans: Vec<(f32, f32)> = Vec::new();
    let mut times: Vec<f32> = Vec::new();
    let all = runs
        .bone
        .iter()
        .flat_map(|c| c.iter().flatten())
        .chain(runs.prop.iter().flatten())
        .chain(runs.fx.iter().flatten());
    for r in all {
        if r.len() > 1 {
            spans.push((r.t0, r.t1));
        }
    }
    // Every key time, for previous / next key and the summary diamonds.
    for t in &clip.tracks {
        times.extend(t.translations.iter().map(|k| k.0));
        times.extend(t.rotations.iter().map(|k| k.0));
        times.extend(t.scales.iter().map(|k| k.0));
    }
    for p in &clip.property_tracks {
        times.extend(p.keys.iter().map(|k| k.time));
    }
    for t in fx_tracks {
        times.extend(t.keys.iter().map(|k| k.time));
    }
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (a, b) in spans {
        match merged.last_mut() {
            Some(m) if a <= m.1 + RUN_GAP => m.1 = m.1.max(b),
            _ => merged.push((a, b)),
        }
    }
    times.sort_by(f32::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    runs.summary = merged;
    runs.times = times;
}

/// The run in `runs` holding key `idx`.
pub(crate) fn run_of(runs: &[Run], idx: usize) -> Option<&Run> {
    let i = runs.partition_point(|r| r.i1 < idx);
    runs.get(i).filter(|r| r.i0 <= idx && idx <= r.i1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A baked stretch of every-frame keys is one run; a lone key and a key a
    /// few frames later stand on their own.
    #[test]
    fn every_frame_keys_form_one_run() {
        let frame = 1.0 / 30.0;
        let mut times: Vec<f32> = (0..30).map(|i| i as f32 * frame).collect();
        times.push(2.0);
        times.push(2.0 + 5.0 * frame);
        let runs = runs_of(times.into_iter());
        assert_eq!(runs.len(), 3);
        assert_eq!((runs[0].i0, runs[0].i1), (0, 29));
        assert_eq!(runs[1].len(), 1);
        assert_eq!(runs[2].len(), 1);
        assert_eq!(run_of(&runs, 12).map(|r| r.i0), Some(0));
        assert_eq!(run_of(&runs, 31).map(|r| r.i0), Some(31));
        assert!(run_of(&runs, 99).is_none());
    }

    /// Keys two frames apart still read as continuous (a 15 fps bake on a
    /// 30 fps timeline), three apart do not.
    #[test]
    fn run_gap_is_two_frames() {
        let frame = 1.0 / 30.0;
        let two = runs_of([0.0, 2.0 * frame, 4.0 * frame].into_iter());
        assert_eq!(two.len(), 1);
        let three = runs_of([0.0, 3.0 * frame].into_iter());
        assert_eq!(three.len(), 2);
    }
}

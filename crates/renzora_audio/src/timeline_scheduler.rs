//! Timeline playback scheduler — turns the `TimelineState` transport into
//! Kira playback.
//!
//! Strategy: every frame, walk the clip list. For each clip whose
//! `[start, start+length)` window contains the current playhead and which
//! isn't already playing, kick off a Kira `StaticSoundData` on the clip's
//! track→bus, with `start_position` advanced into the file by however far the
//! playhead is past the clip start. Hold the resulting `StaticSoundHandle` in
//! a side map keyed by `ClipId` so we can stop it when the transport stops,
//! the playhead seeks outside, or the clip is removed.
//!
//! This is a simple frame-resolution scheduler — fine for arrangement
//! preview but not sample-accurate. Sample-accurate scheduling would route
//! through a Kira `ClockHandle`; tracked separately.

use bevy::prelude::*;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::PlaybackState;
use kira::Tween;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::manager::{amplitude_to_db, KiraAudioManager};
use crate::mixer::MixerState;
use crate::timeline::{ClipId, TimelineState};

/// Per-clip handle map. NonSend because [`StaticSoundHandle`] is.
#[derive(Default)]
pub struct ActiveClips {
    pub by_clip: HashMap<ClipId, StaticSoundHandle>,
    /// Cached natural durations (seconds) per source path so we can trim
    /// clip windows to the underlying file length on first encounter.
    pub durations: HashMap<PathBuf, f64>,
    /// Playhead position observed last frame. Used to detect manual seeks
    /// while playing — a jump significantly larger than the per-frame dt
    /// (forward) or any backward motion means the user dragged the
    /// playhead, so we tear down active clip handles and let the scheduler
    /// re-spawn them at the new position with the correct `start_position`
    /// offset into each file.
    pub last_position: f64,
}

/// Time delta tolerance (seconds) for "is the playhead past this clip start
/// during this frame?" — generous enough to survive a single dropped frame
/// without missing the start, tight enough that a stopped/reseek doesn't
/// retrigger neighbouring clips.
const START_WINDOW: f64 = 0.080;

/// Advance the transport playhead while playing.
pub fn tick_transport(
    mut timeline: ResMut<TimelineState>,
    time: Res<Time>,
) {
    if !timeline.transport.is_playing() {
        return;
    }
    let dt = time.delta_secs() as f64;
    timeline.transport.position += dt;

    // Loop region: snap back to start when we cross the end.
    if timeline.transport.loop_enabled {
        if let Some((lo, hi)) = timeline.transport.loop_region {
            if hi > lo && timeline.transport.position >= hi {
                timeline.transport.position = lo;
            }
        }
    }
}

/// Kick off / tear down clip playback in response to transport changes.
pub fn drive_clip_playback(
    mut timeline: ResMut<TimelineState>,
    mixer: Res<MixerState>,
    audio: Option<NonSendMut<KiraAudioManager>>,
    active: Option<NonSendMut<ActiveClips>>,
) {
    let Some(mut audio) = audio else { return };
    let Some(mut active) = active else { return };

    // If transport is stopped, kill any still-playing clip handles.
    if !timeline.transport.is_playing() {
        if !active.by_clip.is_empty() {
            for (_, mut h) in active.by_clip.drain() {
                let _ = h.stop(Tween::default());
            }
        }
        active.last_position = timeline.transport.position;
        return;
    }

    let now = timeline.transport.position;

    // Detect a manual seek while playing: any backward motion, or a forward
    // jump bigger than a generous frame budget (covers stalls up to ~250ms).
    // When detected, drop every active handle so the spawn loop below
    // restarts each clip with `start_position` set to the right offset into
    // its source file. Without this, an active handle keeps rendering from
    // the original position even though the playhead has jumped — i.e. the
    // user drags the playhead but the audio carries on from where it was.
    if !active.by_clip.is_empty() {
        let prev = active.last_position;
        const MAX_FORWARD_STEP: f64 = 0.25;
        let jumped_back = now + 1e-3 < prev;
        let jumped_far = now > prev + MAX_FORWARD_STEP;
        if jumped_back || jumped_far {
            for (_, mut h) in active.by_clip.drain() {
                let _ = h.stop(Tween::default());
            }
        }
    }

    // Drop handles for clips that are no longer present, no longer audible,
    // or whose start window we've already moved well past.
    let mut to_drop: Vec<ClipId> = Vec::new();
    for (clip_id, h) in active.by_clip.iter() {
        let still_present = timeline.clip(*clip_id);
        let still_audible = still_present
            .map(|c| {
                timeline.is_clip_audible(c)
                    && now >= c.start - START_WINDOW
                    && now < c.start + c.length
            })
            .unwrap_or(false);
        if !still_audible || h.state() == PlaybackState::Stopped {
            to_drop.push(*clip_id);
        }
    }
    for id in to_drop {
        if let Some(mut h) = active.by_clip.remove(&id) {
            let _ = h.stop(Tween::default());
        }
    }

    // Find clips that should be playing right now and aren't yet.
    let clip_count = timeline.clips.len();
    for i in 0..clip_count {
        let (id, track_id, source, start, length, gain, audible) = {
            let clip = &timeline.clips[i];
            (
                clip.id,
                clip.track,
                clip.source.clone(),
                clip.start,
                clip.length,
                clip.gain,
                timeline.is_clip_audible(clip),
            )
        };

        if !audible { continue; }
        if active.by_clip.contains_key(&id) { continue; }

        let in_window = now >= start && now < start + length;
        if !in_window { continue; }

        let bus = timeline
            .track(track_id)
            .map(|t| t.bus_name.clone())
            .unwrap_or_else(|| "Sfx".to_string());
        let track_volume = timeline
            .track(track_id)
            .map(|t| t.volume)
            .unwrap_or(1.0);

        let full_path = audio.resolve_path(source.to_string_lossy().as_ref());
        if !full_path.exists() {
            warn!("[Timeline] Source not found: {}", full_path.display());
            continue;
        }

        match StaticSoundData::from_file(&full_path) {
            Ok(data) => {
                // Cache the file's natural duration.
                let dur_secs = data.duration().as_secs_f64();
                active.durations.insert(source.clone(), dur_secs);

                // Volume = clip gain × track volume × master (master is
                // already handled by Kira main_track sync; don't double-apply).
                let amp = (gain * track_volume).max(0.0) as f64;
                let mut prepped = data.volume(amplitude_to_db(amp));

                // Skip ahead in the file by however much we're already past
                // the clip start (e.g. user pressed play with playhead at 2.0s
                // and clip starts at 0.0s → start file 2s in).
                let into_file = (now - start).max(0.0);
                if into_file > 0.0 {
                    prepped = prepped.start_position(into_file);
                }

                match audio.play_on_bus(prepped, &bus, &mixer) {
                    Ok(handle) => {
                        active.by_clip.insert(id, handle);
                    }
                    Err(e) => warn!("[Timeline] Play failed for clip {:?}: {}", id, e),
                }
            }
            Err(e) => warn!("[Timeline] Load failed for {:?}: {}", full_path, e),
        }
    }

    // Trim clip lengths to the underlying file length once we know it
    // (first time the file gets loaded). Only trim if the user-set length
    // is the default placeholder (very large).
    let durations = active.durations.clone();
    for clip in timeline.clips.iter_mut() {
        if let Some(&natural) = durations.get(&clip.source) {
            if clip.length > natural + 0.001 {
                clip.length = natural;
            }
        }
    }

    active.last_position = now;
}

/// Probe natural durations for any clip source we haven't seen yet, even
/// while the transport is stopped. Without this, a freshly-dropped clip
/// keeps the placeholder length (`AddClip` writes 600s) until the user
/// presses play, which makes the clip rectangle in the arrangement view
/// hugely oversized at drop time. Running this every frame and short-
/// circuiting on cache hits keeps the cost negligible.
pub fn cache_clip_durations(
    mut timeline: ResMut<TimelineState>,
    audio: Option<NonSendMut<KiraAudioManager>>,
    active: Option<NonSendMut<ActiveClips>>,
) {
    let Some(audio) = audio else { return };
    let Some(mut active) = active else { return };

    // Collect unseen sources so we can mutably borrow `active` for the
    // duration cache without aliasing `timeline.clips`.
    let unseen: Vec<PathBuf> = timeline
        .clips
        .iter()
        .map(|c| c.source.clone())
        .filter(|src| !active.durations.contains_key(src))
        .collect();

    for source in unseen {
        let full = audio.resolve_path(source.to_string_lossy().as_ref());
        if !full.exists() {
            continue;
        }
        match StaticSoundData::from_file(&full) {
            Ok(data) => {
                active.durations.insert(source, data.duration().as_secs_f64());
            }
            Err(e) => {
                warn!("[Timeline] Duration probe failed for {:?}: {}", full, e);
                // Insert a non-trimming sentinel (large value) so we don't
                // re-probe a broken file every frame.
                active.durations.insert(source, f64::MAX);
            }
        }
    }

    // Trim placeholder lengths down to the natural file duration.
    for clip in timeline.clips.iter_mut() {
        if let Some(&natural) = active.durations.get(&clip.source) {
            if natural.is_finite() && clip.length > natural + 0.001 {
                clip.length = natural;
            }
        }
    }
}

/// React to manual seeks while stopped — we just drop any stale handles so
/// the next play starts cleanly. (Currently `tick_transport` only changes
/// position while playing, so this is a thin guard.)
pub fn stop_all_clips(active: Option<NonSendMut<ActiveClips>>) {
    let Some(mut active) = active else { return };
    for (_, mut h) in active.by_clip.drain() {
        let _ = h.stop(Tween::default());
    }
}

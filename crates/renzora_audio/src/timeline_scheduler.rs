//! Timeline playback scheduler — turns the `TimelineState` transport into
//! playback.
//!
//! Strategy: every frame, walk the clip list. For each clip whose
//! `[start, start+length)` window contains the current playhead and which isn't
//! already playing, start a voice on the clip's track→bus, offset into the
//! source by however far the playhead is past the clip start. Hold the voice in
//! a side map keyed by `ClipId` so it can be stopped when the transport stops,
//! the playhead seeks outside, or the clip is removed.
//!
//! This is a frame-resolution scheduler — fine for arrangement preview but not
//! sample-accurate. Sample accuracy needs the backend to schedule against its own
//! clock, which is an op this boundary does not have; tracked separately.

use bevy::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

use renzora_plugin::audio::{PlayRequest, StopRequest, StopTarget};

use crate::link::{AudioLink, VoiceId};
use crate::runtime::SoundCache;
use crate::timeline::{ClipId, TimelineState};

/// Per-clip voice map.
///
/// A plain `Resource` rather than `NonSend`: it used to hold backend handles
/// that were `!Send`; a [`VoiceId`] is a number.
#[derive(Resource, Default)]
pub struct ActiveClips {
    pub by_clip: HashMap<ClipId, VoiceId>,
    /// Cached natural durations (seconds) per source path, so clip windows can
    /// be trimmed to the underlying file length on first encounter.
    pub durations: HashMap<PathBuf, f64>,
    /// Playhead position observed last frame. Used to detect manual seeks while
    /// playing — a jump significantly larger than the per-frame dt (forward) or
    /// any backward motion means the user dragged the playhead, so active voices
    /// are torn down and the scheduler re-spawns them at the new position with
    /// the correct offset into each file.
    pub last_position: f64,
}

/// Time delta tolerance (seconds) for "is the playhead past this clip start
/// during this frame?" — generous enough to survive a single dropped frame
/// without missing the start, tight enough that a stop/reseek doesn't retrigger.
const START_WINDOW: f64 = 0.05;

/// Advance the transport.
pub fn tick_transport(mut timeline: ResMut<TimelineState>, time: Res<Time>) {
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

/// Stop every clip voice, and forget them.
fn stop_all(link: &mut AudioLink, active: &mut ActiveClips) {
    for (_, voice) in active.by_clip.drain() {
        link.stop(&StopRequest {
            target: StopTarget::Voice(voice.0),
            fade: 0.0,
        });
    }
}

/// Start and stop clip voices in response to transport changes.
pub fn drive_clip_playback(
    mut timeline: ResMut<TimelineState>,
    mut link: ResMut<AudioLink>,
    mut cache: ResMut<SoundCache>,
    mut active: ResMut<ActiveClips>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    if !timeline.transport.is_playing() {
        if !active.by_clip.is_empty() {
            stop_all(&mut link, &mut active);
        }
        active.last_position = timeline.transport.position;
        return;
    }

    let now = timeline.transport.position;

    // Detect a manual seek while playing: any backward motion, or a forward jump
    // bigger than a generous frame budget (covers stalls up to ~250 ms). Every
    // active voice is dropped so the spawn loop below restarts each clip at the
    // right offset — without this a voice keeps playing from where it was even
    // though the playhead jumped, so dragging the playhead moves the picture and
    // not the sound.
    if !active.by_clip.is_empty() {
        const MAX_FORWARD_STEP: f64 = 0.25;
        let prev = active.last_position;
        if now + 1e-3 < prev || now > prev + MAX_FORWARD_STEP {
            stop_all(&mut link, &mut active);
        }
    }

    // Drop voices for clips that are gone, no longer audible, or whose window
    // the playhead has left.
    let stale: Vec<ClipId> = active
        .by_clip
        .keys()
        .copied()
        .filter(|id| {
            !timeline
                .clip(*id)
                .map(|c| {
                    timeline.is_clip_audible(c)
                        && now >= c.start - START_WINDOW
                        && now < c.start + c.length
                })
                .unwrap_or(false)
        })
        .collect();
    for id in stale {
        if let Some(voice) = active.by_clip.remove(&id) {
            link.stop(&StopRequest {
                target: StopTarget::Voice(voice.0),
                fade: 0.0,
            });
        }
    }

    // Start clips that should be sounding and aren't.
    for i in 0..timeline.clips.len() {
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
        if !audible || active.by_clip.contains_key(&id) {
            continue;
        }
        if !(now >= start && now < start + length) {
            continue;
        }

        let bus = timeline
            .track(track_id)
            .map(|t| t.bus_name.clone())
            .unwrap_or_else(|| "Sfx".to_string());
        let track_volume = timeline.track(track_id).map(|t| t.volume).unwrap_or(1.0);

        let path = source.to_string_lossy().into_owned();
        let Some(sound) = cache.get_or_load(&mut link, project.as_deref(), &path) else {
            continue;
        };
        if let Some(duration) = cache.duration(&path) {
            active.durations.insert(source.clone(), duration);
        }

        let voice = link.next_voice();
        let request = PlayRequest {
            voice: voice.0,
            clip: sound.0,
            bus,
            // Master is applied by the mixer's own strip; applying it here too
            // would square it.
            gain: (gain * track_volume).max(0.0),
            pan: 0.0,
            pitch: 1.0,
            looping: None,
            fade_in: 0.0,
            // Skip into the file by however far the playhead is past the clip
            // start — pressing play with the playhead at 2 s on a clip that
            // starts at 0 s begins 2 s into the source.
            start: (now - start).max(0.0),
            emitter: None,
            reverb_send: 0.0,
            delay_send: 0.0,
        };
        if let Err(e) = link.play(&request) {
            warn!("[timeline] clip {id:?}: {e}");
            continue;
        }
        active.by_clip.insert(id, voice);
    }

    // Trim clip lengths to the underlying file length once it is known.
    let durations = active.durations.clone();
    for clip in timeline.clips.iter_mut() {
        if let Some(&natural) = durations.get(&clip.source) {
            if natural.is_finite() && clip.length > natural + 0.001 {
                clip.length = natural;
            }
        }
    }

    active.last_position = now;
}

/// Learn the natural duration of any clip source not seen yet, even while the
/// transport is stopped.
///
/// Without this a freshly-dropped clip keeps the placeholder length (`AddClip`
/// writes 600 s) until the user presses play, which draws the clip rectangle
/// hugely oversized at drop time. Short-circuiting on cache hits keeps the cost
/// negligible.
pub fn cache_clip_durations(
    mut timeline: ResMut<TimelineState>,
    mut link: ResMut<AudioLink>,
    mut cache: ResMut<SoundCache>,
    mut active: ResMut<ActiveClips>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let unseen: Vec<PathBuf> = timeline
        .clips
        .iter()
        .map(|c| c.source.clone())
        .filter(|src| !active.durations.contains_key(src))
        .collect();

    for source in unseen {
        let path = source.to_string_lossy().into_owned();
        if cache
            .get_or_load(&mut link, project.as_deref(), &path)
            .is_none()
        {
            // A non-trimming sentinel, so a broken file is not re-probed every
            // frame. `SoundCache` also blacklists it, but only once a backend
            // exists — this covers the interval before one loads.
            active.durations.insert(source, f64::MAX);
            continue;
        }
        if let Some(duration) = cache.duration(&path) {
            active.durations.insert(source, duration);
        }
    }

    for clip in timeline.clips.iter_mut() {
        if let Some(&natural) = active.durations.get(&clip.source) {
            if natural.is_finite() && clip.length > natural + 0.001 {
                clip.length = natural;
            }
        }
    }
}

/// Drop every clip voice. For a transport reset while stopped.
pub fn stop_all_clips(mut link: ResMut<AudioLink>, mut active: ResMut<ActiveClips>) {
    stop_all(&mut link, &mut active);
}

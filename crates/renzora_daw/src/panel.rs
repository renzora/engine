//! Timeline / arrangement data model for the DAW panel.
//!
//! The panel UI itself is bevy_ui (ember) native; see [`crate::native`]. This
//! module holds the intent buffer + apply logic that both the native UI and the
//! audio scheduler share, plus the per-frame waveform-request system.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;

use renzora_audio::{ClipId, TimelineState, TrackId, TransportState};

use crate::waveform_cache::WaveformCache;

/// Height of one track lane in the arrangement view, shared with the native UI.
pub const TRACK_H: f32 = 48.0;

/// Lightweight intent the panel records during interaction; applied via
/// [`apply_intents`] after the frame so we never alias the world.
#[derive(Clone, Debug)]
pub enum DawIntent {
    Play,
    Stop,
    SeekTo(f64),
    AddTrack {
        bus: String,
    },
    RemoveTrack(TrackId),
    SetTrackName(TrackId, String),
    SetTrackBus(TrackId, String),
    #[allow(dead_code)] // kept for future track-volume UI wiring
    SetTrackVolume(TrackId, f32),
    SetTrackMute(TrackId, bool),
    SetTrackSolo(TrackId, bool),
    SelectClip(Option<ClipId>),
    // Handled in the apply loop; emitted by the deferred native clip
    // drag/resize + audio-file-drop interactions (see `native` module docs).
    #[allow(dead_code)]
    MoveClip(ClipId, f64),
    #[allow(dead_code)]
    ResizeClipRight(ClipId, f64),
    #[allow(dead_code)] // handled in apply loop; kept for future clip-rename UI
    SetClipName(ClipId, String),
    RemoveClip(ClipId),
    #[allow(dead_code)]
    AddClip {
        track: TrackId,
        source: PathBuf,
        start: f64,
    },
    /// Drop an audio file onto an empty timeline — create a track on the
    /// given bus and place the clip on it in one step.
    #[allow(dead_code)]
    AddTrackWithClip {
        bus: String,
        source: PathBuf,
        start: f64,
    },
    SetBpm(f32),
    SetSnapDiv(u32),
    SetPixelsPerSecond(f32),
}

/// Bridge that funnels intents from the UI back into a system that can mutate
/// `TimelineState`.
#[derive(Resource, Default, Clone)]
pub struct DawIntentBuffer {
    inner: Arc<Mutex<Vec<DawIntent>>>,
}

impl DawIntentBuffer {
    pub fn push(&self, i: DawIntent) {
        if let Ok(mut q) = self.inner.lock() {
            q.push(i);
        }
    }
    pub fn drain(&self) -> Vec<DawIntent> {
        self.inner
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default()
    }
}

/// Apply intents queued during the last frame to `TimelineState`. Runs after
/// panel rendering, before the scheduler.
pub fn apply_intents(buffer: Res<DawIntentBuffer>, mut timeline: ResMut<TimelineState>) {
    let intents = buffer.drain();
    if intents.is_empty() {
        return;
    }
    for intent in intents {
        apply_one_intent(&mut timeline, intent);
    }
}

fn apply_one_intent(timeline: &mut TimelineState, intent: DawIntent) {
    match intent {
        DawIntent::Play => timeline.transport.state = TransportState::Playing,
        DawIntent::Stop => {
            timeline.transport.state = TransportState::Stopped;
        }
        DawIntent::SeekTo(t) => {
            timeline.transport.position = t.max(0.0);
        }
        DawIntent::AddTrack { bus } => {
            let n = timeline.tracks.len() + 1;
            timeline.add_track(format!("Track {}", n), bus);
        }
        DawIntent::RemoveTrack(id) => timeline.remove_track(id),
        DawIntent::SetTrackName(id, name) => {
            if let Some(t) = timeline.track_mut(id) {
                t.name = name;
            }
        }
        DawIntent::SetTrackBus(id, bus) => {
            if let Some(t) = timeline.track_mut(id) {
                t.bus_name = bus;
            }
        }
        DawIntent::SetTrackVolume(id, v) => {
            if let Some(t) = timeline.track_mut(id) {
                t.volume = v.max(0.0);
            }
        }
        DawIntent::SetTrackMute(id, m) => {
            if let Some(t) = timeline.track_mut(id) {
                t.muted = m;
            }
        }
        DawIntent::SetTrackSolo(id, s) => {
            if let Some(t) = timeline.track_mut(id) {
                t.soloed = s;
            }
        }
        DawIntent::SelectClip(id) => timeline.selected_clip = id,
        DawIntent::MoveClip(id, t) => {
            if let Some(c) = timeline.clip_mut(id) {
                c.start = t.max(0.0);
            }
        }
        DawIntent::ResizeClipRight(id, len) => {
            if let Some(c) = timeline.clip_mut(id) {
                c.length = len.max(0.05);
            }
        }
        DawIntent::SetClipName(id, name) => {
            if let Some(c) = timeline.clip_mut(id) {
                c.name = name;
            }
        }
        DawIntent::RemoveClip(id) => timeline.remove_clip(id),
        DawIntent::AddClip {
            track,
            source,
            start,
        } => {
            // Default placeholder length; the scheduler will trim to real
            // duration the first time the file gets loaded.
            timeline.add_clip(track, source, start, 600.0);
        }
        DawIntent::AddTrackWithClip { bus, source, start } => {
            let n = timeline.tracks.len() + 1;
            let track = timeline.add_track(format!("Track {}", n), bus);
            timeline.add_clip(track, source, start, 600.0);
        }
        DawIntent::SetBpm(b) => timeline.transport.bpm = b.clamp(20.0, 999.0),
        DawIntent::SetSnapDiv(d) => timeline.transport.snap_div = d,
        DawIntent::SetPixelsPerSecond(p) => {
            timeline.pixels_per_second = p.clamp(20.0, 600.0);
        }
    }
}

/// System: ensure every clip on the timeline has a peaks request in flight.
/// Cheap to run every frame — `request` is idempotent and fingerprinted.
pub fn request_clip_waveforms(timeline: Res<TimelineState>, cache: Res<WaveformCache>) {
    for clip in &timeline.clips {
        if cache.needs_request(&clip.source) {
            cache.request(clip.source.clone());
        }
    }
}

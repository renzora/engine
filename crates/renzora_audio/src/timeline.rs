//! Timeline / arrangement state — the "DAW project" data model.
//!
//! A `TimelineState` holds tracks, clips, and transport position. The actual
//! Kira playback is driven by the scheduler in `timeline_scheduler.rs`.
//!
//! Tracks each route to a named mixer bus (`bus_name`), so the existing
//! `MixerState` keeps doing what it already does — track volume/pan are
//! a *pre-bus* gain stage, applied via per-clip volume scaling at scheduling
//! time.
//!
//! Time is stored in seconds (f64 for precision over long sessions). BPM and
//! beat snap are UI affordances only — the underlying clock is wall-clock.

use std::path::PathBuf;

use bevy::prelude::*;

/// Stable identifier for a clip. Survives reordering / drag operations
/// without needing to chase indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ClipId(pub u64);

/// Stable identifier for a track. Same rationale as [`ClipId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TrackId(pub u64);

/// One clip on a track — references an audio file by absolute path.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TimelineClip {
    pub id: ClipId,
    pub track: TrackId,
    /// Display name (defaults to the file stem on import).
    pub name: String,
    /// Source audio file (absolute on disk; resolved via the audio manager).
    pub source: PathBuf,
    /// Where on the timeline the clip starts (seconds).
    pub start: f64,
    /// Visual / playback length in seconds. Capped to the source file's
    /// natural duration when the file is known; longer values are treated
    /// as "play to end".
    pub length: f64,
    /// Linear amplitude multiplier (1.0 = unity).
    pub gain: f32,
    pub muted: bool,
}

/// One track lane on the timeline.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TimelineTrack {
    pub id: TrackId,
    pub name: String,
    /// Mixer bus this track plays into. Must match a name in `MixerState`
    /// (built-in: "Master" / "Sfx" / "Music" / "Ambient", or any custom bus
    /// name). Unknown names fall back to the SFX bus per `play_on_bus`.
    pub bus_name: String,
    /// Linear amplitude (0.0–1.5). Pre-bus gain.
    pub volume: f32,
    /// Pan (-1.0 left, 0.0 centre, 1.0 right). Currently advisory — clip
    /// playback follows the bus pan; per-track pan would need a per-track
    /// Kira sub-track to take effect.
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    /// Display tint used by the timeline panel. RGBA 0–255.
    pub color: [u8; 4],
}

/// Transport playback state.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[derive(Default)]
pub enum TransportState {
    #[default]
    Stopped,
    Playing,
}


/// Timeline-level transport / clock.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Transport {
    pub state: TransportState,
    /// Current playhead position in seconds.
    pub position: f64,
    /// Tempo for grid snapping & UI display.
    pub bpm: f32,
    /// Snap divisions per beat (1 = quarter, 2 = eighth, 4 = sixteenth, 0 = no snap).
    pub snap_div: u32,
    /// Optional loop region (start, end) in seconds.
    pub loop_region: Option<(f64, f64)>,
    pub loop_enabled: bool,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            state: TransportState::Stopped,
            position: 0.0,
            bpm: 120.0,
            snap_div: 4, // sixteenth-note grid by default
            loop_region: None,
            loop_enabled: false,
        }
    }
}

impl Transport {
    pub fn is_playing(&self) -> bool {
        self.state == TransportState::Playing
    }

    /// Convert a beat number (4 = bar 2 at 4/4) to seconds.
    pub fn beats_to_seconds(&self, beats: f64) -> f64 {
        beats * 60.0 / self.bpm.max(1.0) as f64
    }

    /// Convert seconds to beats (for ruler labelling).
    pub fn seconds_to_beats(&self, seconds: f64) -> f64 {
        seconds * self.bpm.max(1.0) as f64 / 60.0
    }

    /// Snap a time in seconds to the nearest grid line, if a grid is set.
    pub fn snap_seconds(&self, t: f64) -> f64 {
        if self.snap_div == 0 || self.bpm <= 0.0 {
            return t;
        }
        let beat_len = 60.0 / self.bpm as f64;
        let cell = beat_len / self.snap_div as f64;
        if cell <= 0.0 {
            return t;
        }
        (t / cell).round() * cell
    }
}

/// Top-level timeline resource.
#[derive(Resource, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TimelineState {
    pub tracks: Vec<TimelineTrack>,
    pub clips: Vec<TimelineClip>,
    pub transport: Transport,
    /// Total visible/scrollable duration in seconds. UI uses this to size
    /// the scrollable area; clips beyond this still play.
    pub view_duration: f64,
    /// Pixels per second at the current zoom level. UI-only.
    pub pixels_per_second: f32,
    /// Selected clip ID (single-select for now).
    pub selected_clip: Option<ClipId>,
    /// Counter for handing out fresh ids.
    next_id: u64,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            clips: Vec::new(),
            transport: Transport::default(),
            view_duration: 60.0,
            pixels_per_second: 80.0,
            selected_clip: None,
            next_id: 1,
        }
    }
}

impl TimelineState {
    fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Append a new track that routes to `bus_name`. Returns its id.
    pub fn add_track(&mut self, name: impl Into<String>, bus_name: impl Into<String>) -> TrackId {
        let id = TrackId(self.fresh_id());
        let palette: [[u8; 4]; 6] = [
            [228, 132, 52, 255], // amber
            [135, 90, 228, 255], // violet
            [48, 196, 140, 255], // teal
            [208, 75, 75, 255],  // crimson
            [75, 162, 220, 255], // sky
            [205, 192, 52, 255], // ochre
        ];
        let color = palette[self.tracks.len() % palette.len()];
        self.tracks.push(TimelineTrack {
            id,
            name: name.into(),
            bus_name: bus_name.into(),
            volume: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            color,
        });
        id
    }

    pub fn remove_track(&mut self, id: TrackId) {
        self.tracks.retain(|t| t.id != id);
        self.clips.retain(|c| c.track != id);
    }

    pub fn track(&self, id: TrackId) -> Option<&TimelineTrack> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut TimelineTrack> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    pub fn clip(&self, id: ClipId) -> Option<&TimelineClip> {
        self.clips.iter().find(|c| c.id == id)
    }

    pub fn clip_mut(&mut self, id: ClipId) -> Option<&mut TimelineClip> {
        self.clips.iter_mut().find(|c| c.id == id)
    }

    /// Insert a clip on `track` at `start` seconds, sourced from `path`.
    /// `length` defaults to a placeholder; the scheduler trims to the file's
    /// real duration once it loads.
    pub fn add_clip(&mut self, track: TrackId, path: PathBuf, start: f64, length: f64) -> ClipId {
        let id = ClipId(self.fresh_id());
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("clip")
            .to_string();
        self.clips.push(TimelineClip {
            id,
            track,
            name,
            source: path,
            start: start.max(0.0),
            length: length.max(0.05),
            gain: 1.0,
            muted: false,
        });
        id
    }

    pub fn remove_clip(&mut self, id: ClipId) {
        self.clips.retain(|c| c.id != id);
        if self.selected_clip == Some(id) {
            self.selected_clip = None;
        }
    }

    /// Whether *any* track on the timeline is soloed.
    pub fn any_track_soloed(&self) -> bool {
        self.tracks.iter().any(|t| t.soloed)
    }

    /// Whether a clip would actually be audible (track + clip mute/solo).
    pub fn is_clip_audible(&self, clip: &TimelineClip) -> bool {
        if clip.muted {
            return false;
        }
        let any_solo = self.any_track_soloed();
        let Some(track) = self.track(clip.track) else {
            return false;
        };
        if track.muted {
            return false;
        }
        if any_solo && !track.soloed {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── Transport math ────────────────────────────────────────────────────

    #[test]
    fn beats_to_seconds_at_120bpm() {
        let t = Transport::default(); // bpm=120
                                      // 1 beat at 120 BPM = 0.5s
        assert!((t.beats_to_seconds(1.0) - 0.5).abs() < 1e-9);
        // 4 beats (one bar) = 2s
        assert!((t.beats_to_seconds(4.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn seconds_to_beats_inverts_beats_to_seconds() {
        let t = Transport {
            bpm: 90.0,
            ..Transport::default()
        };
        for beats in [0.0, 1.0, 2.5, 4.0, 7.25] {
            let s = t.beats_to_seconds(beats);
            let back = t.seconds_to_beats(s);
            assert!(
                (back - beats).abs() < 1e-9,
                "round trip failed for {} beats",
                beats
            );
        }
    }

    #[test]
    fn beats_to_seconds_clamps_zero_bpm() {
        // Guard against division by zero — bpm is clamped at 1 internally.
        let t = Transport {
            bpm: 0.0,
            ..Transport::default()
        };
        // bpm=1 → 1 beat = 60s
        assert!((t.beats_to_seconds(1.0) - 60.0).abs() < 1e-9);
    }

    #[test]
    fn snap_seconds_quantizes_to_grid() {
        // 120 bpm, snap_div=4 → 16th-note cell = 0.125s.
        let t = Transport::default();
        // 0.13 should snap to 0.125, 0.20 to 0.25.
        assert!((t.snap_seconds(0.13) - 0.125).abs() < 1e-9);
        assert!((t.snap_seconds(0.20) - 0.25).abs() < 1e-9);
        // Already on a grid line — no change.
        assert!((t.snap_seconds(0.5) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn snap_seconds_passthrough_when_disabled() {
        let t = Transport {
            snap_div: 0,
            ..Transport::default()
        };
        assert_eq!(t.snap_seconds(0.337), 0.337);
    }

    #[test]
    fn is_playing_reflects_state() {
        let mut t = Transport::default();
        assert!(!t.is_playing());
        t.state = TransportState::Playing;
        assert!(t.is_playing());
    }

    // ── TimelineState CRUD ────────────────────────────────────────────────

    #[test]
    fn add_track_returns_distinct_ids() {
        let mut tl = TimelineState::default();
        let a = tl.add_track("Music", "Music");
        let b = tl.add_track("Sfx", "Sfx");
        assert_ne!(a, b);
        assert_eq!(tl.tracks.len(), 2);
    }

    #[test]
    fn track_lookup_finds_by_id() {
        let mut tl = TimelineState::default();
        let id = tl.add_track("Music", "Music");
        let t = tl.track(id).expect("found");
        assert_eq!(t.name, "Music");
        assert_eq!(t.bus_name, "Music");
    }

    #[test]
    fn add_clip_clamps_negative_start_to_zero() {
        let mut tl = TimelineState::default();
        let track = tl.add_track("Music", "Music");
        let clip_id = tl.add_clip(track, PathBuf::from("song.ogg"), -1.0, 5.0);
        let clip = tl.clip(clip_id).expect("found");
        assert_eq!(clip.start, 0.0);
    }

    #[test]
    fn add_clip_enforces_minimum_length() {
        // Implementation clamps length to >= 0.05 to avoid zero-length clips.
        let mut tl = TimelineState::default();
        let track = tl.add_track("Music", "Music");
        let clip_id = tl.add_clip(track, PathBuf::from("song.ogg"), 0.0, 0.0);
        let clip = tl.clip(clip_id).expect("found");
        assert!(clip.length >= 0.05);
    }

    #[test]
    fn add_clip_uses_file_stem_as_name() {
        let mut tl = TimelineState::default();
        let track = tl.add_track("Music", "Music");
        let id = tl.add_clip(track, PathBuf::from("songs/intro.wav"), 0.0, 1.0);
        let clip = tl.clip(id).expect("found");
        assert_eq!(clip.name, "intro");
    }

    #[test]
    fn remove_track_also_removes_its_clips() {
        // Otherwise dangling clips reference a missing track id and the
        // scheduler can't route them anywhere.
        let mut tl = TimelineState::default();
        let track = tl.add_track("Music", "Music");
        tl.add_clip(track, PathBuf::from("a.ogg"), 0.0, 1.0);
        tl.add_clip(track, PathBuf::from("b.ogg"), 1.0, 1.0);
        tl.remove_track(track);
        assert!(tl.tracks.is_empty());
        assert!(tl.clips.is_empty());
    }

    #[test]
    fn remove_clip_also_clears_selection() {
        let mut tl = TimelineState::default();
        let track = tl.add_track("Music", "Music");
        let clip_id = tl.add_clip(track, PathBuf::from("a.ogg"), 0.0, 1.0);
        tl.selected_clip = Some(clip_id);
        tl.remove_clip(clip_id);
        assert_eq!(tl.selected_clip, None);
    }

    #[test]
    fn any_track_soloed_reflects_state() {
        let mut tl = TimelineState::default();
        let id = tl.add_track("Music", "Music");
        assert!(!tl.any_track_soloed());
        tl.track_mut(id).unwrap().soloed = true;
        assert!(tl.any_track_soloed());
    }

    #[test]
    fn is_clip_audible_respects_track_mute() {
        let mut tl = TimelineState::default();
        let track = tl.add_track("Music", "Music");
        let clip_id = tl.add_clip(track, PathBuf::from("a.ogg"), 0.0, 1.0);
        let clip = tl.clip(clip_id).unwrap().clone();
        assert!(tl.is_clip_audible(&clip));
        tl.track_mut(track).unwrap().muted = true;
        assert!(!tl.is_clip_audible(&clip));
    }

    #[test]
    fn is_clip_audible_respects_clip_mute() {
        let mut tl = TimelineState::default();
        let track = tl.add_track("Music", "Music");
        let clip_id = tl.add_clip(track, PathBuf::from("a.ogg"), 0.0, 1.0);
        tl.clip_mut(clip_id).unwrap().muted = true;
        let clip = tl.clip(clip_id).unwrap().clone();
        assert!(!tl.is_clip_audible(&clip));
    }

    #[test]
    fn solo_silences_other_tracks() {
        // Once any track is soloed, non-soloed tracks become inaudible
        // even if they aren't muted — standard DAW behaviour.
        let mut tl = TimelineState::default();
        let lead = tl.add_track("Lead", "Music");
        let pad = tl.add_track("Pad", "Music");
        let lead_clip_id = tl.add_clip(lead, PathBuf::from("l.ogg"), 0.0, 1.0);
        let pad_clip_id = tl.add_clip(pad, PathBuf::from("p.ogg"), 0.0, 1.0);

        tl.track_mut(lead).unwrap().soloed = true;
        let lead_clip = tl.clip(lead_clip_id).unwrap().clone();
        let pad_clip = tl.clip(pad_clip_id).unwrap().clone();
        assert!(tl.is_clip_audible(&lead_clip));
        assert!(!tl.is_clip_audible(&pad_clip));
    }
}

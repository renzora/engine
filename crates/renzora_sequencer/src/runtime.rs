//! Runtime state and per-track-type apply systems.
//!
//! The model in `model.rs` is data; this module is what makes the model
//! actually move things in the world.
//!
//! Architecture:
//! - `SequencerState` owns the active `Sequence` plus playback state.
//! - One `apply_*_tracks` system per track type reads the playhead, finds
//!   the active clip on each track, and writes to the world (e.g. lerps the
//!   editor camera transform from the camera track's keys).
//! - Panel-driven mutations come in through `SequencerBridge` (drained each
//!   frame by `apply_bridge_actions`) to keep the panel `&self` and the
//!   resource `&mut`.

use std::sync::{Arc, Mutex};

use bevy::math::{Quat, Vec3};
use bevy::prelude::*;
use renzora::core::{EditorCamera, EntityTag};

use crate::model::{
    CameraClip, CameraKey, MediaClip, Sequence, Track, TrackKind, TransformClip,
};

// ─── Resource ───────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct SequencerState {
    pub sequence: Sequence,
    pub playhead: f32,
    pub playing: bool,
    pub play_rate: f32,
    pub looping: bool,
    /// Pixels per second.
    pub timeline_zoom: f32,
    /// Left-edge time in seconds.
    pub timeline_scroll: f32,
    pub track_height: f32,
    pub selected_track: Option<usize>,
    pub selected_clip: Option<(usize, usize)>,
    /// Set true when a camera clip is currently driving the camera; the
    /// apply system uses this to know when to release the camera back to
    /// the user.
    pub camera_owned: bool,
}

impl Default for SequencerState {
    fn default() -> Self {
        Self {
            sequence: Sequence::new_demo(),
            playhead: 0.0,
            playing: false,
            play_rate: 1.0,
            looping: true,
            timeline_zoom: 80.0,
            timeline_scroll: 0.0,
            track_height: 26.0,
            selected_track: None,
            selected_clip: None,
            camera_owned: false,
        }
    }
}

// ─── Panel → system bridge ──────────────────────────────────────────────────

#[derive(Resource, Clone, Default)]
pub struct SequencerBridge {
    pub pending: Arc<Mutex<Vec<SequencerAction>>>,
}

#[derive(Debug, Clone)]
pub enum SequencerAction {
    SetPlayhead(f32),
    TogglePlay,
    Stop,
    SetPlayRate(f32),
    SetLooping(bool),
    SetZoom(f32),
    SetScroll(f32),
    SetTrackHeight(f32),
    SelectTrack(Option<usize>),
    SelectClip(Option<(usize, usize)>),
    AddTrack(TrackKind),
    RemoveTrack(usize),
    SetTrackMuted { track: usize, muted: bool },
    SetTrackLocked { track: usize, locked: bool },
    /// Move a clip's start (drag the body).
    MoveClip { track: usize, clip: usize, new_start: f32 },
    /// Resize a clip (drag an edge).
    ResizeClip { track: usize, clip: usize, new_start: f32, new_duration: f32 },
    /// Drop a marker at the playhead.
    AddMarkerAtPlayhead(String),
    /// Add a CameraKey at the current playhead, sampled from the live editor camera.
    AddCameraKeyAtPlayhead,
    /// Stub bake-to-video — adds a placeholder MediaClip to the first Media track.
    StubBakeRange { from: f32, to: f32 },
    SetSequenceDuration(f32),
}

pub fn apply_bridge_actions(
    bridge: Res<SequencerBridge>,
    mut state: ResMut<SequencerState>,
    cameras: Query<&Transform, With<EditorCamera>>,
    projections: Query<&Projection, With<EditorCamera>>,
) {
    let actions: Vec<SequencerAction> = match bridge.pending.lock() {
        Ok(mut g) => g.drain(..).collect(),
        Err(_) => return,
    };

    for action in actions {
        match action {
            SequencerAction::SetPlayhead(t) => {
                state.playhead = t.clamp(0.0, state.sequence.duration);
            }
            SequencerAction::TogglePlay => {
                state.playing = !state.playing;
            }
            SequencerAction::Stop => {
                state.playing = false;
                state.playhead = 0.0;
            }
            SequencerAction::SetPlayRate(r) => {
                state.play_rate = r.clamp(0.05, 10.0);
            }
            SequencerAction::SetLooping(l) => {
                state.looping = l;
            }
            SequencerAction::SetZoom(z) => {
                state.timeline_zoom = z.clamp(8.0, 800.0);
            }
            SequencerAction::SetScroll(s) => {
                state.timeline_scroll = s.max(0.0);
            }
            SequencerAction::SetTrackHeight(h) => {
                state.track_height = h.clamp(16.0, 96.0);
            }
            SequencerAction::SelectTrack(idx) => {
                state.selected_track = idx;
            }
            SequencerAction::SelectClip(sel) => {
                state.selected_clip = sel;
            }
            SequencerAction::AddTrack(kind) => {
                state.sequence.tracks.push(Track {
                    name: kind.type_label().to_string(),
                    muted: false,
                    locked: false,
                    kind,
                });
            }
            SequencerAction::RemoveTrack(idx) => {
                if idx < state.sequence.tracks.len() {
                    state.sequence.tracks.remove(idx);
                    if state.selected_track == Some(idx) {
                        state.selected_track = None;
                    }
                }
            }
            SequencerAction::SetTrackMuted { track, muted } => {
                if let Some(t) = state.sequence.tracks.get_mut(track) {
                    t.muted = muted;
                }
            }
            SequencerAction::SetTrackLocked { track, locked } => {
                if let Some(t) = state.sequence.tracks.get_mut(track) {
                    t.locked = locked;
                }
            }
            SequencerAction::MoveClip { track, clip, new_start } => {
                move_clip(&mut state.sequence, track, clip, new_start);
            }
            SequencerAction::ResizeClip { track, clip, new_start, new_duration } => {
                resize_clip(&mut state.sequence, track, clip, new_start, new_duration);
            }
            SequencerAction::AddMarkerAtPlayhead(label) => {
                let ph = state.playhead;
                if let Some(track) = state
                    .sequence
                    .tracks
                    .iter_mut()
                    .find(|t| matches!(t.kind, TrackKind::Marker { .. }))
                {
                    if let TrackKind::Marker { clips } = &mut track.kind {
                        clips.push(crate::model::MarkerClip {
                            start: ph,
                            duration: 0.0,
                            label,
                        });
                    }
                }
            }
            SequencerAction::AddCameraKeyAtPlayhead => {
                let cam_xf = cameras.iter().next().copied();
                let fov = projections.iter().next().and_then(|p| match p {
                    Projection::Perspective(persp) => Some(persp.fov.to_degrees()),
                    _ => None,
                });
                if let Some(xf) = cam_xf {
                    let ph = state.playhead;
                    add_camera_key_at(&mut state.sequence, ph, xf, fov);
                }
            }
            SequencerAction::StubBakeRange { from, to } => {
                let from = from.max(0.0);
                let to = to.min(state.sequence.duration);
                if to > from {
                    if let Some(track) = state
                        .sequence
                        .tracks
                        .iter_mut()
                        .find(|t| matches!(t.kind, TrackKind::Media { .. }))
                    {
                        if let TrackKind::Media { clips } = &mut track.kind {
                            let idx = clips.len() + 1;
                            clips.push(MediaClip {
                                start: from,
                                duration: to - from,
                                name: format!("bake_{:02}.mp4 (pending)", idx),
                                source_path: String::new(),
                            });
                        }
                    }
                    info!(
                        "[sequencer] Bake stub: {:.2}s..{:.2}s — wire to renzora_record next",
                        from, to
                    );
                }
            }
            SequencerAction::SetSequenceDuration(d) => {
                state.sequence.duration = d.max(0.1);
            }
        }
    }
}

fn move_clip(seq: &mut Sequence, track: usize, clip: usize, new_start: f32) {
    let Some(t) = seq.tracks.get_mut(track) else { return };
    if t.locked {
        return;
    }
    let limit = seq.duration;
    let new_start = new_start.max(0.0);
    match &mut t.kind {
        TrackKind::Camera { clips } => set_clip_start(clips, clip, new_start, limit, |c| &mut c.start, |c| &mut c.duration),
        TrackKind::Transform { clips, .. } => set_clip_start(clips, clip, new_start, limit, |c| &mut c.start, |c| &mut c.duration),
        TrackKind::Media { clips } => set_clip_start(clips, clip, new_start, limit, |c| &mut c.start, |c| &mut c.duration),
        TrackKind::Marker { clips } => set_clip_start(clips, clip, new_start, limit, |c| &mut c.start, |c| &mut c.duration),
    }
}

fn resize_clip(seq: &mut Sequence, track: usize, clip: usize, new_start: f32, new_duration: f32) {
    let Some(t) = seq.tracks.get_mut(track) else { return };
    if t.locked {
        return;
    }
    let limit = seq.duration;
    let new_start = new_start.max(0.0);
    let new_duration = new_duration.max(0.05);
    match &mut t.kind {
        TrackKind::Camera { clips } => set_clip_extent(clips, clip, new_start, new_duration, limit, |c| &mut c.start, |c| &mut c.duration),
        TrackKind::Transform { clips, .. } => set_clip_extent(clips, clip, new_start, new_duration, limit, |c| &mut c.start, |c| &mut c.duration),
        TrackKind::Media { clips } => set_clip_extent(clips, clip, new_start, new_duration, limit, |c| &mut c.start, |c| &mut c.duration),
        TrackKind::Marker { clips } => set_clip_extent(clips, clip, new_start, new_duration, limit, |c| &mut c.start, |c| &mut c.duration),
    }
}

fn set_clip_start<C, FS, FD>(
    clips: &mut [C],
    idx: usize,
    new_start: f32,
    seq_dur: f32,
    start_of: FS,
    dur_of: FD,
) where
    FS: Fn(&mut C) -> &mut f32,
    FD: Fn(&mut C) -> &mut f32,
{
    if let Some(c) = clips.get_mut(idx) {
        let dur = *dur_of(c);
        let max_start = (seq_dur - dur).max(0.0);
        *start_of(c) = new_start.min(max_start);
    }
}

fn set_clip_extent<C, FS, FD>(
    clips: &mut [C],
    idx: usize,
    new_start: f32,
    new_duration: f32,
    seq_dur: f32,
    start_of: FS,
    dur_of: FD,
) where
    FS: Fn(&mut C) -> &mut f32,
    FD: Fn(&mut C) -> &mut f32,
{
    if let Some(c) = clips.get_mut(idx) {
        let max_dur = (seq_dur - new_start).max(0.05);
        let dur = new_duration.min(max_dur);
        *start_of(c) = new_start;
        *dur_of(c) = dur;
    }
}

fn add_camera_key_at(seq: &mut Sequence, time: f32, xf: Transform, fov_deg: Option<f32>) {
    let track = seq
        .tracks
        .iter_mut()
        .find(|t| matches!(t.kind, TrackKind::Camera { .. }));
    let Some(track) = track else {
        return;
    };
    let TrackKind::Camera { clips } = &mut track.kind else {
        return;
    };

    // Pick the clip the playhead is sitting on, or the closest one.
    let clip_idx = clips
        .iter()
        .position(|c| time >= c.start && time <= c.start + c.duration)
        .or_else(|| {
            (0..clips.len()).min_by(|&a, &b| {
                let da = (clips[a].start - time).abs();
                let db = (clips[b].start - time).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
        });

    let Some(clip_idx) = clip_idx else {
        // Empty track — make a new clip starting at the playhead.
        clips.push(CameraClip {
            start: time,
            duration: 2.0,
            name: format!("Clip {}", clips.len() + 1),
            keys: vec![CameraKey {
                t: 0.0,
                translation: xf.translation,
                rotation: xf.rotation,
                fov_deg,
            }],
        });
        return;
    };

    let clip = &mut clips[clip_idx];
    let local_t = (time - clip.start).clamp(0.0, clip.duration);
    let key = CameraKey {
        t: local_t,
        translation: xf.translation,
        rotation: xf.rotation,
        fov_deg,
    };
    // Replace existing key at the same time, or insert in order.
    if let Some(existing) = clip
        .keys
        .iter_mut()
        .find(|k| (k.t - local_t).abs() < 1e-3)
    {
        *existing = key;
    } else {
        clip.keys.push(key);
        clip.keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    }
}

// ─── Playhead advance ───────────────────────────────────────────────────────

pub fn advance_playhead(time: Res<Time>, mut state: ResMut<SequencerState>) {
    if !state.playing {
        return;
    }
    let dt = time.delta_secs() * state.play_rate;
    let mut new_t = state.playhead + dt;
    let dur = state.sequence.duration;
    if new_t >= dur {
        if state.looping {
            new_t = new_t.rem_euclid(dur);
        } else {
            new_t = dur;
            state.playing = false;
        }
    }
    state.playhead = new_t;
}

// ─── Apply: Camera tracks ───────────────────────────────────────────────────

/// While the playhead is inside a camera clip, override the editor camera's
/// transform (and FOV when the clip provides one). When no clip is active,
/// release the camera so the user gets orbit/pan/zoom back.
pub fn apply_camera_tracks(
    mut state: ResMut<SequencerState>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<EditorCamera>>,
) {
    let playhead = state.playhead;
    let active = find_active_camera_pose(&state.sequence, playhead);

    let Ok((mut xf, mut proj)) = cameras.single_mut() else {
        return;
    };

    match active {
        Some(pose) => {
            xf.translation = pose.translation;
            xf.rotation = pose.rotation;
            if let (Some(fov), Projection::Perspective(persp)) = (pose.fov_deg, proj.as_mut()) {
                persp.fov = fov.to_radians();
            }
            state.camera_owned = true;
        }
        None => {
            // Was driven last frame, isn't now — leave the camera where it is
            // and just release ownership. The user takes over from this pose.
            state.camera_owned = false;
        }
    }
}

struct CameraPose {
    translation: Vec3,
    rotation: Quat,
    fov_deg: Option<f32>,
}

fn find_active_camera_pose(seq: &Sequence, playhead: f32) -> Option<CameraPose> {
    for track in &seq.tracks {
        if track.muted {
            continue;
        }
        let TrackKind::Camera { clips } = &track.kind else {
            continue;
        };
        // First clip whose [start, start+duration] contains the playhead.
        let Some(clip) = clips
            .iter()
            .find(|c| playhead >= c.start && playhead <= c.start + c.duration)
        else {
            continue;
        };
        return Some(sample_camera_clip(clip, playhead - clip.start));
    }
    None
}

fn sample_camera_clip(clip: &CameraClip, local_t: f32) -> CameraPose {
    if clip.keys.is_empty() {
        return CameraPose {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            fov_deg: None,
        };
    }
    if clip.keys.len() == 1 {
        let k = &clip.keys[0];
        return CameraPose {
            translation: k.translation,
            rotation: k.rotation,
            fov_deg: k.fov_deg,
        };
    }
    // Find the surrounding pair.
    let (a, b) = match clip.keys.windows(2).find(|w| local_t >= w[0].t && local_t <= w[1].t) {
        Some(w) => (&w[0], &w[1]),
        None => {
            // Extrapolate: clamp to first or last.
            return if local_t <= clip.keys[0].t {
                let k = &clip.keys[0];
                CameraPose {
                    translation: k.translation,
                    rotation: k.rotation,
                    fov_deg: k.fov_deg,
                }
            } else {
                let k = clip.keys.last().unwrap();
                CameraPose {
                    translation: k.translation,
                    rotation: k.rotation,
                    fov_deg: k.fov_deg,
                }
            };
        }
    };
    let span = (b.t - a.t).max(1e-5);
    let u = ((local_t - a.t) / span).clamp(0.0, 1.0);
    let u = smoothstep(u);
    CameraPose {
        translation: a.translation.lerp(b.translation, u),
        rotation: a.rotation.slerp(b.rotation, u),
        fov_deg: match (a.fov_deg, b.fov_deg) {
            (Some(x), Some(y)) => Some(x + (y - x) * u),
            (Some(x), None) | (None, Some(x)) => Some(x),
            _ => None,
        },
    }
}

fn smoothstep(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

// ─── Apply: Transform tracks ────────────────────────────────────────────────

/// Drive named entities' Transforms from any active TransformClip.
pub fn apply_transform_tracks(state: Res<SequencerState>, mut q: Query<(&EntityTag, &mut Transform)>) {
    let playhead = state.playhead;
    for track in &state.sequence.tracks {
        if track.muted {
            continue;
        }
        let TrackKind::Transform { target_tag, clips } = &track.kind else {
            continue;
        };
        let Some(clip) = clips
            .iter()
            .find(|c| playhead >= c.start && playhead <= c.start + c.duration)
        else {
            continue;
        };
        let pose = sample_transform_clip(clip, playhead - clip.start);
        for (tag, mut xf) in q.iter_mut() {
            if &tag.tag == target_tag {
                xf.translation = pose.translation;
                xf.rotation = pose.rotation;
                xf.scale = pose.scale;
            }
        }
    }
}

struct TransformPose {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

fn sample_transform_clip(clip: &TransformClip, local_t: f32) -> TransformPose {
    if clip.keys.is_empty() {
        return TransformPose {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
    }
    if clip.keys.len() == 1 {
        let k = &clip.keys[0];
        return TransformPose {
            translation: k.translation,
            rotation: k.rotation,
            scale: k.scale,
        };
    }
    let (a, b) = match clip.keys.windows(2).find(|w| local_t >= w[0].t && local_t <= w[1].t) {
        Some(w) => (&w[0], &w[1]),
        None => {
            return if local_t <= clip.keys[0].t {
                let k = &clip.keys[0];
                TransformPose {
                    translation: k.translation,
                    rotation: k.rotation,
                    scale: k.scale,
                }
            } else {
                let k = clip.keys.last().unwrap();
                TransformPose {
                    translation: k.translation,
                    rotation: k.rotation,
                    scale: k.scale,
                }
            };
        }
    };
    let span = (b.t - a.t).max(1e-5);
    let u = smoothstep(((local_t - a.t) / span).clamp(0.0, 1.0));
    TransformPose {
        translation: a.translation.lerp(b.translation, u),
        rotation: a.rotation.slerp(b.rotation, u),
        scale: a.scale.lerp(b.scale, u),
    }
}

// ─── Helpers used by the panel ──────────────────────────────────────────────

/// Push an action onto the bridge from the panel UI. Swallows poison —
/// nothing we can do from `&self` if the lock is poisoned.
pub fn push_action(bridge: &SequencerBridge, action: SequencerAction) {
    if let Ok(mut g) = bridge.pending.lock() {
        g.push(action);
    }
}

/// Iterate clips of any kind on a track as `(start, duration, name)`.
/// Used by the panel to draw clip rectangles without caring about the
/// payload type.
pub fn track_clip_views(track: &Track) -> Vec<ClipView> {
    match &track.kind {
        TrackKind::Camera { clips } => clips
            .iter()
            .map(|c| ClipView {
                start: c.start,
                duration: c.duration,
                name: c.name.clone(),
                key_count: c.keys.len(),
            })
            .collect(),
        TrackKind::Transform { clips, .. } => clips
            .iter()
            .map(|c| ClipView {
                start: c.start,
                duration: c.duration,
                name: c.name.clone(),
                key_count: c.keys.len(),
            })
            .collect(),
        TrackKind::Marker { clips } => clips
            .iter()
            .map(|c| ClipView {
                start: c.start,
                duration: c.duration.max(0.0),
                name: c.label.clone(),
                key_count: 0,
            })
            .collect(),
        TrackKind::Media { clips } => clips
            .iter()
            .map(|c| ClipView {
                start: c.start,
                duration: c.duration,
                name: c.name.clone(),
                key_count: 0,
            })
            .collect(),
    }
}

pub struct ClipView {
    pub start: f32,
    pub duration: f32,
    pub name: String,
    pub key_count: usize,
}

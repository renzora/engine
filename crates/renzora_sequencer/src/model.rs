//! Sequence / Track / Clip data model.
//!
//! Sequences are the top-level edit. They contain typed tracks; each track
//! type has its own clip payload. The timeline UI and the per-track-type
//! "apply" systems both walk this model.
//!
//! Serializable so we can save sequences as `.renseq` files later.

use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

/// One timeline edit. Saved to disk as `.renseq` (RON).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sequence {
    pub name: String,
    /// Total length in seconds. Clips beyond this are clipped during playback.
    pub duration: f32,
    /// Playback timebase. Bake-to-video uses this to choose its frame step.
    pub fps: u32,
    pub tracks: Vec<Track>,
}

impl Sequence {
    pub fn new_demo() -> Self {
        Self {
            name: "Untitled".into(),
            duration: 10.0,
            fps: 60,
            tracks: vec![
                Track {
                    name: "Camera".into(),
                    muted: false,
                    locked: false,
                    kind: TrackKind::Camera {
                        clips: vec![CameraClip {
                            start: 0.0,
                            duration: 5.0,
                            name: "Establishing".into(),
                            keys: vec![
                                CameraKey {
                                    t: 0.0,
                                    translation: Vec3::new(0.0, 2.0, 8.0),
                                    rotation: Quat::IDENTITY,
                                    fov_deg: Some(60.0),
                                },
                                CameraKey {
                                    t: 5.0,
                                    translation: Vec3::new(4.0, 3.0, 6.0),
                                    rotation: Quat::IDENTITY,
                                    fov_deg: Some(45.0),
                                },
                            ],
                        }],
                    },
                },
                Track {
                    name: "Markers".into(),
                    muted: false,
                    locked: false,
                    kind: TrackKind::Marker { clips: vec![] },
                },
                Track {
                    name: "Media".into(),
                    muted: false,
                    locked: false,
                    kind: TrackKind::Media { clips: vec![] },
                },
            ],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub muted: bool,
    pub locked: bool,
    pub kind: TrackKind,
}

/// Typed payload per track. Each variant carries its own clip list because
/// clip data shapes diverge (a camera clip holds keyframes, a media clip
/// holds a file path).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TrackKind {
    /// Drives the editor camera's transform/FOV between keyframes during the
    /// clip's time range.
    Camera { clips: Vec<CameraClip> },
    /// Drives a target entity's `Transform` between keyframes. Resolved by
    /// `EntityTag` so re-loading the scene rebinds correctly.
    Transform {
        target_tag: String,
        clips: Vec<TransformClip>,
    },
    /// Time-stamped labels — purely cosmetic, but they show up as bake
    /// chapter marks once we generate them.
    Marker { clips: Vec<MarkerClip> },
    /// Pre-baked video. Not played back live yet (decode pipeline is its own
    /// project) — currently used as a record of what bakes belong to this
    /// sequence so users can re-bake / replace them.
    Media { clips: Vec<MediaClip> },
}

impl TrackKind {
    pub fn type_label(&self) -> &'static str {
        match self {
            TrackKind::Camera { .. } => "Camera",
            TrackKind::Transform { .. } => "Transform",
            TrackKind::Marker { .. } => "Marker",
            TrackKind::Media { .. } => "Media",
        }
    }
}

// ─── Camera clips ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraClip {
    /// Start of the clip on the parent timeline (seconds).
    pub start: f32,
    /// Length of the clip on the timeline (seconds).
    pub duration: f32,
    pub name: String,
    /// Keyframes in *clip-local* time (0 = clip start).
    pub keys: Vec<CameraKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraKey {
    /// Clip-local time in seconds.
    pub t: f32,
    pub translation: Vec3,
    pub rotation: Quat,
    /// Vertical FOV in degrees. `None` = leave existing camera FOV alone.
    pub fov_deg: Option<f32>,
}

// ─── Transform clips ────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransformClip {
    pub start: f32,
    pub duration: f32,
    pub name: String,
    pub keys: Vec<TransformKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransformKey {
    pub t: f32,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

// ─── Marker clips ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkerClip {
    /// Marker time on the parent timeline. (Markers are point-in-time —
    /// `duration` is unused but kept so they share the Clip arithmetic.)
    pub start: f32,
    pub duration: f32,
    pub label: String,
}

// ─── Media clips ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaClip {
    pub start: f32,
    pub duration: f32,
    pub name: String,
    /// Project-relative path to the video file produced by a bake.
    pub source_path: String,
}

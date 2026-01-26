//! Animation component data types
//!
//! Stores animation clips and keyframes for entities.

#![allow(dead_code)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Interpolation method for keyframes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
#[reflect(Default)]
pub enum KeyframeInterpolation {
    #[default]
    Linear,
    Step,
    CubicBezier,
}

/// A single keyframe value
#[derive(Debug, Clone, Reflect, Serialize, Deserialize)]
pub enum KeyframeValue {
    Float(f32),
    Vec3(Vec3),
    Quat(Quat),
    Color([f32; 4]),
}

impl Default for KeyframeValue {
    fn default() -> Self {
        KeyframeValue::Float(0.0)
    }
}

/// A keyframe at a specific time
#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
pub struct Keyframe {
    /// Time in seconds from clip start
    pub time: f32,
    /// The value at this keyframe
    pub value: KeyframeValue,
    /// Interpolation to the next keyframe
    pub interpolation: KeyframeInterpolation,
}

/// Property that can be animated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect, Serialize, Deserialize)]
#[reflect(Default)]
pub enum AnimatableProperty {
    #[default]
    PositionX,
    PositionY,
    PositionZ,
    RotationX,
    RotationY,
    RotationZ,
    ScaleX,
    ScaleY,
    ScaleZ,
    /// For light intensity, etc.
    Custom(u32),
}

impl AnimatableProperty {
    pub fn display_name(&self) -> &'static str {
        match self {
            AnimatableProperty::PositionX => "Position X",
            AnimatableProperty::PositionY => "Position Y",
            AnimatableProperty::PositionZ => "Position Z",
            AnimatableProperty::RotationX => "Rotation X",
            AnimatableProperty::RotationY => "Rotation Y",
            AnimatableProperty::RotationZ => "Rotation Z",
            AnimatableProperty::ScaleX => "Scale X",
            AnimatableProperty::ScaleY => "Scale Y",
            AnimatableProperty::ScaleZ => "Scale Z",
            AnimatableProperty::Custom(_) => "Custom",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            AnimatableProperty::PositionX => "Pos.X",
            AnimatableProperty::PositionY => "Pos.Y",
            AnimatableProperty::PositionZ => "Pos.Z",
            AnimatableProperty::RotationX => "Rot.X",
            AnimatableProperty::RotationY => "Rot.Y",
            AnimatableProperty::RotationZ => "Rot.Z",
            AnimatableProperty::ScaleX => "Scl.X",
            AnimatableProperty::ScaleY => "Scl.Y",
            AnimatableProperty::ScaleZ => "Scl.Z",
            AnimatableProperty::Custom(_) => "Custom",
        }
    }

    pub fn color(&self) -> [u8; 3] {
        match self {
            AnimatableProperty::PositionX => [220, 80, 80],   // Red
            AnimatableProperty::PositionY => [80, 180, 80],   // Green
            AnimatableProperty::PositionZ => [80, 120, 220],  // Blue
            AnimatableProperty::RotationX => [220, 120, 80],  // Orange-red
            AnimatableProperty::RotationY => [120, 200, 80],  // Yellow-green
            AnimatableProperty::RotationZ => [80, 160, 220],  // Cyan-blue
            AnimatableProperty::ScaleX => [200, 80, 160],     // Pink
            AnimatableProperty::ScaleY => [160, 200, 80],     // Lime
            AnimatableProperty::ScaleZ => [80, 200, 200],     // Cyan
            AnimatableProperty::Custom(_) => [160, 160, 160], // Gray
        }
    }
}

/// A track containing keyframes for a single property
#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
pub struct AnimationTrack {
    /// The property this track animates
    pub property: AnimatableProperty,
    /// Keyframes sorted by time
    pub keyframes: Vec<Keyframe>,
    /// Whether this track is muted (not applied during playback)
    pub muted: bool,
    /// Whether this track is locked (can't be edited)
    pub locked: bool,
}

impl AnimationTrack {
    pub fn new(property: AnimatableProperty) -> Self {
        Self {
            property,
            keyframes: Vec::new(),
            muted: false,
            locked: false,
        }
    }

    /// Add a keyframe, maintaining sorted order by time
    pub fn add_keyframe(&mut self, keyframe: Keyframe) {
        let insert_idx = self
            .keyframes
            .iter()
            .position(|k| k.time > keyframe.time)
            .unwrap_or(self.keyframes.len());
        self.keyframes.insert(insert_idx, keyframe);
    }

    /// Remove keyframe at index
    pub fn remove_keyframe(&mut self, index: usize) {
        if index < self.keyframes.len() {
            self.keyframes.remove(index);
        }
    }

    /// Get the interpolated value at a given time
    pub fn sample(&self, time: f32) -> Option<KeyframeValue> {
        if self.keyframes.is_empty() {
            return None;
        }

        // Find the two keyframes to interpolate between
        let mut prev_idx = 0;
        let mut next_idx = 0;

        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.time <= time {
                prev_idx = i;
            }
            if kf.time >= time {
                next_idx = i;
                break;
            }
            next_idx = i;
        }

        let prev_kf = &self.keyframes[prev_idx];
        let next_kf = &self.keyframes[next_idx];

        // If same keyframe or step interpolation, return directly
        if prev_idx == next_idx || prev_kf.interpolation == KeyframeInterpolation::Step {
            return Some(prev_kf.value.clone());
        }

        // Calculate interpolation factor
        let duration = next_kf.time - prev_kf.time;
        if duration <= 0.0 {
            return Some(prev_kf.value.clone());
        }

        let t = (time - prev_kf.time) / duration;

        // Interpolate based on value type
        match (&prev_kf.value, &next_kf.value) {
            (KeyframeValue::Float(a), KeyframeValue::Float(b)) => {
                Some(KeyframeValue::Float(a + (b - a) * t))
            }
            (KeyframeValue::Vec3(a), KeyframeValue::Vec3(b)) => {
                Some(KeyframeValue::Vec3(a.lerp(*b, t)))
            }
            (KeyframeValue::Quat(a), KeyframeValue::Quat(b)) => {
                Some(KeyframeValue::Quat(a.slerp(*b, t)))
            }
            (KeyframeValue::Color(a), KeyframeValue::Color(b)) => {
                Some(KeyframeValue::Color([
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    a[2] + (b[2] - a[2]) * t,
                    a[3] + (b[3] - a[3]) * t,
                ]))
            }
            _ => Some(prev_kf.value.clone()),
        }
    }

    /// Get the duration of this track (time of last keyframe)
    pub fn duration(&self) -> f32 {
        self.keyframes.last().map(|k| k.time).unwrap_or(0.0)
    }
}

/// An animation clip containing multiple tracks
#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
pub struct AnimationClip {
    /// Name of this clip
    pub name: String,
    /// Tracks for different properties
    pub tracks: Vec<AnimationTrack>,
    /// Whether this clip loops
    pub looping: bool,
    /// Playback speed multiplier
    pub speed: f32,
}

impl AnimationClip {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tracks: Vec::new(),
            looping: false,
            speed: 1.0,
        }
    }

    /// Get or create a track for the given property
    pub fn get_or_create_track(&mut self, property: AnimatableProperty) -> &mut AnimationTrack {
        let idx = self
            .tracks
            .iter()
            .position(|t| t.property == property);

        match idx {
            Some(i) => &mut self.tracks[i],
            None => {
                self.tracks.push(AnimationTrack::new(property));
                self.tracks.last_mut().unwrap()
            }
        }
    }

    /// Get a track for the given property
    pub fn get_track(&self, property: AnimatableProperty) -> Option<&AnimationTrack> {
        self.tracks.iter().find(|t| t.property == property)
    }

    /// Get the total duration of this clip
    pub fn duration(&self) -> f32 {
        self.tracks
            .iter()
            .map(|t| t.duration())
            .fold(0.0f32, |a, b| a.max(b))
    }

    /// Sample all tracks at the given time
    pub fn sample(&self, time: f32) -> HashMap<AnimatableProperty, KeyframeValue> {
        let mut values = HashMap::new();
        for track in &self.tracks {
            if !track.muted {
                if let Some(value) = track.sample(time) {
                    values.insert(track.property, value);
                }
            }
        }
        values
    }
}

/// Component storing animation data for an entity
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct AnimationData {
    /// Animation clips available for this entity
    pub clips: Vec<AnimationClip>,
    /// Index of the currently active clip (for playback)
    pub active_clip: Option<usize>,
}

impl AnimationData {
    pub fn new() -> Self {
        Self {
            clips: Vec::new(),
            active_clip: None,
        }
    }

    /// Add a new clip
    pub fn add_clip(&mut self, clip: AnimationClip) -> usize {
        let idx = self.clips.len();
        self.clips.push(clip);
        idx
    }

    /// Get the active clip
    pub fn get_active_clip(&self) -> Option<&AnimationClip> {
        self.active_clip.and_then(|i| self.clips.get(i))
    }

    /// Get the active clip mutably
    pub fn get_active_clip_mut(&mut self) -> Option<&mut AnimationClip> {
        self.active_clip.and_then(|i| self.clips.get_mut(i))
    }

    /// Set the active clip by index
    pub fn set_active_clip(&mut self, index: usize) {
        if index < self.clips.len() {
            self.active_clip = Some(index);
        }
    }
}

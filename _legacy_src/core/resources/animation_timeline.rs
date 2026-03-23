//! Animation timeline editor state

#![allow(dead_code)]

use bevy::prelude::*;
use std::collections::HashSet;

use crate::component_system::data::components::animation::AnimatableProperty;

/// Playback state for the animation timeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimelinePlayState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Selection state for keyframes
#[derive(Debug, Clone, Default)]
pub struct KeyframeSelection {
    /// Selected keyframes: (track_index, keyframe_index)
    pub selected: HashSet<(usize, usize)>,
}

impl KeyframeSelection {
    pub fn clear(&mut self) {
        self.selected.clear();
    }

    pub fn select(&mut self, track: usize, keyframe: usize, extend: bool) {
        if !extend {
            self.selected.clear();
        }
        self.selected.insert((track, keyframe));
    }

    pub fn is_selected(&self, track: usize, keyframe: usize) -> bool {
        self.selected.contains(&(track, keyframe))
    }

    pub fn toggle(&mut self, track: usize, keyframe: usize) {
        let key = (track, keyframe);
        if self.selected.contains(&key) {
            self.selected.remove(&key);
        } else {
            self.selected.insert(key);
        }
    }
}

/// Resource tracking animation timeline editor state
#[derive(Resource, Default)]
pub struct AnimationTimelineState {
    /// Currently selected entity being edited
    pub selected_entity: Option<Entity>,
    /// Selected clip index for editing
    pub selected_clip: Option<usize>,
    /// Current playhead position in seconds
    pub current_time: f32,
    /// Playback state
    pub play_state: TimelinePlayState,
    /// Timeline view start time (for scrolling)
    pub view_start: f32,
    /// Timeline view end time (for scrolling)
    pub view_end: f32,
    /// Zoom level (seconds visible in view)
    pub zoom: f32,
    /// Selected keyframes
    pub keyframe_selection: KeyframeSelection,
    /// Track being dragged (for keyframe dragging)
    pub dragging_keyframe: Option<(usize, usize)>,
    /// Original time of keyframe being dragged
    pub drag_start_time: f32,
    /// Whether the scrubber is being dragged
    pub scrubbing: bool,
    /// Snapping enabled
    pub snap_enabled: bool,
    /// Snap increment in seconds
    pub snap_increment: f32,
    /// Whether to show all transform tracks expanded
    pub tracks_expanded: bool,
    /// Auto-key mode: automatically create keyframes when properties change
    pub auto_key: bool,
    /// Show curves (value over time) instead of keyframe diamonds
    pub show_curves: bool,
    /// Track filter - which properties to show
    pub track_filter: TrackFilter,
}

/// Filter for which tracks to display
#[derive(Debug, Clone, Default)]
pub struct TrackFilter {
    pub show_position: bool,
    pub show_rotation: bool,
    pub show_scale: bool,
    pub show_custom: bool,
}

impl TrackFilter {
    pub fn all() -> Self {
        Self {
            show_position: true,
            show_rotation: true,
            show_scale: true,
            show_custom: true,
        }
    }

    pub fn should_show(&self, property: AnimatableProperty) -> bool {
        match property {
            AnimatableProperty::PositionX
            | AnimatableProperty::PositionY
            | AnimatableProperty::PositionZ => self.show_position,
            AnimatableProperty::RotationX
            | AnimatableProperty::RotationY
            | AnimatableProperty::RotationZ => self.show_rotation,
            AnimatableProperty::ScaleX
            | AnimatableProperty::ScaleY
            | AnimatableProperty::ScaleZ => self.show_scale,
            AnimatableProperty::Custom(_) => self.show_custom,
        }
    }
}

impl AnimationTimelineState {
    pub fn new() -> Self {
        Self {
            view_start: 0.0,
            view_end: 5.0,
            zoom: 5.0,
            snap_increment: 0.1,
            snap_enabled: true,
            tracks_expanded: true,
            track_filter: TrackFilter::all(),
            ..Default::default()
        }
    }

    /// Reset timeline state when selecting a new entity
    pub fn reset_for_entity(&mut self, entity: Entity) {
        self.selected_entity = Some(entity);
        self.selected_clip = None;
        self.current_time = 0.0;
        self.play_state = TimelinePlayState::Stopped;
        self.keyframe_selection.clear();
        self.dragging_keyframe = None;
        self.scrubbing = false;
    }

    /// Update playhead position, respecting bounds and snapping
    pub fn set_time(&mut self, time: f32, clip_duration: f32) {
        let mut t = time.max(0.0);

        if self.snap_enabled && self.snap_increment > 0.0 {
            t = (t / self.snap_increment).round() * self.snap_increment;
        }

        self.current_time = t.min(clip_duration.max(0.1));
    }

    /// Zoom in on timeline
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 0.8).max(0.5);
        self.update_view_range();
    }

    /// Zoom out on timeline
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom * 1.25).min(60.0);
        self.update_view_range();
    }

    /// Update view range based on zoom level
    fn update_view_range(&mut self) {
        let center = (self.view_start + self.view_end) / 2.0;
        self.view_start = (center - self.zoom / 2.0).max(0.0);
        self.view_end = self.view_start + self.zoom;
    }

    /// Scroll timeline view
    pub fn scroll(&mut self, delta: f32) {
        self.view_start = (self.view_start + delta).max(0.0);
        self.view_end = self.view_start + self.zoom;
    }

    /// Center view on current time
    pub fn center_on_playhead(&mut self) {
        self.view_start = (self.current_time - self.zoom / 2.0).max(0.0);
        self.view_end = self.view_start + self.zoom;
    }

    /// Frame the entire clip in view
    pub fn frame_all(&mut self, clip_duration: f32) {
        self.view_start = 0.0;
        self.zoom = (clip_duration * 1.1).max(1.0);
        self.view_end = self.zoom;
    }

    /// Convert a time value to x position in the timeline
    pub fn time_to_x(&self, time: f32, timeline_width: f32) -> f32 {
        let t = (time - self.view_start) / (self.view_end - self.view_start);
        t * timeline_width
    }

    /// Convert an x position in the timeline to time
    pub fn x_to_time(&self, x: f32, timeline_width: f32) -> f32 {
        let t = x / timeline_width;
        self.view_start + t * (self.view_end - self.view_start)
    }

    /// Toggle play/pause
    pub fn toggle_play(&mut self) {
        self.play_state = match self.play_state {
            TimelinePlayState::Stopped | TimelinePlayState::Paused => TimelinePlayState::Playing,
            TimelinePlayState::Playing => TimelinePlayState::Paused,
        };
    }

    /// Stop playback and reset to beginning
    pub fn stop(&mut self) {
        self.play_state = TimelinePlayState::Stopped;
        self.current_time = 0.0;
    }

    /// Advance playhead during playback
    pub fn advance(&mut self, delta_seconds: f32, clip_duration: f32, looping: bool) {
        if self.play_state != TimelinePlayState::Playing {
            return;
        }

        self.current_time += delta_seconds;

        if self.current_time >= clip_duration {
            if looping {
                self.current_time = self.current_time % clip_duration;
            } else {
                self.current_time = clip_duration;
                self.play_state = TimelinePlayState::Paused;
            }
        }
    }
}

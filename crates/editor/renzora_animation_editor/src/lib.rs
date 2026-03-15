//! Animation Editor — animation panel + timeline panel for the Renzora editor.
//!
//! Provides two panels:
//! - `animation` — clip library, state machine states/transitions, parameters, layers
//! - `timeline` — transport bar, time ruler, scrubber, track lanes, keyframe editing

mod animation_panel;
mod timeline_panel;
mod preview;
pub mod studio_preview;
mod studio_preview_panel;

use bevy::prelude::*;
use renzora_editor::AppEditorExt;

use std::sync::{Arc, Mutex};

/// Persistent editor state for the animation editor.
#[derive(Resource)]
pub struct AnimationEditorState {
    /// Currently selected entity for animation editing.
    pub selected_entity: Option<Entity>,
    /// Scrubber time position in seconds.
    pub scrub_time: f32,
    /// Whether preview playback is active.
    pub is_previewing: bool,
    /// Preview playback speed.
    pub preview_speed: f32,
    /// Whether preview playback loops.
    pub preview_looping: bool,
    /// Timeline zoom level (pixels per second).
    pub timeline_zoom: f32,
    /// Timeline scroll offset in seconds.
    pub timeline_scroll: f32,
    /// Currently selected clip name.
    pub selected_clip: Option<String>,
    /// Whether the timeline snap is enabled.
    pub snap_enabled: bool,
    /// Snap interval in seconds.
    pub snap_interval: f32,
    /// Cached duration of the currently selected clip (seconds).
    pub clip_duration: Option<f32>,
    /// Track which clip was last auto-fitted to avoid re-fitting on every frame.
    pub auto_fit_clip: Option<String>,
}

impl Default for AnimationEditorState {
    fn default() -> Self {
        Self {
            selected_entity: None,
            scrub_time: 0.0,
            is_previewing: false,
            preview_speed: 1.0,
            preview_looping: true,
            timeline_zoom: 100.0,
            timeline_scroll: 0.0,
            selected_clip: None,
            snap_enabled: true,
            snap_interval: 1.0 / 30.0,
            clip_duration: None,
            auto_fit_clip: None,
        }
    }
}

/// Bridge for mutation requests from panels to the sync system.
#[derive(Resource, Clone, Default)]
struct AnimEditorBridge {
    pending: Arc<Mutex<Vec<AnimEditorAction>>>,
}

/// Actions that the panels can request.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum AnimEditorAction {
    SelectClip(Option<String>),
    SetScrubTime(f32),
    TogglePreview,
    StopPreview,
    SetPreviewSpeed(f32),
    SetPreviewLooping(bool),
    SetTimelineZoom(f32),
    SetTimelineScroll(f32),
    SetSnapEnabled(bool),
    AutoFitDone(String),
    SetParam { name: String, value: f32 },
    SetBoolParam { name: String, value: bool },
    FireTrigger { name: String },
    SetLayerWeight { layer: String, weight: f32 },
}

/// System that applies pending mutations from the panels.
fn sync_anim_editor_bridge(
    bridge: Res<AnimEditorBridge>,
    mut editor_state: ResMut<AnimationEditorState>,
    anim_queue: Option<ResMut<renzora_animation::AnimationCommandQueue>>,
) {
    let actions: Vec<AnimEditorAction> = {
        let mut pending = bridge.pending.lock().unwrap();
        pending.drain(..).collect()
    };

    let mut anim_queue = anim_queue;

    for action in actions {
        match action {
            AnimEditorAction::SelectClip(name) => {
                editor_state.selected_clip = name;
                editor_state.scrub_time = 0.0; // start from beginning
                editor_state.clip_duration = None; // force re-read
                editor_state.auto_fit_clip = None; // force re-fit
            }
            AnimEditorAction::SetScrubTime(t) => {
                editor_state.scrub_time = t;
            }
            AnimEditorAction::TogglePreview => {
                editor_state.is_previewing = !editor_state.is_previewing;
            }
            AnimEditorAction::StopPreview => {
                editor_state.is_previewing = false;
                editor_state.scrub_time = 0.0;
            }
            AnimEditorAction::SetPreviewSpeed(s) => {
                editor_state.preview_speed = s;
            }
            AnimEditorAction::SetPreviewLooping(l) => {
                editor_state.preview_looping = l;
            }
            AnimEditorAction::SetTimelineZoom(z) => {
                editor_state.timeline_zoom = z.clamp(20.0, 500.0);
            }
            AnimEditorAction::SetTimelineScroll(s) => {
                editor_state.timeline_scroll = s.max(0.0);
            }
            AnimEditorAction::SetSnapEnabled(enabled) => {
                editor_state.snap_enabled = enabled;
            }
            AnimEditorAction::AutoFitDone(clip_name) => {
                editor_state.auto_fit_clip = Some(clip_name);
            }
            AnimEditorAction::SetParam { name, value } => {
                if let (Some(entity), Some(q)) = (editor_state.selected_entity, anim_queue.as_mut()) {
                    q.commands.push(renzora_animation::AnimationCommand::SetParam {
                        entity, name, value,
                    });
                }
            }
            AnimEditorAction::SetBoolParam { name, value } => {
                if let (Some(entity), Some(q)) = (editor_state.selected_entity, anim_queue.as_mut()) {
                    q.commands.push(renzora_animation::AnimationCommand::SetBoolParam {
                        entity, name, value,
                    });
                }
            }
            AnimEditorAction::FireTrigger { name } => {
                if let (Some(entity), Some(q)) = (editor_state.selected_entity, anim_queue.as_mut()) {
                    q.commands.push(renzora_animation::AnimationCommand::Trigger {
                        entity, name,
                    });
                }
            }
            AnimEditorAction::SetLayerWeight { layer, weight } => {
                if let (Some(entity), Some(q)) = (editor_state.selected_entity, anim_queue.as_mut()) {
                    q.commands.push(renzora_animation::AnimationCommand::SetLayerWeight {
                        entity, layer_name: layer, weight,
                    });
                }
            }
        }
    }
}

/// Cache the duration of the selected clip by reading the .anim file from disk.
fn cache_clip_duration(
    mut editor_state: ResMut<AnimationEditorState>,
    animators: Query<&renzora_animation::AnimatorComponent>,
    project: Option<Res<renzora_core::CurrentProject>>,
) {
    let Some(entity) = editor_state.selected_entity else {
        editor_state.clip_duration = None;
        return;
    };

    let Some(clip_name) = editor_state.selected_clip.as_deref() else {
        editor_state.clip_duration = None;
        return;
    };

    // Only re-read if we don't already have a cached duration
    if editor_state.clip_duration.is_some() {
        return;
    }

    let Ok(animator) = animators.get(entity) else {
        return;
    };

    let Some(slot) = animator.clips.iter().find(|s| s.name == clip_name) else {
        return;
    };

    let Some(project) = project else {
        return;
    };

    let anim_path = project.path.join("assets").join(&slot.path);
    if let Ok(content) = std::fs::read_to_string(&anim_path) {
        if let Ok(clip) = ron::from_str::<renzora_animation::AnimClip>(&content) {
            editor_state.clip_duration = Some(clip.duration);
        }
    }
}

/// Sync EditorSelection into AnimationEditorState so the animation panels
/// automatically follow the entity selected in the hierarchy/inspector.
fn sync_selection(
    selection: Res<renzora_editor::EditorSelection>,
    mut editor_state: ResMut<AnimationEditorState>,
) {
    let selected = selection.get();
    if editor_state.selected_entity != selected {
        editor_state.selected_entity = selected;
        // Reset clip selection when entity changes
        editor_state.selected_clip = None;
        editor_state.scrub_time = 0.0;
        editor_state.is_previewing = false;
        editor_state.clip_duration = None;
    }
}

pub struct AnimationEditorPlugin;

impl Plugin for AnimationEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AnimationEditorPlugin");
        let bridge = AnimEditorBridge::default();
        let arc = bridge.pending.clone();

        app.init_resource::<AnimationEditorState>();
        app.init_resource::<preview::PreviewPlaybackState>();
        app.init_resource::<studio_preview::StudioPreviewImage>();
        app.init_resource::<studio_preview::StudioPreviewOrbit>();
        app.init_resource::<studio_preview::StudioPreviewTracker>();
        app.init_resource::<studio_preview::StudioPreviewSettings>();
        app.insert_resource(bridge);

        studio_preview::register_preview_gizmos(app);

        app.add_systems(
            PostStartup,
            studio_preview::setup_studio_preview,
        );

        app.add_systems(
            Update,
            (
                sync_selection,
                sync_anim_editor_bridge,
                cache_clip_duration,
                preview::update_animation_preview,
                preview::sync_preview_animation_graph,
                preview::sync_preview_clear_color,
                studio_preview::resize_preview,
                studio_preview::sync_preview_model,
                studio_preview::propagate_preview_layer,
                studio_preview::auto_fit_preview_camera,
                studio_preview::update_studio_preview_camera,
                studio_preview::draw_preview_skeleton,
                studio_preview::sync_floor_visibility,
            )
                .chain()
                .run_if(in_state(renzora_editor::SplashState::Editor)),
        );

        app.register_panel(animation_panel::AnimationPanel::new(arc.clone()));
        app.register_panel(timeline_panel::TimelinePanel::new(arc));
        app.register_panel(studio_preview_panel::StudioPreviewPanel);
    }
}

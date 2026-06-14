//! Animation Editor — animation panel + timeline panel for the Renzora editor.
//!
//! Provides two panels:
//! - `animation` — clip library, state machine states/transitions, parameters, layers
//! - `timeline` — transport bar, time ruler, scrubber, track lanes, keyframe editing

mod inspector;
mod native_animation;
mod native_inspector;
mod native_params;
mod native_state_machine;
mod native_studio_preview;
mod native_timeline;
mod preview;
mod setup;
pub mod studio_preview;

use bevy::prelude::*;

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
    /// Height (pixels) of each keyframe track row. User-resizable.
    pub track_height: f32,
    /// When armed, edits to tracked property fields auto-key at the playhead.
    pub record_enabled: bool,
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
            track_height: 22.0,
            record_enabled: false,
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
    SetTrackHeight(f32),
    SetRecordEnabled(bool),
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
            AnimEditorAction::SetTrackHeight(h) => {
                editor_state.track_height = h.clamp(14.0, 96.0);
            }
            AnimEditorAction::SetRecordEnabled(enabled) => {
                editor_state.record_enabled = enabled;
            }
            AnimEditorAction::AutoFitDone(clip_name) => {
                editor_state.auto_fit_clip = Some(clip_name);
            }
            AnimEditorAction::SetParam { name, value } => {
                if let (Some(entity), Some(q)) = (editor_state.selected_entity, anim_queue.as_mut())
                {
                    q.commands
                        .push(renzora_animation::AnimationCommand::SetParam {
                            entity,
                            name,
                            value,
                        });
                }
            }
            AnimEditorAction::SetBoolParam { name, value } => {
                if let (Some(entity), Some(q)) = (editor_state.selected_entity, anim_queue.as_mut())
                {
                    q.commands
                        .push(renzora_animation::AnimationCommand::SetBoolParam {
                            entity,
                            name,
                            value,
                        });
                }
            }
            AnimEditorAction::FireTrigger { name } => {
                if let (Some(entity), Some(q)) = (editor_state.selected_entity, anim_queue.as_mut())
                {
                    q.commands
                        .push(renzora_animation::AnimationCommand::Trigger { entity, name });
                }
            }
            AnimEditorAction::SetLayerWeight { layer, weight } => {
                if let (Some(entity), Some(q)) = (editor_state.selected_entity, anim_queue.as_mut())
                {
                    q.commands
                        .push(renzora_animation::AnimationCommand::SetLayerWeight {
                            entity,
                            layer_name: layer,
                            weight,
                        });
                }
            }
        }
    }
}

/// Single owner of the preview playhead: advances `scrub_time` while Play is on,
/// independent of whether the entity has a skeleton, property tracks, or the
/// Studio Preview panel open. Wraps (loop) or stops at the clip duration. The
/// skeletal seek (`update_animation_preview`) and the property sampler both just
/// read `scrub_time`.
fn advance_preview_time(time: Res<Time>, mut state: ResMut<AnimationEditorState>) {
    if !state.is_previewing {
        return;
    }
    let speed = state.preview_speed;
    state.scrub_time += time.delta_secs() * speed;

    if let Some(duration) = state.clip_duration {
        if duration > 0.0 && state.scrub_time >= duration {
            if state.preview_looping {
                state.scrub_time %= duration;
            } else {
                state.scrub_time = duration;
                state.is_previewing = false;
            }
        }
    }
}

/// Cache the duration of the selected clip by reading the .anim file from disk.
fn cache_clip_duration(
    mut editor_state: ResMut<AnimationEditorState>,
    animators: Query<&renzora_animation::AnimatorComponent>,
    project: Option<Res<renzora::core::CurrentProject>>,
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

    let anim_path = project.path.join(&slot.path);
    if let Ok(content) = std::fs::read_to_string(&anim_path) {
        if let Ok(clip) = ron::from_str::<renzora_animation::AnimClip>(&content) {
            editor_state.clip_duration = Some(clip.duration);
        }
    }
}

/// Resolve a raw selection to the entity the animation editor should operate
/// on. Users click whatever is in front of them — a mesh child, a bone, the
/// flattened GLB root — while the `AnimatorComponent` / `MeshInstanceData`
/// live on an ancestor. Walk self → ancestors for an animator first, then for
/// a model root, so every panel works no matter which part of the model was
/// picked.
fn resolve_animation_entity(
    raw: Entity,
    animators: &Query<&renzora_animation::AnimatorComponent>,
    models: &Query<&renzora::core::MeshInstanceData>,
    parents: &Query<&ChildOf>,
) -> Entity {
    let ancestry = |start: Entity| {
        std::iter::successors(Some(start), |&e| parents.get(e).ok().map(|c| c.parent()))
    };
    if let Some(e) = ancestry(raw).find(|&e| animators.contains(e)) {
        return e;
    }
    if let Some(e) =
        ancestry(raw).find(|&e| models.get(e).is_ok_and(|m| m.model_path.is_some()))
    {
        return e;
    }
    raw
}

/// Sync EditorSelection into AnimationEditorState so the animation panels
/// automatically follow the entity selected in the hierarchy/inspector.
fn sync_selection(
    selection: Res<renzora_editor_framework::EditorSelection>,
    mut editor_state: ResMut<AnimationEditorState>,
    animators: Query<&renzora_animation::AnimatorComponent>,
    models: Query<&renzora::core::MeshInstanceData>,
    parents: Query<&ChildOf>,
) {
    let selected = selection
        .get()
        .map(|raw| resolve_animation_entity(raw, &animators, &models, &parents));
    if editor_state.selected_entity != selected {
        editor_state.selected_entity = selected;
        editor_state.scrub_time = 0.0;
        editor_state.is_previewing = false;
        editor_state.clip_duration = None;
        // Auto-select the animator's default clip (or first clip) when
        // the selected entity has an AnimatorComponent, so the timeline
        // doesn't reset to "Select clip…" on every selection change.
        editor_state.selected_clip = selected.and_then(|e| animators.get(e).ok()).and_then(|a| {
            a.default_clip
                .clone()
                .or_else(|| a.clips.first().map(|c| c.name.clone()))
        });
    }
}

#[derive(Default)]
pub struct AnimationEditorPlugin;

impl Plugin for AnimationEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AnimationEditorPlugin");
        let bridge = AnimEditorBridge::default();

        {
            use renzora::AppEditorExt;
            app.register_inspector(inspector::animator_inspector_entry());
        }
        native_inspector::register_animator_native(app);

        app.init_resource::<AnimationEditorState>();
        app.init_resource::<preview::PreviewPlaybackState>();
        app.init_resource::<studio_preview::StudioPreviewImage>();
        app.init_resource::<studio_preview::StudioPreviewOrbit>();
        app.init_resource::<studio_preview::StudioPreviewTracker>();
        app.init_resource::<studio_preview::StudioPreviewSettings>();
        app.insert_resource(bridge);

        studio_preview::register_preview_gizmos(app);
        app.add_observer(studio_preview::hide_new_preview_descendants);

        app.add_systems(PostStartup, studio_preview::setup_studio_preview);

        app.add_systems(
            Update,
            (
                sync_selection,
                sync_anim_editor_bridge,
                cache_clip_duration,
                studio_preview::sync_studio_preview_activation,
            )
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );

        // Single owner of the preview playhead. Runs even when no Studio Preview
        // panel is mounted and for property-only / skeleton-less entities.
        app.add_systems(
            Update,
            advance_preview_time
                .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                .run_if(renzora::not_in_play_mode),
        );

        // Heavy studio-preview work: only run when the Studio Preview panel is
        // actually in the dock tree. Other layouts (e.g. pure Scene) shouldn't
        // pay for an offscreen render + duplicate GLTF clone.
        app.add_systems(
            Update,
            (
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
                .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                .run_if(studio_preview::studio_preview_panel_mounted),
        );

        app.add_plugins(setup::AnimSetupPlugin);
        app.add_plugins(native_animation::NativeAnimationPanel);
        app.add_plugins(native_timeline::NativeAnimTimeline);
        app.add_plugins(native_params::NativeAnimParams);
        app.add_plugins(native_state_machine::NativeStateMachine);
        app.add_plugins(native_studio_preview::NativeStudioPreview);
    }
}

renzora::add!(AnimationEditorPlugin, Editor);

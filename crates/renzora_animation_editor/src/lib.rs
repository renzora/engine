//! Animation Editor — animation panel + timeline panel for the Renzora editor.
//!
//! Provides two panels:
//! - `animation` — clip library, state machine states/transitions, parameters, layers
//! - `timeline` — transport bar, time ruler, scrubber, track lanes, keyframe editing

mod inspector;
mod inspector_ui;
mod panel;
mod params;
mod state_machine;
mod rig;
mod stage;
mod studio_preview_panel;
mod timeline;
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
    /// Start of the preview loop region, seconds: the ruler's start handle.
    /// Looping playback wraps back here instead of to 0, so one stretch of a
    /// long clip can be watched on repeat. Editor-only (the clip's keys and its
    /// runtime playback are untouched); reset to 0 whenever the clip changes.
    pub loop_start: f32,
    /// End of the preview loop region, seconds: the right edge of the ruler's
    /// blue band. `None` means "the clip's end", so the default region follows
    /// the clip when its length changes instead of being left behind at the
    /// old length. Read both ends through [`AnimationEditorState::loop_range`].
    pub loop_end: Option<f32>,
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
    /// The clip each entity was last showing, restored when it is selected
    /// again. Session-only: a stale entry (despawned entity, deleted clip) is
    /// ignored when read.
    pub last_clip: std::collections::HashMap<Entity, String>,
}

impl Default for AnimationEditorState {
    fn default() -> Self {
        Self {
            selected_entity: None,
            scrub_time: 0.0,
            is_previewing: false,
            preview_speed: 1.0,
            preview_looping: true,
            loop_start: 0.0,
            loop_end: None,
            timeline_zoom: 100.0,
            timeline_scroll: 0.0,
            selected_clip: None,
            snap_enabled: true,
            snap_interval: 1.0 / 30.0,
            clip_duration: None,
            auto_fit_clip: None,
            track_height: timeline::ROW_DEFAULT,
            record_enabled: false,
            last_clip: Default::default(),
        }
    }
}

impl AnimationEditorState {
    /// The preview loop region `(start, end)` for a clip `duration` long,
    /// clamped so it always lies inside the clip and spans at least
    /// [`LOOP_MIN_SPAN`]. Clamped on every read rather than only when a handle
    /// moves, because the clip's own length can change under a stored region.
    pub(crate) fn loop_range(&self, duration: f32) -> (f32, f32) {
        loop_range(self.loop_start, self.loop_end, duration)
    }

    /// Forget the loop region: back to the whole clip.
    pub(crate) fn reset_loop(&mut self) {
        self.loop_start = 0.0;
        self.loop_end = None;
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
                editor_state.reset_loop();
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
                editor_state.timeline_zoom = z.clamp(timeline::ZOOM_MIN, timeline::ZOOM_MAX);
            }
            AnimEditorAction::SetTimelineScroll(s) => {
                editor_state.timeline_scroll = s.max(0.0);
            }
            AnimEditorAction::SetSnapEnabled(enabled) => {
                editor_state.snap_enabled = enabled;
            }
            AnimEditorAction::SetTrackHeight(h) => {
                editor_state.track_height = h.clamp(timeline::ROW_MIN, timeline::ROW_MAX);
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
    let prev = state.scrub_time;
    state.scrub_time += time.delta_secs() * speed;

    let Some(duration) = state.clip_duration.filter(|d| *d > 0.0) else { return };
    if state.preview_looping {
        let (start, end) = state.loop_range(duration);
        // Wrap at the loop end only when playback reaches it from inside the
        // region. Started from past it, play runs on to the clip's end first,
        // so parking the playhead after the region and pressing Play does not
        // snap it straight back.
        let limit = if prev <= end + 1e-4 { end } else { duration };
        if state.scrub_time >= limit {
            let span = (end - start).max(1e-3);
            state.scrub_time = start + (state.scrub_time - limit).rem_euclid(span);
        }
    } else if state.scrub_time >= duration {
        state.scrub_time = duration;
        state.is_previewing = false;
    }
}

/// The shortest loop region the handles may leave, seconds: about two frames
/// at 30 fps, so the region never collapses to nothing and wrapping always has
/// a span to wrap within.
pub(crate) const LOOP_MIN_SPAN: f32 = 1.0 / 15.0;

/// A stored loop region clamped into a clip `duration` long. `end: None` is
/// the clip's end. The end is held to at least [`LOOP_MIN_SPAN`] and the start
/// to at least that far before the end (both bounded by the clip itself, so a
/// clip shorter than the minimum loops whole).
pub(crate) fn loop_range(start: f32, end: Option<f32>, duration: f32) -> (f32, f32) {
    let dur = duration.max(0.0);
    let end = end.unwrap_or(dur).clamp(LOOP_MIN_SPAN.min(dur), dur);
    let start = start.clamp(0.0, (end - LOOP_MIN_SPAN).max(0.0));
    (start, end)
}

/// Cache the duration of the selected clip by reading the .anim file from disk.
fn cache_clip_duration(
    mut editor_state: ResMut<AnimationEditorState>,
    animators: Query<&renzora_animation::AnimatorComponent>,
    project: Option<Res<renzora::core::CurrentProject>>,
    timeline_clip: Option<Res<timeline::clip::TimelineClip>>,
) {
    // A particle effect shown with no animation clip: the playhead runs over
    // the effect's own length, which the timeline's stand-in clip carries
    // (and follows as the effect's duration changes, so this is not cached).
    if let Some(d) = timeline_clip.as_ref().and_then(|c| c.effect_only_duration()) {
        if editor_state.clip_duration != Some(d) {
            editor_state.clip_duration = Some(d);
        }
        return;
    }
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
    alive: Query<()>,
) {
    // Only a click on an entity changes what the panels show. Clearing the
    // selection (clicking empty viewport, Escape) used to empty the timeline
    // mid-edit, so a stray click lost the clip being worked on; now the last
    // entity stays until another is picked or it stops existing.
    let selected = match selection.get() {
        Some(raw) => Some(resolve_animation_entity(raw, &animators, &models, &parents)),
        None => editor_state.selected_entity.filter(|&e| alive.get(e).is_ok()),
    };
    if editor_state.selected_entity != selected {
        // Remember the clip the outgoing entity was showing, so coming back
        // to it restores that clip rather than snapping to its default.
        if let (Some(prev), Some(clip)) = (editor_state.selected_entity, editor_state.selected_clip.clone()) {
            editor_state.last_clip.insert(prev, clip);
        }
        editor_state.selected_entity = selected;
        editor_state.scrub_time = 0.0;
        editor_state.reset_loop();
        editor_state.is_previewing = false;
        editor_state.clip_duration = None;
        let animator = selected.and_then(|e| animators.get(e).ok());
        // The remembered clip, if the animator still has it; else the
        // animator's default clip (or its first), so the timeline doesn't
        // reset to "Select clip..." on every selection change.
        let remembered = selected
            .and_then(|e| editor_state.last_clip.get(&e).cloned())
            .filter(|name| animator.is_some_and(|a| a.clips.iter().any(|c| &c.name == name)));
        editor_state.selected_clip = remembered.or_else(|| {
            animator.and_then(|a| a.default_clip.clone().or_else(|| a.clips.first().map(|c| c.name.clone())))
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
            app.register_inspector(inspector::follow_node_inspector_entry());
        }
        inspector_ui::register(app);

        app.init_resource::<AnimationEditorState>();
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

        // The playhead's consumer on the viewport model, which is the only thing
        // that moves a skeleton while editing (the runtime ignores Play there).
        // It used to sit in the Studio Preview set below, so in a layout without
        // that panel the timeline's Play advanced the playhead and the model in
        // the viewport never moved. With no preview clone mounted, its preview
        // half finds nothing and costs a query.
        app.add_systems(
            Update,
            preview::update_animation_preview
                .after(advance_preview_time)
                .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                .run_if(renzora::not_in_play_mode),
        );

        // Heavy studio-preview work: only run when the Studio Preview panel or
        // the Animation workspace's stage (which is the same picture) is in
        // the dock tree. Other layouts (e.g. pure Scene) shouldn't pay for an
        // offscreen render + duplicate GLTF clone.
        app.add_systems(
            Update,
            (
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
        app.add_plugins(panel::AnimationPanel);
        app.add_plugins(timeline::TimelinePanel);
        app.add_plugins(params::AnimParamsPanel);
        app.add_plugins(state_machine::StateMachinePanel);
        app.add_plugins(studio_preview_panel::StudioPreviewPanel);
        app.add_plugins(rig::RigPlugin);
        app.add_plugins(stage::StagePlugin);
    }
}

renzora::add!(AnimationEditorPlugin, Editor);

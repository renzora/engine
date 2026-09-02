//! The animation timeline panel, built on the reusable
//! [`renzora_ember::widgets::timeline_view`] shell: a transport toolbar (scrub
//! transport, loop, clip selector, speed, snap, zoom), track headers (a bone
//! name with its T/R/S channel indicators, or a property track's binding) and
//! keyframe lanes over the shared ruler / playhead / scrub canvas.
//!
//! The selected clip's `.anim` data is loaded from disk into [`clip::TimelineClip`]
//! whenever the `(entity, clip)` selection changes; that cache is also the **edit
//! buffer** — every keyframe drag, delete and capture mutates it and marks it
//! dirty, and Save (or the autosave) flushes it back.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`clip`] | The loaded clip, the lane/selection vocabulary, load + save |
//! | [`undo`] | The snapshot observer that makes every clip edit undoable |
//! | [`build`] | The panel's widget tree and its transport toolbar |
//! | [`snapshots`] | The keyed lists: track headers, keyframes, markers |
//! | [`edit`] | Picking, dragging and the right-click menus |
//! | [`props`] | Property tracks: the deferred world ops behind them |
//! | [`preview`] | Scrub preview, live-edit and record capture |

use bevy::prelude::*;

use renzora_editor_framework::SplashState;
use renzora_ember::panel::RegisterPanelContent;

pub(crate) mod build;
pub(crate) mod clip;
pub(crate) mod edit;
pub(crate) mod preview;
pub(crate) mod props;
pub(crate) mod snapshots;
pub(crate) mod undo;

pub(super) const TRANSLATION: (u8, u8, u8) = (100, 149, 237);
pub(super) const ROTATION: (u8, u8, u8) = (120, 200, 120);
pub(super) const SCALE: (u8, u8, u8) = (200, 120, 120);
/// Color for property-animation lanes (distinct from bone T/R/S channels).
pub(super) const PROPERTY: (u8, u8, u8) = (230, 190, 90);
/// Color for event-marker flags.
pub(super) const MARKER: (u8, u8, u8) = (200, 140, 220);
pub(super) const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];
/// Index of `1.00x` in [`SPEEDS`] — what the dropdown falls back to when the
/// live `preview_speed` isn't one of the presets (a speed set from a script, say).
pub(super) const DEFAULT_SPEED: usize = 2;

pub struct TimelinePanel;

impl Plugin for TimelinePanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<clip::TimelineClip>();
        app.init_resource::<edit::KeyDragState>();
        app.init_resource::<props::TimelineOps>();
        app.init_resource::<preview::RecordState>();
        app.init_resource::<clip::SelectedKey>();
        app.init_resource::<preview::PreviewApplied>();
        app.init_resource::<clip::AutoSaveTimer>();
        // Bridge to the inspector's per-property keyframe buttons. `init_resource`
        // is idempotent — the inspector inits these too, so they exist whichever
        // crate loads first.
        app.init_resource::<renzora::ActiveTimeline>();
        app.init_resource::<renzora::KeyframeRequests>();
        // NOT migrated to `PanelScope::systems` (visibility gating), deliberately.
        // The other four animation panels were; this one must not be, for three
        // reasons that are easy to miss:
        //
        //  * `publish_active_timeline` feeds the `ActiveTimeline` resource that the
        //    *inspector* reads to decide whether to show its per-field add-keyframe
        //    button. Gating it would make those buttons go stale whenever the
        //    timeline tab wasn't the active one.
        //  * `apply_keyframe_requests` applies `KeyframeRequests` queued by that
        //    same inspector button. Gated, pressing add-keyframe with the timeline
        //    hidden would silently do nothing.
        //  * `auto_save_clip` is autosave — pausing it risks losing edits.
        //
        // Splitting the block is not a free fix either: it is `.chain()`ed with a
        // documented ordering requirement ("key_drag must run before anim_sync"),
        // so separating the gated and ungated halves would have to preserve that.
        // Worth doing carefully one day; not worth doing as part of a sweep.
        app.register_panel_content("timeline", false, build::build);
        // panel-systems-ungated: publishes ActiveTimeline (read by the inspector's add-keyframe buttons), applies KeyframeRequests queued by those buttons, and runs autosave
        app.add_systems(
            Update,
            (
                clip::cache_clip,
                props::publish_active_timeline,
                edit::anim_btn_click,
                edit::clip_combo_open,
                // key_drag must run before anim_sync so a freshly-started key
                // drag suppresses the scrub layer the same frame.
                edit::key_drag,
                edit::anim_sync,
                edit::update_anim_play_icon,
                edit::key_context_menu,
                clip::save_clip_click,
                edit::add_marker_click,
                edit::new_clip_click,
                clip::auto_save_clip,
                edit::timeline_wheel_zoom,
                edit::timeline_shortcuts,
                edit::timeline_delete_guard,
                props::prop_header_click,
                props::apply_timeline_ops,
                props::apply_keyframe_requests,
            )
                .chain()
                .run_if(in_state(SplashState::Editor)),
        );

        // Property scrub-preview + record-capture. Editor-only (not in play
        // mode — there the runtime property sampler drives the entity instead).
        // panel-systems-ungated: property scrub-preview drives the VIEWPORT, not this panel
        app.add_systems(
            Update,
            (
                preview::preview_property_animation,
                preview::record_capture,
                preview::live_edit_selected_key,
            )
                .chain()
                .after(props::apply_timeline_ops)
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora::not_in_play_mode),
        );

        app.init_resource::<undo::AnimUndoShadow>();
        // panel-systems-ungated: undo must work from anywhere, not only with the timeline focused
        app.add_systems(
            Update,
            undo::anim_undo_observer
                .after(props::apply_timeline_ops)
                .after(props::apply_keyframe_requests)
                .run_if(in_state(SplashState::Editor))
                .run_if(|c: Option<Res<clip::TimelineClip>>| c.is_some_and(|c| c.clip.is_some())),
        );
    }
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy)]
pub(super) enum AnimBtn {
    SkipBack,
    StepBack,
    PlayPause,
    Stop,
    StepForward,
    SkipForward,
    Loop,
    Snap,
    ZoomIn,
    ZoomOut,
    Record,
    AddProperty,
    AddKey,
}

/// Marker on a property-track header's property dropdown (carries track index).
#[derive(Component)]
pub(super) struct PropTrackCombo(pub(super) usize);
/// Marker on a property-track header's delete button (carries track index).
#[derive(Component)]
pub(super) struct DeletePropTrack(pub(super) usize);
/// Marker on a property-track header's "add key" button (carries track index).
#[derive(Component)]
pub(super) struct AddKeyTrackBtn(pub(super) usize);
/// Marker on the "add track" button in the track-header column corner.
#[derive(Component)]
pub(super) struct AddTrackBtn;
/// Marker on the toolbar "add marker" button.
#[derive(Component)]
pub(super) struct AddMarkerBtn;
/// Marker on the toolbar marker-name text field.
#[derive(Component)]
pub(super) struct MarkerNameField;
#[derive(Component)]
pub(super) struct ClipCombo;
/// The "+" beside the clip selector: creates a new clip named from
/// [`NewClipNameField`] on the selected entity's animator. This is the only way
/// to author a *second* clip on an entity — the empty-state "Create Animation"
/// button hides itself once one clip exists — which directional sprites need
/// (one clip per facing).
#[derive(Component)]
pub(super) struct NewClipBtn;
/// Text field holding the name for the next clip created via [`NewClipBtn`].
#[derive(Component)]
pub(super) struct NewClipNameField;
#[derive(Component)]
pub(super) struct AnimPlayIcon;
#[derive(Component)]
pub(super) struct AnimTimeline;
#[derive(Component)]
pub(super) struct SaveClipBtn;
/// Marker + cursor tracking on the timeline's absolute clips layer. Keyframe
/// picking is done by math against the clip data (cursor → time/track), NOT
/// via per-diamond `Interaction` — the widget's scrub overlay sits above the
/// clips layer and would swallow per-node hits.
#[derive(Component)]
pub(super) struct KeyLane;

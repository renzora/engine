//! The animation timeline panel, built on the reusable
//! [`renzora_ember::widgets::timeline_view`] shell: one toolbar row (clip
//! picker, record, transport, time readout, speed, snap, Keys / Curves), a
//! nested track list with a filter, and keyframe lanes over the shared ruler /
//! playhead / scrub canvas.
//!
//! The selected clip's `.anim` data is loaded from disk into [`clip::TimelineClip`]
//! whenever the `(entity, clip)` selection changes; that cache is also the **edit
//! buffer**: every keyframe drag, delete and capture mutates it and marks it
//! dirty, and Save (or the autosave) flushes it back.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`clip`] | The loaded clip, the lane/selection vocabulary, load + save |
//! | [`layout`] | The visible rows: nesting, collapse, filter, vertical scroll |
//! | [`runs`] | Each lane's keys grouped into runs, rebuilt per edit |
//! | [`undo`] | The snapshot observer that makes every clip edit undoable |
//! | [`build`] | The panel's widget tree and its toolbar |
//! | [`snapshots`] | The keyed lists: track rows, lanes, ruler overlay, markers |
//! | [`edit`] | Picking, dragging, box-select, scrolling and shortcuts |
//! | [`scrollbars`] | The time and row scrollbars |
//! | [`menus`] | The clip picker, overflow and empty-state actions |
//! | [`popover`] | Click a key: its time, value, easing, copy and delete |
//! | [`curves`] | The Curves view of the selected track: geometry, drawing, toolbar |
//! | [`graph`] | The Curves view's pointer: keys, tangent handles, pan, zoom, frame |
//! | [`keymode`] | Bone key interpolation (Auto, Linear, Constant, ...) and its menus |
//! | [`props`] | Property tracks: the deferred world ops behind them |
//! | [`pose`] / [`morph`] | Bones and blend shapes posed by hand, and keying them |
//! | [`fx`] | The Particles group: an effect's keyed parameters, and its clock |
//! | [`preview`] | Scrub preview, live-edit and record capture |

use bevy::prelude::*;

use renzora_editor_framework::SplashState;
use renzora_ember::panel::RegisterPanelContent;

pub(crate) mod build;
pub(crate) mod clip;
pub(crate) mod curves;
pub(crate) mod edit;
pub(crate) mod fx;
pub(crate) mod graph;
pub(crate) mod keymode;
pub(crate) mod layout;
pub(crate) mod menus;
pub(crate) mod morph;
pub(crate) mod popover;
pub(crate) mod pose;
pub(crate) mod preview;
pub(crate) mod props;
pub(crate) mod runs;
pub(crate) mod scrollbars;
pub(crate) mod snapshots;
pub(crate) mod undo;

/// Zoom limits, px per second. The top end is deep enough that a single frame
/// at 30 fps spans well over a hundred pixels, for spacing keys by hand.
pub(crate) const ZOOM_MIN: f32 = 20.0;
pub(crate) const ZOOM_MAX: f32 = 4000.0;
/// Track row height limits and default, px.
pub(crate) const ROW_MIN: f32 = 18.0;
pub(crate) const ROW_MAX: f32 = 64.0;
pub(crate) const ROW_DEFAULT: f32 = 26.0;
/// The ⋯ menu's row height presets: compact, normal, roomy.
pub(super) const ROW_PRESETS: [f32; 3] = [20.0, ROW_DEFAULT, 36.0];

pub(super) const TRANSLATION: (u8, u8, u8) = (248, 113, 113);
pub(super) const ROTATION: (u8, u8, u8) = (110, 231, 160);
pub(super) const SCALE: (u8, u8, u8) = (96, 165, 250);
/// Color for property-animation lanes (distinct from bone T/R/S channels).
pub(super) const PROPERTY: (u8, u8, u8) = (230, 190, 90);
/// Color for blend shape (morph target) lanes.
pub(super) const MORPH: (u8, u8, u8) = (244, 140, 186);
/// Color for event-marker flags.
pub(super) const MARKER: (u8, u8, u8) = (200, 140, 220);
/// Color for the Particles group: the orange of the viewport's particle
/// sparkle, so an effect's rows read as the effect.
pub(super) const PARTICLE: (u8, u8, u8) = (251, 146, 60);
/// Selected keys: amber, with a ring.
pub(super) const SELECTED: (u8, u8, u8) = (255, 210, 120);
pub(super) const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];
/// Index of `1.0x` in [`SPEEDS`]: what the dropdown falls back to when the
/// live `preview_speed` isn't one of the presets (a speed set from a script, say).
pub(super) const DEFAULT_SPEED: usize = 2;

pub struct TimelinePanel;

impl Plugin for TimelinePanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<clip::TimelineClip>();
        app.init_resource::<edit::KeyDragState>();
        app.init_resource::<scrollbars::ScrollbarGrab>();
        app.init_resource::<props::TimelineOps>();
        app.init_resource::<preview::RecordState>();
        app.init_resource::<clip::SelectedKey>();
        app.init_resource::<clip::SelectedKeys>();
        app.init_resource::<preview::PreviewApplied>();
        app.init_resource::<clip::AutoSaveTimer>();
        app.init_resource::<layout::TrackLayout>();
        app.init_resource::<runs::KeyRuns>();
        app.init_resource::<curves::TimelineMode>();
        app.init_resource::<curves::CurveView>();
        app.init_resource::<curves::CurveSel>();
        app.init_resource::<graph::GraphDrag>();
        app.init_resource::<popover::PopoverState>();
        app.init_resource::<popover::KeyClipboard>();
        app.init_resource::<fx::FxTarget>();
        app.init_resource::<pose::PoseLayer>();
        // The hand-made pose over the clip's: captured before the player
        // writes the clip's pose, restored after it, before transforms
        // propagate. See `pose`.
        // panel-systems-ungated: posing happens in the VIEWPORT, not this panel
        app.add_systems(
            PostUpdate,
            (
                pose::capture_pose.before(bevy::app::AnimationSystems),
                pose::apply_pose
                    .after(bevy::app::AnimationSystems)
                    .before(bevy::transform::TransformSystems::Propagate),
            )
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora::not_in_play_mode),
        );
        // Hand-set blend shape weights, put back over the clip's the same way.
        // panel-systems-ungated: the Face panel sets them, the viewport shows them
        app.init_resource::<morph::MorphLayer>();
        app.add_systems(
            PostUpdate,
            morph::apply_morph_pose
                .after(bevy::app::AnimationSystems)
                .before(bevy::transform::TransformSystems::Propagate)
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora::not_in_play_mode),
        );
        // panel-systems-ungated: K and record keying work from the viewport
        app.add_systems(
            Update,
            (
                pose::viewport_key_shortcut.before(props::apply_timeline_ops),
                pose::auto_key_pose.after(props::apply_timeline_ops),
            )
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora::not_in_play_mode),
        );
        // The effect editor's crate inits this too; idempotent either way.
        app.init_resource::<renzora_particles_editor::timeline::TimelineLink>();
        // Bridge to the inspector's per-property keyframe buttons. `init_resource`
        // is idempotent: the inspector inits these too, so they exist whichever
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
        //  * `auto_save_clip` is autosave: pausing it risks losing edits.
        //
        // Splitting the block is not a free fix either: it is `.chain()`ed with a
        // documented ordering requirement ("key_drag must run before ruler_drag"),
        // so separating the gated and ungated halves would have to preserve that.
        app.register_panel_content("timeline", false, build::build);
        // panel-systems-ungated: publishes ActiveTimeline (read by the inspector's add-keyframe buttons), applies KeyframeRequests queued by those buttons, and runs autosave
        app.add_systems(
            Update,
            (
                (
                    // Which effect, then which clip (an effect alone gets a
                    // stand-in clip), then the effect's copy for that clip.
                    (fx::resolve_target, clip::cache_clip, fx::sync_fx).chain(),
                    layout::rebuild_layout,
                    edit::auto_fit_clip,
                    runs::rebuild_runs,
                    props::publish_active_timeline,
                    edit::anim_btn_click,
                    menus::clip_picker_open,
                    menus::overflow_open,
                    menus::empty_card_click,
                    edit::track_row_click,
                    (curves::mode_click, graph::graph_buttons, graph::graph_sync).chain(),
                    curves::interp_click,
                    // The scrollbars claim their press before anything else;
                    // key_drag runs before ruler_drag because a press on the
                    // playhead needle in the lanes hands its scrub to
                    // ruler_drag, which carries it on the same frame.
                    scrollbars::scrollbar_drag,
                    // The Curves view's own drag right after the dopesheet's,
                    // which stands aside while it shows.
                    (edit::key_drag, graph::graph_input).chain(),
                    edit::ruler_drag,
                    // Before key_context_menu (next block): it decides whether a
                    // right release was a pan or a click.
                    (edit::timeline_pan, graph::graph_pan).chain(),
                    edit::anim_sync,
                    edit::sync_header_offset,
                    edit::sync_box_rect,
                    scrollbars::scrollbar_layout,
                )
                    .chain(),
                (
                    edit::update_anim_play_icon,
                    (edit::key_context_menu, graph::graph_context_menu).chain(),
                    edit::track_context_menu,
                    edit::timeline_cursor,
                    clip::save_clip_click,
                    edit::add_marker_click,
                    edit::new_clip_click,
                    menus::rename_clip_click,
                    clip::auto_save_clip,
                    (edit::timeline_wheel, graph::graph_wheel_read, graph::graph_wheel).chain(),
                    menus::row_height_click,
                    edit::timeline_shortcuts,
                    edit::timeline_delete_guard,
                    popover::open_requested_popover,
                    popover::popover_escape,
                    curves::curves_hover,
                    props::prop_header_click,
                    fx::fx_header_click,
                    props::apply_timeline_ops,
                    props::apply_keyframe_requests,
                )
                    .chain(),
            )
                .chain()
                .run_if(in_state(SplashState::Editor)),
        );

        // Property scrub-preview + record-capture. Editor-only (not in play
        // mode: there the runtime property sampler drives the entity instead).
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

        // The playhead drives the shown effect. After the playhead moves, so
        // the effect is seeked to this frame's time, not last frame's.
        // panel-systems-ungated: drives the effect in the VIEWPORT, not this panel
        app.add_systems(
            Update,
            fx::drive_fx_clock
                .after(crate::advance_preview_time)
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
    PrevKey,
    PlayPause,
    NextKey,
    SkipForward,
    Loop,
    Snap,
    ZoomIn,
    ZoomOut,
    FrameAll,
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
/// Marker on the "+ Track" button at the top of the track list.
#[derive(Component)]
pub(super) struct AddTrackBtn;
/// Marker on the overflow menu's "add event" button.
#[derive(Component)]
pub(super) struct AddMarkerBtn;
/// Marker on the overflow menu's event-name text field.
#[derive(Component)]
pub(super) struct MarkerNameField;
/// The "entity / clip ▾" picker at the head of the toolbar.
#[derive(Component)]
pub(super) struct ClipCombo;
/// Creates a new clip on the selected entity's animator: named from a
/// [`NewClipNameField`] when the "New clip" card is open, else an unused
/// `clip`, `clip_2`, ... name. The "+" beside the clip picker carries it too.
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
/// Marker + cursor tracking on the timeline's lanes layer. Keyframe picking is
/// done by math against the clip data (cursor → time/row), NOT via per-diamond
/// `UiInteraction`: the widget's scrub overlay sits above the lanes and would
/// swallow per-node hits.
#[derive(Component)]
pub(super) struct KeyLane;
/// Cursor tracking over the whole pane (ruler included), for the ruler's
/// handles and its scrub.
#[derive(Component)]
pub(super) struct PaneLayer;
/// The track list's filter box.
#[derive(Component)]
pub(super) struct TrackFilterField;
/// A row in the track list: click to select it.
#[derive(Component)]
pub(super) struct TrackRowBtn(pub(super) layout::RowKind);
/// A bone row's caret: click to fold its children away.
#[derive(Component)]
pub(super) struct CollapseBtn(pub(super) usize);
/// The column the track rows are built into (offset by the vertical scroll).
#[derive(Component)]
pub(super) struct HeaderListOffset;
/// The box-select rectangle, shown while dragging on empty lane space.
#[derive(Component)]
pub(super) struct BoxRect;
/// The "⋯" overflow button at the right of the toolbar.
#[derive(Component)]
pub(super) struct OverflowBtn;
/// A row height preset in the "⋯" menu (its height, px).
#[derive(Component)]
pub(super) struct RowHeightBtn(pub(super) f32);

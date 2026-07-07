//! Bevy-native (ember) port of the egui animation `TimelinePanel`, built on the
//! reusable [`renzora_ember::widgets::timeline_view`] shell: a transport toolbar
//! (scrub transport, loop, clip selector, speed presets, snap, zoom), track
//! headers (bone name + T/R/S channel indicators) and keyframe-diamond lanes
//! over the shared ruler / playhead / scrub canvas.
//!
//! The selected clip's `.anim` data is loaded from disk into [`NativeAnimClip`]
//! whenever the `(entity, clip)` selection changes; the header + keyframe
//! snapshots read that cache. Clip drag/resize, the DAW mini-map overview and
//! wheel zoom/pan are deferred (the toolbar zoom buttons + scrub work).

use std::hash::{Hash, Hasher};

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, UiTransform};

use renzora::reflection::list_animatable_fields;
use renzora::{AnimMarker, PropertyKey, PropertyTrack, TrackValue};
use renzora_animation::property_playback::{apply_property_tracks, read_track_value};
use renzora_animation::{AnimClip, AnimatorComponent};
use renzora_editor_framework::{EditorCommands, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, menu_item, screen_menu, text_input, timeline_view, DragRange, EmberTextInput, TimelineView, LANE_INSET};

use crate::{AnimEditorAction, AnimEditorBridge, AnimationEditorState};

const TRANSLATION: (u8, u8, u8) = (100, 149, 237);
const ROTATION: (u8, u8, u8) = (120, 200, 120);
const SCALE: (u8, u8, u8) = (200, 120, 120);
/// Color for property-animation lanes (distinct from bone T/R/S channels).
const PROPERTY: (u8, u8, u8) = (230, 190, 90);
const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

pub struct NativeAnimTimeline;

impl Plugin for NativeAnimTimeline {
    fn build(&self, app: &mut App) {
        app.init_resource::<NativeAnimClip>();
        app.init_resource::<KeyDragState>();
        app.init_resource::<TimelineOps>();
        app.init_resource::<RecordState>();
        app.init_resource::<SelectedKey>();
        app.init_resource::<PreviewApplied>();
        app.init_resource::<AutoSaveTimer>();
        // Bridge to the inspector's per-property keyframe buttons. `init_resource`
        // is idempotent — the inspector inits these too, so they exist whichever
        // crate loads first.
        app.init_resource::<renzora::ActiveTimeline>();
        app.init_resource::<renzora::KeyframeRequests>();
        app.register_panel_content("timeline", false, build);
        app.add_systems(
            Update,
            (
                cache_native_clip,
                publish_active_timeline,
                anim_btn_click,
                speed_btn_click,
                clip_combo_open,
                // key_drag must run before anim_sync so a freshly-started key
                // drag suppresses the scrub layer the same frame.
                key_drag,
                anim_sync,
                update_anim_play_icon,
                key_context_menu,
                save_clip_click,
                add_marker_click,
                new_clip_click,
                auto_save_clip,
                timeline_wheel_zoom,
                timeline_shortcuts,
                timeline_delete_guard,
                prop_header_click,
                apply_timeline_ops,
                apply_keyframe_requests,
            )
                .chain()
                .run_if(in_state(SplashState::Editor)),
        );

        // Property scrub-preview + record-capture. Editor-only (not in play
        // mode — there the runtime property sampler drives the entity instead).
        app.add_systems(
            Update,
            (preview_property_animation, record_capture, live_edit_selected_key)
                .chain()
                .after(apply_timeline_ops)
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora::not_in_play_mode),
        );

        app.init_resource::<AnimUndoShadow>();
        app.add_systems(
            Update,
            anim_undo_observer
                .after(apply_timeline_ops)
                .after(apply_keyframe_requests)
                .run_if(in_state(SplashState::Editor))
                .run_if(|c: Option<Res<NativeAnimClip>>| c.is_some_and(|c| c.clip.is_some())),
        );
    }
}

// ── Undo integration ─────────────────────────────────────────────────────────
//
// Keyframe edits mutate the in-memory clip buffer (`NativeAnimClip.clip`). A
// change-observer records a coarse snapshot of that buffer whenever it changes,
// covering drags, deletes, interp changes, live-record capture and marker edits
// from one place. Full RON serialization is the change signal so no field is
// missed. The clip is scene-attached content, so edits land on the active
// (Scene) stack; per-frame drag spam collapses via the merge key, and the
// global gesture seal splits gestures.

/// Shadow of the clip the observer last saw, its serialized form (the diff key),
/// and the `(entity, clip)` selection it belongs to. Changing selection reseeds.
#[derive(Resource, Default)]
struct AnimUndoShadow {
    key: Option<(Entity, String)>,
    serialized: Option<String>,
    clip: Option<AnimClip>,
}

/// Restore a snapshotted clip — the `restore` fn for the animation `SnapshotCmd`.
/// Writes the buffer and marks it dirty; the timeline + dopesheet rebuild
/// reactively from the buffer, and Save flushes it to disk.
fn restore_anim_clip(world: &mut World, clip: &AnimClip) {
    if let Some(mut c) = world.get_resource_mut::<NativeAnimClip>() {
        c.clip = Some(clip.clone());
        c.dirty = true;
    }
    if let Some(mut sh) = world.get_resource_mut::<AnimUndoShadow>() {
        sh.serialized = ron::to_string(clip).ok();
        sh.clip = Some(clip.clone());
    }
}

fn anim_undo_observer(world: &mut World) {
    let (cur, key) = {
        let Some(c) = world.get_resource::<NativeAnimClip>() else {
            return;
        };
        let Some(clip) = c.clip.clone() else {
            return;
        };
        (clip, c.key.clone())
    };
    let serialized = match ron::to_string(&cur) {
        Ok(s) => s,
        Err(_) => return,
    };
    let (prev_key, prev_serialized, prev_clip) = {
        let sh = world.resource::<AnimUndoShadow>();
        (sh.key.clone(), sh.serialized.clone(), sh.clip.clone())
    };
    if prev_key != key || prev_clip.is_none() {
        let mut sh = world.resource_mut::<AnimUndoShadow>();
        sh.key = key;
        sh.serialized = Some(serialized);
        sh.clip = Some(cur);
        return;
    }
    if prev_serialized.as_deref() == Some(serialized.as_str()) {
        return;
    }
    let before = prev_clip.unwrap();
    let ctx = renzora_undo::active_context(world);
    renzora_undo::record(
        world,
        ctx,
        Box::new(renzora_undo::SnapshotCmd {
            label: "Animation".to_string(),
            before,
            after: cur.clone(),
            restore: restore_anim_clip,
            merge_key: Some("anim-clip".to_string()),
        }),
    );
    let mut sh = world.resource_mut::<AnimUndoShadow>();
    sh.serialized = Some(serialized);
    sh.clip = Some(cur);
}

/// Disk-loaded copy of the currently selected clip, reloaded when the
/// `(entity, clip)` selection changes. Drives the header + keyframe snapshots,
/// and doubles as the edit buffer: keyframe drags/deletes mutate `clip` in
/// place and set `dirty` until the Save button flushes it back to `path`.
#[derive(Resource, Default)]
struct NativeAnimClip {
    key: Option<(Entity, String)>,
    clip: Option<AnimClip>,
    /// Absolute path of the loaded `.anim` (save target).
    path: Option<std::path::PathBuf>,
    /// Unsaved keyframe edits pending.
    dirty: bool,
}

impl NativeAnimClip {
    /// Mutable access to one lane's `(time, …)` key vector, erased over the
    /// channel value types via the times-only view the editor needs. A lane is
    /// either a skeletal bone channel (T/R/S) or a single property track.
    fn lane_times(&mut self, lane: Lane) -> Option<ChannelTimes<'_>> {
        let clip = self.clip.as_mut()?;
        match lane {
            Lane::Bone { track, channel } => {
                let track = clip.tracks.get_mut(track)?;
                Some(match channel {
                    0 => ChannelTimes::T(&mut track.translations),
                    1 => ChannelTimes::R(&mut track.rotations),
                    _ => ChannelTimes::S(&mut track.scales),
                })
            }
            Lane::Prop { track } => {
                let track = clip.property_tracks.get_mut(track)?;
                Some(ChannelTimes::P(&mut track.keys))
            }
        }
    }
}

/// Identifies an editable lane: a skeletal bone channel or a property track.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lane {
    Bone { track: usize, channel: u8 },
    Prop { track: usize },
}

/// The currently selected keyframe (lane + index), highlighted on the dopesheet
/// with its value shown in the toolbar. Cleared when the clip selection changes.
#[derive(Resource, Default)]
struct SelectedKey(Option<SelKey>);

#[derive(Clone, Copy, PartialEq, Eq)]
struct SelKey {
    lane: Lane,
    index: usize,
}

/// Borrowed view over a single keyframe lane — lets the drag/delete systems
/// edit times without caring about the per-channel value payload type.
enum ChannelTimes<'a> {
    T(&'a mut Vec<(f32, [f32; 3])>),
    R(&'a mut Vec<(f32, [f32; 4])>),
    S(&'a mut Vec<(f32, [f32; 3])>),
    P(&'a mut Vec<PropertyKey>),
}

impl ChannelTimes<'_> {
    fn time(&self, idx: usize) -> Option<f32> {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => v.get(idx).map(|k| k.0),
            ChannelTimes::R(v) => v.get(idx).map(|k| k.0),
            ChannelTimes::P(v) => v.get(idx).map(|k| k.time),
        }
    }
    fn set_time(&mut self, idx: usize, t: f32) {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => {
                if let Some(k) = v.get_mut(idx) {
                    k.0 = t;
                }
            }
            ChannelTimes::R(v) => {
                if let Some(k) = v.get_mut(idx) {
                    k.0 = t;
                }
            }
            ChannelTimes::P(v) => {
                if let Some(k) = v.get_mut(idx) {
                    k.time = t;
                }
            }
        }
    }
    fn remove(&mut self, idx: usize) {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => {
                if idx < v.len() {
                    v.remove(idx);
                }
            }
            ChannelTimes::R(v) => {
                if idx < v.len() {
                    v.remove(idx);
                }
            }
            ChannelTimes::P(v) => {
                if idx < v.len() {
                    v.remove(idx);
                }
            }
        }
    }
    fn sort(&mut self) {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => {
                v.sort_by(|a, b| a.0.total_cmp(&b.0));
            }
            ChannelTimes::R(v) => v.sort_by(|a, b| a.0.total_cmp(&b.0)),
            ChannelTimes::P(v) => v.sort_by(|a, b| a.time.total_cmp(&b.time)),
        }
    }
}

/// In-flight keyframe drag. Survives the keyed-list rebuilding the dragged
/// node mid-drag (the rebuild drops `Interaction` state, so the drag is
/// tracked against the raw mouse button instead).
#[derive(Resource, Default)]
struct KeyDragState {
    active: Option<KeyDrag>,
}

struct KeyDrag {
    lane: Lane,
    index: usize,
    start_cursor_x: f32,
    orig_time: f32,
    moved: bool,
}

#[derive(Component, Clone, Copy)]
enum AnimBtn {
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

/// Deferred world ops requested by toolbar buttons / track headers that need
/// `&World` access (reflection enumeration / live-value reads), applied by an
/// exclusive system.
#[derive(Resource, Default)]
struct TimelineOps {
    /// Append a new empty property track (the user picks its property after).
    add_empty_track: bool,
    /// Insert a key at the playhead on every property track.
    add_key: bool,
    /// Open the per-track property picker: (track index, menu screen position).
    open_property_menu: Option<(usize, Vec2)>,
    /// Delete the property track at this index.
    delete_track: Option<usize>,
    /// Insert a key at the playhead on just this one property track.
    add_key_track: Option<usize>,
    /// Delete the currently selected keyframe.
    delete_selected_key: bool,
}

/// Marker on a property-track header's property dropdown (carries track index).
#[derive(Component)]
struct PropTrackCombo(usize);
/// Marker on a property-track header's delete button (carries track index).
#[derive(Component)]
struct DeletePropTrack(usize);
/// Marker on a property-track header's "add key" button (carries track index).
#[derive(Component)]
struct AddKeyTrackBtn(usize);
/// Marker on the "add track" button in the track-header column corner.
#[derive(Component)]
struct AddTrackBtn;
/// Marker on the toolbar "add marker" button.
#[derive(Component)]
struct AddMarkerBtn;
/// Marker on the toolbar marker-name text field.
#[derive(Component)]
struct MarkerNameField;
#[derive(Component)]
struct SpeedBtn(f32);
#[derive(Component)]
struct ClipCombo;
/// The "+" beside the clip selector: creates a new clip named from
/// [`NewClipNameField`] on the selected entity's animator. This is the only way
/// to author a *second* clip on an entity — the empty-state "Create Animation"
/// button hides itself once one clip exists — which directional sprites need
/// (one clip per facing).
#[derive(Component)]
struct NewClipBtn;
/// Text field holding the name for the next clip created via [`NewClipBtn`].
#[derive(Component)]
struct NewClipNameField;
#[derive(Component)]
struct AnimPlayIcon;
#[derive(Component)]
struct AnimTimeline;
#[derive(Component)]
struct SaveClipBtn;
/// Marker + cursor tracking on the timeline's absolute clips layer. Keyframe
/// picking is done by math against the clip data (cursor → time/track), NOT
/// via per-diamond `Interaction` — the widget's scrub overlay sits above the
/// clips layer and would swallow per-node hits.
#[derive(Component)]
struct KeyLane;

// ── Accessors ──────────────────────────────────────────────────────────────────

fn state(w: &World) -> Option<&AnimationEditorState> {
    w.get_resource::<AnimationEditorState>()
}
fn cur_clip(w: &World) -> Option<&AnimClip> {
    w.get_resource::<NativeAnimClip>().and_then(|c| c.clip.as_ref())
}

/// The entity the currently-loaded clip belongs to (the cache's key entity).
/// Apply/capture must target THIS, not the bare `selected_entity` — on a
/// selection change the two differ for a frame, which would otherwise write the
/// old clip's pose onto the newly-selected entity.
fn clip_entity(w: &World) -> Option<Entity> {
    w.get_resource::<NativeAnimClip>()
        .and_then(|c| c.key.as_ref().map(|(e, _)| *e))
}

/// Whether the timeline has a clip to show (vs an empty-state message).
fn ready(w: &World) -> bool {
    cur_clip(w).is_some()
}

fn empty_msg(w: &World) -> String {
    let Some(s) = state(w) else { return String::new() };
    if s.selected_entity.is_none() {
        renzora::lang::t("animation.select_entity_to_animate")
    } else if s
        .selected_entity
        .and_then(|e| w.get::<AnimatorComponent>(e))
        .is_none_or(|a| a.clips.is_empty())
    {
        renzora::lang::t("animation.no_animation_create_below")
    } else if s.selected_clip.is_none() {
        renzora::lang::t("animation.choose_clip_above")
    } else {
        renzora::lang::t("animation.loading_clip")
    }
}

/// Toolbar readout for the selected keyframe: "Rotation @ 1.33s = (…)". Empty
/// when nothing is selected. Rotation values are shown as Euler degrees.
fn selected_key_label(w: &World) -> String {
    let Some(sel) = w.get_resource::<SelectedKey>().and_then(|s| s.0) else { return String::new() };
    let Some(clip) = cur_clip(w) else { return String::new() };
    match sel.lane {
        Lane::Prop { track } => {
            let Some(pt) = clip.property_tracks.get(track) else { return String::new() };
            let Some(key) = pt.keys.get(sel.index) else { return String::new() };
            format!("{} @ {:.2}s = {}", title_case(&pt.field), key.time, fmt_track_value(&key.value))
        }
        Lane::Bone { track, channel } => {
            let Some(bt) = clip.tracks.get(track) else { return String::new() };
            let parts = match channel {
                0 => bt.translations.get(sel.index).map(|(t, v)| ("Translation", *t, fmt_vec3(v))),
                1 => bt.rotations.get(sel.index).map(|(t, v)| ("Rotation", *t, fmt_quat(v))),
                _ => bt.scales.get(sel.index).map(|(t, v)| ("Scale", *t, fmt_vec3(v))),
            };
            match parts {
                Some((label, time, val)) => format!("{} @ {:.2}s = {}", label, time, val),
                None => String::new(),
            }
        }
    }
}

fn fmt_vec3(v: &[f32; 3]) -> String {
    format!("({:.2}, {:.2}, {:.2})", v[0], v[1], v[2])
}

fn fmt_quat(v: &[f32; 4]) -> String {
    let (x, y, z) = bevy::prelude::Quat::from_array(*v).to_euler(bevy::math::EulerRot::XYZ);
    format!("({:.0}°, {:.0}°, {:.0}°)", x.to_degrees(), y.to_degrees(), z.to_degrees())
}

fn fmt_track_value(v: &TrackValue) -> String {
    match v {
        TrackValue::Float(x) => format!("{:.3}", x),
        TrackValue::Vec3(a) => fmt_vec3(a),
        TrackValue::Quat(a) => fmt_quat(a),
        TrackValue::Color(a) => format!("({:.2}, {:.2}, {:.2}, {:.2})", a[0], a[1], a[2], a[3]),
        TrackValue::Bool(b) => b.to_string(),
    }
}

// ── Build ──────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(rgb(panel_bg())),
            Name::new("native-anim-timeline"),
        ))
        .id();

    let toolbar = build_toolbar(commands, fonts);

    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
        .id();

    // Empty-state note + a Create-Animation action right here in the timeline
    // (so the user doesn't have to go hunt for the Animation panel).
    let note = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, row_gap: Val::Px(10.0), ..default() })
        .id();
    let note_lbl = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::justify(bevy::text::Justify::Center)))
        .id();
    bind_text(commands, note_lbl, empty_msg);
    let create_btn = crate::setup::action_button(commands, fonts, "plus-circle", &renzora::lang::t("animation.create_animation"), crate::setup::CreateAnimBtn);
    bind_display(commands, create_btn, crate::setup::can_create_anim);
    commands.entity(note).add_children(&[note_lbl, create_btn]);
    bind_display(commands, note, |w| !ready(w));

    // Shared timeline shell.
    let tl = timeline_view(commands, fonts);
    commands
        .entity(tl.root)
        .insert((AnimTimeline, RelativeCursorPosition::default()));
    // The clips layer doubles as the keyframe hit-test surface.
    commands
        .entity(tl.clips)
        .insert((KeyLane, RelativeCursorPosition::default()));
    bind_display(commands, tl.root, ready);

    let htitle = commands.spawn((Text::new(renzora::lang::t("animation.tracks")), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    let hspacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let add_track = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            AddTrackBtn,
        ))
        .id();
    let add_track_ic = icon_text(commands, &fonts.phosphor, "plus", accent(), 10.0);
    let add_track_lbl = commands.spawn((Text::new(renzora::lang::t("timeline.add_track")), ui_font(&fonts.ui, 10.0), TextColor(rgb(accent())), bevy::text::TextLayout::no_wrap())).id();
    commands.entity(add_track).add_children(&[add_track_ic, add_track_lbl]);
    commands.entity(tl.header_corner).add_children(&[htitle, hspacer, add_track]);
    keyed_list(commands, tl.header_list, header_snapshot);
    keyed_list(commands, tl.clips, keyframe_snapshot);
    keyed_list(commands, tl.markers, marker_snapshot);

    commands.entity(body).add_children(&[note, tl.root]);
    commands.entity(root).add_children(&[toolbar, body]);
    root
}

fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(34.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), padding: UiRect::horizontal(Val::Px(6.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();

    let skip_back = icon_btn(commands, fonts, "skip-back", text_primary(), AnimBtn::SkipBack).0;
    let step_back = icon_btn(commands, fonts, "caret-left", text_primary(), AnimBtn::StepBack).0;
    let (play, play_icon) = icon_btn(commands, fonts, "play", text_primary(), AnimBtn::PlayPause);
    commands.entity(play_icon).insert(AnimPlayIcon);
    let stop = icon_btn(commands, fonts, "stop", text_primary(), AnimBtn::Stop).0;
    let step_fwd = icon_btn(commands, fonts, "caret-right", text_primary(), AnimBtn::StepForward).0;
    let skip_fwd = icon_btn(commands, fonts, "skip-forward", text_primary(), AnimBtn::SkipForward).0;

    let sep1 = vsep(commands);

    let (loop_b, loop_ic) = icon_btn(commands, fonts, "repeat", text_muted(), AnimBtn::Loop);
    bind_text_color(commands, loop_ic, |w| {
        let on = state(w).is_some_and(|s| s.preview_looping);
        rgb(if on { accent() } else { text_muted() })
    });

    let sep2 = vsep(commands);

    // Clip selector.
    let clip_ic = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    let combo = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            ClipCombo,
        ))
        .id();
    let combo_v = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { min_width: Val::Px(96.0), max_width: Val::Px(150.0), overflow: Overflow::clip(), ..default() }, bevy::text::TextLayout::no_wrap())).id();
    bind_text(commands, combo_v, |w| state(w).and_then(|s| s.selected_clip.clone()).unwrap_or_else(|| renzora::lang::t("timeline.select_clip")));
    let combo_c = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(combo).add_children(&[combo_v, combo_c]);
    let _ = clip_ic;

    // Inline "new clip" authoring: a name field + "+" that creates another clip
    // on this entity's animator. The empty-state Create-Animation button is gone
    // once a first clip exists, so this is the path to multiple clips per entity
    // (e.g. one per facing direction). Mirrors the event-marker field below.
    let new_clip_field = text_input(commands, &fonts.ui, "new clip", "");
    commands.entity(new_clip_field).insert((
        NewClipNameField,
        Node {
            min_width: Val::Px(64.0),
            width: Val::Px(84.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            flex_shrink: 0.0,
            ..default()
        },
    ));
    let new_clip_b = icon_btn(commands, fonts, "plus", accent(), NewClipBtn).0;

    let sep3 = vsep(commands);

    // Speed presets.
    let speed_lbl = commands.spawn((Text::new(renzora::lang::t("common.speed")), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    let mut speed_btns: Vec<Entity> = Vec::with_capacity(SPEEDS.len());
    for &s in &SPEEDS {
        let btn = commands
            .spawn((Node { height: Val::Px(18.0), min_width: Val::Px(32.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, padding: UiRect::horizontal(Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(card_bg())), Interaction::default(), SpeedBtn(s)))
            .id();
        let lbl = commands.spawn((Text::new(format!("{:.2}x", s)), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_primary())))).id();
        bind_text_color(commands, lbl, move |w| {
            let active = state(w).is_some_and(|st| (st.preview_speed - s).abs() < 0.01);
            rgb(if active { accent() } else { text_primary() })
        });
        commands.entity(btn).add_child(lbl);
        speed_btns.push(btn);
    }

    let sep4 = vsep(commands);

    let (snap_b, snap_ic) = icon_btn(commands, fonts, "magnet-straight", text_muted(), AnimBtn::Snap);
    bind_text_color(commands, snap_ic, |w| {
        let on = state(w).is_some_and(|s| s.snap_enabled);
        rgb(if on { accent() } else { text_muted() })
    });

    // Record toggle (auto-key inspector edits) — red when armed.
    let (record_b, record_ic) = icon_btn(commands, fonts, "record", text_muted(), AnimBtn::Record);
    bind_text_color(commands, record_ic, |w| {
        let on = state(w).is_some_and(|s| s.record_enabled);
        rgb(if on { (220, 70, 70) } else { text_muted() })
    });
    // Add a property track / insert a key at the playhead.
    let add_prop_b = icon_btn(commands, fonts, "list-plus", text_primary(), AnimBtn::AddProperty).0;
    let add_key_b = icon_btn(commands, fonts, "diamond", text_primary(), AnimBtn::AddKey).0;

    // Event-marker authoring: name field + add-at-playhead button.
    let marker_field = text_input(commands, &fonts.ui, "event", "");
    commands.entity(marker_field).insert((
        MarkerNameField,
        Node {
            min_width: Val::Px(64.0),
            width: Val::Px(80.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            flex_shrink: 0.0,
            ..default()
        },
    ));
    let add_marker_b = icon_btn(commands, fonts, "flag", MARKER, AddMarkerBtn).0;

    // Save — accent-colored while there are unsaved keyframe edits.
    let (save_b, save_ic) = icon_btn(commands, fonts, "floppy-disk", text_muted(), SaveClipBtn);
    bind_text_color(commands, save_ic, |w| {
        let dirty = w.get_resource::<NativeAnimClip>().is_some_and(|c| c.dirty);
        rgb(if dirty { accent() } else { text_muted() })
    });

    // Selected-keyframe readout: "Rotation @ 1.33s = (…)".
    let keyinfo = commands.spawn((Text::new(""), ui_font(&fonts.mono, 10.0), TextColor(rgb(PROPERTY)), bevy::text::TextLayout::no_wrap())).id();
    bind_text(commands, keyinfo, selected_key_label);

    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();

    // Clip length (seconds) — scrub to lengthen/shorten the timeline.
    let len_lbl = commands.spawn((Text::new(renzora::lang::t("timeline.length_short")), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    let len_dv = drag_value(commands, &fonts.mono, "", text_primary(), 2.0, 0.1);
    commands.entity(len_dv).insert(DragRange { min: 0.2, max: 600.0 });
    bind_2way(
        commands,
        len_dv,
        |w: &World| cur_clip(w).map(|c| c.duration).unwrap_or(2.0),
        |w: &mut World, v: &f32| set_clip_duration(w, *v),
    );

    let time = commands.spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, time, |w| {
        let Some(s) = state(w) else { return String::new() };
        let secs = s.scrub_time;
        let frame = (secs * 30.0) as u32;
        format!("{:02}:{:05.2}  f{}", (secs / 60.0) as u32, secs % 60.0, frame)
    });

    let zoom_out = icon_btn(commands, fonts, "magnifying-glass-minus", text_muted(), AnimBtn::ZoomOut).0;
    let zoom_lbl = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, zoom_lbl, |w| format!("{:.0}px/s", state(w).map(|s| s.timeline_zoom).unwrap_or(0.0)));
    let zoom_in = icon_btn(commands, fonts, "magnifying-glass-plus", text_muted(), AnimBtn::ZoomIn).0;

    let mut kids = vec![skip_back, step_back, play, stop, step_fwd, skip_fwd, record_b, sep1, loop_b, sep2, combo, new_clip_field, new_clip_b, sep3, speed_lbl];
    kids.extend(speed_btns);
    kids.extend([sep4, add_prop_b, add_key_b, marker_field, add_marker_b, snap_b, save_b, keyinfo, gap, len_lbl, len_dv, time, zoom_out, zoom_lbl, zoom_in]);
    commands.entity(bar).add_children(&kids);
    bar
}

fn icon_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8), marker: M) -> (Entity, Entity) {
    let btn = commands
        .spawn((Node { width: Val::Px(30.0), height: Val::Px(28.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), marker))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 17.0);
    commands.entity(btn).add_child(ic);
    (btn, ic)
}

fn vsep(commands: &mut Commands) -> Entity {
    commands.spawn((Node { width: Val::Px(1.0), height: Val::Px(22.0), margin: UiRect::horizontal(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(border())))).id()
}

// ── Snapshots ──────────────────────────────────────────────────────────────────

/// A track-header row: a skeletal bone (T/R/S channels) or a property track.
enum HeaderRow {
    Bone { name: String, ht: bool, hr: bool, hs: bool },
    /// Property track: index + its label (`None` until a property is picked).
    Prop { track: usize, label: Option<String> },
}

fn header_snapshot(world: &World) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world) else { return empty() };
    let th = state(world).map(|s| s.track_height).unwrap_or(22.0);
    let mut rows: Vec<HeaderRow> = clip
        .tracks
        .iter()
        .map(|t| HeaderRow::Bone {
            name: t.bone_name.clone(),
            ht: !t.translations.is_empty(),
            hr: !t.rotations.is_empty(),
            hs: !t.scales.is_empty(),
        })
        .collect();
    for (pi, pt) in clip.property_tracks.iter().enumerate() {
        let label = if pt.component.is_empty() { None } else { Some(property_label(pt)) };
        rows.push(HeaderRow::Prop { track: pi, label });
    }
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match row {
                HeaderRow::Bone { name, ht, hr, hs } => (0u8, name, ht, hr, hs, th.to_bits()).hash(&mut h),
                HeaderRow::Prop { track, label } => (1u8, track, label, th.to_bits()).hash(&mut h),
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &rows[i] {
            HeaderRow::Bone { name, ht, hr, hs } => header_row(c, f, i, name, *ht, *hr, *hs, th),
            HeaderRow::Prop { track, label } => prop_header_row(c, f, i, *track, label.as_deref(), th),
        }),
    }
}

/// Header label for a property track: "Component · Field" with the target node
/// in parentheses when not the animator entity itself.
fn property_label(pt: &PropertyTrack) -> String {
    let base = format!("{} · {}", title_case(&pt.component), title_case(&pt.field));
    if pt.target.is_empty() || pt.target == "self" {
        base
    } else {
        format!("{} ({})", base, pt.target)
    }
}

fn title_case(s: &str) -> String {
    s.replace(['_', '.'], " ")
        .split_whitespace()
        .map(|w| {
            let mut ch = w.chars();
            match ch.next() {
                Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A property-track header row: property icon + a clickable property dropdown
/// ("Select property…" until picked) + a delete button.
fn prop_header_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, track: usize, label: Option<&str>, th: f32) -> Entity {
    let bg = if idx.is_multiple_of(2) { row_even() } else { row_odd() };
    let row = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(th), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(rgb(bg))))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "sliders-horizontal", PROPERTY, 10.0);

    // Property dropdown (combo): click to pick / change the bound property.
    let combo = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            PropTrackCombo(track),
        ))
        .id();
    let (txt, col) = match label {
        Some(l) => (l.to_string(), text_primary()),
        None => (renzora::lang::t("timeline.select_property"), text_muted()),
    };
    let combo_lbl = commands.spawn((Text::new(txt), ui_font(&fonts.ui, 11.0), TextColor(rgb(col)), bevy::text::TextLayout::no_wrap(), Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() })).id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 8.0);
    commands.entity(combo).add_children(&[combo_lbl, caret]);

    // Add-key button (keys THIS track at the playhead from the live value).
    let addk = commands
        .spawn((Node { width: Val::Px(16.0), height: Val::Px(16.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), AddKeyTrackBtn(track)))
        .id();
    let addk_ic = icon_text(commands, &fonts.phosphor, "diamond", PROPERTY, 11.0);
    commands.entity(addk).add_child(addk_ic);

    // Delete-track button.
    let del = commands
        .spawn((Node { width: Val::Px(16.0), height: Val::Px(16.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), DeletePropTrack(track)))
        .id();
    let del_ic = icon_text(commands, &fonts.phosphor, "x", text_muted(), 11.0);
    commands.entity(del).add_child(del_ic);

    commands.entity(row).add_children(&[ic, combo, addk, del]);
    row
}

#[allow(clippy::too_many_arguments)]
fn header_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str, ht: bool, hr: bool, hs: bool, th: f32) -> Entity {
    let bg = if idx.is_multiple_of(2) { row_even() } else { row_odd() };
    let row = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(th), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(rgb(bg))))
        .id();
    let bone = icon_text(commands, &fonts.phosphor, "bone", text_muted(), 10.0);
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() })).id();
    let t = channel_letter(commands, fonts, "T", ht, TRANSLATION);
    let r = channel_letter(commands, fonts, "R", hr, ROTATION);
    let s = channel_letter(commands, fonts, "S", hs, SCALE);
    commands.entity(row).add_children(&[bone, lbl, t, r, s]);
    row
}

fn channel_letter(commands: &mut Commands, fonts: &EmberFonts, ch: &str, active: bool, color: (u8, u8, u8)) -> Entity {
    let col = if active { color } else { text_muted() };
    commands.spawn((Text::new(ch.to_string()), ui_font(&fonts.ui, 9.0), TextColor(rgb(col)))).id()
}

/// One renderable timeline element after clustering.
#[derive(Clone, Copy)]
enum KeyElem {
    /// A lone (editable) bone keyframe: (track, channel, key index, time).
    Key(usize, u8, usize, f32),
    /// A run of bone keys denser than the cluster threshold, drawn as one bar:
    /// (track, channel, first time, last time, count).
    Bar(usize, u8, f32, f32, usize),
    /// A lone (editable) property keyframe: (property track, key index, time).
    PKey(usize, usize, f32),
    /// A dense run of property keys drawn as one bar: (property track, first,
    /// last, count).
    PBar(usize, f32, f32, usize),
}

/// Keys closer together than this many pixels merge into a bar. Baked 30 Hz
/// captures render as clean per-channel range bars instead of a wall of
/// overlapping diamonds; zooming in past the threshold reveals (editable)
/// individual keys.
const CLUSTER_PX: f32 = 9.0;
/// How far past the visible window keys are still spawned, in pixels.
const CULL_MARGIN_PX: f32 = 64.0;
/// Upper bound on the lane width used for culling — the actual panel is
/// narrower, so this only ever over-includes slightly.
const MAX_LANE_PX: f32 = 4096.0;

/// Cluster one channel's sorted key list into renderable elements, culled to
/// the visible window. `mk_key`/`mk_bar` build the lane-specific [`KeyElem`]
/// (bone channel vs property track) from `(index, time)` / `(t0, t1, count)`.
fn cluster_channel(
    out: &mut Vec<KeyElem>,
    times: impl Iterator<Item = f32>,
    zoom: f32,
    t_min: f32,
    t_max: f32,
    mk_key: impl Fn(usize, f32) -> KeyElem,
    mk_bar: impl Fn(f32, f32, usize) -> KeyElem,
) {
    let gap = CLUSTER_PX / zoom;
    // (first index, first time, last time, count) of the open cluster.
    let mut run: Option<(usize, f32, f32, usize)> = None;
    let flush = |out: &mut Vec<KeyElem>, run: (usize, f32, f32, usize)| {
        let (i0, t0, t1, n) = run;
        if n >= 3 {
            out.push(mk_bar(t0, t1, n));
        } else {
            for k in 0..n {
                // 1–2 keys: emit individually.
                let t = if k == 0 { t0 } else { t1 };
                out.push(mk_key(i0 + k, t));
            }
        }
    };
    for (idx, t) in times.enumerate() {
        if t < t_min - gap || t > t_max + gap {
            // Outside the window — close any open run that ended in view.
            if let Some(r) = run.take() {
                flush(out, r);
            }
            continue;
        }
        match run.as_mut() {
            Some((_, _, last, n)) if t - *last <= gap => {
                *last = t;
                *n += 1;
            }
            Some(_) => {
                let r = run.take().unwrap();
                flush(out, r);
                run = Some((idx, t, t, 1));
            }
            None => run = Some((idx, t, t, 1)),
        }
    }
    if let Some(r) = run.take() {
        flush(out, r);
    }
}

/// Whether a clustered element corresponds to the selected keyframe.
fn elem_selected(e: &KeyElem, sel: Option<SelKey>) -> bool {
    let Some(sel) = sel else { return false };
    match (e, sel.lane) {
        (KeyElem::Key(ti, ch, idx, _), Lane::Bone { track, channel }) => {
            *ti == track && *ch == channel && *idx == sel.index
        }
        (KeyElem::PKey(pi, idx, _), Lane::Prop { track }) => *pi == track && *idx == sel.index,
        _ => false,
    }
}

fn keyframe_snapshot(world: &World) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world) else { return empty() };
    let Some(s) = state(world) else { return empty() };
    let sel = world.get_resource::<SelectedKey>().and_then(|s| s.0);
    let (zoom, scroll, th) = (s.timeline_zoom, s.timeline_scroll, s.track_height);
    let t_min = scroll - CULL_MARGIN_PX / zoom;
    let t_max = scroll + (MAX_LANE_PX + CULL_MARGIN_PX) / zoom;

    let bone_count = clip.tracks.len();
    let mut elems: Vec<KeyElem> = Vec::new();
    for (ti, track) in clip.tracks.iter().enumerate() {
        cluster_channel(&mut elems, track.translations.iter().map(|k| k.0), zoom, t_min, t_max,
            move |idx, t| KeyElem::Key(ti, 0, idx, t), move |t0, t1, n| KeyElem::Bar(ti, 0, t0, t1, n));
        cluster_channel(&mut elems, track.rotations.iter().map(|k| k.0), zoom, t_min, t_max,
            move |idx, t| KeyElem::Key(ti, 1, idx, t), move |t0, t1, n| KeyElem::Bar(ti, 1, t0, t1, n));
        cluster_channel(&mut elems, track.scales.iter().map(|k| k.0), zoom, t_min, t_max,
            move |idx, t| KeyElem::Key(ti, 2, idx, t), move |t0, t1, n| KeyElem::Bar(ti, 2, t0, t1, n));
    }
    for (pi, pt) in clip.property_tracks.iter().enumerate() {
        cluster_channel(&mut elems, pt.keys.iter().map(|k| k.time), zoom, t_min, t_max,
            move |idx, t| KeyElem::PKey(pi, idx, t), move |t0, t1, n| KeyElem::PBar(pi, t0, t1, n));
    }

    let items: Vec<(u64, u64)> = elems
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match e {
                KeyElem::Key(ti, ch, idx, time) => {
                    (0u8, ti, ch, idx, time.to_bits()).hash(&mut h)
                }
                KeyElem::Bar(ti, ch, t0, t1, n) => {
                    (1u8, ti, ch, t0.to_bits(), t1.to_bits(), n).hash(&mut h)
                }
                KeyElem::PKey(pi, idx, time) => {
                    (2u8, pi, idx, time.to_bits()).hash(&mut h)
                }
                KeyElem::PBar(pi, t0, t1, n) => {
                    (3u8, pi, t0.to_bits(), t1.to_bits(), n).hash(&mut h)
                }
            }
            (zoom.to_bits(), scroll.to_bits(), th.to_bits()).hash(&mut h);
            elem_selected(e, sel).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| {
            let sd = elem_selected(&elems[i], sel);
            match elems[i] {
                KeyElem::Key(ti, ch, idx, time) => diamond(c, ti, ch, idx, time, zoom, scroll, th, sd),
                KeyElem::Bar(ti, ch, t0, t1, _) => key_bar(c, ti, ch, t0, t1, zoom, scroll, th),
                KeyElem::PKey(pi, _idx, time) => prop_diamond(c, bone_count + pi, time, zoom, scroll, th, sd),
                KeyElem::PBar(pi, t0, t1, _) => prop_bar(c, bone_count + pi, t0, t1, zoom, scroll, th),
            }
        }),
    }
}

/// Per-channel vertical placement within a track lane.
fn channel_y(ti: usize, ch: u8, th: f32) -> (f32, (u8, u8, u8)) {
    let off = (th * 0.26).min(14.0);
    let center = ti as f32 * th + th * 0.5;
    match ch {
        0 => (center - off, TRANSLATION),
        1 => (center, ROTATION),
        _ => (center + off, SCALE),
    }
}

#[allow(clippy::too_many_arguments)]
fn diamond(commands: &mut Commands, ti: usize, ch: u8, idx: usize, time: f32, zoom: f32, scroll: f32, th: f32, selected: bool) -> Entity {
    let kf = (th * 0.38).clamp(4.0, 14.0) + if selected { 3.0 } else { 0.0 };
    let (y, color) = channel_y(ti, ch, th);
    let x = (time - scroll) * zoom + LANE_INSET;
    let _ = idx;
    spawn_diamond(commands, x, y, kf, color, selected)
}

/// Spawn a 45°-rotated keyframe diamond, with a white outline when selected.
fn spawn_diamond(commands: &mut Commands, x: f32, y: f32, kf: f32, color: (u8, u8, u8), selected: bool) -> Entity {
    let half = kf * 0.5;
    let mut e = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x - half),
            top: Val::Px(y - half),
            width: Val::Px(kf),
            height: Val::Px(kf),
            border: if selected { UiRect::all(Val::Px(1.5)) } else { UiRect::ZERO },
            ..default()
        },
        BackgroundColor(rgb(color)),
        UiTransform::from_rotation(Rot2::degrees(45.0)),
        bevy::ui::FocusPolicy::Pass,
    ));
    if selected {
        e.insert(BorderColor::all(rgb((255, 255, 255))));
    }
    e.id()
}

/// A dense run of keys drawn as one slim rounded bar in the channel color.
#[allow(clippy::too_many_arguments)]
fn key_bar(commands: &mut Commands, ti: usize, ch: u8, t0: f32, t1: f32, zoom: f32, scroll: f32, th: f32) -> Entity {
    let h = (th * 0.22).clamp(3.0, 8.0);
    let (y, color) = channel_y(ti, ch, th);
    let x0 = (t0 - scroll) * zoom + LANE_INSET;
    let w = ((t1 - t0) * zoom).max(h);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x0 - h * 0.5),
                top: Val::Px(y - h * 0.5),
                width: Val::Px(w + h),
                height: Val::Px(h),
                border_radius: BorderRadius::all(Val::Px(h * 0.5)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(color.0, color.1, color.2, 200)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id()
}

/// A property keyframe diamond, centered in its lane in the property color.
fn prop_diamond(commands: &mut Commands, lane: usize, time: f32, zoom: f32, scroll: f32, th: f32, selected: bool) -> Entity {
    let kf = (th * 0.40).clamp(5.0, 16.0) + if selected { 3.0 } else { 0.0 };
    let y = lane as f32 * th + th * 0.5;
    let x = (time - scroll) * zoom + LANE_INSET;
    spawn_diamond(commands, x, y, kf, PROPERTY, selected)
}

/// A dense run of property keys drawn as one slim rounded bar.
fn prop_bar(commands: &mut Commands, lane: usize, t0: f32, t1: f32, zoom: f32, scroll: f32, th: f32) -> Entity {
    let h = (th * 0.22).clamp(3.0, 8.0);
    let y = lane as f32 * th + th * 0.5;
    let x0 = (t0 - scroll) * zoom + LANE_INSET;
    let w = ((t1 - t0) * zoom).max(h);
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(x0 - h * 0.5), top: Val::Px(y - h * 0.5), width: Val::Px(w + h), height: Val::Px(h), border_radius: BorderRadius::all(Val::Px(h * 0.5)), ..default() },
            BackgroundColor(Color::srgba_u8(PROPERTY.0, PROPERTY.1, PROPERTY.2, 200)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id()
}

/// Color for event-marker flags.
const MARKER: (u8, u8, u8) = (200, 140, 220);

/// Renders each clip marker as a labeled flag + thin full-height line. Visual
/// only (`FocusPolicy::Pass`); deletion is a math hit-test in `key_context_menu`.
fn marker_snapshot(world: &World) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world) else { return empty() };
    let Some(s) = state(world) else { return empty() };
    let (zoom, scroll) = (s.timeline_zoom, s.timeline_scroll);
    let markers: Vec<(f32, String)> = clip.markers.iter().map(|m| (m.time, m.name.clone())).collect();
    let items: Vec<(u64, u64)> = markers
        .iter()
        .enumerate()
        .map(|(i, (t, name))| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (t.to_bits(), name, zoom.to_bits(), scroll.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (t, name) = &markers[i];
            marker_flag(c, f, *t, name, zoom, scroll)
        }),
    }
}

fn marker_flag(commands: &mut Commands, fonts: &EmberFonts, time: f32, name: &str, zoom: f32, scroll: f32) -> Entity {
    let x = (time - scroll) * zoom + LANE_INSET;
    let root = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(0.0), width: Val::Px(0.0), height: Val::Percent(100.0), ..default() }, bevy::ui::FocusPolicy::Pass))
        .id();
    let line = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(1.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgba_u8(MARKER.0, MARKER.1, MARKER.2, 110)), bevy::ui::FocusPolicy::Pass))
        .id();
    let flag = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(1.0), top: Val::Px(0.0), padding: UiRect::axes(Val::Px(3.0), Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BackgroundColor(rgb(MARKER)), bevy::ui::FocusPolicy::Pass))
        .id();
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 8.0), TextColor(rgb((25, 20, 30))), bevy::text::TextLayout::no_wrap())).id();
    commands.entity(flag).add_child(lbl);
    commands.entity(root).add_children(&[line, flag]);
    root
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Systems ────────────────────────────────────────────────────────────────────

fn cache_native_clip(
    mut cache: ResMut<NativeAnimClip>,
    state: Res<AnimationEditorState>,
    animators: Query<&AnimatorComponent>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut selected: ResMut<SelectedKey>,
    mut preview: ResMut<PreviewApplied>,
) {
    let key = match (state.selected_entity, state.selected_clip.as_deref()) {
        (Some(e), Some(c)) => Some((e, c.to_string())),
        _ => None,
    };
    if key == cache.key {
        return;
    }
    selected.0 = None;
    // The preview's stored baseline belongs to the old clip — invalidate it so
    // the manual-pose detector doesn't compare against stale data (which would
    // mis-fire and pause playback when switching/clicking away and back).
    preview.valid = false;
    // Auto-save pending edits before switching away, instead of discarding them
    // — clicking off an entity must not lose unsaved keyframes.
    if cache.dirty {
        if let (Some(clip), Some(path)) = (cache.clip.as_ref(), cache.path.as_ref()) {
            match renzora::core::write_anim_file(clip, path) {
                Ok(()) => info!("[timeline] auto-saved keyframe edits before switching"),
                Err(e) => warn!("[timeline] auto-save failed: {}", e),
            }
        }
    }
    cache.key = key.clone();
    cache.clip = None;
    cache.path = None;
    cache.dirty = false;
    let (Some((entity, clip_name)), Some(project)) = (key, project) else { return };
    let Ok(animator) = animators.get(entity) else { return };
    let Some(slot) = animator.clips.iter().find(|s| s.name == clip_name) else { return };
    let path = project.path.join(&slot.path);
    if let Ok(content) = std::fs::read_to_string(&path) {
        cache.clip = ron::from_str::<AnimClip>(&content).ok();
        cache.path = Some(path);
    }
}

/// Set the clip duration (toolbar Length field), syncing the editor's cached
/// duration so the ruler/playhead range + loop point update immediately.
fn set_clip_duration(world: &mut World, dur: f32) {
    let dur = dur.clamp(0.2, 600.0);
    if let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() {
        let changed = match cache.clip.as_mut() {
            Some(clip) if (clip.duration - dur).abs() > 1e-5 => {
                clip.duration = dur;
                true
            }
            _ => false,
        };
        if changed {
            cache.dirty = true;
        }
    }
    if let Some(mut s) = world.get_resource_mut::<AnimationEditorState>() {
        s.clip_duration = Some(dur);
    }
}

/// Read a property track's live value and key it at `time` (auto-extending the
/// clip if `time` is past the end). Used by right-click "Add keyframe here".
fn add_key_at(world: &mut World, entity: Option<Entity>, track: usize, time: f32) {
    let Some(entity) = entity else { return };
    let track_data = world
        .get_resource::<NativeAnimClip>()
        .and_then(|c| c.clip.as_ref())
        .and_then(|c| c.property_tracks.get(track))
        .cloned();
    let Some(track_data) = track_data else { return };
    let Some(val) = read_track_value(world, entity, &track_data) else {
        warn!("[prop-anim] Add Key (right-click): could not read live value for {}.{}", track_data.component, track_data.field);
        return;
    };
    info!(
        "[prop-anim] Add Key (right-click): {}.{} @ t={:.3} from {:?} -> {:?}",
        track_data.component, track_data.field, time, entity, val
    );
    if let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() {
        let mut dirty = false;
        if let Some(clip) = cache.clip.as_mut() {
            if time > clip.duration {
                clip.duration = time;
            }
            if let Some(pt) = clip.property_tracks.get_mut(track) {
                upsert_key(pt, time, val);
                dirty = true;
            }
        }
        if dirty {
            cache.dirty = true;
        }
    }
}

/// Set an existing keyframe's value to the entity's current live value — the
/// foolproof "pose the object, then set this key to it" action.
fn set_key_to_live(world: &mut World, entity: Option<Entity>, track: usize, idx: usize) {
    let Some(entity) = entity else { return };
    let track_data = world
        .get_resource::<NativeAnimClip>()
        .and_then(|c| c.clip.as_ref())
        .and_then(|c| c.property_tracks.get(track))
        .cloned();
    let Some(track_data) = track_data else { return };
    let Some(val) = read_track_value(world, entity, &track_data) else {
        warn!("[prop-anim] Set Key: could not read live value for {}.{}", track_data.component, track_data.field);
        return;
    };
    if let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() {
        if let Some(key) = cache
            .clip
            .as_mut()
            .and_then(|c| c.property_tracks.get_mut(track))
            .and_then(|pt| pt.keys.get_mut(idx))
        {
            info!(
                "[prop-anim] Set Key {} of {}.{} -> {:?}",
                idx, track_data.component, track_data.field, val
            );
            key.value = val;
        }
        cache.dirty = true;
    }
}

/// Set a property key's interpolation curve and mark the clip dirty (so the
/// scrub preview and the auto-save reflect it immediately).
fn set_key_interp(world: &mut World, track: usize, idx: usize, interp: renzora::Interp) {
    if let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() {
        if let Some(key) = cache
            .clip
            .as_mut()
            .and_then(|c| c.property_tracks.get_mut(track))
            .and_then(|pt| pt.keys.get_mut(idx))
        {
            key.interp = interp;
        }
        cache.dirty = true;
    }
}

/// The interpolation choices offered in a property key's right-click menu, as
/// `(icon, label, interp)`. A curated subset of Bevy's [`EaseFunction`] set —
/// the ones that read clearly on a float/Vec3/Color dopesheet — plus the two
/// non-eased modes. Per-key authoring writes one of these into `PropertyKey`.
fn interp_menu_choices() -> [(&'static str, &'static str, renzora::Interp); 9] {
    use bevy::math::curve::EaseFunction as E;
    use renzora::Interp;
    [
        ("minus", "Linear", Interp::Linear),
        ("stairs", "Stepped (hold)", Interp::Stepped),
        ("chart-line", "Smooth (in-out)", Interp::Eased(E::SmoothStep)),
        ("trend-up", "Ease In", Interp::Eased(E::QuadraticIn)),
        ("trend-down", "Ease Out", Interp::Eased(E::QuadraticOut)),
        ("activity", "Ease In-Out", Interp::Eased(E::CubicInOut)),
        ("arrow-u-up-left", "Back Out (overshoot)", Interp::Eased(E::BackOut)),
        ("circle", "Bounce Out", Interp::Eased(E::BounceOut)),
        ("waves", "Elastic Out", Interp::Eased(E::ElasticOut)),
    ]
}

/// Localized display string for an interpolation choice's English label (the
/// English label stays the identity in `interp_menu_choices`; the `Interp` enum
/// value is what's actually written, so only the menu text is translated).
fn interp_label_tr(label: &str) -> String {
    renzora::lang::t(match label {
        "Linear" => "animation.interp_linear",
        "Stepped (hold)" => "animation.interp_stepped",
        "Smooth (in-out)" => "animation.interp_smooth",
        "Ease In" => "animation.interp_ease_in",
        "Ease Out" => "animation.interp_ease_out",
        "Ease In-Out" => "animation.interp_ease_in_out",
        "Back Out (overshoot)" => "animation.interp_back_out",
        "Bounce Out" => "animation.interp_bounce_out",
        "Elastic Out" => "animation.interp_elastic_out",
        _ => return label.to_string(),
    })
}

fn push(bridge: &AnimEditorBridge, action: AnimEditorAction) {
    if let Ok(mut p) = bridge.pending.lock() {
        p.push(action);
    }
}

fn anim_btn_click(
    q: Query<(&Interaction, &AnimBtn), Changed<Interaction>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Option<Res<NativeAnimClip>>,
    bridge: Option<Res<AnimEditorBridge>>,
    mut ops: ResMut<TimelineOps>,
) {
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    let dur = cache.and_then(|c| c.clip.as_ref().map(|c| c.duration)).unwrap_or(2.0);
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let action = match btn {
            AnimBtn::SkipBack => AnimEditorAction::SetScrubTime(0.0),
            AnimBtn::StepBack => AnimEditorAction::SetScrubTime((state.scrub_time - 1.0 / 30.0).max(0.0)),
            AnimBtn::PlayPause => AnimEditorAction::TogglePreview,
            AnimBtn::Stop => AnimEditorAction::StopPreview,
            AnimBtn::StepForward => AnimEditorAction::SetScrubTime((state.scrub_time + 1.0 / 30.0).min(dur)),
            AnimBtn::SkipForward => AnimEditorAction::SetScrubTime(dur),
            AnimBtn::Loop => AnimEditorAction::SetPreviewLooping(!state.preview_looping),
            AnimBtn::Snap => AnimEditorAction::SetSnapEnabled(!state.snap_enabled),
            AnimBtn::ZoomIn => AnimEditorAction::SetTimelineZoom((state.timeline_zoom * 1.25).min(500.0)),
            AnimBtn::ZoomOut => AnimEditorAction::SetTimelineZoom((state.timeline_zoom * 0.8).max(20.0)),
            AnimBtn::Record => AnimEditorAction::SetRecordEnabled(!state.record_enabled),
            AnimBtn::AddProperty => {
                ops.add_empty_track = true;
                continue;
            }
            AnimBtn::AddKey => {
                ops.add_key = true;
                continue;
            }
        };
        push(&bridge, action);
    }
}

fn speed_btn_click(q: Query<(&Interaction, &SpeedBtn), Changed<Interaction>>, bridge: Option<Res<AnimEditorBridge>>) {
    let Some(bridge) = bridge else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            push(&bridge, AnimEditorAction::SetPreviewSpeed(btn.0));
        }
    }
}

fn anim_sync(
    mut q: Query<&mut TimelineView, With<AnimTimeline>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Option<Res<NativeAnimClip>>,
    bridge: Option<Res<AnimEditorBridge>>,
    drag: Res<KeyDragState>,
) {
    let Some(state) = state else { return };
    let clip = cache.as_ref().and_then(|c| c.clip.as_ref());
    let dur = clip.map(|c| c.duration).unwrap_or(2.0);
    let tracks = clip.map(|c| c.tracks.len() + c.property_tracks.len()).unwrap_or(0);
    for mut v in &mut q {
        v.set_geom(state.timeline_zoom, state.timeline_scroll, state.scrub_time, dur, state.track_height, tracks);
        // A keyframe drag owns the pointer — discard the scrub the overlay
        // reports for the same gesture so the playhead doesn't chase the key.
        if let Some(t) = v.take_scrub() {
            if drag.active.is_none() {
                if let Some(bridge) = &bridge {
                    push(bridge, AnimEditorAction::SetScrubTime(t.clamp(0.0, dur)));
                }
            }
        }
    }
}

fn update_anim_play_icon(state: Option<Res<AnimationEditorState>>, mut q: Query<&mut Text, With<AnimPlayIcon>>) {
    let Some(state) = state else { return };
    let glyph = renzora_ember::font::icon_glyph(if state.is_previewing { "pause" } else { "play" });
    if let Some(g) = glyph {
        let s = g.to_string();
        for mut t in &mut q {
            if t.0 != s {
                t.0 = s.clone();
            }
        }
    }
}

// ── Keyframe editing ───────────────────────────────────────────────────────────

/// Cursor position in the clips-layer's pixel space, or `None` when outside.
fn lane_cursor(
    lane: &Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
) -> Option<Vec2> {
    let (rcp, cn) = lane.iter().next()?;
    if !rcp.cursor_over {
        return None;
    }
    let n = rcp.normalized?;
    let size = cn.size() * cn.inverse_scale_factor();
    Some((n + Vec2::splat(0.5)) * size)
}

struct PickedKey {
    lane: Lane,
    index: usize,
    time: f32,
}

/// Whether the key at `idx` in a sorted time list renders as part of a cluster
/// bar (runs of 3+ within the cluster gap) rather than an editable diamond.
fn in_cluster(times: &[f32], idx: usize, gap: f32) -> bool {
    let mut lo = idx;
    while lo > 0 && times[lo] - times[lo - 1] <= gap && idx - lo < 3 {
        lo -= 1;
    }
    let mut hi = idx;
    while hi + 1 < times.len() && times[hi + 1] - times[hi] <= gap && hi - lo < 3 {
        hi += 1;
    }
    hi - lo + 1 > 2
}

/// Find the editable keyframe nearest to a lane-space point. Keys rendered as
/// cluster bars (runs of 3+ within the cluster gap) are not pickable — zoom in
/// until they split into diamonds.
fn pick_key(clip: &AnimClip, zoom: f32, scroll: f32, th: f32, p: Vec2) -> Option<PickedKey> {
    let radius = (th * 0.30).clamp(5.0, 10.0);
    let gap = CLUSTER_PX / zoom.max(1.0);
    let mut best: Option<(f32, PickedKey)> = None;

    let mut scan = |lane: Lane, y: f32, times: &[f32]| {
        let dy = (p.y - y).abs();
        if dy > radius {
            return;
        }
        for (idx, &t) in times.iter().enumerate() {
            let dx = ((t - scroll) * zoom + LANE_INSET - p.x).abs();
            if dx > radius || in_cluster(times, idx, gap) {
                continue;
            }
            let score = dx.max(dy);
            if best.as_ref().is_none_or(|(s, _)| score < *s) {
                best = Some((score, PickedKey { lane, index: idx, time: t }));
            }
        }
    };

    for (ti, track) in clip.tracks.iter().enumerate() {
        let lane_top = ti as f32 * th;
        if p.y < lane_top - radius || p.y > lane_top + th + radius {
            continue;
        }
        let t_times: Vec<f32> = track.translations.iter().map(|k| k.0).collect();
        let r_times: Vec<f32> = track.rotations.iter().map(|k| k.0).collect();
        let s_times: Vec<f32> = track.scales.iter().map(|k| k.0).collect();
        scan(Lane::Bone { track: ti, channel: 0 }, channel_y(ti, 0, th).0, &t_times);
        scan(Lane::Bone { track: ti, channel: 1 }, channel_y(ti, 1, th).0, &r_times);
        scan(Lane::Bone { track: ti, channel: 2 }, channel_y(ti, 2, th).0, &s_times);
    }

    let bone_count = clip.tracks.len();
    for (pi, pt) in clip.property_tracks.iter().enumerate() {
        let lane = bone_count + pi;
        let y = lane as f32 * th + th * 0.5;
        let times: Vec<f32> = pt.keys.iter().map(|k| k.time).collect();
        scan(Lane::Prop { track: pi }, y, &times);
    }

    best.map(|(_, k)| k)
}

/// Drag a keyframe diamond horizontally to retime it (snap-aware). The drag is
/// tracked against the raw mouse button, not node `Interaction`, because the
/// keyed list rebuilds the diamond while its time changes.
fn key_drag(
    mut drag: ResMut<KeyDragState>,
    buttons: Res<ButtonInput<MouseButton>>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    state: Option<Res<AnimationEditorState>>,
    mut cache: ResMut<NativeAnimClip>,
    mut selected: ResMut<SelectedKey>,
    bridge: Option<Res<AnimEditorBridge>>,
) {
    let Some(state) = state else { return };

    if drag.active.is_some() {
        if !buttons.pressed(MouseButton::Left) {
            // Drag ended — restore sorted key order for playback.
            if let Some(d) = drag.active.take() {
                if d.moved {
                    if let Some(mut chan) = cache.lane_times(d.lane) {
                        chan.sort();
                    }
                }
            }
            return;
        }
        let Some(p) = lane_cursor(&lane) else { return };
        let Some(d) = drag.active.as_mut() else { return };
        let dt = (p.x - d.start_cursor_x) / state.timeline_zoom.max(1.0);
        let mut t = (d.orig_time + dt).max(0.0);
        if state.snap_enabled && state.snap_interval > 0.0 {
            t = (t / state.snap_interval).round() * state.snap_interval;
        }
        let (lane_ref, idx) = (d.lane, d.index);
        if let Some(dur) = cache.clip.as_ref().map(|c| c.duration) {
            t = t.min(dur);
        }
        let mut changed = false;
        if let Some(mut chan) = cache.lane_times(lane_ref) {
            if chan.time(idx).is_some_and(|cur| (cur - t).abs() > 1e-6) {
                chan.set_time(idx, t);
                changed = true;
            }
        }
        if changed {
            if let Some(d) = drag.active.as_mut() {
                d.moved = true;
            }
            cache.dirty = true;
        }
        return;
    }

    // Begin a drag when the press lands on an editable key; clicking empty lane
    // space deselects. Clicking a key selects it (for the highlight + readout).
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(p) = lane_cursor(&lane) else { return };
    let Some(clip) = cache.clip.as_ref() else { return };
    let Some(pick) = pick_key(clip, state.timeline_zoom, state.timeline_scroll, state.track_height, p)
    else {
        selected.0 = None;
        return;
    };
    selected.0 = Some(SelKey { lane: pick.lane, index: pick.index });
    // Jump the playhead to the selected key so its pose shows and live-edit can
    // capture changes back into it.
    if let Some(bridge) = &bridge {
        push(bridge, AnimEditorAction::SetScrubTime(pick.time));
    }
    drag.active = Some(KeyDrag {
        lane: pick.lane,
        index: pick.index,
        start_cursor_x: p.x,
        orig_time: pick.time,
        moved: false,
    });
}

/// Right-click an editable keyframe → context menu with Delete.
fn key_context_menu(
    buttons: Res<ButtonInput<MouseButton>>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Res<NativeAnimClip>,
    mut commands: Commands,
) {
    if !buttons.just_pressed(MouseButton::Right) {
        return;
    }
    let (Some(fonts), Some(state)) = (fonts, state) else { return };
    let Some(p) = lane_cursor(&lane) else { return };
    let Some(clip) = cache.clip.as_ref() else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let (zoom, scroll, th) = (state.timeline_zoom, state.timeline_scroll, state.track_height);

    // Top strip: right-click near a marker flag → delete that marker.
    if p.y < 16.0 {
        if let Some(mi) = clip
            .markers
            .iter()
            .position(|m| ((m.time - scroll) * zoom + LANE_INSET - p.x).abs() < 7.0)
        {
            let menu = screen_menu(&mut commands, cursor.x, cursor.y);
            let label = format!("{} '{}'", renzora::lang::t("timeline.delete_marker"), clip.markers[mi].name);
            let del = menu_item(&mut commands, &fonts, "trash", &label, move |w| {
                if let Some(mut cache) = w.get_resource_mut::<NativeAnimClip>() {
                    if let Some(clip) = cache.clip.as_mut() {
                        if mi < clip.markers.len() {
                            clip.markers.remove(mi);
                        }
                    }
                    cache.dirty = true;
                }
            });
            commands.entity(menu).add_children(&[del]);
            return;
        }
    }

    // Right-click ON a key → delete / interp menu.
    if let Some(pick) = pick_key(clip, zoom, scroll, th, p) {
        let menu = screen_menu(&mut commands, cursor.x, cursor.y);
        let (lane_ref, idx) = (pick.lane, pick.index);
        let del = menu_item(&mut commands, &fonts, "trash", &renzora::lang::t("timeline.delete_keyframe"), move |w| {
            if let Some(mut cache) = w.get_resource_mut::<NativeAnimClip>() {
                if let Some(mut chan) = cache.lane_times(lane_ref) {
                    chan.remove(idx);
                }
                cache.dirty = true;
            }
        });
        let mut kids = vec![del];
        if let Lane::Prop { track } = lane_ref {
            // Foolproof: set this key to whatever the entity is currently posed to.
            let set_item = menu_item(&mut commands, &fonts, "crosshair", &renzora::lang::t("timeline.set_to_current_pose"), move |w| {
                let entity = clip_entity(w);
                set_key_to_live(w, entity, track, idx);
            });
            kids.push(set_item);
            // Per-key interpolation picker: a curated row per easing choice. The
            // active curve is flagged with a "check" icon so the menu doubles as a
            // readout of the key's current interpolation.
            let current = clip
                .property_tracks
                .get(track)
                .and_then(|pt| pt.keys.get(idx))
                .map(|k| k.interp);
            for (icon, label, interp) in interp_menu_choices() {
                let shown_icon = if current == Some(interp) { "check" } else { icon };
                let item = menu_item(&mut commands, &fonts, shown_icon, &interp_label_tr(label), move |w| {
                    set_key_interp(w, track, idx, interp);
                });
                kids.push(item);
            }
        }
        commands.entity(menu).add_children(&kids);
        return;
    }

    // Right-click on empty space over a bound property lane → add a key there.
    let bone_count = clip.tracks.len() as i64;
    let row = (p.y / th.max(1.0)).floor() as i64;
    if row < bone_count {
        return;
    }
    let pi = (row - bone_count) as usize;
    let Some(pt) = clip.property_tracks.get(pi) else { return };
    if pt.component.is_empty() {
        return;
    }
    let mut time = ((p.x - LANE_INSET) / zoom.max(1.0) + scroll).max(0.0);
    if state.snap_enabled && state.snap_interval > 0.0 {
        time = (time / state.snap_interval).round() * state.snap_interval;
    }
    let entity = cache.key.as_ref().map(|(e, _)| *e);
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let add = menu_item(&mut commands, &fonts, "plus", &renzora::lang::t("timeline.add_keyframe_here"), move |w| {
        add_key_at(w, entity, pi, time);
    });
    commands.entity(menu).add_children(&[add]);
}

/// Throttle for periodic auto-save.
#[derive(Resource, Default)]
struct AutoSaveTimer {
    last: f32,
}

/// Auto-save the edit buffer back to disk while dirty (at most ~1×/1.5s), so
/// edits are never lost and Play mode (which reads the `.anim` from disk) picks
/// them up without a manual save.
fn auto_save_clip(time: Res<Time>, mut cache: ResMut<NativeAnimClip>, mut timer: ResMut<AutoSaveTimer>) {
    if !cache.dirty {
        return;
    }
    let now = time.elapsed_secs();
    if now - timer.last < 1.5 {
        return;
    }
    timer.last = now;
    let result = match (cache.clip.as_ref(), cache.path.as_ref()) {
        (Some(clip), Some(path)) => Some(renzora::core::write_anim_file(clip, path)),
        _ => None,
    };
    match result {
        Some(Ok(())) => {
            cache.dirty = false;
            info!("[timeline] auto-saved keyframe edits");
        }
        Some(Err(e)) => warn!("[timeline] auto-save failed: {}", e),
        None => {}
    }
}

/// "Add Marker" button → add an event marker at the playhead, named from the
/// toolbar field (default "event").
fn add_marker_click(
    q: Query<&Interaction, (With<AddMarkerBtn>, Changed<Interaction>)>,
    field: Query<&EmberTextInput, With<MarkerNameField>>,
    state: Option<Res<AnimationEditorState>>,
    mut cache: ResMut<NativeAnimClip>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(state) = state else { return };
    let name = field
        .iter()
        .next()
        .map(|f| f.value.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "event".to_string());
    let time = state.scrub_time;
    if let Some(clip) = cache.clip.as_mut() {
        clip.markers.push(AnimMarker { time, name });
        cache.dirty = true;
    }
}

/// "+" beside the clip selector → create a new clip named from the field on the
/// selected entity's animator, then select it. Empty field falls back to a
/// generic `clip` name. Defers the world mutation through [`EditorCommands`],
/// like the other setup actions, and reuses `setup::create_clip_on_entity` so
/// this and the empty-state button build clips identically.
fn new_clip_click(
    q: Query<&Interaction, (With<NewClipBtn>, Changed<Interaction>)>,
    field: Query<&EmberTextInput, With<NewClipNameField>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let raw = field
        .iter()
        .next()
        .map(|f| f.value.trim().to_string())
        .unwrap_or_default();
    let name = crate::setup::sanitize_clip_name(if raw.is_empty() { "clip" } else { raw.as_str() });
    cmds.push(move |world: &mut World| {
        let Some(entity) = world
            .get_resource::<AnimationEditorState>()
            .and_then(|s| s.selected_entity)
        else {
            return;
        };
        crate::setup::create_clip_on_entity(world, entity, &name);
    });
}

/// Save button → flush the edit buffer back to the `.anim` file on disk.
fn save_clip_click(
    q: Query<&Interaction, (With<SaveClipBtn>, Changed<Interaction>)>,
    mut cache: ResMut<NativeAnimClip>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    if !cache.dirty {
        return;
    }
    let result = match (cache.clip.as_ref(), cache.path.as_ref()) {
        (Some(clip), Some(path)) => renzora::core::write_anim_file(clip, path),
        _ => return,
    };
    match result {
        Ok(()) => {
            cache.dirty = false;
            info!("[timeline] saved keyframe edits");
        }
        Err(e) => warn!("[timeline] save failed: {}", e),
    }
}

/// Keyboard shortcuts, active only while the cursor is over the timeline panel
/// (so they don't clash with global editor shortcuts):
/// Space = play/pause · Home = start · End = end · ←/→ = step frame ·
/// K = add keyframe (all tracks) · N = new track.
///
/// Held off entirely while a UI text field has keyboard focus (`ui_wants_keyboard`)
/// — otherwise typing a clip/marker name into a toolbar field would leak into
/// these actions: `n` spawns a track, `k` adds a key, `,`/`.` scrub, Backspace
/// deletes a keyframe. The resulting track-list churn also stole focus from the
/// field mid-type. Same guard the global keybindings and DAW timeline use.
#[allow(clippy::too_many_arguments)]
fn timeline_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    root: Query<&RelativeCursorPosition, With<AnimTimeline>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Option<Res<NativeAnimClip>>,
    bridge: Option<Res<AnimEditorBridge>>,
    selected: Res<SelectedKey>,
    focus: Option<Res<renzora::InputFocusState>>,
    mut ops: ResMut<TimelineOps>,
) {
    if focus.is_some_and(|f| f.ui_wants_keyboard) {
        return;
    }
    if !root.iter().any(|r| r.cursor_over) {
        return;
    }
    if (keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace))
        && selected.0.is_some()
    {
        ops.delete_selected_key = true;
    }
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    let dur = cache.and_then(|c| c.clip.as_ref().map(|c| c.duration)).unwrap_or(2.0);
    let frame = 1.0 / 30.0;
    if keys.just_pressed(KeyCode::Space) {
        push(&bridge, AnimEditorAction::TogglePreview);
    }
    if keys.just_pressed(KeyCode::Home) {
        push(&bridge, AnimEditorAction::SetScrubTime(0.0));
    }
    if keys.just_pressed(KeyCode::End) {
        push(&bridge, AnimEditorAction::SetScrubTime(dur));
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::Period) {
        push(&bridge, AnimEditorAction::SetScrubTime((state.scrub_time + frame).min(dur)));
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::Comma) {
        push(&bridge, AnimEditorAction::SetScrubTime((state.scrub_time - frame).max(0.0)));
    }
    if keys.just_pressed(KeyCode::KeyK) {
        ops.add_key = true;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        ops.add_empty_track = true;
    }
}

/// Raise `InputFocusState::suppress_entity_delete` while a keyframe is selected
/// and the cursor is over the timeline, so pressing Delete removes the keyframe
/// instead of the selected entity (the global entity-delete shortcut honors it).
fn timeline_delete_guard(
    root: Query<&RelativeCursorPosition, With<AnimTimeline>>,
    selected: Res<SelectedKey>,
    focus: Option<ResMut<renzora::InputFocusState>>,
) {
    let active = selected.0.is_some() && root.iter().any(|r| r.cursor_over);
    if let Some(mut focus) = focus {
        if focus.suppress_entity_delete != active {
            focus.suppress_entity_delete = active;
        }
    }
}

/// Mouse wheel over the timeline → zoom (matches the toolbar zoom buttons).
fn timeline_wheel_zoom(
    mut wheel: MessageReader<MouseWheel>,
    root: Query<&RelativeCursorPosition, With<AnimTimeline>>,
    state: Option<Res<AnimationEditorState>>,
    bridge: Option<Res<AnimEditorBridge>>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy == 0.0 {
        return;
    }
    if !root.iter().any(|r| r.cursor_over) {
        return;
    }
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    let factor = 1.15f32.powf(dy);
    push(
        &bridge,
        AnimEditorAction::SetTimelineZoom((state.timeline_zoom * factor).clamp(20.0, 500.0)),
    );
}

fn clip_combo_open(
    q: Query<(&Interaction, &RelativeCursorPosition, &ComputedNode), (With<ClipCombo>, Changed<Interaction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<AnimationEditorState>>,
    animators: Query<&AnimatorComponent>,
    mut commands: Commands,
) {
    let (Some(fonts), Some(state)) = (fonts, state) else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else { return };
    let Some(entity) = state.selected_entity else { return };
    let Ok(animator) = animators.get(entity) else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let default_clip = animator.default_clip.clone();
    let kids: Vec<Entity> = animator
        .clips
        .iter()
        .map(|slot| {
            let name = slot.name.clone();
            let looping = slot.looping;
            let speed = slot.speed;
            let label = if default_clip.as_deref() == Some(&slot.name) { format!("{} {}", slot.name, renzora::lang::t("animation.default_suffix")) } else { slot.name.clone() };
            menu_item(&mut commands, &fonts, "film-strip", &label, move |w| {
                if let Some(bridge) = w.get_resource::<AnimEditorBridge>() {
                    if let Ok(mut p) = bridge.pending.lock() {
                        p.push(AnimEditorAction::SelectClip(Some(name.clone())));
                    }
                }
                if let Some(mut queue) = w.get_resource_mut::<renzora_animation::AnimationCommandQueue>() {
                    queue.commands.push(renzora_animation::AnimationCommand::Play { entity, name: name.clone(), looping, speed });
                }
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

// ── Property tracks: world ops, preview, record ─────────────────────────────────

/// Tracks the last (entity, time) the scrub preview wrote, so the preview only
/// drives the object when playing or when the playhead actually moved — leaving
/// a stationary playhead free to pose the object for new keyframes.
#[derive(Resource, Default)]
struct PreviewApplied {
    time: f32,
    entity: Option<Entity>,
    valid: bool,
    /// Last time (seconds) diagnostics were logged, to throttle them.
    last_log: f32,
    /// What the preview last wrote per track (aligned with the clip's property
    /// tracks). Used to detect a manual pose (current values diverging from this)
    /// so the preview stops fighting the user's gizmo/inspector edits.
    written: Vec<Option<TrackValue>>,
}

/// Record session baselines: the last live value seen per property track,
/// used to detect a user edit (vs. the sampler's own writes) while recording.
#[derive(Resource, Default)]
struct RecordState {
    session: Option<(Entity, Option<String>)>,
    baselines: std::collections::HashMap<usize, TrackValue>,
}

/// Upsert a keyframe at `time` on a property track (replace if one already sits
/// within epsilon, else insert + re-sort).
fn upsert_key(pt: &mut PropertyTrack, time: f32, value: TrackValue) {
    const EPS: f32 = 1e-4;
    if let Some(k) = pt.keys.iter_mut().find(|k| (k.time - time).abs() < EPS) {
        k.value = value;
    } else {
        pt.keys.push(PropertyKey { time, value, interp: renzora::Interp::Linear });
        pt.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    }
}

fn track_value_close(a: &TrackValue, b: &TrackValue) -> bool {
    const EPS: f32 = 1e-4;
    let close = |x: &[f32], y: &[f32]| x.iter().zip(y).all(|(a, b)| (a - b).abs() < EPS);
    match (a, b) {
        (TrackValue::Float(x), TrackValue::Float(y)) => (x - y).abs() < EPS,
        (TrackValue::Vec3(x), TrackValue::Vec3(y)) => close(x, y),
        (TrackValue::Color(x), TrackValue::Color(y)) => close(x, y),
        (TrackValue::Quat(x), TrackValue::Quat(y)) => close(x, y),
        (TrackValue::Bool(x), TrackValue::Bool(y)) => x == y,
        _ => false,
    }
}

/// Detect clicks on a property track's dropdown / delete button (built inside
/// the keyed header list) and queue the corresponding world op.
fn prop_header_click(
    combos: Query<(&Interaction, &RelativeCursorPosition, &ComputedNode, &PropTrackCombo), Changed<Interaction>>,
    dels: Query<(&Interaction, &DeletePropTrack), Changed<Interaction>>,
    addks: Query<(&Interaction, &AddKeyTrackBtn), Changed<Interaction>>,
    add_track: Query<&Interaction, (With<AddTrackBtn>, Changed<Interaction>)>,
    windows: Query<&Window>,
    mut ops: ResMut<TimelineOps>,
) {
    if add_track.iter().any(|i| *i == Interaction::Pressed) {
        ops.add_empty_track = true;
    }
    for (interaction, rcp, cn, combo) in &combos {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let cursor = windows.iter().find_map(|w| w.cursor_position()).unwrap_or(Vec2::splat(200.0));
        // Anchor the menu just below the combo.
        let size = cn.size() * cn.inverse_scale_factor();
        let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
        ops.open_property_menu = Some((combo.0, Vec2::new(top_left.x, top_left.y + size.y + 2.0)));
    }
    for (interaction, del) in &dels {
        if *interaction == Interaction::Pressed {
            ops.delete_track = Some(del.0);
        }
    }
    for (interaction, addk) in &addks {
        if *interaction == Interaction::Pressed {
            ops.add_key_track = Some(addk.0);
        }
    }
}

/// Apply deferred ops that need full world access (reflection): add/delete a
/// property track, open a track's property picker, or key all tracks.
fn apply_timeline_ops(world: &mut World) {
    let (add_empty, add_key, open_menu, delete, add_key_track, delete_key) = {
        let Some(o) = world.get_resource::<TimelineOps>() else { return };
        (o.add_empty_track, o.add_key, o.open_property_menu, o.delete_track, o.add_key_track, o.delete_selected_key)
    };
    if !add_empty && !add_key && open_menu.is_none() && delete.is_none() && add_key_track.is_none() && !delete_key {
        return;
    }
    if let Some(mut o) = world.get_resource_mut::<TimelineOps>() {
        o.add_empty_track = false;
        o.add_key = false;
        o.open_property_menu = None;
        o.delete_track = None;
        o.add_key_track = None;
        o.delete_selected_key = false;
    }

    if delete_key {
        if let Some(sel) = world.get_resource::<SelectedKey>().and_then(|s| s.0) {
            if let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() {
                if let Some(mut chan) = cache.lane_times(sel.lane) {
                    chan.remove(sel.index);
                }
                cache.dirty = true;
            }
            if let Some(mut s) = world.get_resource_mut::<SelectedKey>() {
                s.0 = None;
            }
        }
    }

    if add_empty {
        add_empty_property_track(world);
    }
    if let Some(track) = delete {
        delete_property_track(world, track);
    }
    if add_key {
        let entity = clip_entity(world);
        let scrub = world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time).unwrap_or(0.0);
        if let Some(entity) = entity {
            insert_key_all_tracks(world, entity, scrub);
        }
    }
    if let Some(track) = add_key_track {
        let entity = clip_entity(world);
        let scrub = world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time).unwrap_or(0.0);
        add_key_at(world, entity, track, scrub);
    }
    if let Some((track, pos)) = open_menu {
        if let Some(entity) = clip_entity(world) {
            open_property_menu_for_track(world, entity, track, pos);
        }
    }
}

/// Mirror the open clip's state into the shared [`renzora::ActiveTimeline`] so
/// the inspector can show per-property keyframe buttons without linking this
/// crate. Uses `cache.key` (not `selected_entity`) so the published entity stays
/// consistent with the loaded track buffer during the one frame they differ on a
/// selection change. Cheap — clones only the bound tracks' identifier strings.
fn publish_active_timeline(
    clip: Res<NativeAnimClip>,
    state: Res<AnimationEditorState>,
    mut active: ResMut<renzora::ActiveTimeline>,
) {
    match clip.clip.as_ref() {
        Some(c) => {
            active.entity = clip.key.as_ref().map(|(e, _)| *e);
            active.scrub_time = state.scrub_time;
            active.tracks = c
                .property_tracks
                .iter()
                .filter(|t| !t.component.is_empty() && !t.field.is_empty())
                .map(|t| (t.component.clone(), t.field.clone()))
                .collect();
        }
        // No clip open → the timeline isn't active. Clear (only touch the
        // resource when it actually needs clearing, to avoid change spam).
        None if active.entity.is_some() || !active.tracks.is_empty() => {
            active.entity = None;
            active.tracks.clear();
        }
        None => {}
    }
}

/// Drain inspector-posted keyframe requests: for each, find the matching bound
/// track on the open clip and key the entity's current live value at the
/// playhead. Exclusive because reading a live property value goes through
/// reflection (`read_track_value`, inside `add_key_at`). `add_key_at` keys from
/// the *track's own* canonical `(component, field)`, so the inspector's guessed
/// request strings only need to locate the track, not drive reflection.
fn apply_keyframe_requests(world: &mut World) {
    let reqs = match world.get_resource_mut::<renzora::KeyframeRequests>() {
        Some(mut r) if !r.is_empty() => r.drain(),
        _ => return,
    };
    if cur_clip(world).is_none() {
        return;
    }
    let entity = clip_entity(world);
    let scrub = world
        .get_resource::<AnimationEditorState>()
        .map(|s| s.scrub_time)
        .unwrap_or(0.0);
    for req in reqs {
        let track = world
            .get_resource::<NativeAnimClip>()
            .and_then(|c| c.clip.as_ref())
            .and_then(|c| {
                c.property_tracks.iter().position(|t| {
                    renzora::norm(&t.component) == renzora::norm(&req.component)
                        && renzora::norm(&t.field) == renzora::norm(&req.field)
                })
            });
        match track {
            Some(idx) => add_key_at(world, entity, idx, scrub),
            None => warn!(
                "[prop-anim] Inspector keyframe: no bound track for {}.{}",
                req.component, req.field
            ),
        }
    }
}

/// Read the live value of every property track and key it at `time`.
fn insert_key_all_tracks(world: &mut World, entity: Entity, time: f32) {
    let tracks: Vec<PropertyTrack> =
        match world.get_resource::<NativeAnimClip>().and_then(|c| c.clip.as_ref()) {
            Some(clip) if !clip.property_tracks.is_empty() => clip.property_tracks.clone(),
            _ => return,
        };
    let values: Vec<Option<TrackValue>> =
        tracks.iter().map(|t| read_track_value(world, entity, t)).collect();
    for (pi, (t, v)) in tracks.iter().zip(&values).enumerate() {
        info!(
            "[prop-anim] Add Key: track {} {}.{} @ t={:.3} from {:?} -> {:?}",
            pi, t.component, t.field, time, entity, v
        );
    }
    if let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() {
        if let Some(clip) = cache.clip.as_mut() {
            let mut any = false;
            for (pi, val) in values.into_iter().enumerate() {
                let Some(val) = val else { continue };
                if let Some(pt) = clip.property_tracks.get_mut(pi) {
                    upsert_key(pt, time, val);
                    any = true;
                }
            }
            cache.dirty = cache.dirty || any;
        }
    }
}

/// Append a new empty property track (the user picks its property via the
/// in-row dropdown afterward).
fn add_empty_property_track(world: &mut World) {
    let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() else { return };
    let Some(clip) = cache.clip.as_mut() else { return };
    clip.property_tracks.push(PropertyTrack {
        target: "self".into(),
        component: String::new(),
        field: String::new(),
        keys: Vec::new(),
    });
    cache.dirty = true;
}

/// Remove the property track at `track`, clearing any selection on it.
fn delete_property_track(world: &mut World, track: usize) {
    if let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() {
        if let Some(clip) = cache.clip.as_mut() {
            if track < clip.property_tracks.len() {
                clip.property_tracks.remove(track);
                cache.dirty = true;
            }
        }
    }
    if let Some(mut sel) = world.get_resource_mut::<SelectedKey>() {
        if matches!(sel.0, Some(SelKey { lane: Lane::Prop { .. }, .. })) {
            sel.0 = None;
        }
    }
}

/// Bind a property track to `component.field` (deduped). Clears existing keys
/// since the value type may change.
fn set_property_track(world: &mut World, track: usize, component: &str, field: &str) {
    let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() else { return };
    let Some(clip) = cache.clip.as_mut() else { return };
    let dup = clip
        .property_tracks
        .iter()
        .enumerate()
        .any(|(i, t)| i != track && t.component == component && t.field == field);
    if dup {
        return;
    }
    if let Some(pt) = clip.property_tracks.get_mut(track) {
        pt.component = component.to_string();
        pt.field = field.to_string();
        pt.keys.clear();
    }
    cache.dirty = true;
}

/// Open the per-track property picker, listing the entity's animatable fields
/// minus those already bound on other tracks (no duplicates).
fn open_property_menu_for_track(world: &mut World, entity: Entity, track: usize, pos: Vec2) {
    let fields = list_animatable_fields(world, entity);
    if fields.is_empty() {
        return;
    }
    // Fields already bound on OTHER tracks are excluded.
    let used: std::collections::HashSet<(String, String)> = world
        .get_resource::<NativeAnimClip>()
        .and_then(|c| c.clip.as_ref())
        .map(|clip| {
            clip.property_tracks
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != track)
                .map(|(_, t)| (t.component.clone(), t.field.clone()))
                .collect()
        })
        .unwrap_or_default();
    let avail: Vec<_> = fields
        .into_iter()
        .filter(|f| !used.contains(&(f.component.clone(), f.field.clone())))
        .collect();
    if avail.is_empty() {
        return;
    }

    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let menu = screen_menu(&mut commands, pos.x, pos.y);
        let kids: Vec<Entity> = avail
            .into_iter()
            .map(|f| {
                let label = format!("{} · {}", title_case(&f.component), f.label);
                let component = f.component;
                let field = f.field;
                menu_item(&mut commands, &fonts, "sliders-horizontal", &label, move |w| {
                    set_property_track(w, track, &component, &field);
                })
            })
            .collect();
        commands.entity(menu).add_children(&kids);
    }
    queue.apply(world);
}

/// Scrub preview: sample the in-memory property tracks at the playhead and write
/// the values onto the selected entity so scrubbing (and Play) animates it live.
/// The playhead itself is advanced by `advance_preview_time` (lib.rs). Suppressed
/// while recording so the user's edits aren't overwritten.
fn preview_property_animation(world: &mut World) {
    let (scrub, record, previewing) = {
        let Some(s) = world.get_resource::<AnimationEditorState>() else { return };
        (s.scrub_time, s.record_enabled, s.is_previewing)
    };
    let Some(entity) = clip_entity(world) else { return };
    if record {
        return;
    }
    // Only drive the entity while playing or when the playhead actually moved.
    // A stationary playhead leaves the object free to be posed for new keys —
    // otherwise the preview would overwrite the user's edit every frame and
    // every captured key would be identical.
    let moved = world.get_resource::<PreviewApplied>().is_none_or(|p| {
        !p.valid || p.entity != Some(entity) || (p.time - scrub).abs() > 1e-6
    });
    if !previewing && !moved {
        return;
    }
    let tracks: Vec<PropertyTrack> =
        match world.get_resource::<NativeAnimClip>().and_then(|c| c.clip.as_ref()) {
            Some(clip) if !clip.property_tracks.is_empty() => clip.property_tracks.clone(),
            _ => return,
        };

    // Manual-pose detection: if the entity's current values diverge from what
    // the preview last wrote, the user posed it (gizmo or inspector). Pause
    // playback, adopt the pose as the new baseline and DON'T overwrite it — so
    // the edit sticks and can be keyed (this is why captures were grabbing the
    // preview's value instead of the user's rotation).
    let (written, pa_entity, pa_valid) = world
        .get_resource::<PreviewApplied>()
        .map(|p| (p.written.clone(), p.entity, p.valid))
        .unwrap_or_default();
    if pa_valid && pa_entity == Some(entity) && written.len() == tracks.len() {
        let current: Vec<Option<TrackValue>> =
            tracks.iter().map(|t| read_track_value(world, entity, t)).collect();
        let manual = current.iter().zip(&written).any(|(c, w)| match (c, w) {
            (Some(cv), Some(wv)) => !track_value_close(cv, wv),
            _ => false,
        });
        if manual {
            for (i, (c, w)) in current.iter().zip(&written).enumerate() {
                if let (Some(cv), Some(wv)) = (c, w) {
                    if !track_value_close(cv, wv) {
                        info!(
                            "[prop-anim] pose changed on track {} -> {:?} (not yet keyed — Add Key / right-click 'Set to current pose', or select a key first to live-edit it)",
                            i, cv
                        );
                    }
                }
            }
            if previewing {
                if let Some(mut s) = world.get_resource_mut::<AnimationEditorState>() {
                    s.is_previewing = false;
                }
            }
            if let Some(mut pa) = world.get_resource_mut::<PreviewApplied>() {
                pa.time = scrub;
                pa.entity = Some(entity);
                pa.valid = true;
                pa.written = current;
            }
            return;
        }
    }

    let moved = world.get_resource::<PreviewApplied>().is_none_or(|p| {
        !p.valid || p.entity != Some(entity) || (p.time - scrub).abs() > 1e-6
    });
    if !previewing && !moved {
        return;
    }

    // Throttle diagnostics to ~2×/sec, and only while actually playing.
    let now = world.resource::<Time>().elapsed_secs();
    let mut verbose = false;
    if previewing {
        if let Some(pa) = world.get_resource::<PreviewApplied>() {
            if now - pa.last_log > 0.5 {
                verbose = true;
            }
        }
    }
    apply_property_tracks(world, entity, &tracks, scrub, verbose);
    // Record what actually LANDED on the entity (read-back), not the raw
    // sampled values: fields that quantize on write — `SpriteSheet.frame` is a
    // u32, so a sampled 2.37 lands as 2 — would otherwise diverge from the
    // next frame's read in the manual-pose check above, which reads as "the
    // user posed it" and pauses playback on the very first Play frame.
    let written_now: Vec<Option<TrackValue>> =
        tracks.iter().map(|t| read_track_value(world, entity, t)).collect();
    if let Some(mut pa) = world.get_resource_mut::<PreviewApplied>() {
        pa.time = scrub;
        pa.entity = Some(entity);
        pa.valid = true;
        pa.written = written_now;
        if verbose {
            pa.last_log = now;
        }
    }
}

/// Live-edit: when a property keyframe is selected and the playhead sits on it,
/// posing the entity updates that keyframe's value. This makes the intuitive
/// "click a key, then move the object to set it" workflow work.
fn live_edit_selected_key(world: &mut World) {
    let (scrub, record, previewing, sel, dragging) = {
        let Some(s) = world.get_resource::<AnimationEditorState>() else { return };
        let sel = world.get_resource::<SelectedKey>().and_then(|x| x.0);
        let dragging = world.get_resource::<KeyDragState>().is_some_and(|d| d.active.is_some());
        (s.scrub_time, s.record_enabled, s.is_previewing, sel, dragging)
    };
    let Some(entity) = clip_entity(world) else { return };
    if record || previewing || dragging {
        return;
    }
    let Some(SelKey { lane: Lane::Prop { track }, index }) = sel else { return };
    let track_data = world
        .get_resource::<NativeAnimClip>()
        .and_then(|c| c.clip.as_ref())
        .and_then(|c| c.property_tracks.get(track))
        .cloned();
    let Some(track_data) = track_data else { return };
    let Some(stored) = track_data.keys.get(index).map(|k| k.value) else { return };
    let key_time = track_data.keys[index].time;
    // Only when the playhead is on the key (so the preview is showing it); a
    // little slack covers snap/float drift after the select-jump.
    if (key_time - scrub).abs() > 0.05 {
        return;
    }
    let Some(live) = read_track_value(world, entity, &track_data) else { return };
    if track_value_close(&stored, &live) {
        return;
    }
    info!(
        "[prop-anim] live-edit selected key {} of {}.{}: {:?} -> {:?}",
        index, track_data.component, track_data.field, stored, live
    );
    if let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() {
        if let Some(key) = cache
            .clip
            .as_mut()
            .and_then(|c| c.property_tracks.get_mut(track))
            .and_then(|pt| pt.keys.get_mut(index))
        {
            key.value = live;
        }
        cache.dirty = true;
    }
}

/// Record capture: while armed, detect user edits to tracked fields (live value
/// diverging from the per-track baseline) and auto-key them at the playhead.
fn record_capture(world: &mut World) {
    let Some(state) = world.get_resource::<AnimationEditorState>() else { return };
    if !state.record_enabled {
        return;
    }
    let scrub = state.scrub_time;
    let clip_name = state.selected_clip.clone();
    let Some(entity) = clip_entity(world) else { return };
    let tracks: Vec<PropertyTrack> =
        match world.get_resource::<NativeAnimClip>().and_then(|c| c.clip.as_ref()) {
            Some(clip) if !clip.property_tracks.is_empty() => clip.property_tracks.clone(),
            _ => return,
        };

    // Reset baselines when the record target changes.
    {
        let mut rec = world.get_resource_or_insert_with(RecordState::default);
        if rec.session.as_ref() != Some(&(entity, clip_name.clone())) {
            rec.session = Some((entity, clip_name.clone()));
            rec.baselines.clear();
        }
    }

    let mut captures: Vec<(usize, TrackValue)> = Vec::new();
    for (pi, track) in tracks.iter().enumerate() {
        let Some(live) = read_track_value(world, entity, track) else { continue };
        let mut rec = world.get_resource_mut::<RecordState>().unwrap();
        match rec.baselines.get(&pi).copied() {
            None => {
                rec.baselines.insert(pi, live);
            }
            Some(base) => {
                if !track_value_close(&base, &live) {
                    rec.baselines.insert(pi, live);
                    captures.push((pi, live));
                }
            }
        }
    }
    if captures.is_empty() {
        return;
    }
    if let Some(mut cache) = world.get_resource_mut::<NativeAnimClip>() {
        if let Some(clip) = cache.clip.as_mut() {
            for (pi, val) in captures {
                if let Some(pt) = clip.property_tracks.get_mut(pi) {
                    info!("[prop-anim] Record: keyed track {} @ t={:.3} -> {:?}", pi, scrub, val);
                    upsert_key(pt, scrub, val);
                }
            }
            cache.dirty = true;
        }
    }
}

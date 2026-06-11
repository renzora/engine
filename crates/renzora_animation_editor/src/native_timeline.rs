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

use renzora_animation::{AnimClip, AnimatorComponent};
use renzora_editor_framework::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{menu_item, screen_menu, timeline_view, TimelineView};

use crate::{AnimEditorAction, AnimEditorBridge, AnimationEditorState};

const TRANSLATION: (u8, u8, u8) = (100, 149, 237);
const ROTATION: (u8, u8, u8) = (120, 200, 120);
const SCALE: (u8, u8, u8) = (200, 120, 120);
const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

pub struct NativeAnimTimeline;

impl Plugin for NativeAnimTimeline {
    fn build(&self, app: &mut App) {
        app.init_resource::<NativeAnimClip>();
        app.init_resource::<KeyDragState>();
        app.register_panel_content("timeline", false, build);
        app.add_systems(
            Update,
            (
                cache_native_clip,
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
                timeline_wheel_zoom,
            )
                .chain()
                .run_if(in_state(SplashState::Editor)),
        );
    }
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
    /// Mutable access to one channel's `(time, value-len)` key vector, erased
    /// over the channel value types via the times-only view the editor needs.
    fn channel_times(&mut self, ti: usize, ch: u8) -> Option<ChannelTimes<'_>> {
        let track = self.clip.as_mut()?.tracks.get_mut(ti)?;
        Some(match ch {
            0 => ChannelTimes::T(&mut track.translations),
            1 => ChannelTimes::R(&mut track.rotations),
            _ => ChannelTimes::S(&mut track.scales),
        })
    }
}

/// Borrowed view over a single keyframe channel — lets the drag/delete systems
/// edit times without caring about the per-channel value payload type.
enum ChannelTimes<'a> {
    T(&'a mut Vec<(f32, [f32; 3])>),
    R(&'a mut Vec<(f32, [f32; 4])>),
    S(&'a mut Vec<(f32, [f32; 3])>),
}

impl ChannelTimes<'_> {
    fn time(&self, idx: usize) -> Option<f32> {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => v.get(idx).map(|k| k.0),
            ChannelTimes::R(v) => v.get(idx).map(|k| k.0),
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
        }
    }
    fn sort(&mut self) {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => {
                v.sort_by(|a, b| a.0.total_cmp(&b.0));
            }
            ChannelTimes::R(v) => v.sort_by(|a, b| a.0.total_cmp(&b.0)),
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
    track: usize,
    channel: u8,
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
}
#[derive(Component)]
struct SpeedBtn(f32);
#[derive(Component)]
struct ClipCombo;
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

/// Whether the timeline has a clip to show (vs an empty-state message).
fn ready(w: &World) -> bool {
    cur_clip(w).is_some()
}

fn empty_msg(w: &World) -> String {
    let Some(s) = state(w) else { return String::new() };
    if s.selected_entity.is_none() {
        "Select an animated entity in the Hierarchy".into()
    } else if s
        .selected_entity
        .and_then(|e| w.get::<AnimatorComponent>(e))
        .is_none_or(|a| a.clips.is_empty())
    {
        "No clips on this entity — use \"Scan for clips\" in the Animation panel".into()
    } else if s.selected_clip.is_none() {
        "Choose a clip from the toolbar's clip menu above".into()
    } else {
        "Loading clip…".into()
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

    // Empty-state note.
    let note = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() })
        .id();
    let note_lbl = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::new_with_justify(bevy::text::Justify::Center)))
        .id();
    bind_text(commands, note_lbl, empty_msg);
    commands.entity(note).add_child(note_lbl);
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

    let htitle = commands.spawn((Text::new("Bones"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    commands.entity(tl.header_corner).add_child(htitle);
    keyed_list(commands, tl.header_list, header_snapshot);
    keyed_list(commands, tl.clips, keyframe_snapshot);

    commands.entity(body).add_children(&[note, tl.root]);
    commands.entity(root).add_children(&[toolbar, body]);
    root
}

fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(26.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), padding: UiRect::horizontal(Val::Px(6.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() },
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
    let combo_v = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { min_width: Val::Px(96.0), max_width: Val::Px(150.0), overflow: Overflow::clip(), ..default() }, bevy::text::TextLayout::new_with_no_wrap())).id();
    bind_text(commands, combo_v, |w| state(w).and_then(|s| s.selected_clip.clone()).unwrap_or_else(|| "Select clip…".into()));
    let combo_c = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(combo).add_children(&[combo_v, combo_c]);
    let _ = clip_ic;

    let sep3 = vsep(commands);

    // Speed presets.
    let speed_lbl = commands.spawn((Text::new("Speed"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
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

    // Save — accent-colored while there are unsaved keyframe edits.
    let (save_b, save_ic) = icon_btn(commands, fonts, "floppy-disk", text_muted(), SaveClipBtn);
    bind_text_color(commands, save_ic, |w| {
        let dirty = w.get_resource::<NativeAnimClip>().is_some_and(|c| c.dirty);
        rgb(if dirty { accent() } else { text_muted() })
    });

    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();

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

    let mut kids = vec![skip_back, step_back, play, stop, step_fwd, skip_fwd, sep1, loop_b, sep2, combo, sep3, speed_lbl];
    kids.extend(speed_btns);
    kids.extend([sep4, snap_b, save_b, gap, time, zoom_out, zoom_lbl, zoom_in]);
    commands.entity(bar).add_children(&kids);
    bar
}

fn icon_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8), marker: M) -> (Entity, Entity) {
    let btn = commands
        .spawn((Node { width: Val::Px(22.0), height: Val::Px(20.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), marker))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 13.0);
    commands.entity(btn).add_child(ic);
    (btn, ic)
}

fn vsep(commands: &mut Commands) -> Entity {
    commands.spawn((Node { width: Val::Px(1.0), height: Val::Px(16.0), margin: UiRect::horizontal(Val::Px(2.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(border())))).id()
}

// ── Snapshots ──────────────────────────────────────────────────────────────────

fn header_snapshot(world: &World) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world) else { return empty() };
    let th = state(world).map(|s| s.track_height).unwrap_or(22.0);
    // (bone, has_t, has_r, has_s)
    let rows: Vec<(String, bool, bool, bool)> = clip
        .tracks
        .iter()
        .map(|t| (t.bone_name.clone(), !t.translations.is_empty(), !t.rotations.is_empty(), !t.scales.is_empty()))
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, (name, ht, hr, hs))| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (name, ht, hr, hs, th.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, ht, hr, hs) = &rows[i];
            header_row(c, f, i, name, *ht, *hr, *hs, th)
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn header_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str, ht: bool, hr: bool, hs: bool, th: f32) -> Entity {
    let bg = if idx.is_multiple_of(2) { row_even() } else { row_odd() };
    let row = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(th), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(rgb(bg))))
        .id();
    let bone = icon_text(commands, &fonts.phosphor, "bone", text_muted(), 10.0);
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::new_with_no_wrap(), Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() })).id();
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
    /// A lone (editable) keyframe: (track, channel, key index, time).
    Key(usize, u8, usize, f32),
    /// A run of keys denser than the cluster threshold, drawn as one bar:
    /// (track, channel, first time, last time, count).
    Bar(usize, u8, f32, f32, usize),
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
/// the visible window.
fn cluster_channel(
    out: &mut Vec<KeyElem>,
    ti: usize,
    ch: u8,
    times: impl Iterator<Item = f32>,
    zoom: f32,
    t_min: f32,
    t_max: f32,
) {
    let gap = CLUSTER_PX / zoom;
    // (first index, first time, last time, count) of the open cluster.
    let mut run: Option<(usize, f32, f32, usize)> = None;
    let flush = |out: &mut Vec<KeyElem>, run: (usize, f32, f32, usize)| {
        let (i0, t0, t1, n) = run;
        if n >= 3 {
            out.push(KeyElem::Bar(ti, ch, t0, t1, n));
        } else {
            for k in 0..n {
                // 1–2 keys: emit individually.
                let t = if k == 0 { t0 } else { t1 };
                out.push(KeyElem::Key(ti, ch, i0 + k, t));
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

fn keyframe_snapshot(world: &World) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world) else { return empty() };
    let Some(s) = state(world) else { return empty() };
    let (zoom, scroll, th) = (s.timeline_zoom, s.timeline_scroll, s.track_height);
    let t_min = scroll - CULL_MARGIN_PX / zoom;
    let t_max = scroll + (MAX_LANE_PX + CULL_MARGIN_PX) / zoom;

    let mut elems: Vec<KeyElem> = Vec::new();
    for (ti, track) in clip.tracks.iter().enumerate() {
        cluster_channel(&mut elems, ti, 0, track.translations.iter().map(|k| k.0), zoom, t_min, t_max);
        cluster_channel(&mut elems, ti, 1, track.rotations.iter().map(|k| k.0), zoom, t_min, t_max);
        cluster_channel(&mut elems, ti, 2, track.scales.iter().map(|k| k.0), zoom, t_min, t_max);
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
            }
            (zoom.to_bits(), scroll.to_bits(), th.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| match elems[i] {
            KeyElem::Key(ti, ch, idx, time) => diamond(c, ti, ch, idx, time, zoom, scroll, th),
            KeyElem::Bar(ti, ch, t0, t1, _) => key_bar(c, ti, ch, t0, t1, zoom, scroll, th),
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
fn diamond(commands: &mut Commands, ti: usize, ch: u8, idx: usize, time: f32, zoom: f32, scroll: f32, th: f32) -> Entity {
    let kf = (th * 0.38).clamp(4.0, 14.0);
    let half = kf * 0.5;
    let (y, color) = channel_y(ti, ch, th);
    let x = (time - scroll) * zoom;
    let _ = idx;
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(x - half), top: Val::Px(y - half), width: Val::Px(kf), height: Val::Px(kf), ..default() },
            BackgroundColor(rgb(color)),
            UiTransform::from_rotation(Rot2::degrees(45.0)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id()
}

/// A dense run of keys drawn as one slim rounded bar in the channel color.
#[allow(clippy::too_many_arguments)]
fn key_bar(commands: &mut Commands, ti: usize, ch: u8, t0: f32, t1: f32, zoom: f32, scroll: f32, th: f32) -> Entity {
    let h = (th * 0.22).clamp(3.0, 8.0);
    let (y, color) = channel_y(ti, ch, th);
    let x0 = (t0 - scroll) * zoom;
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
) {
    let key = match (state.selected_entity, state.selected_clip.as_deref()) {
        (Some(e), Some(c)) => Some((e, c.to_string())),
        _ => None,
    };
    if key == cache.key {
        return;
    }
    if cache.dirty {
        warn!("[timeline] discarding unsaved keyframe edits (clip selection changed)");
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
    let tracks = clip.map(|c| c.tracks.len()).unwrap_or(0);
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
    track: usize,
    channel: u8,
    index: usize,
    time: f32,
}

/// Find the editable keyframe nearest to a lane-space point. Keys rendered as
/// cluster bars (runs of 3+ within the cluster gap) are not pickable — zoom in
/// until they split into diamonds.
fn pick_key(clip: &AnimClip, zoom: f32, scroll: f32, th: f32, p: Vec2) -> Option<PickedKey> {
    let radius = (th * 0.30).clamp(5.0, 10.0);
    let gap = CLUSTER_PX / zoom.max(1.0);
    let mut best: Option<(f32, PickedKey)> = None;

    let mut scan = |ti: usize, ch: u8, times: &[f32]| {
        let (y, _) = channel_y(ti, ch, th);
        let dy = (p.y - y).abs();
        if dy > radius {
            return;
        }
        for (idx, &t) in times.iter().enumerate() {
            let dx = ((t - scroll) * zoom - p.x).abs();
            if dx > radius {
                continue;
            }
            // Cluster membership: runs of 3+ render as bars, not diamonds.
            let mut lo = idx;
            while lo > 0 && times[lo] - times[lo - 1] <= gap && idx - lo < 3 {
                lo -= 1;
            }
            let mut hi = idx;
            while hi + 1 < times.len() && times[hi + 1] - times[hi] <= gap && hi - lo < 3 {
                hi += 1;
            }
            if hi - lo + 1 > 2 {
                continue;
            }
            let score = dx.max(dy);
            if best.as_ref().is_none_or(|(s, _)| score < *s) {
                best = Some((score, PickedKey { track: ti, channel: ch, index: idx, time: t }));
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
        scan(ti, 0, &t_times);
        scan(ti, 1, &r_times);
        scan(ti, 2, &s_times);
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
) {
    let Some(state) = state else { return };

    if drag.active.is_some() {
        if !buttons.pressed(MouseButton::Left) {
            // Drag ended — restore sorted key order for playback.
            if let Some(d) = drag.active.take() {
                if d.moved {
                    if let Some(mut chan) = cache.channel_times(d.track, d.channel) {
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
        let (ti, ch, idx) = (d.track, d.channel, d.index);
        if let Some(dur) = cache.clip.as_ref().map(|c| c.duration) {
            t = t.min(dur);
        }
        let mut changed = false;
        if let Some(mut chan) = cache.channel_times(ti, ch) {
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

    // Begin a drag when the press lands on an editable key.
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(p) = lane_cursor(&lane) else { return };
    let Some(clip) = cache.clip.as_ref() else { return };
    let Some(pick) = pick_key(clip, state.timeline_zoom, state.timeline_scroll, state.track_height, p)
    else {
        return;
    };
    drag.active = Some(KeyDrag {
        track: pick.track,
        channel: pick.channel,
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
    let Some(pick) = pick_key(clip, state.timeline_zoom, state.timeline_scroll, state.track_height, p)
    else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let (ti, ch, idx) = (pick.track, pick.channel, pick.index);
    let del = menu_item(&mut commands, &fonts, "trash", "Delete keyframe", move |w| {
        if let Some(mut cache) = w.get_resource_mut::<NativeAnimClip>() {
            if let Some(mut chan) = cache.channel_times(ti, ch) {
                chan.remove(idx);
            }
            cache.dirty = true;
        }
    });
    commands.entity(menu).add_children(&[del]);
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
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };
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
            let label = if default_clip.as_deref() == Some(&slot.name) { format!("{} (default)", slot.name) } else { slot.name.clone() };
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

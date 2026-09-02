//! Picking, dragging and the right-click menus — everything the pointer and the
//! keyboard do to the dopesheet.
//!
//! Keyframe picking is done by **math against the clip data** (cursor →
//! time/track), not by per-diamond `Interaction`: the timeline widget's scrub
//! overlay sits above the clips layer and would swallow per-node hits. A drag is
//! likewise tracked against the raw mouse button, because the keyed list rebuilds
//! the dragged diamond the moment its time changes.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::AnimMarker;
use renzora_animation::{AnimClip, AnimatorComponent};
use renzora_editor_framework::EditorCommands;
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{menu_item, screen_menu, EmberTextInput, TimelineView, LANE_INSET};

use crate::{AnimEditorAction, AnimEditorBridge, AnimationEditorState};

use super::clip::{clip_entity, Lane, SelKey, SelectedKey, TimelineClip};
use super::props::{add_key_at, interp_menu_choices, interp_label_tr, set_key_interp, set_key_to_live, TimelineOps};
use super::snapshots::{channel_y, CLUSTER_PX};
use super::{
    AddMarkerBtn, AnimBtn, AnimPlayIcon, AnimTimeline, ClipCombo, KeyLane, MarkerNameField,
    NewClipBtn, NewClipNameField,
};

pub(super) fn push(bridge: &AnimEditorBridge, action: AnimEditorAction) {
    if let Ok(mut p) = bridge.pending.lock() {
        p.push(action);
    }
}

pub(super) fn anim_btn_click(
    q: Query<(&Interaction, &AnimBtn), Changed<Interaction>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Option<Res<TimelineClip>>,
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

pub(super) fn anim_sync(
    mut q: Query<&mut TimelineView, With<AnimTimeline>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Option<Res<TimelineClip>>,
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

pub(super) fn update_anim_play_icon(
    state: Option<Res<AnimationEditorState>>,
    mut q: Query<&mut Text, With<AnimPlayIcon>>,
) {
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

// ── Picking ──────────────────────────────────────────────────────────────────

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

/// In-flight keyframe drag. Survives the keyed-list rebuilding the dragged
/// node mid-drag (the rebuild drops `Interaction` state, so the drag is
/// tracked against the raw mouse button instead).
#[derive(Resource, Default)]
pub(crate) struct KeyDragState {
    pub(super) active: Option<KeyDrag>,
}

pub(super) struct KeyDrag {
    lane: Lane,
    index: usize,
    start_cursor_x: f32,
    orig_time: f32,
    moved: bool,
}

/// Drag a keyframe diamond horizontally to retime it (snap-aware).
pub(super) fn key_drag(
    mut drag: ResMut<KeyDragState>,
    buttons: Res<ButtonInput<MouseButton>>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    state: Option<Res<AnimationEditorState>>,
    mut cache: ResMut<TimelineClip>,
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
pub(super) fn key_context_menu(
    buttons: Res<ButtonInput<MouseButton>>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Res<TimelineClip>,
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
                if let Some(mut cache) = w.get_resource_mut::<TimelineClip>() {
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
            if let Some(mut cache) = w.get_resource_mut::<TimelineClip>() {
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

// ── Toolbar actions and shortcuts ────────────────────────────────────────────

/// "Add Marker" button → add an event marker at the playhead, named from the
/// toolbar field (default "event").
pub(super) fn add_marker_click(
    q: Query<&Interaction, (With<AddMarkerBtn>, Changed<Interaction>)>,
    field: Query<&EmberTextInput, With<MarkerNameField>>,
    state: Option<Res<AnimationEditorState>>,
    mut cache: ResMut<TimelineClip>,
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
pub(super) fn new_clip_click(
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
pub(super) fn timeline_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    root: Query<&RelativeCursorPosition, With<AnimTimeline>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Option<Res<TimelineClip>>,
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
pub(super) fn timeline_delete_guard(
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
pub(super) fn timeline_wheel_zoom(
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

pub(super) fn clip_combo_open(
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

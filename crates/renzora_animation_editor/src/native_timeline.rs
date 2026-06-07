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
        app.register_panel_content("timeline", false, build);
        app.add_systems(
            Update,
            (
                cache_native_clip,
                anim_btn_click,
                speed_btn_click,
                clip_combo_open,
                anim_sync,
                update_anim_play_icon,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

/// Disk-loaded copy of the currently selected clip, reloaded when the
/// `(entity, clip)` selection changes. Drives the header + keyframe snapshots.
#[derive(Resource, Default)]
struct NativeAnimClip {
    key: Option<(Entity, String)>,
    clip: Option<AnimClip>,
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
        "Select an entity with animations".into()
    } else if s.selected_entity.and_then(|e| w.get::<AnimatorComponent>(e)).is_none() {
        "No AnimatorComponent on selected entity".into()
    } else if s.selected_entity.and_then(|e| w.get::<AnimatorComponent>(e)).is_some_and(|a| a.clips.is_empty()) {
        "No animation clips assigned".into()
    } else if s.selected_clip.is_none() {
        "Select a clip to edit".into()
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
    commands.entity(tl.root).insert(AnimTimeline);
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
    kids.extend([sep4, snap_b, gap, time, zoom_out, zoom_lbl, zoom_in]);
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

fn keyframe_snapshot(world: &World) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world) else { return empty() };
    let Some(s) = state(world) else { return empty() };
    let (zoom, scroll, th) = (s.timeline_zoom, s.timeline_scroll, s.track_height);
    // (track_idx, channel, time): channel 0=T, 1=R, 2=S
    let mut keys: Vec<(usize, u8, f32)> = Vec::new();
    for (ti, track) in clip.tracks.iter().enumerate() {
        for &(time, _) in &track.translations {
            keys.push((ti, 0, time));
        }
        for &(time, _) in &track.rotations {
            keys.push((ti, 1, time));
        }
        for &(time, _) in &track.scales {
            keys.push((ti, 2, time));
        }
    }
    let items: Vec<(u64, u64)> = keys
        .iter()
        .enumerate()
        .map(|(i, (ti, ch, time))| {
            let mut k = hasher();
            (i, ti, ch).hash(&mut k);
            let mut h = hasher();
            (time.to_bits(), zoom.to_bits(), scroll.to_bits(), th.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| {
            let (ti, ch, time) = keys[i];
            diamond(c, ti, ch, time, zoom, scroll, th)
        }),
    }
}

fn diamond(commands: &mut Commands, ti: usize, ch: u8, time: f32, zoom: f32, scroll: f32, th: f32) -> Entity {
    let kf = (th * 0.38).clamp(4.0, 14.0);
    let half = kf * 0.5;
    let off = (th * 0.26).min(14.0);
    let center = ti as f32 * th + th * 0.5;
    let (color, y) = match ch {
        0 => (TRANSLATION, center - off),
        1 => (ROTATION, center),
        _ => (SCALE, center + off),
    };
    let x = (time - scroll) * zoom;
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(x - half), top: Val::Px(y - half), width: Val::Px(kf), height: Val::Px(kf), ..default() },
            BackgroundColor(rgb(color)),
            UiTransform::from_rotation(Rot2::degrees(45.0)),
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
    cache.key = key.clone();
    cache.clip = None;
    let (Some((entity, clip_name)), Some(project)) = (key, project) else { return };
    let Ok(animator) = animators.get(entity) else { return };
    let Some(slot) = animator.clips.iter().find(|s| s.name == clip_name) else { return };
    let path = project.path.join(&slot.path);
    if let Ok(content) = std::fs::read_to_string(&path) {
        cache.clip = ron::from_str::<AnimClip>(&content).ok();
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
) {
    let Some(state) = state else { return };
    let clip = cache.as_ref().and_then(|c| c.clip.as_ref());
    let dur = clip.map(|c| c.duration).unwrap_or(2.0);
    let tracks = clip.map(|c| c.tracks.len()).unwrap_or(0);
    for mut v in &mut q {
        v.set_geom(state.timeline_zoom, state.timeline_scroll, state.scrub_time, dur, state.track_height, tracks);
        if let Some(t) = v.take_scrub() {
            if let Some(bridge) = &bridge {
                push(bridge, AnimEditorAction::SetScrubTime(t.clamp(0.0, dur)));
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

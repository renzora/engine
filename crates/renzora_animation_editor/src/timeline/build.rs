//! The panel's widget tree and its toolbar.
//!
//! One 40px row: the clip picker and record on the left, the transport in a
//! single pill with the time readout in the middle, playback speed, snap and the
//! Keys / Curves switch on the right. Clip length, zoom, events and Save moved
//! off the bar (length is the end handle in the ruler, zoom is Ctrl+wheel); the
//! rest of them live in the "⋯" menu at the far right, so nothing the old
//! 30-control bar could do was lost.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{
    bind_2way, bind_bg, bind_display, bind_text, bind_text_color, keyed_list, keyed_list_tokened,
};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{dropdown_compact, text_input, timeline_view, HoverTooltip};
use renzora_ember::UiInteraction;

use crate::{AnimEditorAction, AnimEditorBridge};

use super::clip::{cur_clip, ready, state};
use super::curves::{curves_snapshot, curves_token, TimelineMode};
use super::edit::push;
use super::menus::build_empty_state;
use super::snapshots::{
    header_snapshot, header_token, keyframe_snapshot, lanes_token, marker_snapshot, ruler_snapshot,
};
use super::{
    AddTrackBtn, AnimBtn, AnimPlayIcon, AnimTimeline, BoxRect, ClipCombo, HeaderListOffset,
    KeyLane, NewClipBtn, OverflowBtn, PaneLayer, TrackFilterField, DEFAULT_SPEED, SPEEDS,
};

/// The fixed blue of the menus' primary buttons. (Play now takes the theme's
/// accent instead; see `build_toolbar`.)
pub(super) const PLAY_BLUE: (u8, u8, u8) = (59, 98, 184);

pub(super) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(rgb(panel_bg())),
            Name::new("anim-timeline"),
        ))
        .id();

    let toolbar = build_toolbar(commands, fonts);

    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
        .id();

    let empty = build_empty_state(commands, fonts);
    bind_display(commands, empty, |w| !ready(&Rx::new(w.untracked())));

    // Shared timeline shell.
    let tl = timeline_view(commands, fonts);
    commands.entity(tl.root).insert((AnimTimeline, RelativeCursorPosition::default()));
    commands.entity(tl.clips).insert((KeyLane, RelativeCursorPosition::default()));
    commands.entity(tl.markers).insert((PaneLayer, RelativeCursorPosition::default()));
    bind_display(commands, tl.root, ready);

    // Track-list head: the filter and "+ Track".
    let filter = text_input(commands, &fonts.ui, &renzora::lang::t_or("timeline.filter_tracks", "Filter tracks"), "");
    commands.entity(filter).insert((
        TrackFilterField,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            height: Val::Px(20.0),
            padding: UiRect::axes(Val::Px(renzora_ember::widgets::PAD_X), Val::Px(1.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
    ));
    let add_track = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), height: Val::Px(20.0), padding: UiRect::horizontal(Val::Px(7.0)), border_radius: BorderRadius::all(Val::Px(5.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            HoverTooltip::new(renzora::lang::t_or("timeline.add_track_tip", "Add a property track")),
            AddTrackBtn,
        ))
        .id();
    hover_wash(commands, add_track);
    let add_track_lbl = commands
        .spawn((Text::new(format!("+ {}", renzora::lang::t_or("timeline.track_short", "Track"))), ui_font(&fonts.ui, 12.0), TextColor(rgb((122, 162, 247))), bevy::text::TextLayout::no_wrap(), PASS_THROUGH))
        .id();
    commands.entity(add_track).add_child(add_track_lbl);
    commands.entity(tl.header_corner).add_children(&[filter, add_track]);

    commands.entity(tl.header_list).insert(HeaderListOffset);
    keyed_list_tokened(commands, tl.header_list, header_token, header_snapshot);

    // Lanes: keys, the curves view, and the box-select rectangle.
    let full = || Node {
        position_type: PositionType::Absolute,
        top: Val::Px(0.0),
        left: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    };
    let keys_layer = commands.spawn((full(), PASS_THROUGH, Name::new("timeline-keys"))).id();
    keyed_list_tokened(commands, keys_layer, lanes_token, keyframe_snapshot);
    bind_display(commands, keys_layer, |w| !w.get_resource::<TimelineMode>().is_some_and(|m| m.curves));
    let curves_layer = commands.spawn((full(), BackgroundColor(rgb(panel_bg())), PASS_THROUGH, Name::new("timeline-curves"))).id();
    keyed_list_tokened(commands, curves_layer, curves_token, curves_snapshot);
    bind_display(commands, curves_layer, |w| w.get_resource::<TimelineMode>().is_some_and(|m| m.curves));
    let hover = super::curves::build_hover_readout(commands, fonts);
    let box_rect = commands
        .spawn((
            Node { position_type: PositionType::Absolute, display: Display::None, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(Color::srgba(0.38, 0.55, 0.9, 0.08)),
            BorderColor::all(Color::srgba(0.48, 0.64, 0.97, 0.7)),
            PASS_THROUGH,
            BoxRect,
            Name::new("timeline-box-select"),
        ))
        .id();
    commands.entity(tl.clips).add_children(&[keys_layer, curves_layer, hover, box_rect]);

    // Over the whole pane: event flags, then the ruler's loop strip, end
    // handle and playhead badge.
    let flags = commands.spawn((full(), PASS_THROUGH)).id();
    keyed_list(commands, flags, marker_snapshot);
    let ruler_overlay = commands.spawn((full(), PASS_THROUGH)).id();
    keyed_list(commands, ruler_overlay, ruler_snapshot);
    let [hbar, vbar] = super::scrollbars::build_scrollbars(commands);
    commands.entity(tl.markers).add_children(&[flags, ruler_overlay, hbar, vbar]);

    commands.entity(body).add_children(&[empty, tl.root]);
    commands.entity(root).add_children(&[toolbar, body]);
    root
}

fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(40.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(10.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();

    // ── Left: "entity / clip ▾", new clip, record ──
    let picker = commands
        .spawn((
            Node { height: Val::Px(28.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(7.0), padding: UiRect::horizontal(Val::Px(10.0)), border_radius: BorderRadius::all(Val::Px(7.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            ClipCombo,
        ))
        .id();
    // Bare text at rest, the theme's button hover under the pointer: the same
    // treatment as the Assets breadcrumb, rather than a grey pill.
    bind_bg(commands, picker, move |w| match w.get::<UiInteraction>(picker) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => w
            .get_resource::<renzora_ember::style::Theme>()
            .map(|t| t.button.bg_hover.color())
            .unwrap_or_else(|| rgb(hover_bg())),
        _ => Color::NONE,
    });
    let ent_lbl = commands.spawn((Text::new(""), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
    bind_text(commands, ent_lbl, |w| {
        // An effect shown on its own is named by its file, which is what the
        // effect editor's tab says too (its preview entity has no name of use).
        if let Some(file) = effect_only(w).map(|fx| fx.path.rsplit(['/', '\\']).next().unwrap_or("").to_string()) {
            return file;
        }
        state(w.untracked())
            .and_then(|s| s.selected_entity)
            .and_then(|e| w.get::<Name>(e).map(|n| n.as_str().to_string()))
            .unwrap_or_else(|| renzora::lang::t_or("timeline.no_entity", "No entity"))
    });
    let slash = commands.spawn((Text::new("/"), ui_font(&fonts.ui, 13.0), TextColor(rgb((90, 90, 102))), PASS_THROUGH)).id();
    let clip_lbl = commands.spawn((Text::new(""), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), Node { max_width: Val::Px(180.0), overflow: Overflow::clip(), ..default() }, PASS_THROUGH)).id();
    bind_text(commands, clip_lbl, |w| {
        if let Some(fx) = effect_only(w) {
            return fx.effect_name();
        }
        state(w.untracked())
            .and_then(|s| s.selected_clip.clone())
            .unwrap_or_else(|| renzora::lang::t_or("timeline.no_clip", "No clip"))
    });
    bind_text_color(commands, clip_lbl, |w| {
        let has = effect_only(w).is_some() || state(w.untracked()).is_some_and(|s| s.selected_clip.is_some());
        rgb(if has { text_primary() } else { text_muted() })
    });
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 11.0);
    commands.entity(caret).insert(PASS_THROUGH);
    commands.entity(picker).add_children(&[ent_lbl, slash, clip_lbl, caret]);

    let (new_clip, _) = icon_btn(commands, fonts, "plus", text_primary(), NewClipBtn);
    commands.entity(new_clip).insert(HoverTooltip::new(renzora::lang::t_or("timeline.new_clip_tip", "New clip")));
    bind_display(commands, new_clip, |w| state(w.untracked()).is_some_and(|s| s.selected_entity.is_some()));

    let sep1 = vsep(commands);
    let (record, record_ic) = icon_btn_raw(commands, fonts, "record", text_muted(), AnimBtn::Record);
    commands.entity(record).insert(HoverTooltip::new(renzora::lang::t_or(
        "timeline.record_tip",
        "Record: every change you make is keyed at the playhead",
    )));
    bind_text_color(commands, record_ic, |w| {
        let on = state(w.untracked()).is_some_and(|s| s.record_enabled);
        rgb(if on { (248, 113, 113) } else { (200, 110, 110) })
    });
    bind_bg(commands, record, move |w| {
        let on = state(w.untracked()).is_some_and(|s| s.record_enabled);
        if on {
            return Color::srgba(0.97, 0.44, 0.44, 0.16);
        }
        match w.get::<UiInteraction>(record) {
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
            _ => Color::NONE,
        }
    });
    let left_tail = [sep1, record];
    for e in left_tail {
        bind_display(commands, e, ready);
    }

    let gap_l = commands.spawn((Node { flex_grow: 1.0, ..default() }, PASS_THROUGH)).id();

    // ── Centre: the transport pill and the time readout ──
    let pill = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(9.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(window_bg())),
        ))
        .id();
    let first = tb_icon(commands, fonts, "skip-back", AnimBtn::SkipBack, "timeline.first_frame", "First frame");
    let prev = tb_icon(commands, fonts, "caret-left", AnimBtn::PrevKey, "timeline.prev_key", "Previous key");
    let play = commands
        .spawn((
            Node { width: Val::Px(32.0), height: Val::Px(26.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(6.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(accent())),
            UiInteraction::default(),
            HoverTooltip::new(renzora::lang::t_or("timeline.play_tip", "Play / pause (Space)")),
            AnimBtn::PlayPause,
        ))
        .id();
    // The theme's accent, so Play follows a theme change like every other
    // accented control.
    bind_bg(commands, play, |_| rgb(accent()));
    let play_ic = icon_text(commands, &fonts.phosphor, "play", (255, 255, 255), 14.0);
    commands.entity(play_ic).insert((AnimPlayIcon, PASS_THROUGH));
    commands.entity(play).add_child(play_ic);
    let next = tb_icon(commands, fonts, "caret-right", AnimBtn::NextKey, "timeline.next_key", "Next key");
    let last = tb_icon(commands, fonts, "skip-forward", AnimBtn::SkipForward, "timeline.last_frame", "Last frame");
    let (loop_b, loop_ic) = icon_btn(commands, fonts, "repeat", text_muted(), AnimBtn::Loop);
    commands.entity(loop_b).insert(HoverTooltip::new(renzora::lang::t_or("timeline.loop_tip", "Loop")));
    bind_text_color(commands, loop_ic, |w| {
        let on = state(w.untracked()).is_some_and(|s| s.preview_looping);
        rgb(if on { accent() } else { text_muted() })
    });
    commands.entity(pill).add_children(&[first, prev, play, next, last, loop_b]);

    let time = build_readout(commands, fonts);
    for e in [pill, time] {
        bind_display(commands, e, ready);
    }

    let gap_r = commands.spawn((Node { flex_grow: 1.0, ..default() }, PASS_THROUGH)).id();

    // Shown with no clip in place of the transport.
    let waiting = commands.spawn((Text::new(renzora::lang::t_or("timeline.transport_waits", "Transport appears once there is a clip")), ui_font(&fonts.ui, 12.0), TextColor(rgb((106, 106, 118))), bevy::text::TextLayout::no_wrap())).id();
    bind_display(commands, waiting, |w| !ready(w) && state(w.untracked()).is_some_and(|s| s.selected_entity.is_some()));

    // ── Right: curves tools, speed, snap, Keys / Curves, overflow ──
    let curves_tools = super::curves::build_interp_buttons(commands, fonts);
    bind_display(commands, curves_tools, |w| ready(w) && w.get_resource::<TimelineMode>().is_some_and(|m| m.curves));

    let speed_labels: Vec<String> = SPEEDS.iter().map(|s| if *s >= 1.0 { format!("{s:.1}x") } else { format!("{s}x") }).collect();
    let speed_refs: Vec<&str> = speed_labels.iter().map(|s| s.as_str()).collect();
    let speed_dd = dropdown_compact(commands, fonts, &speed_refs, DEFAULT_SPEED, 58.0);
    commands.entity(speed_dd).insert(HoverTooltip::new(renzora::lang::t_or("timeline.speed_tip", "Playback speed")));
    bind_2way(
        commands,
        speed_dd,
        |w: &Rx| {
            state(w.untracked())
                .and_then(|st| SPEEDS.iter().position(|s| (st.preview_speed - s).abs() < 0.01))
                .unwrap_or(DEFAULT_SPEED)
        },
        |w: &mut World, i: &usize| {
            let Some(&speed) = SPEEDS.get(*i) else { return };
            let Some(bridge) = w.get_resource::<AnimEditorBridge>() else { return };
            push(bridge, AnimEditorAction::SetPreviewSpeed(speed));
        },
    );

    let (snap_b, snap_ic) = icon_btn_raw(commands, fonts, "magnet-straight", text_muted(), AnimBtn::Snap);
    commands.entity(snap_b).insert(HoverTooltip::new(renzora::lang::t_or("timeline.snap_tip", "Snap keys to frames")));
    bind_text_color(commands, snap_ic, |w| {
        let on = state(w.untracked()).is_some_and(|s| s.snap_enabled);
        rgb(if on { (122, 162, 247) } else { text_muted() })
    });
    bind_bg(commands, snap_b, move |w| {
        let on = state(w.untracked()).is_some_and(|s| s.snap_enabled);
        if on {
            return Color::srgba(0.38, 0.55, 0.9, 0.16);
        }
        match w.get::<UiInteraction>(snap_b) {
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
            _ => Color::NONE,
        }
    });
    let sep3 = vsep(commands);
    let mode = super::curves::build_mode_switch(commands, fonts);
    let (overflow, _) = icon_btn(commands, fonts, "dots-three", text_primary(), OverflowBtn);
    commands.entity(overflow).insert((RelativeCursorPosition::default(), HoverTooltip::new(renzora::lang::t_or("timeline.more_tip", "More: length, events, zoom, row height, save"))));
    for e in [speed_dd, snap_b, sep3, mode, overflow] {
        bind_display(commands, e, ready);
    }

    commands.entity(bar).add_children(&[
        picker, new_clip, sep1, record, gap_l, pill, time, waiting, gap_r, curves_tools, speed_dd, snap_b, sep3, mode, overflow,
    ]);
    bar
}

/// The effect the timeline shows on a stand-in clip (no animation clip), for
/// the clip picker's labels. Read tracked, so the labels follow it.
fn effect_only<'w>(w: &Rx<'w>) -> Option<&'w super::fx::FxDoc> {
    let cache = w.get_resource::<super::clip::TimelineClip>()?;
    cache.fx.as_ref().filter(|_| cache.is_effect_only())
}

/// `mm:ss.cc`.
pub(super) fn clock(secs: f32) -> String {
    let secs = secs.max(0.0);
    format!("{:02}:{:05.2}", (secs / 60.0) as u32, secs % 60.0)
}

/// Widths of the readout's slots, px: each clock holds `mm:ss.cc` in the
/// 12px mono face, the frame chip up to `f99999`.
const CLOCK_W: f32 = 62.0;
const FRAME_W: f32 = 50.0;

/// The time readout beside the transport: the playhead's clock, the clip's
/// length, and the frame number, each in a fixed-width slot.
///
/// It was one text run, `00:00.24  /  00:00.47   f7`, and the toolbar centres
/// the transport and the readout together, so every time the frame number
/// gained or lost a digit the run changed width and the play buttons jumped
/// sideways while the clip played. Fixed slots keep the group's width
/// constant whatever the numbers say.
fn build_readout(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), margin: UiRect::left(Val::Px(10.0)), flex_shrink: 0.0, ..default() },
            PASS_THROUGH,
        ))
        .id();
    let slot = |commands: &mut Commands, width: f32, justify: JustifyContent| {
        commands
            .spawn((Node { width: Val::Px(width), flex_shrink: 0.0, justify_content: justify, align_items: AlignItems::Center, ..default() }, PASS_THROUGH))
            .id()
    };
    let text = |commands: &mut Commands, size: f32, color: (u8, u8, u8)| {
        commands
            .spawn((Text::new(""), ui_font(&fonts.mono, size), TextColor(rgb(color)), bevy::text::TextLayout::no_wrap(), PASS_THROUGH))
            .id()
    };

    let now_slot = slot(commands, CLOCK_W, JustifyContent::FlexEnd);
    let now = text(commands, 12.0, text_primary());
    bind_text(commands, now, |w| state(w.untracked()).map(|s| clock(s.scrub_time)).unwrap_or_default());
    commands.entity(now_slot).add_child(now);

    let sep = commands.spawn((Text::new("/"), ui_font(&fonts.mono, 12.0), TextColor(rgb(text_muted())), PASS_THROUGH)).id();

    let total_slot = slot(commands, CLOCK_W, JustifyContent::FlexStart);
    let total = text(commands, 12.0, text_muted());
    bind_text(commands, total, |w| clock(cur_clip(w.untracked()).map(|c| c.duration).unwrap_or(0.0)));
    commands.entity(total_slot).add_child(total);

    // The frame number in a chip of its own, so it reads as a separate unit
    // rather than the tail of the clock.
    let chip = commands
        .spawn((
            Node { width: Val::Px(FRAME_W), height: Val::Px(20.0), flex_shrink: 0.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(faint_bg())),
            PASS_THROUGH,
        ))
        .id();
    let frame = text(commands, 11.0, text_muted());
    bind_text(commands, frame, |w| state(w.untracked()).map(|s| format!("f{}", (s.scrub_time * 30.0).round() as i64)).unwrap_or_default());
    commands.entity(chip).add_child(frame);

    commands.entity(row).add_children(&[now_slot, sep, total_slot, chip]);
    row
}

/// A transport button: icon plus a tooltip.
fn tb_icon<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, marker: M, key: &str, fallback: &str) -> Entity {
    let (b, _) = icon_btn(commands, fonts, icon, text_primary(), marker);
    commands.entity(b).insert(HoverTooltip::new(renzora::lang::t_or(key, fallback)));
    b
}

/// A 26px icon button with a faint hover wash.
pub(super) fn icon_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8), marker: M) -> (Entity, Entity) {
    let (btn, ic) = icon_btn_raw(commands, fonts, icon, color, marker);
    hover_wash(commands, btn);
    (btn, ic)
}

/// [`icon_btn`] without the hover wash, for a button that paints its own
/// background (an on/off toggle): two bindings on one background would fight.
pub(super) fn icon_btn_raw<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8), marker: M) -> (Entity, Entity) {
    let btn = commands
        .spawn((Node { width: Val::Px(26.0), height: Val::Px(26.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(6.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), UiInteraction::default(), marker))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 14.0);
    commands.entity(ic).insert(PASS_THROUGH);
    commands.entity(btn).add_child(ic);
    (btn, ic)
}

/// A faint white wash while hovered.
pub(super) fn hover_wash(commands: &mut Commands, btn: Entity) {
    bind_bg(commands, btn, move |w| match w.get::<UiInteraction>(btn) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });
}

fn vsep(commands: &mut Commands) -> Entity {
    commands.spawn((Node { width: Val::Px(1.0), height: Val::Px(18.0), margin: UiRect::horizontal(Val::Px(6.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb((42, 42, 52))))).id()
}

//! The panel's widget tree and its transport toolbar.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, bind_text_color, keyed_list};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    drag_value, dropdown_compact, text_input, timeline_view, DragRange,
};

use crate::{AnimEditorAction, AnimEditorBridge};

use super::clip::{cur_clip, empty_msg, ready, set_clip_duration, state, TimelineClip};
use super::edit::push;
use super::snapshots::{header_snapshot, keyframe_snapshot, marker_snapshot, selected_key_label};
use super::{
    AddMarkerBtn, AddTrackBtn, AnimBtn, AnimPlayIcon, AnimTimeline, ClipCombo, KeyLane,
    MarkerNameField, NewClipBtn, NewClipNameField, SaveClipBtn, DEFAULT_SPEED, MARKER, PROPERTY,
    SPEEDS,
};

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
    bind_display(commands, note, |w| !ready(&Rx::new(w.untracked())));

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
        let on = state(w.untracked()).is_some_and(|s| s.preview_looping);
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
    bind_text(commands, combo_v, |w| state(w.untracked()).and_then(|s| s.selected_clip.clone()).unwrap_or_else(|| renzora::lang::t("timeline.select_clip")));
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

    // Speed: one dropdown rather than five preset buttons in a row. Four of the
    // five were always showing you an option you hadn't picked, for a setting
    // that is usually left at 1.00x — a lot of bar for that. `bind_2way` keeps
    // the box showing whatever `preview_speed` actually is, including a speed
    // set from somewhere other than this control.
    let speed_lbl = commands.spawn((Text::new(renzora::lang::t("common.speed")), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    let speed_labels: Vec<String> = SPEEDS.iter().map(|s| format!("{s:.2}x")).collect();
    let speed_refs: Vec<&str> = speed_labels.iter().map(|s| s.as_str()).collect();
    let speed_dd = dropdown_compact(commands, fonts, &speed_refs, DEFAULT_SPEED, 62.0);
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

    let sep4 = vsep(commands);

    let (snap_b, snap_ic) = icon_btn(commands, fonts, "magnet-straight", text_muted(), AnimBtn::Snap);
    bind_text_color(commands, snap_ic, |w| {
        let on = state(w.untracked()).is_some_and(|s| s.snap_enabled);
        rgb(if on { accent() } else { text_muted() })
    });

    // Record toggle (auto-key inspector edits) — red when armed.
    let (record_b, record_ic) = icon_btn(commands, fonts, "record", text_muted(), AnimBtn::Record);
    bind_text_color(commands, record_ic, |w| {
        let on = state(w.untracked()).is_some_and(|s| s.record_enabled);
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
        let dirty = w.get_resource::<TimelineClip>().is_some_and(|c| c.dirty);
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
        |w: &Rx| cur_clip(w.untracked()).map(|c| c.duration).unwrap_or(2.0),
        |w: &mut World, v: &f32| set_clip_duration(w, *v),
    );

    let time = commands.spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, time, |w| {
        let Some(s) = state(w.untracked()) else { return String::new() };
        let secs = s.scrub_time;
        let frame = (secs * 30.0) as u32;
        format!("{:02}:{:05.2}  f{}", (secs / 60.0) as u32, secs % 60.0, frame)
    });

    let zoom_out = icon_btn(commands, fonts, "magnifying-glass-minus", text_muted(), AnimBtn::ZoomOut).0;
    let zoom_lbl = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, zoom_lbl, |w| format!("{:.0}px/s", state(w.untracked()).map(|s| s.timeline_zoom).unwrap_or(0.0)));
    let zoom_in = icon_btn(commands, fonts, "magnifying-glass-plus", text_muted(), AnimBtn::ZoomIn).0;

    let mut kids = vec![skip_back, step_back, play, stop, step_fwd, skip_fwd, record_b, sep1, loop_b, sep2, combo, new_clip_field, new_clip_b, sep3, speed_lbl, speed_dd];
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

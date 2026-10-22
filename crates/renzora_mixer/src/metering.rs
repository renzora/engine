//! The mixer's **Metering** page: every bus's level on one long horizontal
//! meter, with a peak-hold mark, the held peak in dB, and a clip light.
//!
//! The strips already have meters, but a strip's meter is as short as the
//! panel is tall and moves too fast to read a number off. This page is for the
//! question "how hot is this bus running", so it holds what the meters can
//! only flash: the highest peak of the last couple of seconds, and whether a
//! bus has gone over full scale at all since you last looked.
//!
//! Hold and clip live in [`MeterHold`], fed from `ChannelStrip::peak_level`
//! (post-fader, linear) by [`update_meter_hold`]. A clip stays lit until you
//! click it, or **Reset peaks**: a clip that clears itself is one you miss.

use std::collections::HashMap;

use bevy::prelude::*;

use renzora_audio::MixerState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_text, bind_with};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{scroll_view, vu_meter_bound_horizontal, HoverTooltip};
use renzora_ember::UiInteraction;

use super::routing::source_buses;
use super::strips::{bus_name, read, tint, BusRef};

/// How long a peak is held before the mark starts to fall, seconds.
const HOLD_SECS: f64 = 2.0;
/// How fast a released hold falls, in linear amplitude per second.
const FALL_PER_SEC: f32 = 0.6;
const NAME_W: f32 = 120.0;

/// One bus's held peak and clip state.
#[derive(Clone, Copy, Default)]
pub(crate) struct Held {
    peak: f32,
    at: f64,
    clipped: bool,
}

/// Held peaks and clip lights, keyed by bus routing key (permanent, unlike a
/// display name or a position in the list).
#[derive(Resource, Default)]
pub(crate) struct MeterHold(HashMap<String, Held>);

impl MeterHold {
    fn get(&self, key: &str) -> Held {
        self.0.get(key).copied().unwrap_or_default()
    }
}

/// Click a clip light to clear it.
#[derive(Component)]
pub(crate) struct ClipReset(String);

/// The page's "Reset peaks" button.
#[derive(Component)]
pub(crate) struct ResetPeaks;

pub(crate) fn update_meter_hold(time: Res<Time>, mixer: Option<Res<MixerState>>, mut hold: ResMut<MeterHold>) {
    let Some(mixer) = mixer else { return };
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    let mut buses = vec![BusRef::Master];
    buses.extend(source_buses(&mixer));
    for bus in buses {
        let key = bus.key(&mixer);
        let Some(level) = bus.get(&mixer).map(|s| s.peak_level) else { continue };
        let h = hold.0.entry(key).or_default();
        if level >= h.peak {
            h.peak = level;
            h.at = now;
        } else if now - h.at > HOLD_SECS {
            h.peak = (h.peak - FALL_PER_SEC * dt).max(level);
        }
        if level >= 1.0 {
            h.clipped = true;
        }
    }
}

pub(crate) fn meter_clicks(
    clips: Query<(&UiInteraction, &ClipReset), Changed<UiInteraction>>,
    reset: Query<&UiInteraction, (With<ResetPeaks>, Changed<UiInteraction>)>,
    mut hold: ResMut<MeterHold>,
) {
    if reset.iter().any(|i| *i == UiInteraction::Pressed) {
        hold.0.clear();
        return;
    }
    for (i, c) in &clips {
        if *i == UiInteraction::Pressed {
            if let Some(h) = hold.0.get_mut(&c.0) {
                h.clipped = false;
                h.peak = 0.0;
            }
        }
    }
}

fn held(rx: &Rx, key: &str) -> Held {
    rx.get_resource::<MeterHold>().map(|h| h.get(key)).unwrap_or_default()
}

fn db_text(v: f32) -> String {
    if v <= 0.0001 {
        "-\u{221e} dB".to_string()
    } else {
        format!("{:+.1} dB", 20.0 * v.log10())
    }
}

fn label(commands: &mut Commands, font: &bevy::text::FontSource, text: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(font, size), TextColor(rgb(color)), bevy::text::TextLayout::no_wrap()))
        .id()
}

pub(crate) fn body(commands: &mut Commands, fonts: &EmberFonts, mixer: &MixerState) -> Entity {
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("mixer-metering"),
        ))
        .id();

    // Header: a caption and Reset peaks.
    let head = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::bottom(Val::Px(4.0)),
            ..default()
        })
        .id();
    let caption = label(
        commands,
        &fonts.ui,
        &renzora::lang::t_or("mixer.metering_caption", "Post-fader peak. The mark holds the highest peak for 2 seconds."),
        text_muted(),
        11.0,
    );
    let spring = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let reset = commands
        .spawn((
            Node {
                height: Val::Px(24.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            UiInteraction::default(),
            ResetPeaks,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, reset, move |rx| match rx.get::<UiInteraction>(reset) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
        _ => rgb(card_bg()),
    });
    let rt = label(commands, &fonts.ui, &renzora::lang::t_or("mixer.reset_peaks", "Reset peaks"), text_primary(), 12.0);
    commands.entity(reset).add_child(rt);
    commands.entity(head).add_children(&[caption, spring, reset]);

    let mut rows = vec![head];
    for bus in source_buses(mixer) {
        rows.push(row(commands, fonts, bus, bus.key(mixer), false));
    }
    let rule = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(border())),
        ))
        .id();
    rows.push(rule);
    rows.push(row(commands, fonts, BusRef::Master, "Master".to_string(), true));
    commands.entity(list).add_children(&rows);
    scroll_view(commands, list)
}

fn row(commands: &mut Commands, fonts: &EmberFonts, bus: BusRef, key: String, master: bool) -> Entity {
    let r = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(if master { 34.0 } else { 28.0 }),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            flex_shrink: 0.0,
            ..default()
        })
        .id();

    let name = commands
        .spawn(Node {
            width: Val::Px(NAME_W),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    let bar = commands
        .spawn((
            Node { width: Val::Px(4.0), height: Val::Px(18.0), border_radius: BorderRadius::all(Val::Px(2.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(Color::NONE),
        ))
        .id();
    bind_bg(commands, bar, move |rx| tint(read(rx, bus, |s| s.color)));
    let mut font = ui_font(&fonts.ui, if master { 13.0 } else { 12.0 });
    if master {
        font.weight = bevy::text::FontWeight::SEMIBOLD;
    }
    let t = commands
        .spawn((Text::new(""), font, TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap()))
        .id();
    bind_text(commands, t, move |rx| bus_name(rx, bus));
    commands.entity(name).add_children(&[bar, t]);

    // The meter, with the hold mark laid over it.
    let track = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(40.0),
            height: Val::Px(if master { 14.0 } else { 10.0 }),
            position_type: PositionType::Relative,
            ..default()
        })
        .id();
    let vu = vu_meter_bound_horizontal(commands, move |rx| read(rx, bus, |s| s.peak_level.clamp(0.0, 1.0)));
    commands.entity(vu).entry::<Node>().and_modify(|mut n| {
        n.width = Val::Percent(100.0);
        n.height = Val::Percent(100.0);
        n.min_width = Val::Px(0.0);
    });
    let mark = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(-2.0),
                bottom: Val::Px(-2.0),
                width: Val::Px(2.0),
                left: Val::Percent(0.0),
                ..default()
            },
            BackgroundColor(rgb(text_primary())),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    let key_mark = key.clone();
    bind_with(
        commands,
        mark,
        move |rx: &Rx| (held(rx, &key_mark).peak.clamp(0.0, 1.0) * 1000.0).round() as u32,
        |world: &mut World, e: Entity, permille: &u32| {
            if let Some(mut n) = world.get_mut::<Node>(e) {
                n.left = Val::Percent(*permille as f32 / 10.0);
                n.display = if *permille == 0 { Display::None } else { Display::Flex };
            }
        },
    );
    commands.entity(track).add_children(&[vu, mark]);

    let readout = label(commands, &fonts.mono, "", value_text(), 11.0);
    commands.entity(readout).insert(Node { width: Val::Px(70.0), flex_shrink: 0.0, ..default() });
    let key_text = key.clone();
    bind_text(commands, readout, move |rx| db_text(held(rx, &key_text).peak));

    // The clip light: red and latched once the bus has gone over full scale.
    let clip = commands
        .spawn((
            Node {
                width: Val::Px(40.0),
                height: Val::Px(18.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            UiInteraction::default(),
            ClipReset(key.clone()),
            HoverTooltip::new(renzora::lang::t_or("mixer.clip_tip", "Lights when this bus goes over full scale. Click to clear.")),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let key_clip = key.clone();
    bind_bg(commands, clip, move |rx| {
        if held(rx, &key_clip).clipped {
            rgb(close_red())
        } else {
            rgb(card_bg())
        }
    });
    let ct = label(commands, &fonts.ui, &renzora::lang::t_or("mixer.clip", "CLIP"), text_muted(), 9.0);
    let key_ct = key;
    bind_with(
        commands,
        ct,
        move |rx: &Rx| held(rx, &key_ct).clipped,
        |world: &mut World, e: Entity, on: &bool| {
            if let Some(mut c) = world.get_mut::<TextColor>(e) {
                c.0 = rgb(if *on { text_primary() } else { text_muted() });
            }
        },
    );
    commands.entity(clip).add_child(ct);

    commands.entity(r).add_children(&[name, track, readout, clip]);
    r
}

//! The mixer's **Routing** page: where each bus goes, and how much of it is
//! sent to the shared reverb and delay.
//!
//! One row per bus, in the board's order, then Master. A row's Output lists
//! Master and every bus it could feed without closing a loop
//! ([`MixerState::would_loop`]); the backend refuses a loop too, but a menu
//! that offers one is a menu that lies. The trail at the end of the row spells
//! out the whole path ("Sfx > Group > Master"), because a bus routed into a
//! bus routed into a bus is otherwise a puzzle you solve one dropdown at a
//! time.
//!
//! The page is rebuilt whenever the set of buses, a name or any output
//! changes (see [`routing_hash`]): each dropdown's option list depends on
//! every other bus's output, so binding the options in place would have to
//! rebuild them anyway.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_audio::MixerState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_text};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, dropdown, scroll_view, DragRange};

use super::strips::{bus_name, read, tint, write, BusRef};

/// Width of the name column, and of each control column.
const NAME_W: f32 = 140.0;
const OUTPUT_W: f32 = 180.0;
const SEND_W: f32 = 110.0;

/// Every non-master bus, in board order.
pub(crate) fn source_buses(mixer: &MixerState) -> Vec<BusRef> {
    let mut v = vec![BusRef::Sfx, BusRef::Music, BusRef::Ambient];
    v.extend((0..mixer.custom_buses.len()).map(BusRef::Custom));
    v
}

/// What the page's shape depends on: the buses, their names and their outputs.
pub(crate) fn routing_hash(mixer: &MixerState, h: &mut impl Hasher) {
    for b in source_buses(mixer) {
        b.key(mixer).hash(h);
        b.get(mixer).map(|s| s.output.clone()).hash(h);
    }
    for b in &mixer.custom_buses {
        b.name.hash(h);
    }
}

/// The path from `bus` to Master, by display name: "Sfx > Group > Master".
fn trail(rx: &Rx, bus: BusRef) -> String {
    let Some(mixer) = rx.get_resource::<MixerState>() else { return String::new() };
    let mut parts = vec![bus_name(rx, bus)];
    let mut cur = bus.get(mixer).and_then(|s| s.output.clone());
    // Bounded, so a loop already in a hand-edited project cannot hang the UI.
    for _ in 0..=mixer.custom_buses.len() + 4 {
        let Some(key) = cur else { break };
        let Some(next) = BusRef::from_key(mixer, &key) else { break };
        if next == BusRef::Master {
            break;
        }
        parts.push(bus_name(rx, next));
        cur = next.get(mixer).and_then(|s| s.output.clone());
    }
    parts.push("Master".to_string());
    parts.join("  \u{203a}  ")
}

pub(crate) fn body(commands: &mut Commands, fonts: &EmberFonts, mixer: &MixerState) -> Entity {
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                row_gap: Val::Px(2.0),
                ..default()
            },
            Name::new("mixer-routing"),
        ))
        .id();

    let mut rows = vec![header(commands, fonts)];
    for bus in source_buses(mixer) {
        rows.push(row(commands, fonts, mixer, bus));
    }
    rows.push(master_row(commands, fonts));
    commands.entity(list).add_children(&rows);
    scroll_view(commands, list)
}

fn cell(commands: &mut Commands, width: f32) -> Entity {
    commands
        .spawn(Node {
            width: Val::Px(width),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        })
        .id()
}

fn label(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, size),
            TextColor(rgb(color)),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id()
}

fn row_node(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(36.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(14.0),
            padding: UiRect::horizontal(Val::Px(8.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_shrink: 0.0,
            ..default()
        })
        .id()
}

fn header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let r = row_node(commands);
    commands.entity(r).entry::<Node>().and_modify(|mut n| n.height = Val::Px(26.0));
    let mut kids = Vec::new();
    for (text, w) in [
        (renzora::lang::t_or("mixer.bus", "Bus"), NAME_W + 12.0),
        (renzora::lang::t_or("mixer.output", "Output"), OUTPUT_W),
        (renzora::lang::t_or("mixer.reverb_send", "Reverb send"), SEND_W),
        (renzora::lang::t_or("mixer.delay_send", "Delay send"), SEND_W),
    ] {
        let c = cell(commands, w);
        let t = label(commands, fonts, &text.to_uppercase(), text_muted(), 10.0);
        commands.entity(c).add_child(t);
        kids.push(c);
    }
    let path = label(commands, fonts, &renzora::lang::t_or("mixer.signal_path", "Signal path").to_uppercase(), text_muted(), 10.0);
    kids.push(path);
    commands.entity(r).add_children(&kids);
    r
}

/// The colour bar and name every row starts with.
fn name_cell(commands: &mut Commands, fonts: &EmberFonts, bus: BusRef) -> Entity {
    let c = cell(commands, NAME_W);
    commands.entity(c).entry::<Node>().and_modify(|mut n| n.column_gap = Val::Px(10.0));
    let bar = commands
        .spawn((
            Node {
                width: Val::Px(4.0),
                height: Val::Px(22.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(tint(BUS_GRAY)),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, bar, move |rx| tint(read(rx, bus, |s| s.color)));
    let name = label(commands, fonts, "", text_primary(), 13.0);
    bind_text(commands, name, move |rx| bus_name(rx, bus));
    commands.entity(c).add_children(&[bar, name]);
    c
}

const BUS_GRAY: [u8; 3] = [130, 130, 140];

fn row(commands: &mut Commands, fonts: &EmberFonts, mixer: &MixerState, bus: BusRef) -> Entity {
    let r = row_node(commands);
    let name = name_cell(commands, fonts, bus);

    // Output: Master, then every bus this one could feed without a loop.
    let key = bus.key(mixer);
    let mut targets: Vec<(Option<String>, String)> = vec![(None, "Master".to_string())];
    for other in source_buses(mixer) {
        let k = other.key(mixer);
        if k == key || mixer.would_loop(&key, &k) {
            continue;
        }
        let shown = match other {
            BusRef::Custom(i) => mixer.custom_buses[i].name.clone(),
            _ => other.label().to_string(),
        };
        targets.push((Some(k), shown));
    }
    let labels: Vec<&str> = targets.iter().map(|(_, l)| l.as_str()).collect();
    let current = bus.get(mixer).and_then(|s| s.output.clone());
    let sel = targets.iter().position(|(k, _)| *k == current).unwrap_or(0);
    let out_cell = cell(commands, OUTPUT_W);
    let dd = dropdown(commands, fonts, &labels, sel);
    commands.entity(dd).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    let keys_a: Vec<Option<String>> = targets.iter().map(|(k, _)| k.clone()).collect();
    let keys_b = keys_a.clone();
    bind_2way(
        commands,
        dd,
        move |rx| {
            let cur = read(rx, bus, |s| s.output.clone());
            keys_a.iter().position(|k| *k == cur).unwrap_or(0)
        },
        move |w, i: &usize| {
            let target = keys_b.get(*i).cloned().flatten();
            write(w, bus, move |s| s.output = target.clone());
        },
    );
    commands.entity(out_cell).add_child(dd);

    let reverb = send_cell(commands, fonts, bus, true);
    let delay = send_cell(commands, fonts, bus, false);

    let path = label(commands, fonts, "", text_muted(), 12.0);
    commands.entity(path).insert(Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() });
    bind_text(commands, path, move |rx| trail(rx, bus));

    commands.entity(r).add_children(&[name, out_cell, reverb, delay, path]);
    r
}

/// A send level as a 0-100 drag value: the model is 0..1, but "35" reads as a
/// level and "0.35" reads as a coefficient.
fn send_cell(commands: &mut Commands, fonts: &EmberFonts, bus: BusRef, reverb: bool) -> Entity {
    let c = cell(commands, SEND_W);
    let dv = drag_value(commands, &fonts.ui, "", text_muted(), 0.0, 1.0);
    commands.entity(dv).insert(DragRange { min: 0.0, max: 100.0 });
    commands.entity(dv).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        dv,
        move |rx| read(rx, bus, |s| if reverb { s.reverb_send } else { s.delay_send }) * 100.0,
        move |w, v: &f32| {
            let v = (*v / 100.0).clamp(0.0, 1.0);
            write(w, bus, move |s| {
                if reverb {
                    s.reverb_send = v;
                } else {
                    s.delay_send = v;
                }
            });
        },
    );
    commands.entity(c).add_child(dv);
    c
}

/// Master's row: it has no output to pick (it is the output) and no sends
/// (the effect returns land on it).
fn master_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let r = row_node(commands);
    commands.entity(r).insert(BackgroundColor(rgb(section_bg()).with_alpha(0.5)));
    commands.entity(r).entry::<Node>().and_modify(|mut n| n.margin = UiRect::top(Val::Px(6.0)));
    let name = name_cell(commands, fonts, BusRef::Master);
    let out = cell(commands, OUTPUT_W);
    let t = label(commands, fonts, &renzora::lang::t_or("mixer.audio_device", "Audio device"), text_muted(), 12.0);
    commands.entity(out).add_child(t);
    let note = label(
        commands,
        fonts,
        &renzora::lang::t_or("mixer.master_note", "Everything ends here. Reverb and delay return to Master."),
        text_muted(),
        12.0,
    );
    commands.entity(r).add_children(&[name, out, note]);
    r
}

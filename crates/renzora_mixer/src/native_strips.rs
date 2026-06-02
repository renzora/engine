//! Bevy-native (ember) Mixer — channel strips wired two-way to `MixerState`.
//!
//! Increment A: the strips themselves (master / sfx / music / ambient + custom
//! buses). Each strip's volume fader, pan knob, mute/solo buttons and VU meter
//! are bound to the bus's `ChannelStrip` through the generic `bind_2way` (one
//! line per control, no panel-specific binder). The FX-insert popover and bus
//! add/rename/drag management are Increment B.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_audio::{ChannelStrip, MixerState};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{rgb, ACCENT_BLUE, TEXT_MUTED, TEXT_PRIMARY};
use renzora_ember::widgets::{fader, knob, mixer_button, vu_meter_bound};

const RED: (u8, u8, u8) = (225, 90, 80);
/// Max linear volume (1.0 = unity, 1.5 = +3.5 dB head-room).
const VOL_MAX: f64 = 1.5;

/// Registers the bevy-native Mixer content.
pub struct NativeMixer;

impl Plugin for NativeMixer {
    fn build(&self, app: &mut App) {
        app.register_panel_content("mixer", false, build);
    }
}

// Bus accessor shorthand: `Option<&ChannelStrip>` so a vanished custom bus
// degrades gracefully instead of panicking.
fn read<R: Default, Sel: Fn(&MixerState) -> Option<&ChannelStrip>>(w: &World, sel: Sel, f: impl Fn(&ChannelStrip) -> R) -> R {
    w.get_resource::<MixerState>().and_then(|m| sel(m).map(&f)).unwrap_or_default()
}

fn write<SelMut: Fn(&mut MixerState) -> Option<&mut ChannelStrip>>(w: &mut World, sel: SelMut, f: impl Fn(&mut ChannelStrip)) {
    if let Some(mut m) = w.get_resource_mut::<MixerState>() {
        if let Some(s) = sel(&mut m) {
            f(s);
        }
    }
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            padding: UiRect::all(Val::Px(8.0)),
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let master = strip(commands, fonts, "Master", |m| Some(&m.master), |m| Some(&mut m.master));
    let sfx = strip(commands, fonts, "SFX", |m| Some(&m.sfx), |m| Some(&mut m.sfx));
    let music = strip(commands, fonts, "Music", |m| Some(&m.music), |m| Some(&mut m.music));
    let ambient = strip(commands, fonts, "Ambient", |m| Some(&m.ambient), |m| Some(&mut m.ambient));

    let custom = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    keyed_list(commands, custom, custom_snapshot);

    commands.entity(root).add_children(&[master, sfx, music, ambient, custom]);
    root
}

/// One channel strip: name, fader + VU, pan knob, mute/solo — all two-way bound
/// to the bus the `sel`/`sel_mut` accessors point at.
fn strip<Sel, SelMut>(commands: &mut Commands, fonts: &EmberFonts, name: &str, sel: Sel, sel_mut: SelMut) -> Entity
where
    Sel: Fn(&MixerState) -> Option<&ChannelStrip> + Copy + Send + Sync + 'static,
    SelMut: Fn(&mut MixerState) -> Option<&mut ChannelStrip> + Copy + Send + Sync + 'static,
{
    let col = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb((26, 26, 32))),
            BorderColor::all(rgb((48, 48, 58))),
            Name::new("mixer-strip"),
        ))
        .id();

    let label = commands
        .spawn((Text::new(name), ui_font(&fonts.ui, 11.0), TextColor(rgb(TEXT_PRIMARY))))
        .id();

    let meters = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let vol = fader(commands, 0.0);
    bind_2way(
        commands,
        vol,
        move |w| read(w, sel, |s| ((s.volume / VOL_MAX) as f32).clamp(0.0, 1.0)),
        move |w, v| {
            let nv = *v as f64 * VOL_MAX;
            write(w, sel_mut, move |s| s.volume = nv);
        },
    );
    let vu = vu_meter_bound(commands, move |w| read(w, sel, |s| (s.peak_level / VOL_MAX as f32).clamp(0.0, 1.0)));
    commands.entity(meters).add_children(&[vol, vu]);

    // Pan: -1..1 mapped to the knob's 0..1.
    let pan = knob(commands, 0.5);
    bind_2way(
        commands,
        pan,
        move |w| read(w, sel, |s| (((s.panning + 1.0) / 2.0) as f32).clamp(0.0, 1.0)),
        move |w, v| {
            let np = (*v as f64) * 2.0 - 1.0;
            write(w, sel_mut, move |s| s.panning = np);
        },
    );
    let pan_label = commands
        .spawn((Text::new("Pan"), ui_font(&fonts.ui, 9.0), TextColor(rgb(TEXT_MUTED))))
        .id();

    let buttons = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(5.0),
            ..default()
        })
        .id();
    let mute = mixer_button(commands, fonts, "M", rgb(RED));
    bind_2way(
        commands,
        mute,
        move |w| read(w, sel, |s| s.muted),
        move |w, v| {
            let nv = *v;
            write(w, sel_mut, move |s| s.muted = nv);
        },
    );
    let solo = mixer_button(commands, fonts, "S", rgb(ACCENT_BLUE));
    bind_2way(
        commands,
        solo,
        move |w| read(w, sel, |s| s.soloed),
        move |w, v| {
            let nv = *v;
            write(w, sel_mut, move |s| s.soloed = nv);
        },
    );
    commands.entity(buttons).add_children(&[mute, solo]);

    commands
        .entity(col)
        .add_children(&[label, meters, pan, pan_label, buttons]);
    col
}

fn custom_snapshot(world: &World) -> KeyedSnapshot {
    let names: Vec<String> = world
        .get_resource::<MixerState>()
        .map(|m| m.custom_buses.iter().map(|(n, _)| n.clone()).collect())
        .unwrap_or_default();
    // Key by bus name (stable identity); hash by index so a reorder rebuilds the
    // strip with fresh accessors.
    let items: Vec<(u64, u64)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            n.hash(&mut h);
            (h.finish(), i as u64)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            strip(
                c,
                f,
                &names[i],
                move |m: &MixerState| m.custom_buses.get(i).map(|(_, s)| s),
                move |m: &mut MixerState| m.custom_buses.get_mut(i).map(|(_, s)| s),
            )
        }),
    }
}

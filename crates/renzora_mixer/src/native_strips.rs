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
use renzora_editor::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_bg, keyed_list, react, KeyedSnapshot};
use renzora_ember::theme::{rgb, ACCENT_BLUE, TEXT_MUTED, TEXT_PRIMARY};
use renzora_ember::widgets::{
    fader, icon_popover, knob, mixer_button, text_input, vu_meter_bound, EmberTextInput,
};

const RED: (u8, u8, u8) = (225, 90, 80);
/// Max linear volume (1.0 = unity, 1.5 = +3.5 dB head-room).
const VOL_MAX: f64 = 1.5;

/// Click target for a custom bus's delete (×) button.
#[derive(Component)]
struct BusDelete(usize);

/// The "create custom bus" text field + button markers.
#[derive(Component)]
struct BusNameInput;
#[derive(Component)]
struct BusCreate;

/// Registers the bevy-native Mixer content + its bus-management systems.
pub struct NativeMixer;

impl Plugin for NativeMixer {
    fn build(&self, app: &mut App) {
        app.register_panel_content("mixer", false, build);
        app.add_systems(
            Update,
            (bus_create, bus_delete).run_if(in_state(SplashState::Editor)),
        );
    }
}

/// Create a custom bus when the field is submitted (Enter) or Create is clicked.
fn bus_create(
    create: Query<&Interaction, (With<BusCreate>, Changed<Interaction>)>,
    mut input: Query<&mut EmberTextInput, With<BusNameInput>>,
    mut mixer: ResMut<MixerState>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    let clicked = create.iter().any(|i| *i == Interaction::Pressed);
    for mut inp in &mut input {
        let entered = inp.value.contains('\n');
        if !clicked && !entered {
            continue;
        }
        let name = inp.value.split('\n').next().unwrap_or("").trim().to_string();
        let (text_e, ph) = (inp.text_entity, inp.placeholder.clone());
        if !name.is_empty() {
            mixer.custom_buses.push((name, ChannelStrip::default()));
        }
        inp.value.clear();
        if let Ok((mut t, mut c)) = texts.get_mut(text_e) {
            *t = Text::new(ph);
            c.0 = rgb(TEXT_MUTED);
        }
    }
}

/// Delete a custom bus when its × button is clicked.
fn bus_delete(
    buttons: Query<(&Interaction, &BusDelete), Changed<Interaction>>,
    mut mixer: ResMut<MixerState>,
) {
    for (interaction, del) in &buttons {
        if *interaction == Interaction::Pressed && del.0 < mixer.custom_buses.len() {
            mixer.custom_buses.remove(del.0);
        }
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

    let master = strip(commands, fonts, "Master", None, |m| Some(&m.master), |m| Some(&mut m.master));
    let sfx = strip(commands, fonts, "SFX", None, |m| Some(&m.sfx), |m| Some(&mut m.sfx));
    let music = strip(commands, fonts, "Music", None, |m| Some(&m.music), |m| Some(&mut m.music));
    let ambient = strip(commands, fonts, "Ambient", None, |m| Some(&m.ambient), |m| Some(&mut m.ambient));

    let custom = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    keyed_list(commands, custom, custom_snapshot);

    let add = add_bus_field(commands, fonts);

    commands.entity(root).add_children(&[master, sfx, music, ambient, custom, add]);
    root
}

/// The "+ new custom bus" field at the end of the strip row.
fn add_bus_field(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Px(120.0),
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .id();
    let label = commands
        .spawn((Text::new("New bus"), ui_font(&fonts.ui, 10.0), TextColor(rgb(TEXT_MUTED))))
        .id();
    let input = text_input(commands, &fonts.ui, "Bus name", "");
    commands.entity(input).insert(BusNameInput);
    let create = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((42, 42, 52))),
            Interaction::default(),
            BusCreate,
            Name::new("mixer-add-bus"),
        ))
        .id();
    let create_label = commands
        .spawn((Text::new("Create"), ui_font(&fonts.ui, 11.0), TextColor(rgb(TEXT_PRIMARY))))
        .id();
    commands.entity(create).add_child(create_label);
    commands.entity(col).add_children(&[label, input, create]);
    col
}

/// One channel strip: name, fader + VU, pan knob, mute/solo — all two-way bound
/// to the bus the `sel`/`sel_mut` accessors point at. `delete` (custom buses
/// only) adds a × button that removes that bus.
fn strip<Sel, SelMut>(commands: &mut Commands, fonts: &EmberFonts, name: &str, delete: Option<usize>, sel: Sel, sel_mut: SelMut) -> Entity
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

    // Header: name + (custom buses) a × delete button.
    let header = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let label = commands
        .spawn((Text::new(name), ui_font(&fonts.ui, 11.0), TextColor(rgb(TEXT_PRIMARY))))
        .id();
    let cog = device_cog(commands, fonts, sel, sel_mut);
    let mut header_kids = vec![label, cog];
    if let Some(i) = delete {
        let del = commands
            .spawn((
                Node {
                    width: Val::Px(16.0),
                    height: Val::Px(16.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(rgb((50, 40, 40))),
                Interaction::default(),
                BusDelete(i),
                Name::new("mixer-bus-delete"),
            ))
            .id();
        let x = commands
            .spawn((Text::new("\u{00d7}"), ui_font(&fonts.ui, 12.0), TextColor(rgb((220, 120, 110)))))
            .id();
        commands.entity(del).add_child(x);
        header_kids.push(del);
    }
    commands.entity(header).add_children(&header_kids);

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
        .add_children(&[header, meters, pan, pan_label, buttons]);
    col
}

/// Read a bus's current input/output device name.
fn read_device<Sel: Fn(&MixerState) -> Option<&ChannelStrip>>(w: &World, sel: Sel, input: bool) -> Option<String> {
    read(w, sel, |s| {
        if input {
            s.input_device.clone()
        } else {
            s.output_device.clone()
        }
    })
}

/// The per-strip device-routing cog: a gear that opens a popover with selectable
/// input/output device lists, writing the bus's `input_device`/`output_device`.
fn device_cog<Sel, SelMut>(commands: &mut Commands, fonts: &EmberFonts, sel: Sel, sel_mut: SelMut) -> Entity
where
    Sel: Fn(&MixerState) -> Option<&ChannelStrip> + Copy + Send + Sync + 'static,
    SelMut: Fn(&mut MixerState) -> Option<&mut ChannelStrip> + Copy + Send + Sync + 'static,
{
    let content = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Px(200.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();

    let mut kids: Vec<Entity> = Vec::new();
    kids.push(section_label(commands, fonts, "Input device"));
    kids.push(device_row(commands, fonts, "(none)", None, true, sel, sel_mut));
    for name in renzora_audio::list_input_devices() {
        kids.push(device_row(commands, fonts, &name.clone(), Some(name), true, sel, sel_mut));
    }
    kids.push(spacer(commands, 6.0));
    kids.push(section_label(commands, fonts, "Output device"));
    kids.push(device_row(commands, fonts, "(none)", None, false, sel, sel_mut));
    for name in renzora_audio::list_output_devices() {
        kids.push(device_row(commands, fonts, &name.clone(), Some(name), false, sel, sel_mut));
    }
    commands.entity(content).add_children(&kids);

    icon_popover(commands, fonts, "gear", 13.0, content)
}

fn section_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(TEXT_PRIMARY))))
        .id()
}

fn spacer(commands: &mut Commands, h: f32) -> Entity {
    commands
        .spawn(Node {
            height: Val::Px(h),
            ..default()
        })
        .id()
}

/// A selectable device row: highlights when it's the bus's current device, and
/// on click writes it (one-shot edge-detected via `react`).
fn device_row<Sel, SelMut>(commands: &mut Commands, fonts: &EmberFonts, label: &str, device: Option<String>, input: bool, sel: Sel, sel_mut: SelMut) -> Entity
where
    Sel: Fn(&MixerState) -> Option<&ChannelStrip> + Copy + Send + Sync + 'static,
    SelMut: Fn(&mut MixerState) -> Option<&mut ChannelStrip> + Copy + Send + Sync + 'static,
{
    let row = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            Name::new("device-row"),
        ))
        .id();
    let t = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 10.5), TextColor(rgb(TEXT_PRIMARY))))
        .id();
    commands.entity(row).add_child(t);

    // Highlight when selected.
    let dev_hi = device.clone();
    bind_bg(commands, row, move |w| {
        if read_device(w, sel, input).as_deref() == dev_hi.as_deref() {
            rgb(ACCENT_BLUE).with_alpha(0.30)
        } else {
            Color::NONE
        }
    });

    // Click → set the device (edge-detected).
    let dev_set = device;
    let mut was_pressed = false;
    react(commands, move |world| {
        if world.get_entity(row).is_err() {
            return false;
        }
        let pressed = matches!(world.get::<Interaction>(row), Some(Interaction::Pressed));
        if pressed && !was_pressed {
            let d = dev_set.clone();
            write(world, sel_mut, move |s| {
                if input {
                    s.input_device = d.clone();
                } else {
                    s.output_device = d.clone();
                }
            });
        }
        was_pressed = pressed;
        true
    });
    row
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
                Some(i),
                move |m: &MixerState| m.custom_buses.get(i).map(|(_, s)| s),
                move |m: &mut MixerState| m.custom_buses.get_mut(i).map(|(_, s)| s),
            )
        }),
    }
}

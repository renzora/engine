//! Bevy-native (ember) Mixer — channel strips wired two-way to `MixerState`.
//!
//! Each strip's volume fader, pan knob, mute/solo buttons and VU meter are bound
//! to the bus's `ChannelStrip` through the generic `bind_2way` (one line per
//! control, no panel-specific binder).
//!
//! Everything that isn't a live control lives on the strip's **right-click
//! menu** rather than on the strip: colour, device routing, rename, delete. The
//! strip used to carry a settings cog whose popover was anchored to it, so a
//! strip near an edge of the panel opened its device list off-screen — and at
//! 74px wide there is no room for chrome next to the name anyway. Ember's
//! [`screen_menu_flip`] is positioned in window coordinates and clamped to the
//! window ([`ScreenMenu`](renzora_ember::widgets::ScreenMenu)), so the menu finds
//! room by itself wherever the strip happens to sit.
//!
//! Naming is the same story from the other side: the `+` tile creates a bus
//! outright (`MixerState::add_bus` names it) instead of opening a form first,
//! and the name is changed by double-clicking the strip header — the hierarchy
//! panel's inline rename, and for the same reason (the edit happens where the
//! name is shown).

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;

use renzora_audio::{rename_custom_bus, ChannelStrip, MixerState, BUS_COLORS};
use renzora::SplashState;
use renzora_ember::reactive::Rx;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::tracked::{
    bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list,
};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    fader, knob, menu_header, menu_item, menu_item_styled, menu_sep, menu_submenu, mixer_button,
    screen_menu_flip, text_input, vu_meter_bound, EmberTextInput, MenuAction,
};

const RED: (u8, u8, u8) = (225, 90, 80);
/// Max linear volume (1.0 = unity, 1.5 = +3.5 dB head-room).
const VOL_MAX: f64 = 1.5;
/// Max gap between the two presses of a header double-click, in seconds. Matches
/// ember's own text-input double-click window.
const DOUBLE_CLICK_SECS: f64 = 0.4;
/// Width of the inline rename field. Deliberately wider than the 74px strip and
/// centred over it: a field that fitted inside the strip would be ~45px, which is
/// not a box you can type a name into. It floats over its neighbours for the few
/// seconds the rename lasts.
const RENAME_W: f32 = 150.0;

/// An RGB triple from [`BUS_COLORS`] / `ChannelStrip::color` as a bevy `Color`.
/// The theme's `rgb` takes a tuple; strip colours are stored as arrays so they
/// compare and serialize cleanly.
fn tint(color: [u8; 3]) -> Color {
    rgb((color[0], color[1], color[2]))
}

/// Which bus a strip's controls address.
///
/// This replaces the pair of accessor closures the strips used to carry: the
/// context menu is built by a *system*, long after `build` ran, so it needs
/// something it can read off the strip under the cursor — a `Component`, not a
/// closure baked into the builder.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum BusRef {
    Master,
    Sfx,
    Music,
    Ambient,
    /// Index into `MixerState::custom_buses`.
    Custom(usize),
}

impl BusRef {
    fn get(self, mixer: &MixerState) -> Option<&ChannelStrip> {
        match self {
            BusRef::Master => Some(&mixer.master),
            BusRef::Sfx => Some(&mixer.sfx),
            BusRef::Music => Some(&mixer.music),
            BusRef::Ambient => Some(&mixer.ambient),
            BusRef::Custom(i) => mixer.custom_buses.get(i).map(|(_, s)| s),
        }
    }

    fn get_mut(self, mixer: &mut MixerState) -> Option<&mut ChannelStrip> {
        match self {
            BusRef::Master => Some(&mut mixer.master),
            BusRef::Sfx => Some(&mut mixer.sfx),
            BusRef::Music => Some(&mut mixer.music),
            BusRef::Ambient => Some(&mut mixer.ambient),
            BusRef::Custom(i) => mixer.custom_buses.get_mut(i).map(|(_, s)| s),
        }
    }

    /// `Some(index)` for a user-created bus. The four built-ins answer `None`:
    /// their names are the routing keys `play_on_bus` matches literally, so they
    /// can be neither renamed nor deleted.
    fn custom(self) -> Option<usize> {
        match self {
            BusRef::Custom(i) => Some(i),
            _ => None,
        }
    }
}

/// Click target for a custom bus's delete (×) button.
#[derive(Component)]
struct BusDelete(usize);

/// The `+` tile. No form behind it — [`MixerState::add_bus`] picks the name.
#[derive(Component)]
struct BusAdd;

/// The clickable name area in a strip header (label + its hidden rename field).
#[derive(Component, Clone, Copy)]
struct BusHeader(BusRef);

/// Marks a strip's inline rename field with the custom-bus index it renames.
#[derive(Component)]
struct BusRenameInput(usize);

/// The custom bus currently being renamed (`None` = nobody). The field is built
/// with every custom strip and simply hidden, so switching into rename mode is a
/// `bind_display` flip rather than a rebuild — which matters because a rebuild
/// would despawn the very field the user is typing into.
#[derive(Resource, Default)]
struct MixerRename(Option<usize>);

/// Registers the bevy-native Mixer content + its bus-management systems.
pub struct NativeMixer;

impl Plugin for NativeMixer {
    fn build(&self, app: &mut App) {
        app.init_resource::<MixerRename>()
            .register_panel_content("mixer", false, build)
            .systems(
                Update,
                (
                    bus_add,
                    bus_delete,
                    strip_context_menu,
                    header_double_click,
                    rename_focus,
                    rename_commit,
                )
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}

/// Create a custom bus, pre-named, when the `+` tile is clicked.
fn bus_add(
    buttons: Query<&Interaction, (With<BusAdd>, Changed<Interaction>)>,
    mut mixer: ResMut<MixerState>,
    mut rename: ResMut<MixerRename>,
) {
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        // Drop any rename in flight: its index is about to mean a different bus
        // if the list shifts, and the user's attention is on the new strip.
        rename.0 = None;
        mixer.add_bus();
    }
}

/// Delete a custom bus when its × button is clicked.
fn bus_delete(
    buttons: Query<(&Interaction, &BusDelete), Changed<Interaction>>,
    mut mixer: ResMut<MixerState>,
    mut rename: ResMut<MixerRename>,
) {
    for (interaction, del) in &buttons {
        if *interaction == Interaction::Pressed && del.0 < mixer.custom_buses.len() {
            mixer.custom_buses.remove(del.0);
            rename.0 = None;
        }
    }
}

// Bus accessor shorthand: `Option<&ChannelStrip>` so a vanished custom bus
// degrades gracefully instead of panicking. Reads go through the `Rx` rather
// than `untracked()`, so a mixer binding only recomputes on frames where the
// mixer actually changed — with a fader, a VU, a knob, two toggles, a dB readout
// and a colour per strip, that is the difference between ~8 recomputes per strip
// per frame and none.
fn read<R: Default>(rx: &Rx, bus: BusRef, f: impl Fn(&ChannelStrip) -> R) -> R {
    rx.get_resource::<MixerState>()
        .and_then(|m| bus.get(m).map(&f))
        .unwrap_or_default()
}

fn write(w: &mut World, bus: BusRef, f: impl Fn(&mut ChannelStrip)) {
    if let Some(mut m) = w.get_resource_mut::<MixerState>() {
        if let Some(s) = bus.get_mut(&mut m) {
            f(s);
        }
    }
}

/// A bus's display name — the built-ins' fixed labels, or the current name of
/// the custom bus at that index.
fn bus_name(rx: &Rx, bus: BusRef) -> String {
    match bus {
        BusRef::Master => "Master".to_string(),
        BusRef::Sfx => "SFX".to_string(),
        BusRef::Music => "Music".to_string(),
        BusRef::Ambient => "Ambient".to_string(),
        BusRef::Custom(i) => rx
            .get_resource::<MixerState>()
            .and_then(|m| m.custom_buses.get(i).map(|(n, _)| n.clone()))
            .unwrap_or_default(),
    }
}

fn renaming(rx: &Rx, index: usize) -> bool {
    rx.get_resource::<MixerRename>()
        .and_then(|r| r.0)
        .is_some_and(|i| i == index)
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // `align_items: Stretch` + a full-height root is what makes the strips fill
    // the panel. They used to be `FlexStart` at a fixed 120px fader, so a mixer
    // in a half-height bottom panel and one filling the screen looked identical
    // and both left most of the panel empty — while the fader, the control you
    // actually aim at, stayed too short to aim at precisely.
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            padding: UiRect::all(Val::Px(10.0)),
            column_gap: Val::Px(6.0),
            overflow: Overflow::clip(),
            ..default()
        })
        .id();

    let sfx = strip(commands, fonts, false, BusRef::Sfx);
    let music = strip(commands, fonts, false, BusRef::Music);
    let ambient = strip(commands, fonts, false, BusRef::Ambient);

    let custom = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    keyed_list(commands, custom, custom_snapshot);

    let add = add_bus_button(commands, fonts);

    // Master last and fenced off, the way a desk is laid out: everything to the
    // left feeds it, so it isn't a peer of the buses and shouldn't sit in their
    // run. The rule is the cheapest way to say that.
    let rule = commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                margin: UiRect::horizontal(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(border())),
            Name::new("mixer-master-rule"),
        ))
        .id();
    let master = strip(commands, fonts, true, BusRef::Master);

    // A flex spacer before the rule pins Master to the right edge however many
    // buses exist, instead of letting it drift with the count.
    let spring = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() },
            Name::new("mixer-spring"),
        ))
        .id();

    commands
        .entity(root)
        .add_children(&[sfx, music, ambient, custom, add, spring, rule, master]);
    root
}

/// The "add a bus" tile at the end of the bus run: a `+` the width of half a
/// strip, which creates the bus on click.
///
/// It was a permanently-open form — a label, a text field and a button — sitting
/// in the strip row at full strip width, then a popover carrying that same form.
/// Both made you name a bus before you could see one. The bus is named for you
/// now (`Bus 1`, `Bus 2`, …), so the tile does the thing instead of asking about
/// it, and the name is a double-click on the new strip's header away.
fn add_bus_button(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // The tile: a placeholder column the same height as a strip, so the row reads
    // as "…and there could be another one here".
    let tile = commands
        .spawn((
            Node {
                width: Val::Px(38.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            BusAdd,
            hover_pointer(),
            renzora_ember::widgets::HoverTooltip::new("Add a bus"),
            Name::new("mixer-add-bus-tile"),
        ))
        .id();
    let glyph = renzora_ember::font::icon_text(commands, &fonts.phosphor, "plus", text_muted(), 16.0);
    commands.entity(tile).add_child(glyph);
    // Light up on hover so a tile that does something on click looks like it.
    bind_bg(commands, tile, move |rx| match rx.get::<Interaction>(tile) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });
    tile
}

/// Pointer cursor on hover — the one-liner every clickable tile here wants.
fn hover_pointer() -> renzora_ember::cursor_icon::HoverCursor {
    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

/// A bus's linear volume as a dB string. `0.0` is silence, not `-inf` dB
/// arithmetic, so it gets the symbol rather than a number.
fn db_text(v: f64) -> String {
    if v <= 0.0001 {
        "-\u{221e}".to_string()
    } else {
        format!("{:+.1}", 20.0 * v.log10())
    }
}

/// One channel strip: colour bar, name, pan knob, fader + VU, dB readout,
/// mute/solo — all two-way bound to `bus`. Custom buses additionally get a ×
/// delete button and an inline rename field.
fn strip(commands: &mut Commands, fonts: &EmberFonts, is_master: bool, bus: BusRef) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Px(74.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            // Master reads as a surface above the buses rather than beside them.
            BackgroundColor(rgb(if is_master { section_bg() } else { window_bg() })),
            BorderColor::all(rgb(border())),
            // Geometry-based hit test for the right-click menu: a strip is mostly
            // covered by its own controls, so an `Interaction` on the root would
            // only ever fire on the gaps between them.
            RelativeCursorPosition::default(),
            bus,
            Name::new("mixer-strip"),
        ))
        .id();
    // The strip's colour reads as a tinted frame rather than a filled block: at
    // 74px a solid colour would fight the fader and VU meter for attention.
    bind_with(
        commands,
        col,
        move |rx: &Rx| read(rx, bus, |s| s.color),
        |world: &mut World, e: Entity, color: &[u8; 3]| {
            if let Some(mut border) = world.get_mut::<BorderColor>(e) {
                *border = BorderColor::all(tint(*color).with_alpha(0.55));
            }
        },
    );

    // Colour bar — the part you actually scan when picking a strip out of a row.
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(3.0),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(tint(BUS_COLORS[11])),
            Name::new("mixer-strip-color"),
        ))
        .id();
    bind_bg(commands, bar, move |rx| tint(read(rx, bus, |s| s.color)));

    let header = strip_header(commands, fonts, is_master, bus);

    // The one node that grows. Everything above and below it is fixed, so the
    // fader and its meter absorb whatever height the panel has — which is the
    // whole point of a fader: the taller it is, the finer you can set it.
    let meters = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(8.0),
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(40.0),
            ..default()
        })
        .id();
    let vol = fader(commands, 0.0);
    // Both widgets ship at a fixed 120px; let them stretch. Their fills and
    // hit-testing are percentage/normalized based, so height is free to vary.
    grow_vertically(commands, vol);
    bind_2way(
        commands,
        vol,
        move |rx| read(rx, bus, |s| ((s.volume / VOL_MAX) as f32).clamp(0.0, 1.0)),
        move |w, v| {
            let nv = *v as f64 * VOL_MAX;
            write(w, bus, move |s| s.volume = nv);
        },
    );
    let vu = vu_meter_bound(commands, move |rx| {
        read(rx, bus, |s| (s.peak_level / VOL_MAX as f32).clamp(0.0, 1.0))
    });
    grow_vertically(commands, vu);
    commands.entity(meters).add_children(&[vol, vu]);

    // The number the fader was never showing. A fader with no readout tells you
    // where the cap is, not what the gain is — and "somewhere near the top" is
    // not a level you can reproduce or talk about.
    let db = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    bind_text(commands, db, move |rx| db_text(read(rx, bus, |s| s.volume)));

    // Pan: -1..1 mapped to the knob's 0..1. Above the fader and smaller than it
    // was — it's a trim, and it had been outweighing the control it trims.
    let pan = knob(commands, 0.5);
    commands.queue(move |w: &mut World| {
        if let Some(mut n) = w.get_mut::<Node>(pan) {
            n.width = Val::Px(30.0);
            n.height = Val::Px(30.0);
        }
    });
    bind_2way(
        commands,
        pan,
        move |rx| read(rx, bus, |s| (((s.panning + 1.0) / 2.0) as f32).clamp(0.0, 1.0)),
        move |w, v| {
            let np = (*v as f64) * 2.0 - 1.0;
            write(w, bus, move |s| s.panning = np);
        },
    );
    let pan_label = commands
        .spawn((Text::new("Pan"), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_muted()))))
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
        move |rx| read(rx, bus, |s| s.muted),
        move |w, v| {
            let nv = *v;
            write(w, bus, move |s| s.muted = nv);
        },
    );
    let solo = mixer_button(commands, fonts, "S", rgb(accent()));
    bind_2way(
        commands,
        solo,
        move |rx| read(rx, bus, |s| s.soloed),
        move |w, v| {
            let nv = *v;
            write(w, bus, move |s| s.soloed = nv);
        },
    );
    commands.entity(buttons).add_children(&[mute, solo]);

    commands
        .entity(col)
        .add_children(&[bar, header, pan, pan_label, meters, db, buttons]);
    col
}

/// Let a fixed-height widget stretch to its parent's height.
fn grow_vertically(commands: &mut Commands, e: Entity) {
    commands.queue(move |w: &mut World| {
        if let Some(mut n) = w.get_mut::<Node>(e) {
            n.height = Val::Auto;
            n.flex_grow = 1.0;
            n.min_height = Val::Px(0.0);
        }
    });
}

/// The strip header: the bus name (double-click to rename, custom buses only)
/// and, for custom buses, a × that deletes it.
fn strip_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    is_master: bool,
    bus: BusRef,
) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .id();

    // The name area owns the double-click, and is the anchor the rename field
    // floats over. It's a node rather than the bare text so the whole width of
    // the header is a rename target, not just the glyphs.
    let name_area = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            BusHeader(bus),
            Name::new("mixer-strip-name"),
        ))
        .id();

    let mut name_font = ui_font(&fonts.ui, 11.0);
    if is_master {
        name_font.weight = bevy::text::FontWeight::SEMIBOLD;
    }
    let label = commands
        .spawn((
            Text::new(""),
            name_font,
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    bind_text(commands, label, move |rx| bus_name(rx, bus));
    let mut name_kids = vec![label];

    if let Some(index) = bus.custom() {
        commands.entity(name_area).insert(hover_pointer());
        bind_display(commands, label, move |rx| !renaming(rx, index));

        let input = text_input(commands, &fonts.ui, "Name", "");
        commands.entity(input).insert((
            BusRenameInput(index),
            // Floated over the strip and centred on the name: see `RENAME_W`.
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-RENAME_W / 2.0)),
                width: Val::Px(RENAME_W),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                // Horizontal padding must stay at ember's `PAD_X` (8px) or the
                // caret hit-test measures from the wrong origin.
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                display: Display::None,
                ..default()
            },
            // Above the neighbouring strips it overhangs.
            GlobalZIndex(60),
        ));
        bind_display(commands, input, move |rx| renaming(rx, index));
        name_kids.push(input);
    }
    commands.entity(name_area).add_children(&name_kids);

    let mut kids = vec![name_area];
    if let Some(index) = bus.custom() {
        let del = commands
            .spawn((
                Node {
                    width: Val::Px(14.0),
                    height: Val::Px(14.0),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(rgb(section_bg())),
                Interaction::default(),
                BusDelete(index),
                hover_pointer(),
                Name::new("mixer-bus-delete"),
            ))
            .id();
        let x = commands
            .spawn((
                Text::new("\u{00d7}"),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(close_red())),
            ))
            .id();
        commands.entity(del).add_child(x);
        // Hidden while renaming so the field has the whole header to itself.
        bind_display(commands, del, move |rx| !renaming(rx, index));
        kids.push(del);
    }
    commands.entity(row).add_children(&kids);
    row
}

// ── Inline rename ────────────────────────────────────────────────────────────

/// Double-click a strip's name → rename it (custom buses only).
fn header_double_click(
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    headers: Query<(&Interaction, &BusHeader)>,
    mut rename: ResMut<MixerRename>,
    mut last_click: Local<Option<(usize, f64)>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(index) = headers
        .iter()
        .find(|(i, _)| matches!(i, Interaction::Pressed))
        .and_then(|(_, h)| h.0.custom())
    else {
        return;
    };
    let now = time.elapsed_secs_f64();
    if last_click.is_some_and(|(i, t)| i == index && now - t < DOUBLE_CLICK_SECS) {
        *last_click = None;
        rename.0 = Some(index);
        return;
    }
    *last_click = Some((index, now));
}

/// Focus the rename field (and select its contents) one frame after the rename
/// starts.
///
/// The delay is load-bearing: the double-click that started the rename is still
/// in flight on the frame `MixerRename` is set, and the field is still hidden, so
/// ember's `text_input_focus` sees a press that landed in no input and blurs
/// every field — including one focused earlier in the same frame.
fn rename_focus(
    rename: Res<MixerRename>,
    mixer: Option<Res<MixerState>>,
    mut inputs: Query<(&mut EmberTextInput, &BusRenameInput)>,
    mut last: Local<Option<usize>>,
    mut armed: Local<Option<usize>>,
) {
    if let Some(index) = armed.take() {
        let name = mixer
            .as_deref()
            .and_then(|m| m.custom_buses.get(index).map(|(n, _)| n.clone()))
            .unwrap_or_default();
        for (mut input, target) in &mut inputs {
            if target.0 != index {
                continue;
            }
            // Re-seed from state: the field survives a cancelled rename, so
            // whatever was half-typed then must not come back now.
            input.value = name.clone();
            input.focused = true;
            input.select_all = true;
            input.sel_anchor = None;
            input.caret_index = name.chars().count();
        }
    }
    if rename.0 != *last {
        *last = rename.0;
        *armed = rename.0;
    }
}

/// Commit (Enter / click away) or cancel (Escape) the active rename.
fn rename_commit(
    mut rename: ResMut<MixerRename>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut inputs: Query<(&mut EmberTextInput, &RelativeCursorPosition, &BusRenameInput)>,
    mut commands: Commands,
    mut had_focus: Local<bool>,
) {
    let Some(index) = rename.0 else {
        *had_focus = false;
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        rename.0 = None;
        *had_focus = false;
        return;
    }
    let Some((mut input, rcp, _)) = inputs.iter_mut().find(|(_, _, t)| t.0 == index) else {
        rename.0 = None;
        *had_focus = false;
        return;
    };
    // Clicking inside the field to move the caret must keep it editing; the
    // header's click layer sits under it and can otherwise steal focus. Decided
    // on the field's own geometry rather than pick order.
    if mouse.just_pressed(MouseButton::Left) && rcp.cursor_over && !input.focused {
        input.focused = true;
    }
    if input.focused {
        *had_focus = true;
    }
    // Nothing commits until the field has actually held focus — otherwise the
    // second press of the double-click that opened it would close it again.
    if !*had_focus {
        return;
    }

    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let clicked_away = mouse.just_pressed(MouseButton::Left) && !rcp.cursor_over;
    if !enter && !clicked_away {
        return;
    }
    let new = input.value.replace('\n', "").trim().to_string();
    rename.0 = None;
    *had_focus = false;
    if new.is_empty() {
        return;
    }
    // `rename_custom_bus` re-points every AudioPlayer / timeline track that
    // routes by the old name; a rejected name (taken, unchanged) is a no-op.
    commands.queue(move |world: &mut World| {
        rename_custom_bus(world, index, &new);
    });
}

// ── Right-click menu ─────────────────────────────────────────────────────────

/// Right-click a strip → its menu: rename, colour, device routing, delete.
///
/// The menu is rebuilt on every open and any click inside it closes the menu, so
/// "what is selected right now" is fixed for the menu's whole lifetime — the
/// swatch ring and the device ticks are read once here rather than bound.
fn strip_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    fonts: Option<Res<EmberFonts>>,
    mixer: Option<Res<MixerState>>,
    strips: Query<(&RelativeCursorPosition, &BusRef)>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let (Some(fonts), Some(mixer)) = (fonts, mixer) else {
        return;
    };
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Some(bus) = strips
        .iter()
        .find(|(rcp, _)| rcp.cursor_over)
        .map(|(_, b)| *b)
    else {
        return;
    };
    let Some(current) = bus.get(&mixer) else { return };
    let (color, input_dev, output_dev) = (
        current.color,
        current.input_device.clone(),
        current.output_device.clone(),
    );

    let menu = screen_menu_flip(&mut commands, cursor.x, cursor.y, window.height());
    let mut kids: Vec<Entity> = Vec::new();

    if let Some(index) = bus.custom() {
        kids.push(menu_item(
            &mut commands,
            &fonts,
            "pencil-simple",
            "Rename",
            move |w| {
                if let Some(mut r) = w.get_resource_mut::<MixerRename>() {
                    r.0 = Some(index);
                }
            },
        ));
        kids.push(menu_sep(&mut commands));
    }

    kids.push(menu_header(&mut commands, &fonts, "Colour"));
    kids.push(swatch_grid(&mut commands, bus, color));

    kids.push(menu_sep(&mut commands));
    kids.push(device_submenu(&mut commands, &fonts, bus, true, input_dev.as_deref()));
    kids.push(device_submenu(&mut commands, &fonts, bus, false, output_dev.as_deref()));

    if let Some(index) = bus.custom() {
        kids.push(menu_sep(&mut commands));
        kids.push(menu_item_styled(
            &mut commands,
            &fonts,
            "trash",
            "Delete bus",
            close_red(),
            close_red(),
            move |w| {
                if let Some(mut m) = w.get_resource_mut::<MixerState>() {
                    if index < m.custom_buses.len() {
                        m.custom_buses.remove(index);
                    }
                }
                if let Some(mut r) = w.get_resource_mut::<MixerRename>() {
                    r.0 = None;
                }
            },
        ));
    }

    commands.entity(menu).add_children(&kids);
}

/// A wrapping grid of colour swatches; clicking one recolours the strip (and
/// closes the menu, via the swatch's [`MenuAction`]). `current` is ringed rather
/// than ticked — a tick would hide the colour it marks.
fn swatch_grid(commands: &mut Commands, bus: BusRef, current: [u8; 3]) -> Entity {
    let grid = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Px(196.0),
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(2.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    let swatches: Vec<Entity> = BUS_COLORS
        .iter()
        .map(|&color| {
            commands
                .spawn((
                    Node {
                        width: Val::Px(16.0),
                        height: Val::Px(16.0),
                        border: UiRect::all(Val::Px(if color == current { 2.0 } else { 1.0 })),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(tint(color)),
                    BorderColor::all(if color == current {
                        rgb(text_primary())
                    } else {
                        rgb(border())
                    }),
                    Interaction::default(),
                    hover_pointer(),
                    MenuAction(Box::new(move |w: &mut World| {
                        write(w, bus, move |s| s.color = color);
                    })),
                    Name::new("mixer-color-swatch"),
                ))
                .id()
        })
        .collect();
    commands.entity(grid).add_children(&swatches);
    grid
}

/// An "Input device ▸" / "Output device ▸" submenu listing "(none)" plus every
/// device cpal reports, ticked at `current`.
///
/// Devices are enumerated when the menu opens rather than when the panel is
/// built, so a mic plugged in after the editor started shows up without a panel
/// rebuild — which the old cog popover, built once with the panel, never did.
fn device_submenu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    bus: BusRef,
    input: bool,
    current: Option<&str>,
) -> Entity {
    let (icon, label) = if input {
        ("microphone", "Input device")
    } else {
        ("speaker-high", "Output device")
    };
    let (row, content) = menu_submenu(commands, fonts, icon, label);

    let devices = if input {
        renzora_audio::list_input_devices()
    } else {
        renzora_audio::list_output_devices()
    };
    let mut items = vec![device_row(commands, fonts, "(none)", None, bus, input, current.is_none())];
    for name in devices {
        let on = current == Some(name.as_str());
        items.push(device_row(commands, fonts, &name.clone(), Some(name), bus, input, on));
    }
    commands.entity(content).add_children(&items);
    row
}

/// A selectable device row: ticked when it is the bus's current device, and on
/// click writes it (and closes the menu, via [`MenuAction`]).
#[allow(clippy::too_many_arguments)]
fn device_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    device: Option<String>,
    bus: BusRef,
    input: bool,
    selected: bool,
) -> Entity {
    let (icon, icon_color) = if selected {
        ("check", accent())
    } else {
        ("circle", text_muted())
    };
    menu_item_styled(
        commands,
        fonts,
        icon,
        label,
        icon_color,
        text_primary(),
        move |w: &mut World| {
            let device = device.clone();
            write(w, bus, move |s| {
                if input {
                    s.input_device = device.clone();
                } else {
                    s.output_device = device.clone();
                }
            });
        },
    )
}

fn custom_snapshot(world: &Rx) -> KeyedSnapshot {
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
        build: Box::new(move |c, f, i| strip(c, f, false, BusRef::Custom(i))),
    }
}

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
//!
//! The panel's own **shape** is the one thing a menu can't own, so it sits on a
//! top bar. On the left, the page: **Mixer** (these strips), **Routing** (where
//! each bus goes and what it sends, [`crate::routing`]) and **Metering** (long
//! meters with peak hold and clip lights, [`crate::metering`]). On the right,
//! for the Mixer page, strip size (**Compact** / **Large**), **Horizontal**
//! (channels as rows rather than columns, [`MixerLayout`]) and **Add bus**.
//! They are labelled controls, not icon keys: you pick a shape by looking at
//! the result, and an icon you have to hover to decode makes that guesswork.
//! Changing any of them rebuilds the body wholesale; see [`body_snapshot`].

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;

use renzora_audio::{rename_custom_bus, ChannelStrip, MixerState, BUS_COLORS};
use renzora::SplashState;
use renzora_ember::reactive::Rx;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::tracked::{
    bind_2way, bind_bg, bind_display, bind_text, keyed_list,
};
use renzora_ember::theme::*;
use renzora_ember::UiInteraction;
use renzora_ember::widgets::{
    fader, fader_horizontal, knob_pivoted, menu_header, menu_item, menu_item_styled, menu_sep,
    menu_submenu, mixer_button, screen_menu_flip, scroll_view, segmented, text_input, vu_meter_bound,
    vu_meter_bound_horizontal, EmberTextInput, HoverTooltip, KnobWheel, MenuAction,
};

const RED: (u8, u8, u8) = (225, 90, 80);
/// Max linear volume (1.0 = unity, 1.5 = +3.5 dB head-room).
const VOL_MAX: f64 = 1.5;
/// Max gap between the two presses of a header double-click, in seconds. Matches
/// ember's own text-input double-click window.
const DOUBLE_CLICK_SECS: f64 = 0.4;
/// Width of the inline rename field. Deliberately wider than a vertical strip and
/// centred over it: a field that fitted inside the strip would be ~45px, which is
/// not a box you can type a name into. It floats over its neighbours for the few
/// seconds the rename lasts.
const RENAME_W: f32 = 150.0;

// ── Panel layout ─────────────────────────────────────────────────────────────

/// Which page the panel shows.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub(crate) enum MixerTab {
    #[default]
    Mixer,
    Routing,
    Metering,
}

impl MixerTab {
    const ALL: [MixerTab; 3] = [MixerTab::Mixer, MixerTab::Routing, MixerTab::Metering];

    fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// How much room a strip gets.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
enum Density {
    /// Roomier strips ("Large"): a bigger pan knob with its caption, and a dB
    /// scale beside the fader.
    Wide,
    /// Tighter strips, for fitting a large board in a short bottom panel.
    ///
    /// The default, because that short bottom panel is where the mixer lives.
    /// Wide was the default and assumed a board given real height; docked along
    /// the bottom at its usual size it simply meant fewer channels visible, and
    /// the first thing to do on opening the panel was to switch density.
    #[default]
    Compact,
}

/// Which way the channels run.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
enum Orientation {
    /// Strips stand up as columns across the panel — the classic desk.
    #[default]
    Vertical,
    /// Strips lie down as rows stacked down the panel, like a channel list.
    /// Fits far more buses in a wide, short panel, which is the shape the mixer
    /// usually gets when it's docked at the bottom.
    Horizontal,
}

/// The panel's shape, driven by the top bar.
///
/// Everything the strips measure themselves against is derived here rather than
/// spread through the builders, so a layout that reads badly is fixed by one
/// number, and the two orientations can't drift apart.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Hash, Default)]
struct MixerLayout {
    density: Density,
    orientation: Orientation,
}

impl MixerLayout {
    fn compact(self) -> bool {
        self.density == Density::Compact
    }

    fn horizontal(self) -> bool {
        self.orientation == Orientation::Horizontal
    }

    /// A strip's fixed dimension: its width when strips stand up, its height
    /// when they lie down. The other dimension is stretched by the run.
    ///
    /// The vertical minimum is set by the mute/solo pair (two 22px keys and a
    /// 5px gap), not by anything visual — below ~60px the buttons no longer fit
    /// the strip's content box.
    fn strip_span(self) -> f32 {
        match (self.orientation, self.density) {
            // Wide enough for a dB scale beside the fader and a full-width
            // mute/solo pair (design A); the old 62/80 left neither room.
            (Orientation::Vertical, Density::Compact) => 84.0,
            (Orientation::Vertical, Density::Wide) => 112.0,
            (Orientation::Horizontal, Density::Compact) => 40.0,
            (Orientation::Horizontal, Density::Wide) => 52.0,
        }
    }

    /// Diameter of the pan knob.
    fn knob(self) -> f32 {
        // Big enough to hold its value in the middle (see `strip`).
        if self.compact() {
            32.0
        } else {
            40.0
        }
    }

    /// Gap between a strip's controls.
    fn gap(self) -> f32 {
        if self.compact() {
            4.0
        } else {
            6.0
        }
    }

    /// A strip's inner padding — tight across the fixed dimension, since that's
    /// the one the density setting is rationing.
    fn strip_padding(self) -> UiRect {
        let across = Val::Px(if self.compact() { 4.0 } else { 6.0 });
        let along = Val::Px(if self.compact() { 6.0 } else { 8.0 });
        if self.horizontal() {
            UiRect::axes(along, across)
        } else {
            UiRect::axes(across, along)
        }
    }

    fn name_font(self) -> f32 {
        if self.compact() {
            10.0
        } else {
            11.0
        }
    }

    fn db_font(self) -> f32 {
        if self.compact() {
            9.0
        } else {
            10.0
        }
    }

    /// Whether the pan knob gets its "Pan" caption. Compact strips drop it — at
    /// 62px the word is the widest thing in the strip, and a knob sitting on its
    /// own above the fader is not something you mistake for anything else. A row
    /// drops it too: there the caption would cost width the fader wants.
    fn pan_label(self) -> bool {
        !self.compact() && !self.horizontal()
    }

    /// Preferred width of the name column in a horizontal strip — a *basis*, not
    /// a fixed width: it's the first thing to give ground when the panel gets
    /// narrow, because a clipped name still identifies the channel and a clipped
    /// mute button is unusable. (A vertical strip's name simply takes the strip's
    /// width.)
    fn header_w(self) -> f32 {
        if self.compact() {
            72.0
        } else {
            96.0
        }
    }
}

fn layout_of(rx: &Rx) -> MixerLayout {
    rx.get_resource::<MixerLayout>().copied().unwrap_or_default()
}

fn tab_of(rx: &Rx) -> MixerTab {
    rx.get_resource::<MixerTab>().copied().unwrap_or_default()
}

/// The top-bar Horizontal toggle.
#[derive(Component)]
struct OrientationBtn;

/// Which bus a strip's controls address.
///
/// This replaces the pair of accessor closures the strips used to carry: the
/// context menu is built by a *system*, long after `build` ran, so it needs
/// something it can read off the strip under the cursor — a `Component`, not a
/// closure baked into the builder.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BusRef {
    Master,
    Sfx,
    Music,
    Ambient,
    /// Index into `MixerState::custom_buses`.
    Custom(usize),
}

impl BusRef {
    /// The bus's permanent routing key.
    pub(crate) fn key(self, mixer: &MixerState) -> String {
        match self {
            BusRef::Master => "Master".to_string(),
            BusRef::Sfx => "Sfx".to_string(),
            BusRef::Music => "Music".to_string(),
            BusRef::Ambient => "Ambient".to_string(),
            BusRef::Custom(i) => mixer.custom_buses.get(i).map(|b| b.key.clone()).unwrap_or_default(),
        }
    }

    /// The bus with routing key `key`.
    pub(crate) fn from_key(mixer: &MixerState, key: &str) -> Option<BusRef> {
        Some(match key {
            "Master" => BusRef::Master,
            "Sfx" => BusRef::Sfx,
            "Music" => BusRef::Music,
            "Ambient" => BusRef::Ambient,
            _ => BusRef::Custom(mixer.bus_index(key)?),
        })
    }

    /// A built-in's fixed label (custom buses are named by the user; see
    /// [`bus_name`]).
    pub(crate) fn label(self) -> &'static str {
        match self {
            BusRef::Master => "Master",
            BusRef::Sfx => "SFX",
            BusRef::Music => "Music",
            BusRef::Ambient => "Ambient",
            BusRef::Custom(_) => "",
        }
    }

    pub(crate) fn get(self, mixer: &MixerState) -> Option<&ChannelStrip> {
        match self {
            BusRef::Master => Some(&mixer.master),
            BusRef::Sfx => Some(&mixer.sfx),
            BusRef::Music => Some(&mixer.music),
            BusRef::Ambient => Some(&mixer.ambient),
            BusRef::Custom(i) => mixer.custom_buses.get(i).map(|b| &b.strip),
        }
    }

    fn get_mut(self, mixer: &mut MixerState) -> Option<&mut ChannelStrip> {
        match self {
            BusRef::Master => Some(&mut mixer.master),
            BusRef::Sfx => Some(&mut mixer.sfx),
            BusRef::Music => Some(&mut mixer.music),
            BusRef::Ambient => Some(&mut mixer.ambient),
            BusRef::Custom(i) => mixer.custom_buses.get_mut(i).map(|b| &mut b.strip),
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

/// The top bar's Add bus button. No form behind it: [`MixerState::add_bus`]
/// picks the name.
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
pub struct MixerStrips;

impl Plugin for MixerStrips {
    fn build(&self, app: &mut App) {
        app.init_resource::<MixerRename>()
            .init_resource::<MixerLayout>()
            .init_resource::<MixerTab>()
            .init_resource::<crate::metering::MeterHold>()
            .register_panel_content("mixer", false, build)
            .systems(
                Update,
                (
                    bus_add,
                    bus_delete,
                    orientation_click,
                    strip_context_menu,
                    header_double_click,
                    rename_focus,
                    rename_commit,
                    // Always on, not only while Metering is shown: a clip that
                    // happened while you were on another page is exactly the
                    // one worth knowing about.
                    crate::metering::update_meter_hold,
                    crate::metering::meter_clicks,
                )
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}

/// Create a custom bus, pre-named, when the `+` tile is clicked.
fn bus_add(
    buttons: Query<&UiInteraction, (With<BusAdd>, Changed<UiInteraction>)>,
    mut mixer: ResMut<MixerState>,
    mut rename: ResMut<MixerRename>,
) {
    if buttons.iter().any(|i| *i == UiInteraction::Pressed) {
        // Drop any rename in flight: its index is about to mean a different bus
        // if the list shifts, and the user's attention is on the new strip.
        rename.0 = None;
        mixer.add_bus();
    }
}

/// Delete a custom bus when its × button is clicked.
fn bus_delete(
    buttons: Query<(&UiInteraction, &BusDelete), Changed<UiInteraction>>,
    mut mixer: ResMut<MixerState>,
    mut rename: ResMut<MixerRename>,
) {
    for (interaction, del) in &buttons {
        if *interaction == UiInteraction::Pressed && del.0 < mixer.custom_buses.len() {
            // Through `remove_bus`, so buses that fed this one go back to
            // Master instead of keeping a key nothing answers to.
            mixer.remove_bus(del.0);
            rename.0 = None;
        }
    }
}

/// Flip the channels between columns and rows.
///
/// Like a density change (see `top_bar`), it cancels any rename in flight: the
/// change respawns every strip, including the field being typed into, and a
/// half-typed name left pointing at a despawned entity is worse than no rename.
fn orientation_click(
    buttons: Query<&UiInteraction, (With<OrientationBtn>, Changed<UiInteraction>)>,
    mut layout: ResMut<MixerLayout>,
    mut rename: ResMut<MixerRename>,
) {
    if buttons.iter().any(|i| *i == UiInteraction::Pressed) {
        layout.orientation = match layout.orientation {
            Orientation::Vertical => Orientation::Horizontal,
            Orientation::Horizontal => Orientation::Vertical,
        };
        rename.0 = None;
    }
}

// Bus accessor shorthand: `Option<&ChannelStrip>` so a vanished custom bus
// degrades gracefully instead of panicking. Reads go through the `Rx` rather
// than `untracked()`, so a mixer binding only recomputes on frames where the
// mixer actually changed — with a fader, a VU, a knob, two toggles, a dB readout
// and a colour per strip, that is the difference between ~8 recomputes per strip
// per frame and none.
pub(crate) fn read<R: Default>(rx: &Rx, bus: BusRef, f: impl Fn(&ChannelStrip) -> R) -> R {
    rx.get_resource::<MixerState>()
        .and_then(|m| bus.get(m).map(&f))
        .unwrap_or_default()
}

pub(crate) fn write(w: &mut World, bus: BusRef, f: impl Fn(&mut ChannelStrip)) {
    if let Some(mut m) = w.get_resource_mut::<MixerState>() {
        if let Some(s) = bus.get_mut(&mut m) {
            f(s);
        }
    }
}

/// A bus's display name — the built-ins' fixed labels, or the current name of
/// the custom bus at that index.
pub(crate) fn bus_name(rx: &Rx, bus: BusRef) -> String {
    match bus {
        BusRef::Master | BusRef::Sfx | BusRef::Music | BusRef::Ambient => bus.label().to_string(),
        BusRef::Custom(i) => rx
            .get_resource::<MixerState>()
            .and_then(|m| m.custom_buses.get(i).map(|b| b.name.clone()))
            .unwrap_or_default(),
    }
}

fn renaming(rx: &Rx, index: usize) -> bool {
    rx.get_resource::<MixerRename>()
        .and_then(|r| r.0)
        .is_some_and(|i| i == index)
}

/// The panel: a top bar over the strip run.
fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("mixer-root"),
        ))
        .id();

    let bar = top_bar(commands, fonts);

    // The run lives in a keyed list of exactly one item so a layout change can
    // rebuild it from scratch — the two orientations are different node trees
    // (a row of columns vs a scrolling column of rows), not one tree with
    // different numbers in it, so there is nothing to re-bind in place.
    let host = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("mixer-body"),
        ))
        .id();
    keyed_list(commands, host, body_snapshot);

    commands.entity(root).add_children(&[bar, host]);
    root
}

/// One item, hashed over the layout and the bus *keys*.
///
/// Keys rather than display names: a key is permanent (`Bus::key`), so a rename
/// leaves the hash alone and the strip being typed into survives. A bus added,
/// deleted or reordered does change the set, and that genuinely wants a rebuild
/// — the strips address buses by index.
///
/// The page is in the hash too, and Routing adds its own shape (names and
/// outputs, see [`crate::routing::routing_hash`]). Metering needs only the
/// keys: its levels are bound, not built.
fn body_snapshot(rx: &Rx) -> KeyedSnapshot {
    let layout = layout_of(rx);
    let tab = tab_of(rx);
    let Some(mixer) = rx.get_resource::<MixerState>() else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    let keys: Vec<String> = mixer.custom_buses.iter().map(|b| b.key.clone()).collect();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    tab.hash(&mut h);
    keys.hash(&mut h);
    match tab {
        MixerTab::Mixer => layout.hash(&mut h),
        MixerTab::Routing => crate::routing::routing_hash(mixer, &mut h),
        MixerTab::Metering => {}
    }
    let customs = keys.len();
    let board = mixer.clone();
    KeyedSnapshot {
        items: vec![(0, h.finish())],
        build: Box::new(move |c, f, _| match tab {
            MixerTab::Mixer => body(c, f, layout, customs),
            MixerTab::Routing => crate::routing::body(c, f, &board),
            MixerTab::Metering => crate::metering::body(c, f, &board),
        }),
    }
}

/// The strip run: the three source buses, the user's buses, the `+` tile, then
/// Master fenced off behind a rule.
fn body(commands: &mut Commands, fonts: &EmberFonts, layout: MixerLayout, customs: usize) -> Entity {
    let horizontal = layout.horizontal();

    // `align_items: Stretch` is what makes the strips fill the panel across the
    // run's cross axis. They used to be `FlexStart` at a fixed 120px fader, so a
    // mixer in a half-height bottom panel and one filling the screen looked
    // identical and both left most of the panel empty — while the fader, the
    // control you actually aim at, stayed too short to aim at precisely.
    let run = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                // Rows size to their contents so the scroll view below has
                // something to overflow; columns fill the panel's height.
                height: if horizontal {
                    Val::Auto
                } else {
                    Val::Percent(100.0)
                },
                flex_direction: if horizontal {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                align_items: AlignItems::Stretch,
                padding: UiRect::all(Val::Px(10.0)),
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("mixer-run"),
        ))
        .id();

    let mut kids = vec![
        strip(commands, fonts, false, BusRef::Sfx, layout),
        strip(commands, fonts, false, BusRef::Music, layout),
        strip(commands, fonts, false, BusRef::Ambient, layout),
    ];
    for i in 0..customs {
        kids.push(strip(commands, fonts, false, BusRef::Custom(i), layout));
    }

    // A flex spacer before the rule pins Master to the far edge however many
    // buses exist, instead of letting it drift with the count. Rows don't get
    // one: they scroll, so there is no fixed far edge to pin to, and pushing
    // Master past the fold would hide the strip you look at most.
    if !horizontal {
        let spring = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    ..default()
                },
                Name::new("mixer-spring"),
            ))
            .id();
        kids.push(spring);
    }

    // Master last and fenced off, the way a desk is laid out: everything before
    // it feeds it, so it isn't a peer of the buses and shouldn't sit in their
    // run. The rule is the cheapest way to say that.
    let rule = commands
        .spawn((
            Node {
                width: if horizontal {
                    Val::Auto
                } else {
                    Val::Px(1.0)
                },
                height: if horizontal {
                    Val::Px(1.0)
                } else {
                    Val::Auto
                },
                margin: if horizontal {
                    UiRect::vertical(Val::Px(6.0))
                } else {
                    UiRect::horizontal(Val::Px(6.0))
                },
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(border())),
            Name::new("mixer-master-rule"),
        ))
        .id();
    kids.push(rule);
    kids.push(strip(commands, fonts, true, BusRef::Master, layout));

    commands.entity(run).add_children(&kids);

    // Rows overflow downwards as soon as there are more than a handful of buses
    // — which is exactly the board you'd pick rows for — so they get a scroll
    // view. Columns keep the old clip-at-the-edge behaviour: a vertical scroller
    // would do nothing for a run that overflows sideways.
    if horizontal {
        scroll_view(commands, run)
    } else {
        run
    }
}

// ── Top bar ──────────────────────────────────────────────────────────────────

/// Height of the bar. Short, because every pixel it takes is a pixel off the
/// faders below it, but tall enough for labelled controls.
const BAR_H: f32 = 32.0;
/// The panel's top bar: the page tabs on the left; on the right, for the Mixer
/// page, strip size, Horizontal and Add bus.
///
/// The tabs and the size switch are ember's [`segmented`] control, two-way
/// bound to [`MixerTab`] and [`MixerLayout`], rather than hand-built keys.
fn top_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BAR_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            // `header_bg` + a hairline under it is what the asset browser's and
            // console's toolbars use; a panel's chrome strip should look the same
            // wherever you meet it.
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
            Name::new("mixer-top-bar"),
        ))
        .id();

    // ── Page tabs ──
    let labels = [
        renzora::lang::t_or("mixer.tab.mixer", "Mixer"),
        renzora::lang::t_or("mixer.tab.routing", "Routing"),
        renzora::lang::t_or("mixer.tab.metering", "Metering"),
    ];
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let tabs = segmented(commands, &fonts.ui, &refs, 0);
    bind_2way(
        commands,
        tabs,
        |rx| tab_of(rx).index(),
        |w, i: &usize| {
            let tab = MixerTab::ALL.get(*i).copied().unwrap_or_default();
            if let Some(mut t) = w.get_resource_mut::<MixerTab>() {
                if *t != tab {
                    *t = tab;
                }
            }
            // The page change respawns the strips, rename field included.
            if let Some(mut r) = w.get_resource_mut::<MixerRename>() {
                r.0 = None;
            }
        },
    );

    let spring = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                ..default()
            },
            Name::new("mixer-bar-spring"),
        ))
        .id();

    // ── Mixer-page controls (hidden on the other pages, which have no strips) ──
    let shape = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("mixer-bar-shape"),
        ))
        .id();
    bind_display(commands, shape, |rx| tab_of(rx) == MixerTab::Mixer);

    let sizes = [
        renzora::lang::t_or("mixer.compact", "Compact"),
        renzora::lang::t_or("mixer.large", "Large"),
    ];
    let size_refs: Vec<&str> = sizes.iter().map(String::as_str).collect();
    let size = segmented(commands, &fonts.ui, &size_refs, 0);
    bind_2way(
        commands,
        size,
        |rx| if layout_of(rx).compact() { 0 } else { 1 },
        |w, i: &usize| {
            let density = if *i == 0 { Density::Compact } else { Density::Wide };
            if let Some(mut l) = w.get_resource_mut::<MixerLayout>() {
                if l.density != density {
                    l.density = density;
                }
            }
            if let Some(mut r) = w.get_resource_mut::<MixerRename>() {
                r.0 = None;
            }
        },
    );

    let horizontal = horizontal_toggle(commands, fonts);
    let sep = commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                height: Val::Px(16.0),
                margin: UiRect::horizontal(Val::Px(2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(border())),
        ))
        .id();
    let add = bar_button(commands, fonts, "plus", &renzora::lang::t_or("mixer.add_bus", "Add bus"));
    commands.entity(add).insert(BusAdd);
    bind_bg(commands, add, move |rx| match rx.get::<UiInteraction>(add) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });
    commands.entity(shape).add_children(&[size, horizontal, sep, add]);

    commands.entity(bar).add_children(&[tabs, spring, shape]);
    bar
}

/// A labelled top-bar button: icon and text. The caller binds its background
/// (one binding per node, or two would fight over it).
fn bar_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::horizontal(Val::Px(9.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            hover_pointer(),
            Name::new("mixer-bar-button"),
        ))
        .id();
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    let text = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(btn).add_children(&[glyph, text]);
    btn
}

/// The Horizontal toggle: lit in the accent while channels run as rows.
fn horizontal_toggle(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let btn = bar_button(commands, fonts, "rows", &renzora::lang::t_or("mixer.horizontal", "Horizontal"));
    commands.entity(btn).insert((
        OrientationBtn,
        HoverTooltip::new(renzora::lang::t_or("mixer.horizontal_tip", "Run channels as rows instead of columns")),
    ));
    bind_bg(commands, btn, move |rx| {
        if layout_of(rx).horizontal() {
            return rgb(accent()).with_alpha(0.22);
        }
        match rx.get::<UiInteraction>(btn) {
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
            _ => Color::NONE,
        }
    });
    btn
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

// ── Channel strip ────────────────────────────────────────────────────────────

/// One channel strip: colour bar, name, pan knob, fader + VU, dB readout,
/// mute/solo — all two-way bound to `bus`. Custom buses additionally get a ×
/// delete button and an inline rename field.
///
/// The same seven parts in the same order whichever way the strip runs; only
/// their geometry changes, which is why this is one builder rather than two.
fn strip(
    commands: &mut Commands,
    fonts: &EmberFonts,
    is_master: bool,
    bus: BusRef,
    layout: MixerLayout,
) -> Entity {
    let horizontal = layout.horizontal();
    let span = Val::Px(layout.strip_span());
    let col = commands
        .spawn((
            Node {
                width: if horizontal { Val::Auto } else { span },
                height: if horizontal { span } else { Val::Auto },
                flex_direction: if horizontal {
                    FlexDirection::Row
                } else {
                    FlexDirection::Column
                },
                align_items: AlignItems::Center,
                column_gap: Val::Px(layout.gap()),
                row_gap: Val::Px(layout.gap()),
                padding: layout.strip_padding(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                // A row is stretched to the panel's width, but a flex item's
                // automatic minimum is its *content* width — so without this a
                // narrow panel left the strip wider than the panel and the mute
                // key hanging off the right-hand edge, outside the frame. Pinned
                // to zero, the row stays panel-width and its contents give ground
                // in the order set below instead.
                min_width: if horizontal {
                    Val::Px(0.0)
                } else {
                    Val::Auto
                },
                // Only rows clip: a column's rename field overhangs its
                // neighbours on purpose, and clipping there would cut the box the
                // user is typing into.
                overflow: if horizontal {
                    Overflow::clip()
                } else {
                    Overflow::visible()
                },
                ..default()
            },
            // Each strip is a card a step above the panel; Master a step above
            // that, so it reads as the surface the buses feed.
            BackgroundColor(rgb(if is_master { card_bg() } else { header_bg() })),
            BorderColor::all(rgb(border())),
            // Geometry-based hit test for the right-click menu: a strip is mostly
            // covered by its own controls, so an `UiInteraction` on the root would
            // only ever fire on the gaps between them.
            RelativeCursorPosition::default(),
            bus,
            Name::new("mixer-strip"),
        ))
        .id();
    // The strip's colour is its top bar; the frame stays neutral, so a row of
    // strips reads as a row of cards and not a row of coloured outlines.

    // Colour bar — the part you actually scan when picking a strip out of a run.
    // It runs the full length of the strip's leading edge either way.
    let bar = commands
        .spawn((
            Node {
                width: if horizontal {
                    Val::Px(3.0)
                } else {
                    Val::Percent(100.0)
                },
                height: if horizontal {
                    Val::Auto
                } else {
                    Val::Px(3.0)
                },
                align_self: AlignSelf::Stretch,
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(tint(BUS_COLORS[11])),
            Name::new("mixer-strip-color"),
        ))
        .id();
    bind_bg(commands, bar, move |rx| tint(read(rx, bus, |s| s.color)));

    let header = strip_header(commands, fonts, is_master, bus, layout);

    // The one node that grows. Everything before and after it is fixed, so the
    // fader and its meter absorb whatever room the panel has — which is the
    // whole point of a fader: the longer it is, the finer you can set it.
    let meters = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: if horizontal {
                    AlignItems::Center
                } else {
                    AlignItems::Stretch
                },
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(if layout.compact() { 6.0 } else { 8.0 }),
                width: if horizontal {
                    Val::Auto
                } else {
                    Val::Percent(100.0)
                },
                flex_grow: 1.0,
                flex_shrink: 1.0,
                min_width: Val::Px(0.0),
                min_height: if horizontal {
                    Val::Px(0.0)
                } else {
                    Val::Px(40.0)
                },
                ..default()
            },
            Name::new("mixer-strip-meters"),
        ))
        .id();
    let vol = if horizontal {
        fader_horizontal(commands, 0.0)
    } else {
        fader(commands, 0.0)
    };
    // Both widgets ship at a fixed 120px along their travel axis; let them
    // stretch. Their fills and hit-testing are percentage/normalized based, so
    // that length is free to vary.
    grow_along(commands, vol, horizontal, 2.0, 34.0, true);
    bind_2way(
        commands,
        vol,
        move |rx| read(rx, bus, |s| ((s.volume / VOL_MAX) as f32).clamp(0.0, 1.0)),
        move |w, v| {
            let nv = *v as f64 * VOL_MAX;
            write(w, bus, move |s| s.volume = nv);
        },
    );
    // Against full scale, not `VOL_MAX`. Dividing by the fader's 1.5 head-room
    // meant a signal at 0 dBFS only ever filled two thirds of the meter, so a mix
    // that was actually clipping looked comfortable.
    let vu = if horizontal {
        vu_meter_bound_horizontal(commands, move |rx| {
            read(rx, bus, |s| s.peak_level.clamp(0.0, 1.0))
        })
    } else {
        vu_meter_bound(commands, move |rx| {
            read(rx, bus, |s| s.peak_level.clamp(0.0, 1.0))
        })
    };
    grow_along(commands, vu, horizontal, 1.0, 16.0, false);
    // Columns get a dB scale beside the fader: the fader's travel is linear
    // in amplitude, so 0 dB sits two thirds up and -24 near the bottom, which
    // is not where anyone would guess.
    if !horizontal {
        let scale = db_scale(commands, fonts);
        commands.entity(meters).add_children(&[scale, vol, vu]);
    } else {
        commands.entity(meters).add_children(&[vol, vu]);
    }

    // The number the fader was never showing. A fader with no readout tells you
    // where the cap is, not what the gain is — and "somewhere near the top" is
    // not a level you can reproduce or talk about.
    let db = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, layout.db_font()),
            TextColor(rgb(value_text())),
            // Fixed width in a row so a changing readout can't shove the
            // mute/solo keys sideways under the cursor.
            Node {
                width: if horizontal {
                    Val::Px(38.0)
                } else {
                    Val::Auto
                },
                flex_shrink: 0.0,
                ..default()
            },
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    bind_text(commands, db, move |rx| db_text(read(rx, bus, |s| s.volume)));

    // Pan: -1..1 mapped to the knob's 0..1. Before the fader and smaller than it
    // was — it's a trim, and it had been outweighing the control it trims.
    // Pivoted at centre: pan is a direction from the middle, not a level, so the
    // fill has to grow either side of 12 o'clock. A fill-from-the-left knob draws
    // itself half full at dead centre and looks panned left.
    let pan = knob_pivoted(commands, 0.5, 0.5);
    let knob_size = layout.knob();
    commands.queue(move |w: &mut World| {
        if let Some(mut n) = w.get_mut::<Node>(pan) {
            n.width = Val::Px(knob_size);
            n.height = Val::Px(knob_size);
            n.flex_shrink = 0.0;
            n.align_items = AlignItems::Center;
            n.justify_content = JustifyContent::Center;
        }
    });
    // The value in the knob's middle: "C" at centre, "L20" / "R35" either
    // side. At centre the dial has no fill at all, so without this a centred
    // pan looked like no control.
    let pan_value = commands
        .spawn((
            Text::new("C"),
            ui_font(&fonts.mono, if layout.compact() { 8.0 } else { 9.0 }),
            TextColor(rgb(value_text())),
            bevy::text::TextLayout::no_wrap(),
            renzora_ember::interaction::PASS_THROUGH,
        ))
        .id();
    bind_text(commands, pan_value, move |rx| {
        let p = read(rx, bus, |s| s.panning);
        let n = (p * 100.0).round() as i32;
        match n {
            0 => "C".to_string(),
            n if n < 0 => format!("L{}", -n),
            n => format!("R{n}"),
        }
    });
    commands.entity(pan).add_child(pan_value);
    // Wheel over the knob pans it: 5% of the full L..R throw per notch (10 on
    // the readout), 1% with Ctrl (2 on the readout). Dragging a 32px dial to an exact value
    // was fiddly, and the wheel lands on centre when it crosses it. The wheel
    // nudges the knob's `Bound`, so it reaches the bus through the `bind_2way`
    // below just like a drag (audio backend and saved config alike).
    commands.entity(pan).insert(KnobWheel { step: 0.05, fine_step: 0.01 });
    bind_2way(
        commands,
        pan,
        move |rx| read(rx, bus, |s| (((s.panning + 1.0) / 2.0) as f32).clamp(0.0, 1.0)),
        move |w, v| {
            let np = (*v as f64) * 2.0 - 1.0;
            write(w, bus, move |s| s.panning = np);
        },
    );

    // Full strip width in a column, so each key is a real target rather than
    // two small squares in the middle of the strip.
    let buttons = commands
        .spawn((
            Node {
                width: if horizontal { Val::Auto } else { Val::Percent(100.0) },
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("mixer-strip-buttons"),
        ))
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
    if !horizontal {
        for key in [mute, solo] {
            commands.queue(move |w: &mut World| {
                if let Some(mut n) = w.get_mut::<Node>(key) {
                    n.flex_grow = 1.0;
                    n.flex_basis = Val::Px(0.0);
                    n.width = Val::Auto;
                }
            });
        }
    }
    commands.entity(buttons).add_children(&[mute, solo]);

    // Where the bus goes, when it is anywhere but Master: "> Group". Shown on
    // the strip itself so a bus feeding a group never passes for one feeding
    // the output (the Routing page is where it is changed).
    let route = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, layout.db_font()),
            TextColor(rgb(accent())),
            bevy::text::TextLayout::no_wrap(),
            Node { flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
            HoverTooltip::new(renzora::lang::t_or("mixer.routed_tip", "This bus feeds another bus. Change it on the Routing page.")),
        ))
        .id();
    bind_text(commands, route, move |rx| {
        let Some(mixer) = rx.get_resource::<MixerState>() else { return String::new() };
        bus.get(mixer)
            .and_then(|s| s.output.clone())
            .and_then(|k| BusRef::from_key(mixer, &k))
            .map(|to| format!("\u{203a} {}", bus_name(rx, to)))
            .unwrap_or_default()
    });
    bind_display(commands, route, move |rx| {
        !is_master && read(rx, bus, |s| s.output.clone()).is_some()
    });

    let mut kids = vec![bar, header, route];
    // The bus's insert chain, when the plugin host has put one on it: one
    // line per effect, so the strip says what is processing it. Columns only;
    // a row has no height to spare. Nothing here edits the chain (the plugin
    // host owns that), so with no host loaded the section simply never shows.
    if !horizontal {
        kids.push(inserts(commands, fonts, bus));
    }
    kids.push(pan);
    if layout.pan_label() {
        let pan_label = commands
            .spawn((
                Text::new("Pan"),
                ui_font(&fonts.ui, 9.0),
                TextColor(rgb(text_muted())),
            ))
            .id();
        kids.push(pan_label);
    }
    kids.extend([meters, db, buttons]);
    commands.entity(col).add_children(&kids);
    col
}

/// An RGB triple from [`BUS_COLORS`] / `ChannelStrip::color` as a bevy `Color`.
/// The theme's `rgb` takes a tuple; strip colours are stored as arrays so they
/// compare and serialize cleanly.
pub(crate) fn tint(color: [u8; 3]) -> Color {
    rgb((color[0], color[1], color[2]))
}

/// The insert list under a column's name: up to three effect names, then a
/// "+N more". Rebuilt from `BusInsertsSummary` (keyed by bus name), which
/// only the marketplace audio-plugin host fills.
fn inserts(commands: &mut Commands, fonts: &EmberFonts, bus: BusRef) -> Entity {
    let col = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() },
            Name::new("mixer-strip-inserts"),
        ))
        .id();
    let names = move |rx: &Rx| -> Vec<String> {
        let name = bus_name(rx, bus);
        rx.get_resource::<renzora_audio::BusInsertsSummary>()
            .map(|s| s.slots(&name).iter().map(|f| f.display_name.clone()).collect())
            .unwrap_or_default()
    };
    bind_display(commands, col, move |rx| !names(rx).is_empty());
    let font = fonts.ui.clone();
    keyed_list(commands, col, move |rx| {
        let all = names(rx);
        let shown: Vec<String> = all.iter().take(3).cloned().collect();
        let more = all.len().saturating_sub(3);
        let mut items: Vec<(u64, u64)> = shown
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                n.hash(&mut h);
                (i as u64, h.finish())
            })
            .collect();
        if more > 0 {
            items.push((u64::MAX, more as u64));
        }
        let font = font.clone();
        KeyedSnapshot {
            items,
            build: Box::new(move |c, _f, i| {
                let label = shown.get(i).cloned().unwrap_or_else(|| format!("+{more} more"));
                let chip = c
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(16.0),
                            padding: UiRect::horizontal(Val::Px(5.0)),
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(rgb(faint_bg())),
                    ))
                    .id();
                let t = c
                    .spawn((Text::new(label), ui_font(&font, 9.0), TextColor(rgb(value_text())), bevy::text::TextLayout::no_wrap()))
                    .id();
                c.entity(chip).add_child(t);
                chip
            }),
        }
    });
    col
}

/// The marks of [`db_scale`], in dB.
const DB_MARKS: [f64; 5] = [3.0, 0.0, -6.0, -12.0, -24.0];

/// A column of dB labels placed where those levels sit on the fader's travel
/// (linear amplitude, 0 to [`VOL_MAX`]).
fn db_scale(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                flex_shrink: 0.0,
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("mixer-db-scale"),
        ))
        .id();
    let marks: Vec<Entity> = DB_MARKS
        .iter()
        .map(|db| {
            let pos = (10f64.powf(db / 20.0) / VOL_MAX).clamp(0.0, 1.0);
            let text = if *db > 0.0 { format!("+{db}") } else { format!("{db}") };
            commands
                .spawn((
                    Text::new(text),
                    ui_font(&fonts.mono, 8.0),
                    TextColor(rgb(text_muted()).with_alpha(0.8)),
                    bevy::text::TextLayout::no_wrap(),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        // Centred on the mark: the glyphs are ~8px tall.
                        top: Val::Percent(((1.0 - pos) * 100.0) as f32),
                        margin: UiRect::top(Val::Px(-5.0)),
                        ..default()
                    },
                ))
                .id()
        })
        .collect();
    commands.entity(col).add_children(&marks);
    col
}

/// Half a fader cap, which is how far the cap overhangs each end of the fader's
/// own box — the cap is centred on the value, so at 0% and 100% half of it sits
/// outside. Standing up, that overhang lands in the strip's vertical padding and
/// nobody notices; lying down it landed *on the dB readout*, drawing the cap's
/// ribs straight through the number. Rows reserve it as margin instead.
const CAP_OVERHANG: f32 = 14.0;

/// Let a fader or meter stretch instead of keeping its shipped 120px length.
///
/// A vertical strip stretches it through the meters row's `align_items:
/// Stretch`, so all this has to do is release the fixed height. A horizontal one
/// has to divide the row's width instead, hence `row_share` (the fader takes
/// twice the meter's share — you aim at one of them and only read the other) and
/// `row_min`, so a crowded strip shrinks the fader rather than erasing it.
fn grow_along(
    commands: &mut Commands,
    e: Entity,
    horizontal: bool,
    row_share: f32,
    row_min: f32,
    cap_room: bool,
) {
    commands.queue(move |w: &mut World| {
        if let Some(mut n) = w.get_mut::<Node>(e) {
            if horizontal {
                n.flex_grow = row_share;
                n.flex_basis = Val::Px(0.0);
                n.width = Val::Auto;
                n.min_width = Val::Px(row_min);
                if cap_room {
                    n.margin = UiRect::horizontal(Val::Px(CAP_OVERHANG));
                }
            } else {
                n.flex_grow = 1.0;
                n.height = Val::Auto;
                n.min_height = Val::Px(0.0);
            }
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
    layout: MixerLayout,
) -> Entity {
    let horizontal = layout.horizontal();
    let row = commands
        .spawn((
            Node {
                // A row's name gets its own column so every strip's fader starts
                // at the same x — a ragged left edge is what makes a list of
                // channels hard to read down. It's a flex *basis* rather than a
                // width, and the only part of the row that shrinks readily: a
                // clipped name still identifies the channel, whereas a clipped
                // mute key is a control you can no longer hit.
                width: if horizontal {
                    Val::Auto
                } else {
                    Val::Percent(100.0)
                },
                flex_basis: if horizontal {
                    Val::Px(layout.header_w())
                } else {
                    Val::Auto
                },
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(2.0),
                flex_grow: 0.0,
                flex_shrink: if horizontal { 4.0 } else { 0.0 },
                min_width: if horizontal {
                    Val::Px(36.0)
                } else {
                    Val::Auto
                },
                ..default()
            },
            Name::new("mixer-strip-header"),
        ))
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
                justify_content: if horizontal {
                    JustifyContent::FlexStart
                } else {
                    JustifyContent::Center
                },
                align_items: AlignItems::Center,
                // No `overflow: clip` here, however tempting for a long name:
                // the rename field is an absolutely-positioned child wider than
                // this node on purpose (see `RENAME_W`), and clipping would cut
                // the box the user is typing into down to the label's width.
                ..default()
            },
            UiInteraction::default(),
            BusHeader(bus),
            Name::new("mixer-strip-name"),
        ))
        .id();

    let mut name_font = ui_font(&fonts.ui, layout.name_font());
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
            // Floated over the strip: see `RENAME_W`. A column centres it on the
            // name and lets it overhang the neighbouring strips; a row can't —
            // the row clips to its own frame (see `strip`), so a centred field
            // would lose its left end. There it opens from the name's left edge
            // and reaches rightwards over the controls instead.
            Node {
                position_type: PositionType::Absolute,
                left: if horizontal {
                    Val::Px(0.0)
                } else {
                    Val::Percent(50.0)
                },
                margin: if horizontal {
                    UiRect::ZERO
                } else {
                    UiRect::left(Val::Px(-RENAME_W / 2.0))
                },
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
            // Above the neighbouring strips it overhangs. Tiered rather than
            // absolute: the Mixer is a default resident of the global bottom
            // panel (`GlobalZIndex(100)`), and a bare 60 would put the rename
            // field under that panel's own background — you'd double-click a
            // channel name and get nothing. See `renzora_ember::stacking`.
            renzora_ember::stacking::ZTier(60),
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
                UiInteraction::default(),
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
    headers: Query<(&UiInteraction, &BusHeader)>,
    mut rename: ResMut<MixerRename>,
    mut last_click: Local<Option<(usize, f64)>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(index) = headers
        .iter()
        .find(|(i, _)| matches!(i, UiInteraction::Pressed))
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
            .and_then(|m| m.custom_buses.get(index).map(|b| b.name.clone()))
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
    devices: Option<Res<renzora_audio::AudioDevices>>,
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
    let devices = devices.map(|d| d.clone()).unwrap_or_default();
    kids.push(device_submenu(&mut commands, &fonts, &devices, bus, true, input_dev.as_deref()));
    kids.push(device_submenu(&mut commands, &fonts, &devices, bus, false, output_dev.as_deref()));

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
                    m.remove_bus(index);
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
                    UiInteraction::default(),
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
    devices: &renzora_audio::AudioDevices,
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

    // From the resource the audio API keeps, rather than by asking the backend
    // here: enumerating is a system call, and this runs while a menu is being
    // built. With no backend loaded the lists are empty and the menu offers only
    // "(none)", which is the honest answer.
    let devices = if input {
        devices.inputs.clone()
    } else {
        devices.outputs.clone()
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

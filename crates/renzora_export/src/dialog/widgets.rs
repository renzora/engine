//! Small builders the dialog's panes share.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_with};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::toggle_switch;
use renzora_ember::UiInteraction;

use crate::overlay::{ExportOverlayState, PackagingMode, PluginLinkMode};
use crate::templates::Platform;

use super::{Choice, ChoiceCard, EXPORT_BLUE, EXPORT_BLUE_HOT};

pub(super) fn platform_icon(p: Platform) -> &'static str {
    match p {
        Platform::WindowsX64 | Platform::WindowsArm64 => "windows-logo",
        Platform::LinuxX64 | Platform::LinuxArm64 => "linux-logo",
        Platform::MacOSX64 | Platform::MacOSArm64 => "apple-logo",
        Platform::IOSArm64 => "device-mobile",
        Platform::TvOSArm64 => "television-simple",
        Platform::AndroidArm64 | Platform::AndroidX86_64 => "android-logo",
        Platform::FireTVArm64 => "television",
        Platform::WebWasm32 => "globe",
    }
}

pub(super) fn fullscreen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        right: Val::Px(0.0),
        bottom: Val::Px(0.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    }
}

pub(super) fn ca(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::srgba_u8(r, g, b, a)
}

/// A bound on/off control for this dialog: a switch, not a checkbox.
///
/// Every setting in the export dialog is "include this / don't" — an on-off
/// state, not an item ticked off a list — and it is the same decision the
/// Settings plugin panel already expresses with a switch. A checkbox also had a
/// practical problem here: unchecked, it is a 1px border, which is why the rows
/// behind it needed a fill to stay legible at all (see [`row_fill`]). A switch
/// carries its own filled track and reads at a glance either way.
///
/// The `BLOCK` is not optional. UI nodes pass the pointer through by default, so a
/// control that does not block hands its press to every node behind it — and
/// these sit inside rows and cards that have their own handlers.
pub(super) fn switch_control(commands: &mut Commands, on: bool) -> Entity {
    let sw = toggle_switch(commands, on);
    commands.entity(sw).insert(BLOCK);
    sw
}

pub(super) fn cursor() -> renzora_ember::cursor_icon::HoverCursor {
    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

pub(super) fn txt(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands.spawn((Text::new(s.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id()
}

pub(super) fn icon_title(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let row = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }, PASS_THROUGH)).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 16.0);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 15.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

pub(super) fn pill_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(section_bg())), UiInteraction::default(), cursor())).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 11.0);
    commands.entity(ic).insert(PASS_THROUGH);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), PASS_THROUGH)).id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

/// A compact icon+label button for the sidebar's Duplicate / Remove pair, and
/// for the Packaging tab's "Download engine source".
pub(super) fn small_button<M: Component>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    marker: M,
) -> Entity {
    let btn = commands
        .spawn((
            Node { flex_grow: 1.0, height: Val::Px(26.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(5.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            marker,
            cursor(),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered)) {
            ca(255, 255, 255, 10)
        } else {
            Color::NONE
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 13.0);
    commands.entity(ic).insert(PASS_THROUGH);
    let tx = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), PASS_THROUGH, bevy::text::TextLayout::no_wrap())).id();
    commands.entity(btn).add_children(&[ic, tx]);
    btn
}

/// Reactively drive a node's border colour.
pub(super) fn bind_border(
    commands: &mut Commands,
    target: Entity,
    value: impl Fn(&Rx) -> Color + Send + Sync + 'static,
) {
    bind_with(commands, target, value, |w, e, c: &Color| {
        if let Some(mut b) = w.get_mut::<BorderColor>(e) {
            *b = BorderColor::all(*c);
        }
    });
}

/// The hairline between rows in a settings list.
pub(super) fn hairline() -> Color {
    ca(255, 255, 255, 14)
}

/// A group heading inside a tab.
///
/// Plain text rather than the collapsible ember `section` the tabs used to be
/// built from. A tab holds three or four groups at most, and a fold control on
/// each one was a click spent to hide something you then had to remember was
/// there.
pub(super) fn heading(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
            Node { margin: UiRect { top: Val::Px(6.0), bottom: Val::Px(4.0), ..default() }, ..default() },
        ))
        .id()
}

/// A vertical group: a heading and the rows under it.
pub(super) fn group(commands: &mut Commands, fonts: &EmberFonts, label: &str, rows: &[Entity]) -> Entity {
    let col = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, margin: UiRect::bottom(Val::Px(14.0)), ..default() })
        .id();
    let h = heading(commands, fonts, label);
    commands.entity(col).add_child(h);
    commands.entity(col).add_children(rows);
    col
}

/// One line of a settings list: the label and a one-line hint on the left, the
/// control on the right.
///
/// The hint sits under its label instead of in a paragraph below the whole
/// group, which is where it used to be, and where it explained a control the
/// reader had already scrolled past.
pub(super) fn setting_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    hint: Option<&str>,
    control: Entity,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::vertical(Val::Px(8.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(hairline()),
        ))
        .id();
    let text = commands
        .spawn(Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    let l = txt(commands, fonts, label, 12.5, text_primary());
    commands.entity(text).add_child(l);
    if let Some(hint) = hint {
        let h = txt(commands, fonts, hint, 11.0, text_muted());
        commands.entity(text).add_child(h);
    }
    commands.entity(row).add_children(&[text, control]);
    row
}

/// A [`setting_row`] whose control is a switch bound to one field.
pub(super) fn switch_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    hint: Option<&str>,
    get: fn(&ExportOverlayState) -> bool,
    set: fn(&mut ExportOverlayState, bool),
) -> Entity {
    let sw = switch_control(commands, false);
    bind_2way(
        commands,
        sw,
        move |w| w.get_resource::<ExportOverlayState>().map(get).unwrap_or(false),
        move |w, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                set(&mut s, *v);
            }
        },
    );
    setting_row(commands, fonts, label, hint, sw)
}

/// A small rounded label: a mode's "For release", a feature's "changed", a tab's
/// count.
pub(super) fn tag(commands: &mut Commands, fonts: &EmberFonts, label: &str, fg: (u8, u8, u8), bg: Color) -> Entity {
    let pill = commands
        .spawn((
            Node { padding: UiRect::axes(Val::Px(7.0), Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(9.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(bg),
            PASS_THROUGH,
        ))
        .id();
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(fg)), PASS_THROUGH, bevy::text::TextLayout::no_wrap()))
        .id();
    commands.entity(pill).add_child(t);
    pill
}

/// Whether `choice` is the option currently in effect.
///
/// Reads through the same platform rules the export applies rather than the raw
/// field, so a card never shows selected for something the build will not do:
/// the web has no middle packaging mode (its `SingleBinary` is the template) and
/// always links plugins in.
pub(super) fn choice_selected(s: &ExportOverlayState, choice: Choice) -> bool {
    let web = matches!(s.platform, Platform::WebWasm32);
    match choice {
        Choice::Packaging(m) => {
            let current = if web && s.packaging_mode == PackagingMode::SingleBinary {
                PackagingMode::SeparateFiles
            } else {
                s.packaging_mode
            };
            current == m
        }
        Choice::Link(l) => {
            // A lean export always links in; see `link_plugins_in` in the
            // overlay for why a lean binary cannot load plugin files.
            let lean = s.packaging_mode == PackagingMode::LeanSingleBinary;
            let current = if web || lean { PluginLinkMode::LinkIn } else { s.plugin_link_mode };
            current == l
        }
    }
}

/// A large option card: radio ring, title, an optional badge, and a sentence
/// saying what picking it means.
///
/// Used where the old dialog had a row of radio buttons followed by two
/// paragraphs explaining them. The explanation is now on the option it
/// explains, and the badge says the one thing most people need ("For release" /
/// "Quick test") before they read any of it.
pub(super) fn choice_card(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    badge: Option<(&str, bool)>,
    desc: &str,
    choice: Choice,
) -> Entity {
    let card = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            ChoiceCard(choice),
            cursor(),
        ))
        .id();
    bind_bg(commands, card, move |w| {
        let sel = w.get_resource::<ExportOverlayState>().is_some_and(|s| choice_selected(s, choice));
        let hot = matches!(w.get::<UiInteraction>(card), Some(UiInteraction::Hovered));
        let (r, g, b) = accent();
        if sel {
            ca(r, g, b, 34)
        } else if hot {
            ca(255, 255, 255, 12)
        } else {
            rgb(card_bg())
        }
    });
    bind_border(commands, card, move |w| {
        let sel = w.get_resource::<ExportOverlayState>().is_some_and(|s| choice_selected(s, choice));
        if sel { rgb(accent()) } else { rgb(border()) }
    });

    let top = commands
        .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }, PASS_THROUGH))
        .id();
    let ring = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BorderColor::all(rgb(text_muted())),
            PASS_THROUGH,
        ))
        .id();
    bind_border(commands, ring, move |w| {
        let sel = w.get_resource::<ExportOverlayState>().is_some_and(|s| choice_selected(s, choice));
        if sel { rgb(accent()) } else { rgb(text_muted()) }
    });
    let dot = commands
        .spawn((Node { width: Val::Px(6.0), height: Val::Px(6.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(Color::NONE), PASS_THROUGH))
        .id();
    bind_bg(commands, dot, move |w| {
        let sel = w.get_resource::<ExportOverlayState>().is_some_and(|s| choice_selected(s, choice));
        if sel { rgb(accent()) } else { Color::NONE }
    });
    commands.entity(ring).add_child(dot);
    let t = commands
        .spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), PASS_THROUGH, Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }))
        .id();
    commands.entity(top).add_children(&[ring, t]);
    if let Some((label, release)) = badge {
        let b = if release {
            tag(commands, fonts, label, (143, 220, 178), ca(76, 195, 138, 40))
        } else {
            tag(commands, fonts, label, text_muted(), ca(255, 255, 255, 16))
        };
        commands.entity(top).add_child(b);
    }
    let d = commands
        .spawn((Text::new(desc.to_string()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_muted())), PASS_THROUGH, Node { margin: UiRect::left(Val::Px(24.0)), ..default() }))
        .id();
    commands.entity(card).add_children(&[top, d]);
    card
}

/// A row of [`choice_card`]s sharing the width.
pub(super) fn card_row(commands: &mut Commands, cards: &[Entity]) -> Entity {
    let row = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Stretch, column_gap: Val::Px(10.0), ..default() })
        .id();
    commands.entity(row).add_children(cards);
    row
}

/// A tinted notice: an icon and a sentence, coloured for how much it matters.
pub(super) fn notice(commands: &mut Commands, fonts: &EmberFonts, icon: &str, text: &str, fg: (u8, u8, u8)) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(ca(fg.0, fg.1, fg.2, 18)),
            BorderColor::all(ca(fg.0, fg.1, fg.2, 70)),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, fg, 14.0);
    let t = commands
        .spawn((Text::new(text.to_string()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), ..default() }))
        .id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

/// A compact filled button in the export blue, for the one action a notice or
/// card is asking for.
pub(super) fn primary_button<M: Component>(commands: &mut Commands, fonts: &EmberFonts, label: &str, marker: M) -> Entity {
    let btn = commands
        .spawn((
            Node { height: Val::Px(26.0), flex_shrink: 0.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, padding: UiRect::horizontal(Val::Px(10.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(EXPORT_BLUE)),
            UiInteraction::default(),
            marker,
            cursor(),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hot = matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed));
        rgb(if hot { EXPORT_BLUE_HOT } else { EXPORT_BLUE })
    });
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.5), TextColor(Color::WHITE), PASS_THROUGH, bevy::text::TextLayout::no_wrap()))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

/// The family a platform is listed under in the preset sidebar.
pub(super) fn platform_family(p: Platform) -> (u8, &'static str) {
    match p {
        Platform::WindowsX64 | Platform::WindowsArm64 | Platform::LinuxX64 | Platform::LinuxArm64 | Platform::MacOSX64 | Platform::MacOSArm64 => (0, "export.family.desktop"),
        Platform::AndroidArm64 | Platform::AndroidX86_64 | Platform::IOSArm64 => (1, "export.family.mobile"),
        Platform::FireTVArm64 | Platform::TvOSArm64 => (2, "export.family.tv"),
        Platform::WebWasm32 => (3, "export.family.web"),
    }
}

/// The platform's name without its architecture, for "Export Web" and the like.
pub(super) fn platform_short(p: Platform) -> &'static str {
    match p {
        Platform::WindowsX64 | Platform::WindowsArm64 => "Windows",
        Platform::LinuxX64 | Platform::LinuxArm64 => "Linux",
        Platform::MacOSX64 | Platform::MacOSArm64 => "macOS",
        Platform::AndroidArm64 | Platform::AndroidX86_64 => "Android",
        Platform::IOSArm64 => "iOS",
        Platform::FireTVArm64 => "Fire TV",
        Platform::TvOSArm64 => "Apple TV",
        Platform::WebWasm32 => "Web",
    }
}

/// The name of a packaging mode as this platform's cards label it.
pub(super) fn mode_label(p: Platform, m: PackagingMode) -> String {
    let web = matches!(p, Platform::WebWasm32);
    renzora::lang::t(match (web, m) {
        (true, PackagingMode::LeanSingleBinary) => "export.packaging.web_lean",
        (true, _) => "export.packaging.web_template",
        (false, PackagingMode::LeanSingleBinary) => "export.packaging.lean",
        (false, PackagingMode::SingleBinary) => "export.packaging.single_exe",
        (false, PackagingMode::SeparateFiles) => "export.packaging.separate",
    })
}

pub(super) fn style_input(commands: &mut Commands, input: Entity) {
    commands.entity(input).insert(Node { flex_grow: 1.0, height: Val::Px(28.0), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() });
}

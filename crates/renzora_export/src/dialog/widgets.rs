//! Small builders the dialog's panes share.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg};
use renzora_ember::theme::*;
use renzora_ember::widgets::toggle_switch;

use crate::overlay::ExportOverlayState;
use crate::templates::Platform;

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
/// The `Block` is not optional. Bevy 0.19 defaults `FocusPolicy` to `Pass`, so a
/// control that does not block hands its press to every node behind it — and
/// these sit inside rows and cards that have their own handlers.
pub(super) fn switch_control(commands: &mut Commands, on: bool) -> Entity {
    let sw = toggle_switch(commands, on);
    commands.entity(sw).insert(FocusPolicy::Block);
    sw
}

/// The faint fill every feature row carries.
///
/// This is half of what `row_stripe` used to do. That function alternated two
/// tints down the list, and the alternation is gone — it read as banding, and
/// the job it was doing (telling one row from the next) only existed because
/// every row was three lines tall. The help text is a tooltip now, a row is one
/// line, and a hairline rule separates them far more quietly.
///
/// The tint itself has to stay, and the reason is not decorative: **an unchecked
/// checkbox is a 1px border over `Color::NONE`, and against the bare panel that
/// border is invisible.** The stripe was originally two tints rather than
/// tint-and-nothing for exactly this — with even rows untinted, half the feature
/// list appeared to have no control at all and every other row looked like a
/// label. Drop this fill and that returns for the whole list.
pub(super) fn row_fill() -> Color {
    ca(255, 255, 255, 8)
}

pub(super) fn cursor() -> renzora_ember::cursor_icon::HoverCursor {
    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

pub(super) fn txt(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands.spawn((Text::new(s.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id()
}

pub(super) fn labeled(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let t = txt(commands, fonts, label, 12.0, text_muted());
    commands.entity(row).add_child(t);
    row
}

pub(super) fn icon_title(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let row = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }, FocusPolicy::Pass)).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 16.0);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 15.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

pub(super) fn section_label(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

pub(super) fn icon_msg(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8)) -> (Entity, Entity) {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 12.0);
    let t = commands.spawn((Text::new(String::new()), ui_font(&fonts.ui, 11.0), TextColor(rgb(color)))).id();
    commands.entity(row).add_children(&[ic, t]);
    (row, t)
}

pub(super) fn check_state(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: fn(&ExportOverlayState) -> bool, set: fn(&mut ExportOverlayState, bool)) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let cb = switch_control(commands, false);
    bind_2way(commands, cb, move |w| w.get_resource::<ExportOverlayState>().map(get).unwrap_or(false), move |w, v: &bool| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { set(&mut s, *v); } });
    let t = txt(commands, fonts, label, 12.0, text_primary());
    commands.entity(row).add_children(&[cb, t]);
    row
}

pub(super) fn pill_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(section_bg())), Interaction::default(), cursor())).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 11.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
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
            Interaction::default(),
            marker,
            cursor(),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if matches!(w.get::<Interaction>(btn), Some(Interaction::Hovered)) {
            ca(255, 255, 255, 10)
        } else {
            Color::NONE
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 13.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let tx = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), FocusPolicy::Pass, bevy::text::TextLayout::no_wrap())).id();
    commands.entity(btn).add_children(&[ic, tx]);
    btn
}

pub(super) fn style_input(commands: &mut Commands, input: Entity) {
    commands.entity(input).insert(Node { flex_grow: 1.0, height: Val::Px(28.0), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() });
}

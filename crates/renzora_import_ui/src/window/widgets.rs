//! Small builders the panes and the toast share, plus the two accessors every
//! settings binding goes through.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::checkbox;

use crate::overlay::ImportOverlayState;

pub(crate) fn hover_cursor() -> renzora_ember::cursor_icon::HoverCursor {
    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

pub(super) fn txt(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands.spawn((Text::new(s.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id()
}

pub(super) fn icon_label(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, color: (u8, u8, u8), size: f32) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, size);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

/// An icon + a bindable message text. Returns `(row, message_text_entity)`.
pub(super) fn icon_msg(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8)) -> (Entity, Entity) {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 12.0);
    let t = commands.spawn((Text::new(String::new()), ui_font(&fonts.ui, 12.0), TextColor(rgb(color)))).id();
    commands.entity(row).add_children(&[ic, t]);
    (row, t)
}

/// A settings row: a left-aligned label and a right-aligned control.
pub(super) fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, control: Entity) -> Entity {
    let row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: Val::Px(12.0), min_height: Val::Px(26.0), ..default() }).id();
    let t = txt(commands, fonts, label, 12.0, text_primary());
    commands.entity(row).add_children(&[t, control]);
    row
}

/// A boolean settings row: label on the left, checkbox on the right.
pub(super) fn toggle_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: fn(&renzora_import::settings::ImportSettings) -> bool, set: fn(&mut renzora_import::settings::ImportSettings, bool)) -> Entity {
    let cb = checkbox(commands, false);
    bind_2way(commands, cb, move |w| g_settings(w, get), move |w, v: &bool| s_settings(w, |s| set(s, *v)));
    field_row(commands, fonts, label, cb)
}

pub(super) fn pill_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(rgb(accent())), Interaction::default(), hover_cursor()))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, (255, 255, 255), 12.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

pub(super) fn g_settings<T>(w: &Rx, get: impl Fn(&renzora_import::settings::ImportSettings) -> T) -> T
where
    T: Default,
{
    w.get_resource::<ImportOverlayState>().map(|s| get(&s.settings)).unwrap_or_default()
}

pub(super) fn s_settings(w: &mut World, set: impl FnOnce(&mut renzora_import::settings::ImportSettings)) {
    if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() {
        set(&mut s.settings);
    }
}

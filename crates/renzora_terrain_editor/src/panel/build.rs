//! Panel root: the enable pill, the inactive hint, and the Sculpt / Paint tab
//! bar over the two content columns.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::cursor_icon::HoverCursor;

use renzora_terrain::data::TerrainTab;

use super::paint::paint_content;
use super::sculpt::sculpt_content;
use super::{settings_tab, tool_active, EnableToggle, TabBtn};

pub(super) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            Name::new("terrain-tools"),
        ))
        .id();

    // ── Enable / disable toggle (full-width pill) ────────────────────────────
    let toggle = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            EnableToggle,
            Name::new("terrain-enable"),
        ))
        .id();
    bind_bg(commands, toggle, move |w| {
        if tool_active(w) {
            rgb(accent())
        } else if matches!(
            w.get::<Interaction>(toggle),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            rgb(card_bg())
        }
    });
    let toggle_icon = icon_text(commands, &fonts.phosphor, "mountains", text_primary(), 14.0);
    bind_text_color(commands, toggle_icon, |w| {
        if tool_active(w) {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    let toggle_label = commands
        .spawn((
            Text::new("Enable Terrain Mode"),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text(commands, toggle_label, |w| {
        if tool_active(w) {
            "Terrain Mode Active".to_string()
        } else {
            "Enable Terrain Mode".to_string()
        }
    });
    bind_text_color(commands, toggle_label, |w| {
        if tool_active(w) {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    commands
        .entity(toggle)
        .add_children(&[toggle_icon, toggle_label]);

    // ── Inactive hint (shown only when the tool is off) ──────────────────────
    let hint = commands
        .spawn((
            Text::new("Select a terrain entity and enable terrain mode to begin editing."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_display(commands, hint, |w| !tool_active(w));

    // ── Active body (tabs + content; shown only when the tool is on) ─────────
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, body, tool_active);

    let tabs = tab_bar(commands, fonts);

    // Sculpt + Paint content, toggled by the active tab.
    let sculpt = sculpt_content(commands, fonts);
    bind_display(commands, sculpt, |w| settings_tab(w) == TerrainTab::Sculpt);
    let paint = paint_content(commands, fonts);
    bind_display(commands, paint, |w| settings_tab(w) == TerrainTab::Paint);

    commands.entity(body).add_children(&[tabs, sculpt, paint]);

    commands.entity(root).add_children(&[toggle, hint, body]);
    root
}

// ── Tab bar ──────────────────────────────────────────────────────────────────

fn tab_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let sculpt = tab_button(commands, fonts, "mountains", "Sculpt", TerrainTab::Sculpt);
    let paint = tab_button(commands, fonts, "paint-brush", "Paint", TerrainTab::Paint);
    commands.entity(row).add_children(&[sculpt, paint]);
    row
}

fn tab_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    tab: TerrainTab,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TabBtn { tab },
            Name::new(format!("terrain-tab:{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if settings_tab(w) == tab {
            rgb(accent())
        } else if matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(popup_bg())
        } else {
            rgb(card_bg())
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    bind_text_color(commands, ic, move |w| {
        if settings_tab(w) == tab {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text_color(commands, lbl, move |w| {
        if settings_tab(w) == tab {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    commands.entity(btn).add_children(&[ic, lbl]);
    btn
}

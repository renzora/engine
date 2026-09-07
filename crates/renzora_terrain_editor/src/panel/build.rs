//! Panel root: the enable pill, the inactive hint, and the Sculpt / Paint tab
//! bar over the two content columns.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::cursor_icon::HoverCursor;

use renzora_terrain::data::TerrainTab;

use super::paint::paint_content;
use super::sculpt::sculpt_content;
use super::{settings_tab, TabBtn};

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

    // No enable toggle, and no "select a terrain and enable terrain mode" hint.
    // This body only exists because the Terrain component is on screen, which
    // means a terrain is selected -- so the gate was asking a question the
    // inspector had already answered, and answering it wrong left a panel of
    // greyed-out tools with a button you had to find first. Clicking a brush is
    // the enable now: it acts on the entity whose component you are looking at.

    // ── Tabs + content ──────────────────────────────────────────────────────
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let tabs = tab_bar(commands, fonts);

    // Sculpt + Paint content, toggled by the active tab.
    let sculpt = sculpt_content(commands, fonts);
    bind_display(commands, sculpt, |w| settings_tab(w) == TerrainTab::Sculpt);
    let paint = paint_content(commands, fonts);
    bind_display(commands, paint, |w| settings_tab(w) == TerrainTab::Paint);
    // Built by `renzora_foliage_editor`. It was briefly a **Foliage** inspector
    // section of its own, sitting under Terrain — which read as a second
    // component on an entity that only has one. It is a third way of painting
    // the same terrain, so it is a third tab.
    let foliage = renzora_foliage_editor::panel::build(commands, fonts);
    bind_display(commands, foliage, |w| settings_tab(w) == TerrainTab::Foliage);

    // The generator, as a section rather than the full-width bar it used to be
    // across the top of the scene. Shown only while the Generate tool is the
    // active one, the same gate the bar carried.
    let generate = crate::generate_bar::build(commands, fonts);
    bind_display(commands, generate, |w| {
        w.get_resource::<renzora_editor_framework::ActiveTool>().copied()
            == Some(renzora_editor_framework::ActiveTool::TerrainGenerate)
    });

    commands.entity(body).add_children(&[tabs, sculpt, paint, foliage, generate]);

    commands.entity(root).add_child(body);
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
    let foliage = tab_button(commands, fonts, "tree", "Foliage", TerrainTab::Foliage);
    commands.entity(row).add_children(&[sculpt, paint, foliage]);
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

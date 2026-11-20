//! The component body: the material slot, the mode tabs, and the four mode
//! panes, one shown at a time.
//!
//! All four panes are built up front and toggled by display, not rebuilt on a
//! tab switch: rebuilding would respawn a few hundred nodes per click and
//! re-run every binding's first pass, and the panes hold no per-mode state
//! that a rebuild would refresh anyway.

use bevy::prelude::*;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::UiInteraction;

use crate::terrain_inspector::{ShownTerrainTab, TerrainInspectorTab};

use super::manage::manage_content;
use super::material_slot::material_slot;
use super::paint::paint_content;
use super::sculpt::sculpt_content;
use super::ModeTabBtn;

fn shown(w: &Rx) -> TerrainInspectorTab {
    w.get_resource::<ShownTerrainTab>().map(|s| s.0).unwrap_or_default()
}

pub(super) fn build(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            Name::new("terrain-component"),
        ))
        .id();

    let slot = material_slot(commands, fonts, terrain);
    let tabs = tab_bar(commands, fonts);

    let manage = manage_content(commands, fonts, terrain);
    bind_display(commands, manage, |w| shown(w) == TerrainInspectorTab::Manage);
    let sculpt = sculpt_content(commands, fonts, terrain);
    bind_display(commands, sculpt, |w| shown(w) == TerrainInspectorTab::Sculpt);
    let paint = paint_content(commands, fonts, terrain);
    bind_display(commands, paint, |w| shown(w) == TerrainInspectorTab::Paint);
    // Built by `renzora_foliage_editor`: foliage is a third way of painting
    // the same terrain, so it is a third brush tab here rather than a
    // component of its own.
    let foliage = renzora_foliage_editor::panel::build(commands, fonts, terrain);
    bind_display(commands, foliage, |w| shown(w) == TerrainInspectorTab::Foliage);

    commands.entity(root).add_children(&[slot, tabs, manage, sculpt, paint, foliage]);
    root
}

/// The four mode tabs, equal widths in one rounded strip.
fn tab_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Name::new("terrain-mode-tabs"),
        ))
        .id();
    let icons = ["squares-four", "mountains", "paint-brush", "plant"];
    for (tab, icon) in TerrainInspectorTab::ALL.into_iter().zip(icons) {
        let btn = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    min_width: Val::Px(0.0),
                    height: Val::Px(28.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(5.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                UiInteraction::default(),
                HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                ModeTabBtn(tab),
                Name::new(format!("terrain-mode:{}", tab.label())),
            ))
            .id();
        bind_bg(commands, btn, move |w| {
            if shown(w) == tab {
                rgb(accent())
            } else if matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)) {
                rgb(hover_bg())
            } else {
                Color::NONE
            }
        });
        let fg = move |w: &Rx| if shown(w) == tab { rgb(on_accent()) } else { rgb(text_muted()) };
        let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 13.0);
        commands.entity(ic).insert(PASS_THROUGH);
        bind_text_color(commands, ic, fg);
        let lbl = commands
            .spawn((Text::new(tab.label()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())), PASS_THROUGH))
            .id();
        bind_text_color(commands, lbl, fg);
        commands.entity(btn).add_children(&[ic, lbl]);
        commands.entity(bar).add_child(btn);
    }
    bar
}

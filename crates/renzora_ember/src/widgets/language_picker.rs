//! The language picker: a centered [`overlay`](super::overlay) holding a search
//! box over a grid of every registered language, one tile each.
//!
//! It replaced two drop-ups (the status bar's and the splash rail's) that each
//! listed the languages in a 160px column. With thirty built-in packs, plus any
//! a user drops into `languages/`, that column was a long scroll of single
//! lines; the grid shows them all at once and the search box finds one by its
//! native name or its code. Both callers now share this one widget, so the two
//! places a language is picked cannot drift apart.
//!
//! A caller only marks its trigger with [`LanguageOverlayTrigger`]: pressing it
//! opens the overlay, and picking a tile applies the language
//! (`renzora_lang_api::set_active`), saves it as the editor preference
//! (`renzora_project_api::save_language`) and closes. Whatever relocalizes on a
//! language change (the shell's chrome rebuild, the splash's) takes it from
//! there. The overlay is spawned at the root, not under the trigger, so a
//! rebuild of the chrome the trigger lives in cannot take it down mid-pick.

use bevy::prelude::*;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::interaction::{BLOCK, PASS_THROUGH};
use crate::reactive::tracked::{bind_bg, bind_with};
use crate::theme::*;
use crate::UiInteraction;

use super::overlay::overlay_sized;
use super::scroll_area::scroll_view_bar;
use super::text_input::{text_input, EmberTextInput};

/// Put this on any pressable node to make it open the language overlay.
#[derive(Component, Default)]
pub struct LanguageOverlayTrigger;

/// The open language overlay's root. Only one is ever open.
#[derive(Component)]
pub(crate) struct LanguageOverlay;

/// The overlay's search box.
#[derive(Component)]
pub(crate) struct LanguageSearch;

/// One language tile: its code, and the lowercase text the search matches
/// against (native name and code, so both "deutsch" and "de" find German).
#[derive(Component)]
pub(crate) struct LanguageTile {
    code: String,
    haystack: String,
}

/// Open the overlay when a trigger is pressed, unless one is already open.
pub(crate) fn language_overlay_open(
    triggers: Query<&UiInteraction, (Changed<UiInteraction>, With<LanguageOverlayTrigger>)>,
    open: Query<(), With<LanguageOverlay>>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    if !triggers.iter().any(|i| *i == UiInteraction::Pressed) || !open.is_empty() {
        return;
    }
    let Some(fonts) = fonts else { return };
    language_overlay(&mut commands, &fonts);
}

/// Spawn the language overlay and return its root.
pub fn language_overlay(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut langs = renzora_lang_api::available();
    // Alphabetical by native name, so the grid has an order a reader can scan;
    // registration order is just the order the packs happened to load in.
    langs.sort_by_key(|m| display_name(m).to_lowercase());
    let active = renzora_lang_api::active_code();

    let title = renzora_lang_api::t_or("settings.row.language", "Language");
    let (root, content) = overlay_sized(commands, fonts, &title, 600.0, 520.0, true);
    commands.entity(root).insert((LanguageOverlay, Name::new("language-overlay")));

    let search = text_input(commands, &fonts.ui, &renzora_lang_api::t("common.search"), "");
    commands.entity(search).insert((
        LanguageSearch,
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    let search_bar = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(12.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    commands.entity(search_bar).add_child(search);

    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_content: AlignContent::FlexStart,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            Name::new("language-grid"),
        ))
        .id();
    let tiles: Vec<Entity> = langs
        .iter()
        .map(|m| language_tile(commands, fonts, m, m.code == active))
        .collect();
    commands.entity(grid).add_children(&tiles);
    let scroll = scroll_view_bar(commands, grid);

    commands.entity(content).add_children(&[search_bar, scroll]);
    root
}

/// A language's native name, or its code when the pack left the name empty.
fn display_name(m: &renzora_lang_api::LocaleMeta) -> String {
    if m.name.is_empty() {
        m.code.clone()
    } else {
        m.name.clone()
    }
}

/// One tile: the native name over the code, with a check on the active one.
/// Three to a row in the 600px card.
fn language_tile(
    commands: &mut Commands,
    fonts: &EmberFonts,
    meta: &renzora_lang_api::LocaleMeta,
    is_active: bool,
) -> Entity {
    let name = display_name(meta);
    let tile = commands
        .spawn((
            Node {
                width: Val::Px(176.0),
                height: Val::Px(48.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(divider())),
            UiInteraction::default(),
            BLOCK,
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            LanguageTile {
                code: meta.code.clone(),
                haystack: format!("{} {}", name, meta.code).to_lowercase(),
            },
            Name::new(format!("language-tile:{}", meta.code)),
        ))
        .id();
    let hovered = move |w: &crate::reactive::Rx| {
        matches!(
            w.get::<UiInteraction>(tile),
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
        )
    };
    bind_bg(commands, tile, move |w| {
        if is_active {
            rgb(selection())
        } else if hovered(w) {
            rgb(hover_bg())
        } else {
            rgb(card_bg())
        }
    });
    bind_with(
        commands,
        tile,
        move |w| if is_active || hovered(w) { rgb(accent()) } else { rgb(divider()) },
        |world, target, color: &Color| {
            if let Some(mut border) = world.get_mut::<BorderColor>(target) {
                *border = BorderColor::all(*color);
            }
        },
    );

    let globe = icon_text(
        commands,
        &fonts.phosphor,
        if is_active { "check-circle" } else { "globe" },
        if is_active { accent() } else { text_muted() },
        16.0,
    );
    commands.entity(globe).insert(PASS_THROUGH);

    let col = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(0.0),
                flex_grow: 1.0,
                row_gap: Val::Px(1.0),
                ..default()
            },
            PASS_THROUGH,
        ))
        .id();
    let name_text = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            PASS_THROUGH,
        ))
        .id();
    let code_text = commands
        .spawn((
            Text::new(meta.code.clone()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
            PASS_THROUGH,
        ))
        .id();
    commands.entity(col).add_children(&[name_text, code_text]);
    commands.entity(tile).add_children(&[globe, col]);
    tile
}

/// Focus the search box as the overlay opens, so typing filters at once.
pub(crate) fn language_search_focus(mut q: Query<&mut EmberTextInput, Added<LanguageSearch>>) {
    for mut input in &mut q {
        input.focused = true;
    }
}

/// Hide the tiles the search text does not match.
pub(crate) fn language_search_filter(
    search: Query<&EmberTextInput, (With<LanguageSearch>, Changed<EmberTextInput>)>,
    mut tiles: Query<(&LanguageTile, &mut Node)>,
) {
    let Ok(input) = search.single() else { return };
    let query = input.value.trim().to_lowercase();
    for (tile, mut node) in &mut tiles {
        let display = if query.is_empty() || tile.haystack.contains(&query) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

/// Pick the pressed tile's language: apply it, save it, close the overlay.
pub(crate) fn language_tile_pick(
    tiles: Query<(&UiInteraction, &LanguageTile), Changed<UiInteraction>>,
    overlays: Query<Entity, With<LanguageOverlay>>,
    mut commands: Commands,
) {
    let Some((_, tile)) = tiles.iter().find(|(i, _)| **i == UiInteraction::Pressed) else {
        return;
    };
    renzora_lang_api::set_active(&tile.code);
    if let Err(e) = renzora_project_api::save_language(&tile.code) {
        warn!("could not save the language preference: {e}");
    }
    for overlay in &overlays {
        commands.entity(overlay).despawn();
    }
}

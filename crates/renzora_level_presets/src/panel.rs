//! Bevy-native (ember) port of the egui Level Presets panel (`level_presets`).
//!
//! The egui panel is a template browser: a header with a live entity count, a
//! Scale field + Clear button, and a responsive grid of preset cards (icon +
//! label). Clicking a card selects it and (re)spawns that template level; the
//! Clear button despawns the active level.
//!
//! This native version reuses the same [`LevelPresetsState`] resource and the
//! same [`LevelCommand`] queue that `process_level_commands` (in `lib.rs`)
//! drains — so the apply/clear logic is shared verbatim with the egui path.
//! Card selection tinting and the entity-count label are driven reactively, so
//! the keyed card grid only rebuilds on structure changes (never per-select).

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_text_color, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, DragRange};
use renzora::SplashState;

use crate::state::{LevelCommand, LevelPreset, LevelPresetsState};

const TILE: f32 = 88.0;
/// Height of the icon block on a card. It was 60px around a 26px glyph, which
/// is where most of the card's emptiness came from — two thirds of a tall card
/// was blank, and eleven of them filled the panel with gaps.
const ICON_AREA: f32 = 34.0;

pub struct LevelPresetsPanel;

impl Plugin for LevelPresetsPanel {
    fn build(&self, app: &mut App) {
        // `false`: this panel owns its own vertical scroll over the card grid.
        app.register_panel_content("level_presets", false, build)
            .systems(
            Update,
            (preset_card_click, clear_btn_click).run_if(in_state(SplashState::Editor)),
        );
    }
}

#[derive(Component)]
struct PresetCard(LevelPreset);
#[derive(Component)]
struct ClearBtn;

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("level-presets-root"),
        ))
        .id();

    // ── Header: "Level Templates" + live entity count ────────────────────────
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    let title = commands
        .spawn((
            Text::new("Level Templates"),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let count = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, count, |w| {
        let s = w.resource::<LevelPresetsState>();
        format!("({} entities)", s.entity_count)
    });
    bind_display(commands, count, |w| {
        w.resource::<LevelPresetsState>().has_active_level
    });
    // Clear lives in the header, right-aligned. It was down in the scale row,
    // which put a destructive action next to a value field for no reason —
    // Clear acts on the level named by the title and the count beside it, so
    // that is the row it belongs on.
    let header_gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let clear = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(close_red())),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ClearBtn,
            Name::new("level-presets-clear"),
        ))
        .id();
    let clear_lbl = commands
        .spawn((
            Text::new("Clear"),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(clear).add_child(clear_lbl);
    bind_display(commands, clear, |w| {
        w.resource::<LevelPresetsState>().has_active_level
    });
    commands
        .entity(header)
        .add_children(&[title, count, header_gap, clear]);

    // ── Scale field + Clear button ───────────────────────────────────────────
    let scale_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    let scale_lbl = commands
        .spawn((
            Text::new("Scale"),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    // Scrubbable scale field, clamped to the egui slider's 0.5..=5.0 range.
    let scale_dv = drag_value(
        commands,
        &fonts.ui,
        "",
        accent(),
        1.0,
        0.02,
    );
    commands.entity(scale_dv).insert(DragRange { min: 0.5, max: 5.0 });
    bind_2way(
        commands,
        scale_dv,
        |w: &Rx| w.resource::<LevelPresetsState>().scale,
        |w: &mut World, v: &f32| {
            if let Some(mut s) = w.get_resource_mut::<LevelPresetsState>() {
                if s.scale != *v {
                    s.scale = *v;
                }
            }
        },
    );
    // What Scale actually does — the field on its own says a number, not what
    // the number is for.
    let scale_hint = commands
        .spawn((
            Text::new("Multiplies the template's size when it spawns"),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(placeholder())),
        ))
        .id();
    commands
        .entity(scale_row)
        .add_children(&[scale_lbl, scale_dv, scale_hint]);

    // ── Card grid (scrolls) + footer description/note ────────────────────────
    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    keyed_list(commands, grid, cards_snapshot);

    // ── Pinned detail for the selected template ──────────────────────────────
    //
    // This used to be two loose lines of text at the bottom of the *scrolled*
    // column, under the cards. Which meant the description of the template you
    // had just clicked was somewhere below the fold, and the static note under
    // it read as a caption for whichever card happened to be last. Pinned
    // outside the scroll and given a surface of its own, it is a panel about
    // the selection — always visible, always about the card that is lit up.
    let detail_name = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text(commands, detail_name, |w| {
        w.resource::<LevelPresetsState>().selected.label().to_string()
    });
    let desc = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, desc, |w| {
        w.resource::<LevelPresetsState>().selected.description().to_string()
    });
    let note = commands
        .spawn((
            Text::new("Spawns meshes, lights, and a camera as scene entities"),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(placeholder())),
        ))
        .id();
    let detail = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            Name::new("level-presets-detail"),
        ))
        .id();
    commands
        .entity(detail)
        .add_children(&[detail_name, desc, note]);

    // The grid scrolls; the detail below it does not.
    let scroll = renzora_ember::widgets::scroll_view(commands, grid);

    commands
        .entity(root)
        .add_children(&[header, scale_row, scroll, detail]);
    root
}

/// The card grid is keyed only on the static preset list (selection tinting is
/// reactive per-card via `bind_bg`/`bind_text_color`), so it builds exactly once.
fn cards_snapshot(_world: &Rx) -> KeyedSnapshot {
    let presets = LevelPreset::ALL;
    let items: Vec<(u64, u64)> = presets
        .iter()
        .enumerate()
        .map(|(i, _)| (i as u64, 0))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| preset_card(c, f, presets[i])),
    }
}

fn preset_card(commands: &mut Commands, fonts: &EmberFonts, preset: LevelPreset) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Px(TILE),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(2.0),
                padding: UiRect::vertical(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            PresetCard(preset),
            Name::new(format!("preset:{}", preset.label())),
        ))
        .id();

    // Selected → accent tint; hovered → hover surface; else faint.
    bind_bg(commands, card, move |w| {
        let s = w.resource::<LevelPresetsState>();
        if s.selected == preset {
            rgb(accent()).with_alpha(0.3)
        } else if matches!(
            w.get::<Interaction>(card),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            rgb(section_bg())
        }
    });
    // Selected → solid accent border; hovered → dim accent; else none.
    bind_with_border(commands, card, preset);

    // Icon (accent when selected/hovered, else primary text).
    let icon = icon_text(commands, &fonts.phosphor, preset.icon_name(), text_primary(), 26.0);
    commands.entity(icon).insert(Node {
        height: Val::Px(ICON_AREA),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    });
    bind_text_color(commands, icon, move |w| {
        let s = w.resource::<LevelPresetsState>();
        let hot = s.selected == preset
            || matches!(
                w.get::<Interaction>(card),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            );
        if hot { rgb(accent()) } else { rgb(text_primary()) }
    });

    // Label. `text_primary` rather than muted: on a grid of eleven cards whose
    // icons are generic glyphs, the name is what you actually read.
    let label = commands
        .spawn((
            Text::new(preset.label()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::justify(bevy::text::Justify::Center),
        ))
        .id();

    commands.entity(card).add_children(&[icon, label]);
    card
}

/// Reactive accent border for a card: solid accent when selected, a dim accent
/// on hover, transparent otherwise (mirrors the egui rect_stroke logic).
fn bind_with_border(commands: &mut Commands, card: Entity, preset: LevelPreset) {
    renzora_ember::reactive::tracked::bind_with(
        commands,
        card,
        move |w| {
            let s = w.resource::<LevelPresetsState>();
            if s.selected == preset {
                2u8
            } else if matches!(
                w.get::<Interaction>(card),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            ) {
                1u8
            } else {
                0u8
            }
        },
        |w, e, level: &u8| {
            let color = match level {
                2 => rgb(accent()),
                1 => rgb(accent()).with_alpha(0.6),
                _ => Color::NONE,
            };
            if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                *b = BorderColor::all(color);
            }
        },
    );
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Click a card → select it and (re)spawn its level. Mirrors the egui click
/// handler: a Spawn command is queued when the selection changes OR there is no
/// active level yet (re-clicking the already-selected preset with a live level
/// is a no-op, matching egui).
fn preset_card_click(
    q: Query<(&Interaction, &PresetCard), Changed<Interaction>>,
    mut state: ResMut<LevelPresetsState>,
) {
    for (interaction, card) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let changed = state.selected != card.0;
        state.selected = card.0;
        if changed || !state.has_active_level {
            state.commands.push(LevelCommand::Spawn);
        }
    }
}

/// Clear button → queue a Clear command (despawns the active level).
fn clear_btn_click(
    q: Query<&Interaction, (With<ClearBtn>, Changed<Interaction>)>,
    mut state: ResMut<LevelPresetsState>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) && state.has_active_level {
        state.commands.push(LevelCommand::Clear);
    }
}

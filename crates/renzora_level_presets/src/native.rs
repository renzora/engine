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
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, DragRange};
use renzora::SplashState;

use crate::state::{LevelCommand, LevelPreset, LevelPresetsState};

const TILE: f32 = 88.0;
const ICON_AREA: f32 = 60.0;

pub struct NativeLevelPresets;

impl Plugin for NativeLevelPresets {
    fn build(&self, app: &mut App) {
        // `false`: this panel owns its own vertical scroll over the card grid.
        app.register_panel_content("level_presets", false, build);
        app.add_systems(
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
    commands.entity(header).add_children(&[title, count]);

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
        |w: &World| w.resource::<LevelPresetsState>().scale,
        |w: &mut World, v: &f32| {
            if let Some(mut s) = w.get_resource_mut::<LevelPresetsState>() {
                if s.scale != *v {
                    s.scale = *v;
                }
            }
        },
    );
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    // Clear button — only visible when there's an active level.
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
        .entity(scale_row)
        .add_children(&[scale_lbl, scale_dv, gap, clear]);

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

    // Description of the currently-selected preset.
    let desc = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
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
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
        ))
        .id();

    // Inner scroll column holding the grid + footer text.
    let scroll_col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    commands.entity(scroll_col).add_children(&[grid, desc, note]);
    let scroll = renzora_ember::widgets::scroll_view(commands, scroll_col);

    commands.entity(root).add_children(&[header, scale_row, scroll]);
    root
}

/// The card grid is keyed only on the static preset list (selection tinting is
/// reactive per-card via `bind_bg`/`bind_text_color`), so it builds exactly once.
fn cards_snapshot(_world: &World) -> KeyedSnapshot {
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

    // Label.
    let label = commands
        .spawn((
            Text::new(preset.label()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::new_with_justify(bevy::text::Justify::Center),
        ))
        .id();

    commands.entity(card).add_children(&[icon, label]);
    card
}

/// Reactive accent border for a card: solid accent when selected, a dim accent
/// on hover, transparent otherwise (mirrors the egui rect_stroke logic).
fn bind_with_border(commands: &mut Commands, card: Entity, preset: LevelPreset) {
    renzora_ember::reactive::bind_with(
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

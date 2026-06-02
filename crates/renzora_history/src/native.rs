//! Bevy-native (ember) History panel — a faithful bevy_ui port of the egui
//! History panel: Undo Stack / Current State / Redo Stack sections of clickable
//! rows that jump through the undo/redo stack.
//!
//! Built once into its dock pane; the row list is a reactive `keyed_list` that
//! diffs `(key, hash)` and rebuilds only changed rows (no manual gate). Hover is
//! a `bind_bg` effect. Clicks push undo/redo through `EditorCommands` — the same
//! write path the egui panel used.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_editor::EditorCommands;
use renzora_ember::dock::{tab_pane, DockLeaf, TabPane};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_bg, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{rgb, ACCENT_BLUE, HEADER_BG, PLACEHOLDER, TEXT_MUTED, TEXT_PRIMARY};
use renzora_undo::UndoStacks;

const PANEL_ID: &str = "history";
const ROW_H: f32 = 22.0;
/// Selection-stroke tint behind the current state (egui used alpha 40/255).
const CURRENT_BG: (u8, u8, u8) = ACCENT_BLUE;
const CURRENT_BG_A: f32 = 40.0 / 255.0;
/// Hover wash on past/future rows (egui used white @ 12/255).
const HOVER_A: f32 = 12.0 / 255.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum HistoryAction {
    Undo(usize),
    Redo(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum RowKind {
    Past,
    Current,
    Future,
}

/// A clickable history row. `action` is `None` for the current state.
#[derive(Component)]
pub(crate) struct HistoryRow {
    action: Option<HistoryAction>,
}

/// One entry in the flattened list (section header / hint / row / empty state).
#[derive(Clone)]
enum Item {
    Header(&'static str),
    Hint(&'static str),
    Row {
        icon: &'static str,
        label: String,
        kind: RowKind,
        action: Option<HistoryAction>,
    },
    Empty,
}

/// Content hash for an item — bump only when its rendering would differ.
fn hash_item(it: &Item) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    match it {
        Item::Header(s) => (0u8, s).hash(&mut h),
        Item::Hint(s) => (1u8, s).hash(&mut h),
        Item::Empty => 2u8.hash(&mut h),
        Item::Row {
            icon,
            label,
            kind,
            action,
        } => (3u8, icon, label, kind, action).hash(&mut h),
    }
    h.finish()
}

/// Flatten the undo/redo labels into the display list (mirrors the egui layout).
fn build_items(undo: &[String], redo: &[String]) -> Vec<Item> {
    if undo.is_empty() && redo.is_empty() {
        return vec![Item::Empty];
    }

    let mut items = Vec::new();
    let n_undo = undo.len();

    items.push(Item::Header("Undo Stack"));
    if n_undo <= 1 {
        items.push(Item::Hint("No earlier states."));
    } else {
        // Exclude the most recent entry — it IS the current state.
        for (i, label) in undo.iter().take(n_undo - 1).enumerate() {
            items.push(Item::Row {
                icon: "arrow-bend-up-left",
                label: label.clone(),
                kind: RowKind::Past,
                action: Some(HistoryAction::Undo(n_undo - 1 - i)),
            });
        }
    }

    items.push(Item::Header("Current State"));
    let current_label = undo.last().map(|s| s.as_str()).unwrap_or("Initial state");
    items.push(Item::Row {
        icon: "caret-right",
        label: current_label.to_string(),
        kind: RowKind::Current,
        action: None,
    });

    items.push(Item::Header("Redo Stack"));
    if redo.is_empty() {
        items.push(Item::Hint("Nothing to redo."));
    } else {
        // Most-immediate redo first (back of the deque).
        for (i, label) in redo.iter().rev().enumerate() {
            items.push(Item::Row {
                icon: "arrow-bend-up-right",
                label: label.clone(),
                kind: RowKind::Future,
                action: Some(HistoryAction::Redo(i + 1)),
            });
        }
    }

    items
}

// ── Build helpers ─────────────────────────────────────────────────────────

fn section_header(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(HEADER_BG)),
            Name::new("history-section"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id();
    commands.entity(row).add_child(text);
    row
}

fn section_hint(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    let row = commands
        .spawn((Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            ..default()
        },))
        .id();
    let t = commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id();
    commands.entity(row).add_child(t);
    row
}

fn history_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    kind: RowKind,
    action: Option<HistoryAction>,
) -> Entity {
    let (icon_color, label_color) = match kind {
        RowKind::Current => (ACCENT_BLUE, TEXT_PRIMARY),
        RowKind::Past => (TEXT_MUTED, TEXT_PRIMARY),
        RowKind::Future => (TEXT_MUTED, TEXT_MUTED),
    };
    let current = kind == RowKind::Current;
    let base = if current {
        rgb(CURRENT_BG).with_alpha(CURRENT_BG_A)
    } else {
        Color::NONE
    };
    let mut row = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(ROW_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::left(Val::Px(12.0)),
            column_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(base),
        Interaction::default(),
        HistoryRow { action },
        Name::new("history-row"),
    ));
    if !current {
        row.insert(renzora_hui::cursor_icon::HoverCursor(
            bevy::window::SystemCursorIcon::Pointer,
        ));
    }
    let row = row.id();
    // Hover wash (past/future only) — value-diffed effect, no per-frame system.
    bind_bg(commands, row, move |world| {
        if current {
            return base;
        }
        match world.get::<Interaction>(row) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(1.0, 1.0, 1.0, HOVER_A)
            }
            _ => Color::NONE,
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, icon_color, 12.0);
    let tx = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(label_color)),
        ))
        .id();
    commands.entity(row).add_children(&[ic, tx]);
    row
}

fn empty_state(commands: &mut Commands, fonts: &EmberFonts, icon: &str, title: &str, subtitle: &str) -> Entity {
    let root = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        },))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, PLACEHOLDER, 32.0);
    let t = commands
        .spawn((
            Text::new(title),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    let s = commands
        .spawn((
            Text::new(subtitle),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id();
    commands.entity(root).add_children(&[ic, t, s]);
    root
}

fn build_item(commands: &mut Commands, fonts: &EmberFonts, it: &Item) -> Entity {
    match it {
        Item::Header(s) => section_header(commands, fonts, s),
        Item::Hint(s) => section_hint(commands, fonts, s),
        Item::Empty => empty_state(
            commands,
            fonts,
            "clock-counter-clockwise",
            "No History",
            "Actions you perform will appear here.",
        ),
        Item::Row {
            icon,
            label,
            kind,
            action,
        } => history_row(commands, fonts, icon, label, *kind, *action),
    }
}

/// The keyed-list snapshot: index-keyed (the list reshuffles wholesale on
/// undo/redo, which is rare), content-hashed so unchanged rows are kept.
fn history_snapshot(world: &World) -> KeyedSnapshot {
    let data: Vec<Item> = match world.get_resource::<UndoStacks>() {
        Some(stacks) => {
            let (undo, redo) = stacks.labels(&stacks.active);
            build_items(&undo, &redo)
        }
        None => Vec::new(),
    };
    let items: Vec<(u64, u64)> = data
        .iter()
        .enumerate()
        .map(|(i, it)| (i as u64, hash_item(it)))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| build_item(c, f, &data[i])),
    }
}

// ── Registration ────────────────────────────────────────────────────────────

pub fn register_native_history(app: &mut App) {
    use renzora::NativePanelExt;
    use renzora_editor::SplashState;
    app.register_native_panel(PANEL_ID);
    app.add_systems(
        Update,
        (history_content_system, history_click).run_if(in_state(SplashState::Editor)),
    );
}

// ── Systems ───────────────────────────────────────────────────────────────

/// Build the history list pane once (lazily) when its tab is first activated.
pub(crate) fn history_content_system(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    leaves: Query<&DockLeaf>,
    children: Query<&Children>,
    panes: Query<&TabPane>,
) {
    let Some(_fonts) = fonts else {
        return;
    };
    for leaf in &leaves {
        if leaf.active != PANEL_ID {
            continue;
        }
        let exists = children.get(leaf.content).is_ok_and(|kids| {
            kids.iter()
                .any(|c| panes.get(c).is_ok_and(|p| p.id == PANEL_ID))
        });
        if exists {
            continue;
        }
        let list = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    padding: UiRect::new(Val::Px(0.0), Val::Px(0.0), Val::Px(4.0), Val::Px(8.0)),
                    ..default()
                },
                Name::new("history-list"),
            ))
            .id();
        // Reactive keyed list drives the rows from here on (build once).
        keyed_list(&mut commands, list, history_snapshot);
        let pane = tab_pane(&mut commands, PANEL_ID, list, true);
        commands.entity(leaf.content).add_child(pane);
    }
}

/// Row click → push the corresponding undo/redo onto `EditorCommands`.
pub(crate) fn history_click(
    rows: Query<(&Interaction, &HistoryRow), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else {
        return;
    };
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match row.action {
            Some(HistoryAction::Undo(n)) => {
                cmds.push(move |world: &mut World| {
                    for _ in 0..n {
                        renzora_undo::undo_once(world);
                    }
                });
            }
            Some(HistoryAction::Redo(n)) => {
                cmds.push(move |world: &mut World| {
                    for _ in 0..n {
                        renzora_undo::redo_once(world);
                    }
                });
            }
            None => {}
        }
    }
}

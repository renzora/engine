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

use renzora_editor_framework::EditorCommands;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_bg, keyed_list};
use renzora_ember::theme::*;
use renzora_undo::UndoStacks;

const PANEL_ID: &str = "history";
const ROW_H: f32 = 22.0;
/// Selection-stroke tint behind the current state (egui used alpha 40/255).
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
            BackgroundColor(rgb(header_bg())),
            Name::new("history-section"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
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
            TextColor(rgb(text_muted())),
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
        RowKind::Current => (accent(), text_primary()),
        RowKind::Past => (text_muted(), text_primary()),
        RowKind::Future => (text_muted(), text_muted()),
    };
    let current = kind == RowKind::Current;
    let base = if current {
        rgb(accent()).with_alpha(CURRENT_BG_A)
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
        row.insert(renzora_ember::cursor_icon::HoverCursor(
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
    let ic = icon_text(commands, &fonts.phosphor, icon, placeholder(), 32.0);
    let t = commands
        .spawn((
            Text::new(title),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let s = commands
        .spawn((
            Text::new(subtitle),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
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
fn history_snapshot(world: &Rx) -> KeyedSnapshot {
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
    use renzora_editor_framework::SplashState;
    // Build once; the reactive keyed list drives the rows from here on.
    app.register_panel_content(PANEL_ID, true, |commands, _fonts| {
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
        keyed_list(commands, list, history_snapshot);
        list
    })
    .systems(Update, history_click.run_if(in_state(SplashState::Editor)));
}

// ── Systems ───────────────────────────────────────────────────────────────

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

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Compact rendering of the flattened list, so a test can assert on layout
    /// without matching six-field struct variants inline.
    fn sketch(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|it| match it {
                Item::Header(s) => format!("header:{s}"),
                Item::Hint(s) => format!("hint:{s}"),
                Item::Empty => "empty".to_string(),
                Item::Row { label, kind, action, .. } => {
                    let k = match kind {
                        RowKind::Past => "past",
                        RowKind::Current => "current",
                        RowKind::Future => "future",
                    };
                    let a = match action {
                        Some(HistoryAction::Undo(n)) => format!("undo{n}"),
                        Some(HistoryAction::Redo(n)) => format!("redo{n}"),
                        None => "-".to_string(),
                    };
                    format!("{k}/{a}:{label}")
                }
            })
            .collect()
    }

    #[test]
    fn a_completely_empty_history_renders_only_the_empty_state() {
        assert_eq!(sketch(&build_items(&[], &[])), vec!["empty"]);
    }

    /// The undo stack's last entry *is* the current state, so a stack of one has
    /// no earlier state to jump back to. Listing it under "Undo Stack" as well
    /// would offer the user an undo that goes nowhere.
    #[test]
    fn a_single_undo_entry_is_the_current_state_and_not_an_earlier_one() {
        let items = build_items(&labels(&["Move Cube"]), &[]);
        assert_eq!(
            sketch(&items),
            vec![
                "header:Undo Stack",
                "hint:No earlier states.",
                "header:Current State",
                "current/-:Move Cube",
                "header:Redo Stack",
                "hint:Nothing to redo.",
            ]
        );
    }

    /// The step count is how many undos it takes to reach that row, so the
    /// oldest entry must carry the largest number. Getting this backwards jumps
    /// the user to the wrong state.
    #[test]
    fn past_rows_count_the_undo_steps_needed_to_reach_them() {
        let items = build_items(&labels(&["Add Light", "Move Cube", "Scale Cube"]), &[]);
        assert_eq!(
            sketch(&items),
            vec![
                "header:Undo Stack",
                "past/undo2:Add Light",
                "past/undo1:Move Cube",
                "header:Current State",
                "current/-:Scale Cube",
                "header:Redo Stack",
                "hint:Nothing to redo.",
            ]
        );
    }

    /// The redo deque has the most-immediate redo at its back, and the panel
    /// lists nearest-first — so the display order is the reverse of storage.
    #[test]
    fn redo_rows_are_listed_most_immediate_first() {
        let items = build_items(&labels(&["Base"]), &labels(&["Furthest", "Nearest"]));
        assert_eq!(
            sketch(&items),
            vec![
                "header:Undo Stack",
                "hint:No earlier states.",
                "header:Current State",
                "current/-:Base",
                "header:Redo Stack",
                "future/redo1:Nearest",
                "future/redo2:Furthest",
            ]
        );
    }

    /// Redo-only is reachable: undo everything, and the undo stack empties while
    /// the redo stack stays full. There is no label to show as current then.
    #[test]
    fn an_empty_undo_stack_still_names_a_current_state() {
        let items = build_items(&[], &labels(&["Move Cube"]));
        assert_eq!(
            sketch(&items),
            vec![
                "header:Undo Stack",
                "hint:No earlier states.",
                "header:Current State",
                "current/-:Initial state",
                "header:Redo Stack",
                "future/redo1:Move Cube",
            ]
        );
    }

    #[test]
    fn exactly_one_row_is_ever_the_current_state() {
        let items = build_items(&labels(&["a", "b", "c"]), &labels(&["d", "e"]));
        let current = items
            .iter()
            .filter(|it| matches!(it, Item::Row { kind: RowKind::Current, .. }))
            .count();
        assert_eq!(current, 1);
    }

    /// Only the current row is unclickable; every other row must carry an action
    /// or clicking it silently does nothing.
    #[test]
    fn every_row_but_the_current_one_is_actionable() {
        let items = build_items(&labels(&["a", "b", "c"]), &labels(&["d"]));
        for it in &items {
            if let Item::Row { kind, action, .. } = it {
                match kind {
                    RowKind::Current => assert!(action.is_none()),
                    _ => assert!(action.is_some(), "a {kind:?} row must be clickable"),
                }
            }
        }
    }

    // ── the reconcile hash ───────────────────────────────────────────────────

    /// The panel reuses row entities whose hash is unchanged. If the hash missed
    /// a field, an edited row would keep rendering its old content.
    #[test]
    fn the_item_hash_tracks_every_rendered_field() {
        // Enum variants have no functional-update syntax, so vary one field at a
        // time through a builder.
        let row = |icon, label: &str, kind, action| Item::Row {
            icon,
            label: label.to_string(),
            kind,
            action,
        };
        let undo1 = Some(HistoryAction::Undo(1));
        let key = hash_item(&row("caret-right", "Move Cube", RowKind::Past, undo1));

        assert_ne!(key, hash_item(&row("caret-right", "Scale Cube", RowKind::Past, undo1)));
        assert_ne!(key, hash_item(&row("caret-right", "Move Cube", RowKind::Future, undo1)));
        assert_ne!(
            key,
            hash_item(&row("caret-right", "Move Cube", RowKind::Past, Some(HistoryAction::Undo(2))))
        );
        assert_ne!(
            key,
            hash_item(&row("arrow-bend-up-left", "Move Cube", RowKind::Past, undo1))
        );
        // An undo and a redo of the same depth are different destinations.
        assert_ne!(
            hash_item(&row("i", "l", RowKind::Past, Some(HistoryAction::Undo(1)))),
            hash_item(&row("i", "l", RowKind::Past, Some(HistoryAction::Redo(1))))
        );
    }

    #[test]
    fn identical_items_hash_identically() {
        let a = Item::Header("Undo Stack");
        let b = Item::Header("Undo Stack");
        assert_eq!(hash_item(&a), hash_item(&b));
    }

    /// A header, a hint and an empty state carrying the same text must not
    /// collide — the variant tag is what keeps them apart.
    #[test]
    fn different_item_kinds_do_not_collide() {
        let header = hash_item(&Item::Header("Undo Stack"));
        let hint = hash_item(&Item::Hint("Undo Stack"));
        let empty = hash_item(&Item::Empty);
        assert_ne!(header, hint);
        assert_ne!(header, empty);
        assert_ne!(hint, empty);
    }

    impl std::fmt::Debug for RowKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self {
                RowKind::Past => "past",
                RowKind::Current => "current",
                RowKind::Future => "future",
            })
        }
    }
}

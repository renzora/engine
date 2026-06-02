//! Bevy-native (ember) History panel — a faithful bevy_ui port of the egui
//! History panel: Undo Stack / Current State / Redo Stack sections of clickable
//! rows that jump through the undo/redo stack.
//!
//! Built once into its dock pane; the row list rebuilds only when the
//! `UndoStacks` labels change (compared via a signature). Clicks push undo/redo
//! through `EditorCommands` — the same write path the egui panel used.

use bevy::prelude::*;

use renzora_editor::EditorCommands;
use renzora_ember::dock::{tab_pane, DockLeaf, TabPane};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{rgb, ACCENT_BLUE, HEADER_BG, PLACEHOLDER, TEXT_MUTED, TEXT_PRIMARY};
use renzora_undo::UndoStacks;

const PANEL_ID: &str = "history";
const ROW_H: f32 = 22.0;
/// Selection-stroke tint behind the current state (egui used alpha 40/255).
const CURRENT_BG: (u8, u8, u8) = ACCENT_BLUE;
const CURRENT_BG_A: f32 = 40.0 / 255.0;
/// Hover wash on past/future rows (egui used white @ 12/255).
const HOVER_A: f32 = 12.0 / 255.0;

#[derive(Clone, Copy, PartialEq)]
enum HistoryAction {
    Undo(usize),
    Redo(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum RowKind {
    Past,
    Current,
    Future,
}

/// On the list container: the last-rendered labels, to rebuild only on change.
#[derive(Component)]
pub(crate) struct HistoryView {
    sig: (Vec<String>, Vec<String>),
}

/// A clickable history row. `action` is `None` for the current state.
#[derive(Component)]
pub(crate) struct HistoryRow {
    action: Option<HistoryAction>,
    current: bool,
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
    let bg = if current {
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
        BackgroundColor(bg),
        Interaction::default(),
        HistoryRow { action, current },
        Name::new("history-row"),
    ));
    if !current {
        row.insert(renzora_hui::cursor_icon::HoverCursor(
            bevy::window::SystemCursorIcon::Pointer,
        ));
    }
    let row = row.id();
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

/// Rebuild the list's children from the current undo/redo labels.
fn populate(commands: &mut Commands, fonts: &EmberFonts, list: Entity, undo: &[String], redo: &[String]) {
    if undo.is_empty() && redo.is_empty() {
        let e = empty_state(
            commands,
            fonts,
            "clock-counter-clockwise",
            "No History",
            "Actions you perform will appear here.",
        );
        commands.entity(list).add_child(e);
        return;
    }

    let mut kids: Vec<Entity> = Vec::new();
    let n_undo = undo.len();

    kids.push(section_header(commands, fonts, "Undo Stack"));
    if n_undo <= 1 {
        kids.push(section_hint(commands, fonts, "No earlier states."));
    } else {
        // Exclude the most recent entry — it IS the current state.
        for (i, label) in undo.iter().take(n_undo - 1).enumerate() {
            kids.push(history_row(
                commands,
                fonts,
                "arrow-bend-up-left",
                label,
                RowKind::Past,
                Some(HistoryAction::Undo(n_undo - 1 - i)),
            ));
        }
    }

    kids.push(section_header(commands, fonts, "Current State"));
    let current_label = undo.last().map(|s| s.as_str()).unwrap_or("Initial state");
    kids.push(history_row(
        commands,
        fonts,
        "caret-right",
        current_label,
        RowKind::Current,
        None,
    ));

    kids.push(section_header(commands, fonts, "Redo Stack"));
    if redo.is_empty() {
        kids.push(section_hint(commands, fonts, "Nothing to redo."));
    } else {
        // Most-immediate redo first (back of the deque).
        for (i, label) in redo.iter().rev().enumerate() {
            kids.push(history_row(
                commands,
                fonts,
                "arrow-bend-up-right",
                label,
                RowKind::Future,
                Some(HistoryAction::Redo(i + 1)),
            ));
        }
    }

    commands.entity(list).add_children(&kids);
}

// ── Registration ────────────────────────────────────────────────────────────

pub fn register_native_history(app: &mut App) {
    use renzora::NativePanelExt;
    use renzora_editor::SplashState;
    app.register_native_panel(PANEL_ID);
    app.add_systems(
        Update,
        (
            (history_content_system, history_refresh).chain(),
            history_click,
            history_hover,
        )
            .run_if(in_state(SplashState::Editor)),
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
                HistoryView {
                    sig: (Vec::new(), vec!["\u{0}".to_string()]), // force first rebuild
                },
                Name::new("history-list"),
            ))
            .id();
        let pane = tab_pane(&mut commands, PANEL_ID, list, true);
        commands.entity(leaf.content).add_child(pane);
    }
}

/// Rebuild the row list when the undo/redo labels change.
pub(crate) fn history_refresh(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    stacks: Option<Res<UndoStacks>>,
    mut views: Query<(Entity, &mut HistoryView)>,
    children: Query<&Children>,
) {
    let (Some(fonts), Some(stacks)) = (fonts, stacks) else {
        return;
    };
    let (undo, redo) = stacks.labels(&stacks.active);
    let sig = (undo.clone(), redo.clone());
    for (list, mut view) in &mut views {
        if view.sig == sig {
            continue;
        }
        view.sig = sig.clone();
        if let Ok(kids) = children.get(list) {
            for k in kids.iter() {
                commands.entity(k).despawn();
            }
        }
        populate(&mut commands, &fonts, list, &undo, &redo);
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

/// Hover wash on past/future rows (current row keeps its selection tint).
pub(crate) fn history_hover(
    mut rows: Query<(&Interaction, &HistoryRow, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, row, mut bg) in &mut rows {
        if row.current {
            continue;
        }
        bg.0 = match interaction {
            Interaction::Hovered | Interaction::Pressed => Color::srgba(1.0, 1.0, 1.0, HOVER_A),
            Interaction::None => Color::NONE,
        };
    }
}

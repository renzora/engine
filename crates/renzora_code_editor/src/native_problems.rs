//! Bevy-native (ember) Problems panel — `ScriptError`s from every open editor
//! tab, plus [`ContentProblems`], which files nobody has open report into.
//! Clicking a row jumps to it, when it belongs to an open tab.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_editor_framework::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_display, keyed_list};
use renzora_ember::theme::*;

use renzora::content_problems::{ContentProblems, ProblemSeverity};

use crate::state::CodeEditorState;

#[derive(Component)]
struct ProblemRow {
    /// `None` when the reporting file is not open in a tab.
    file_idx: Option<usize>,
    line: usize,
}

/// One panel row, from either source.
#[derive(Clone, Hash)]
struct Row {
    file_idx: Option<usize>,
    /// Shown under the message; a file name for a tab, a project-relative path
    /// for a content problem.
    location: String,
    message: String,
    line: Option<usize>,
    column: Option<usize>,
    is_error: bool,
}

pub fn register_native_problems(app: &mut App) {
    app.register_panel_content("problems", true, build)
        .systems(Update, problems_goto_click.run_if(in_state(SplashState::Editor)));
}

fn has_problems(w: &Rx) -> bool {
    w.get_resource::<CodeEditorState>().is_some_and(|s| s.open_files.iter().any(|f| f.error.is_some()))
        || w.get_resource::<ContentProblems>().is_some_and(|p| !p.is_empty())
}

/// Which open tab, if any, a content problem's project-relative path belongs to.
///
/// `Path::ends_with` matches whole components, so `mat.material` cannot claim
/// `other_mat.material`.
fn open_tab_for(state: Option<&CodeEditorState>, rel: &str) -> Option<usize> {
    state?
        .open_files
        .iter()
        .position(|f| f.path.ends_with(rel))
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-problems"),
        ))
        .id();

    // Empty state.
    let note = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(4.0), padding: UiRect::vertical(Val::Px(14.0)), ..default() })
        .id();
    let check = icon_text(commands, &fonts.phosphor, "check-circle", play_green(), 20.0);
    let note_lbl = commands.spawn((Text::new(renzora::lang::t("code.no_problems")), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    commands.entity(note).add_children(&[check, note_lbl]);
    bind_display(commands, note, |w| !has_problems(w));

    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, list, problems_snapshot);

    commands.entity(root).add_children(&[note, list]);
    root
}

fn problems_snapshot(world: &Rx) -> KeyedSnapshot {
    let state = world.get_resource::<CodeEditorState>();
    let mut rows: Vec<Row> = Vec::new();

    if let Some(state) = state.as_ref() {
        for (idx, file) in state.open_files.iter().enumerate() {
            let Some(error) = file.error.as_ref() else { continue };
            rows.push(Row {
                file_idx: Some(idx),
                location: file.name.clone(),
                message: error.message.lines().next().unwrap_or_default().to_string(),
                line: error.line,
                column: error.column,
                is_error: true,
            });
        }
    }

    if let Some(problems) = world.get_resource::<ContentProblems>() {
        for (path, problem) in problems.iter() {
            rows.push(Row {
                file_idx: open_tab_for(state, path),
                location: path.to_string(),
                message: problem.message.lines().next().unwrap_or_default().to_string(),
                line: problem.line,
                column: None,
                is_error: problem.severity == ProblemSeverity::Error,
            });
        }
    }

    if rows.is_empty() {
        return empty();
    }

    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut k = hasher();
            (i, &row.location).hash(&mut k);
            let mut h = hasher();
            row.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| problem_row(c, f, &rows[i])),
    }
}

fn problem_row(commands: &mut Commands, fonts: &EmberFonts, item: &Row) -> Entity {
    let Row { file_idx, location: name, message: msg, line, column: col, is_error } = item;
    let (line, col) = (*line, *col);
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), min_height: Val::Px(32.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ProblemRow { file_idx: *file_idx, line: line.unwrap_or(1).max(1) },
        ))
        .id();
    let (glyph, tint) = if *is_error { ("warning", close_red()) } else { ("warning-circle", warn_amber()) };
    let icon = icon_text(commands, &fonts.phosphor, glyph, tint, 14.0);
    let text_col = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() }).id();
    let msg_lbl = commands.spawn((Text::new(msg.to_string()), ui_font(&fonts.mono, 11.5), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap())).id();
    let location = match (line, col) {
        (Some(l), Some(c)) => format!("{}:{}:{}", name, l, c),
        (Some(l), None) => format!("{}:{}", name, l),
        _ => name.to_string(),
    };
    let loc_lbl = commands.spawn((Text::new(location), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    commands.entity(text_col).add_children(&[msg_lbl, loc_lbl]);
    commands.entity(row).add_children(&[icon, text_col]);
    row
}

fn problems_goto_click(q: Query<(&Interaction, &ProblemRow), Changed<Interaction>>, state: Option<ResMut<CodeEditorState>>) {
    let Some(mut state) = state else { return };
    for (interaction, row) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(idx) = row.file_idx {
            state.active_tab = Some(idx);
            state.pending_goto_line = Some(row.line);
        }
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

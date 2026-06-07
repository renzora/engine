//! Bevy-native (ember) Problems panel — aggregates `ScriptError`s across every
//! open editor tab. Clicking a row switches to that file and jumps to the error
//! line (writes `active_tab` + `pending_goto_line`). Mirrors the egui
//! `ProblemsPanel`.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_editor_framework::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;

use crate::state::CodeEditorState;

#[derive(Component)]
struct ProblemRow {
    file_idx: usize,
    line: usize,
}

pub fn register_native_problems(app: &mut App) {
    app.register_panel_content("problems", true, build);
    app.add_systems(Update, problems_goto_click.run_if(in_state(SplashState::Editor)));
}

fn has_problems(w: &World) -> bool {
    w.get_resource::<CodeEditorState>().is_some_and(|s| s.open_files.iter().any(|f| f.error.is_some()))
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
    let note_lbl = commands.spawn((Text::new("No problems detected"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    commands.entity(note).add_children(&[check, note_lbl]);
    bind_display(commands, note, |w| !has_problems(w));

    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, list, problems_snapshot);

    commands.entity(root).add_children(&[note, list]);
    root
}

fn problems_snapshot(world: &World) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<CodeEditorState>() else { return empty() };
    // (file_idx, file_name, message first line, line, col)
    let problems: Vec<(usize, String, String, Option<usize>, Option<usize>)> = state
        .open_files
        .iter()
        .enumerate()
        .filter_map(|(idx, f)| {
            f.error.as_ref().map(|e| (idx, f.name.clone(), e.message.lines().next().unwrap_or("").to_string(), e.line, e.column))
        })
        .collect();
    let items: Vec<(u64, u64)> = problems
        .iter()
        .map(|(idx, name, msg, line, col)| {
            let mut k = hasher();
            idx.hash(&mut k);
            let mut h = hasher();
            (name, msg, line, col).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (idx, name, msg, line, col) = &problems[i];
            problem_row(c, f, *idx, name, msg, *line, *col)
        }),
    }
}

fn problem_row(commands: &mut Commands, fonts: &EmberFonts, file_idx: usize, name: &str, msg: &str, line: Option<usize>, col: Option<usize>) -> Entity {
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), min_height: Val::Px(32.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ProblemRow { file_idx, line: line.unwrap_or(1).max(1) },
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "warning", close_red(), 14.0);
    let text_col = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() }).id();
    let msg_lbl = commands.spawn((Text::new(msg.to_string()), ui_font(&fonts.mono, 11.5), TextColor(rgb(text_primary())), bevy::text::TextLayout::new_with_no_wrap())).id();
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
        if *interaction == Interaction::Pressed {
            state.active_tab = Some(row.file_idx);
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

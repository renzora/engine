//! Bevy-native (ember) port of the egui `ShaderCompilerLogPanel`: a success /
//! "no output" status line, or a scrollable list of compile errors with a
//! `[line:col] message` location prefix in monospace.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, KeyedSnapshot};
use renzora_ember::theme::*;

use crate::ShaderEditorState;

pub struct NativeShaderCompilerLog;

impl Plugin for NativeShaderCompilerLog {
    fn build(&self, app: &mut App) {
        app.register_panel_content("shader_compiler_log", true, build);
    }
}

fn has_errors(w: &World) -> bool {
    w.get_resource::<ShaderEditorState>().is_some_and(|s| !s.compile_errors.is_empty())
}
fn compiled(w: &World) -> bool {
    w.get_resource::<ShaderEditorState>().is_some_and(|s| s.compiled_wgsl.is_some())
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(6.0)), row_gap: Val::Px(2.0), ..default() },
            Name::new("native-shader-compiler-log"),
        ))
        .id();

    // Success line.
    let success = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() })
        .id();
    let check = icon_text(commands, &fonts.phosphor, "check-circle", play_green(), 12.0);
    let success_lbl = commands.spawn((Text::new(renzora::lang::t("shader_preview.compiled_ok")), ui_font(&fonts.ui, 11.0), TextColor(rgb(play_green())))).id();
    commands.entity(success).add_children(&[check, success_lbl]);
    bind_display(commands, success, |w| !has_errors(w) && compiled(w));

    // No-output line.
    let no_output = commands.spawn((Text::new(renzora::lang::t("shader_preview.no_output")), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_display(commands, no_output, |w| !has_errors(w) && !compiled(w));

    // Error list.
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).id();
    renzora_ember::virtual_scroll::virtual_scroll(commands, list, 6, error_snapshot);

    commands.entity(root).add_children(&[success, no_output, list]);
    root
}

fn error_snapshot(world: &World) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<ShaderEditorState>() else { return empty() };
    let errs: Vec<(String, Option<usize>, Option<usize>)> = state
        .compile_errors
        .iter()
        .map(|e| (e.message.clone(), e.line, e.column))
        .collect();
    let items: Vec<(u64, u64)> = errs
        .iter()
        .enumerate()
        .map(|(i, (msg, line, col))| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (msg, line, col).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (msg, line, col) = &errs[i];
            let loc = match (line, col) {
                (Some(l), Some(cl)) => format!("[{}:{}] ", l, cl),
                (Some(l), None) => format!("[line {}] ", l),
                _ => String::new(),
            };
            let row = c
                .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() })
                .id();
            let icon = icon_text(c, &f.phosphor, "warning", close_red(), 12.0);
            let lbl = c.spawn((Text::new(format!("{}{}", loc, msg)), ui_font(&f.mono, 11.0), TextColor(rgb(close_red())))).id();
            c.entity(row).add_children(&[icon, lbl]);
            row
        }),
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

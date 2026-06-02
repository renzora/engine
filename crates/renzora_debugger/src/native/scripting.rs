//! Bevy-native Scripting diagnostics panel — totals header + a per-script timing
//! table. Pure reader over `ScriptingDiagState`; the table is a `keyed_list`
//! keyed by script path.

use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{rgb, TEXT_MUTED, TEXT_PRIMARY};
use renzora_scripting::perf::ScriptPerf;

use crate::panels::scripting::ScriptingDiagState;

use super::{root, section};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const ERR_RED: (u8, u8, u8) = (230, 110, 110);

pub(super) fn register_scripting(app: &mut App) {
    app.register_panel_content("scripting_diag", true, build_scripting);
}

fn scr<R: Default>(w: &World, f: impl FnOnce(&ScriptingDiagState) -> R) -> R {
    w.get_resource::<ScriptingDiagState>().map(f).unwrap_or_default()
}

fn format_duration(d: Duration) -> String {
    let us = d.as_micros();
    if us == 0 {
        "\u{2014}".to_string()
    } else if us < 1_000 {
        format!("{} \u{b5}s", us)
    } else if us < 1_000_000 {
        format!("{:.2} ms", us as f64 / 1_000.0)
    } else {
        format!("{:.2} s", us as f64 / 1_000_000.0)
    }
}

fn format_count(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('\u{2026}');
        out
    }
}

fn duration_color(d: Duration, base: Color) -> Color {
    let micros = d.as_micros();
    if micros >= 5_000 {
        rgb((230, 110, 110))
    } else if micros >= 1_000 {
        rgb((230, 180, 80))
    } else {
        base
    }
}

fn stat_row<V>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: V) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(SECONDARY))))
        .id();
    let gap = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let v = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(TEXT_PRIMARY))))
        .id();
    bind_text(commands, v, value);
    commands.entity(row).add_children(&[l, gap, v]);
    row
}

fn build_scripting(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    // Header totals.
    let tot_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let tot = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 24.0), TextColor(rgb(TEXT_PRIMARY))))
        .id();
    bind_text(commands, tot, |w| format_duration(scr(w, |s| s.totals.total_last_update)));
    let tot_u = commands
        .spawn((
            Text::new("last frame"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(tot_row).add_children(&[tot, tot_u]);

    let summary = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(SECONDARY))))
        .id();
    bind_text(commands, summary, |w| {
        scr(w, |s| {
            format!(
                "avg {} \u{b7} {} script{} \u{b7} {} calls",
                format_duration(s.totals.total_avg_update),
                s.totals.script_count,
                if s.totals.script_count == 1 { "" } else { "s" },
                s.totals.total_calls,
            )
        })
    });

    let errors = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(ERR_RED))))
        .id();
    bind_text(commands, errors, |w| {
        scr(w, |s| {
            format!(
                "\u{26a0} {} script{} threw errors ({} total)",
                s.totals.scripts_with_errors,
                if s.totals.scripts_with_errors == 1 { "" } else { "s" },
                s.totals.total_errors,
            )
        })
    });
    bind_display(commands, errors, |w| scr(w, |s| s.totals.scripts_with_errors > 0));

    let e_ents = stat_row(commands, fonts, "Entities w/ ScriptComponent", |w| {
        scr(w, |s| s.entities_with_script).to_string()
    });
    let e_att = stat_row(commands, fonts, "Total attachments", |w| {
        scr(w, |s| s.total_script_attachments).to_string()
    });
    let e_back = stat_row(commands, fonts, "Backends registered", |w| {
        scr(w, |s| s.backend_count).to_string()
    });
    let e_folder = stat_row(commands, fonts, "Scripts folder", |w| {
        scr(w, |s| s.scripts_folder.clone()).unwrap_or_else(|| "<unset>".to_string())
    });

    // Table.
    let table_label = section(commands, fonts, "Per-script timing");
    let header = column_header(commands, fonts);
    bind_display(commands, header, |w| !scr(w, |s| s.per_script.is_empty()));
    let body = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, body, table_snapshot);

    commands.entity(root).add_children(&[
        tot_row, summary, errors, e_ents, e_att, e_back, e_folder, table_label, header, body,
    ]);
    root
}

const COLS: [(&str, f32); 5] = [
    ("Script", 220.0),
    ("Last", 60.0),
    ("Avg", 60.0),
    ("Max", 60.0),
    ("Calls", 60.0),
];

fn column_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            padding: UiRect::left(Val::Px(4.0)),
            ..default()
        })
        .id();
    let cells: Vec<Entity> = COLS
        .iter()
        .map(|(label, width)| cell(commands, fonts, label, *width, rgb(TEXT_MUTED), 10.0, false))
        .collect();
    commands.entity(row).add_children(&cells);
    let sep = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb((60, 60, 72))),
        ))
        .id();
    commands.entity(col).add_children(&[row, sep]);
    col
}

fn cell(commands: &mut Commands, fonts: &EmberFonts, text: &str, width: f32, color: Color, size: f32, mono: bool) -> Entity {
    let font = if mono { &fonts.mono } else { &fonts.ui };
    commands
        .spawn((
            Text::new(text),
            ui_font(font, size),
            TextColor(color),
            Node {
                width: Val::Px(width),
                ..default()
            },
        ))
        .id()
}

// ── Table rows ───────────────────────────────────────────────────────────────

struct ScriptRow {
    name: String,
    last: String,
    avg: String,
    max: String,
    calls: String,
    name_color: Color,
    last_color: Color,
    value_color: Color,
    hooks: Option<String>,
    error: Option<String>,
}

fn make_row(path: &Path, perf: &ScriptPerf, current_frame: u64) -> ScriptRow {
    let stale = current_frame.saturating_sub(perf.last_seen_frame) > 2;
    let name_color = if stale { rgb(TEXT_MUTED) } else { rgb(TEXT_PRIMARY) };
    let value_color = if stale { rgb(TEXT_MUTED) } else { rgb(SECONDARY) };
    let name_color = if perf.error_count > 0 {
        rgb((230, 130, 110))
    } else {
        name_color
    };
    let display = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    let mut bits: Vec<String> = Vec::new();
    if perf.last_on_ready > Duration::ZERO {
        bits.push(format!("ready: {}", format_duration(perf.last_on_ready)));
    }
    if perf.last_on_rpc > Duration::ZERO {
        bits.push(format!("rpc: {}", format_duration(perf.last_on_rpc)));
    }
    if perf.last_on_ui > Duration::ZERO {
        bits.push(format!("ui: {}", format_duration(perf.last_on_ui)));
    }
    let hooks = if bits.is_empty() {
        None
    } else {
        Some(bits.join("  \u{b7}  "))
    };
    let error = perf
        .last_error
        .as_ref()
        .map(|e| format!("\u{2717} {}", truncate(e, 200)));

    ScriptRow {
        name: display,
        last: format_duration(perf.last_on_update),
        avg: format_duration(perf.avg_on_update),
        max: format_duration(perf.max_on_update),
        calls: format_count(perf.on_update_calls),
        name_color,
        last_color: duration_color(perf.last_on_update, value_color),
        value_color,
        hooks,
        error,
    }
}

fn table_snapshot(world: &World) -> KeyedSnapshot {
    let scripts = scr(world, |s| s.per_script.clone());
    let frame = scr(world, |s| s.current_frame);
    if scripts.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new("(no scripts have ticked yet)"),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(TEXT_MUTED)),
                ))
                .id()
            }),
        };
    }
    let rows: Vec<ScriptRow> = scripts.iter().map(|(p, perf)| make_row(p, perf, frame)).collect();
    let items: Vec<(u64, u64)> = scripts
        .iter()
        .map(|(p, perf)| {
            let mut hk = std::collections::hash_map::DefaultHasher::new();
            p.hash(&mut hk);
            let key = hk.finish();
            let mut hh = std::collections::hash_map::DefaultHasher::new();
            (
                perf.last_on_update,
                perf.avg_on_update,
                perf.max_on_update,
                perf.on_update_calls,
                perf.error_count,
                perf.last_error.is_some(),
                perf.last_seen_frame == frame,
            )
                .hash(&mut hh);
            (key, hh.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| script_row(c, f, &rows[i])),
    }
}

fn script_row(commands: &mut Commands, fonts: &EmberFonts, r: &ScriptRow) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(1.0),
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        })
        .id();
    let main = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            padding: UiRect::left(Val::Px(4.0)),
            ..default()
        })
        .id();
    let c0 = cell(commands, fonts, &r.name, 220.0, r.name_color, 11.0, true);
    let c1 = cell(commands, fonts, &r.last, 60.0, r.last_color, 11.0, true);
    let c2 = cell(commands, fonts, &r.avg, 60.0, r.value_color, 11.0, true);
    let c3 = cell(commands, fonts, &r.max, 60.0, r.value_color, 11.0, true);
    let c4 = cell(commands, fonts, &r.calls, 60.0, r.value_color, 11.0, true);
    commands.entity(main).add_children(&[c0, c1, c2, c3, c4]);
    let mut kids = vec![main];

    if let Some(hooks) = &r.hooks {
        kids.push(
            commands
                .spawn((
                    Text::new(hooks.clone()),
                    ui_font(&fonts.ui, 10.0),
                    TextColor(rgb(TEXT_MUTED)),
                    Node {
                        margin: UiRect::left(Val::Px(18.0)),
                        ..default()
                    },
                ))
                .id(),
        );
    }
    if let Some(err) = &r.error {
        kids.push(
            commands
                .spawn((
                    Text::new(err.clone()),
                    ui_font(&fonts.mono, 10.0),
                    TextColor(rgb((220, 80, 80))),
                    Node {
                        margin: UiRect::left(Val::Px(18.0)),
                        ..default()
                    },
                ))
                .id(),
        );
    }
    commands.entity(col).add_children(&kids);
    col
}

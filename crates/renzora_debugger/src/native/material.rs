//! Bevy-native Material Resolver panel — cache counts + per-material compile
//! timing table + recent failures. Reads `MaterialCache` + `MaterialPerfStats`;
//! gated on both being present.

use std::hash::{Hash, Hasher};
use std::time::Duration;

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_shader::material::perf::{MaterialPerf, MaterialPerfStats, MAX_RECENT_FAILURES};
use renzora_shader::material::resolver::MaterialCache;

use super::{root, section};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const ERR_RED: (u8, u8, u8) = (230, 110, 110);

pub(super) fn register_material_resolver(app: &mut App) {
    app.register_panel_content("material_resolver_diag", true, build_material_resolver);
}

fn present(w: &World) -> bool {
    w.get_resource::<MaterialCache>().is_some() && w.get_resource::<MaterialPerfStats>().is_some()
}
fn cache<R: Default>(w: &World, f: impl FnOnce(&MaterialCache) -> R) -> R {
    w.get_resource::<MaterialCache>().map(f).unwrap_or_default()
}
fn mperf<R: Default>(w: &World, f: impl FnOnce(&MaterialPerfStats) -> R) -> R {
    w.get_resource::<MaterialPerfStats>().map(f).unwrap_or_default()
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
    let ms = d.as_micros() as f64 / 1_000.0;
    if ms >= 100.0 {
        rgb((230, 110, 110))
    } else if ms >= 20.0 {
        rgb((230, 180, 80))
    } else {
        base
    }
}
fn short_path(path: &str) -> String {
    let p = std::path::Path::new(path);
    if let (Some(parent), Some(name)) = (p.parent().and_then(|p| p.file_name()), p.file_name()) {
        format!("{}/{}", parent.to_string_lossy(), name.to_string_lossy())
    } else {
        path.to_string()
    }
}

fn cell(commands: &mut Commands, fonts: &EmberFonts, text: &str, width: f32, color: Color) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.mono, 11.0),
            TextColor(color),
            // Single line, clipped to the column so long names don't bleed over.
            bevy::text::TextLayout::new_with_no_wrap(),
            Node {
                width: Val::Px(width),
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id()
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
        .spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, v, value);
    commands.entity(row).add_children(&[l, gap, v]);
    row
}

fn build_material_resolver(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    let absent = commands
        .spawn((
            Text::new("(MaterialCache / MaterialPerfStats resource not present)"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_display(commands, absent, |w| !present(w));

    let content = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, content, present);

    // Header.
    let tot_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let tot = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 28.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, tot, |w| {
        cache(w, |c| (c.standard_count() + c.graph_count() + c.code_count()).to_string())
    });
    let tot_u = commands
        .spawn((
            Text::new("materials cached"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(tot_row).add_children(&[tot, tot_u]);

    let breakdown = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(SECONDARY))))
        .id();
    bind_text(commands, breakdown, |w| {
        cache(w, |c| {
            format!(
                "{} std \u{b7} {} graph \u{b7} {} code  \u{b7}  {} master-meta entries",
                c.standard_count(),
                c.graph_count(),
                c.code_count(),
                c.master_meta_count(),
            )
        })
    });

    let hits = stat_row(commands, fonts, "Cache hits (lifetime)", |w| {
        format_count(mperf(w, |p| p.total_cache_hits))
    });
    let compiles = stat_row(commands, fonts, "Compiles ran", |w| {
        format_count(mperf(w, |p| p.total_compiles))
    });
    let ctime = stat_row(commands, fonts, "Total compile time", |w| {
        format_duration(mperf(w, |p| p.total_compile_time))
    });
    let fails = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(ERR_RED))))
        .id();
    bind_text(commands, fails, |w| {
        mperf(w, |p| {
            format!(
                "\u{26a0} {} compile failure{}",
                p.total_failures,
                if p.total_failures == 1 { "" } else { "s" }
            )
        })
    });
    bind_display(commands, fails, |w| mperf(w, |p| p.total_failures) > 0);

    // Perf table.
    let table_label = section(commands, fonts, "Per-material compile timing (sorted by last compile)");
    let header = column_header(commands, fonts);
    bind_display(commands, header, |w| !mperf(w, |p| p.per_path.is_empty()));
    let body = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, body, table_snapshot);

    // Failures.
    let fail_label = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(8.0)), ..default() }))
        .id();
    bind_text(commands, fail_label, |w| {
        mperf(w, |p| format!("Recent failures ({} of {} kept)", p.recent_failures.len(), MAX_RECENT_FAILURES))
    });
    bind_display(commands, fail_label, |w| !mperf(w, |p| p.recent_failures.is_empty()));
    let fail_body = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, fail_body, failures_snapshot);

    commands.entity(content).add_children(&[
        tot_row, breakdown, hits, compiles, ctime, fails, table_label, header, body, fail_label,
        fail_body,
    ]);
    commands.entity(root).add_children(&[absent, content]);
    root
}

const COLS: [(&str, f32); 6] = [
    ("Material", 200.0),
    ("Kind", 40.0),
    ("Last", 60.0),
    ("Max", 60.0),
    ("Compiles", 60.0),
    ("Hits", 50.0),
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
        .map(|(label, width)| cell(commands, fonts, label, *width, rgb(text_muted())))
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

// ── Table rows ───────────────────────────────────────────────────────────────

struct MatRow {
    name: String,
    kind: String,
    last: String,
    max: String,
    compiles: String,
    hits: String,
    name_color: Color,
    last_color: Color,
    error: Option<String>,
}

fn make_row(path: &str, p: &MaterialPerf) -> MatRow {
    let name_color = if p.fail_count > 0 {
        rgb((230, 130, 110))
    } else {
        rgb(text_primary())
    };
    MatRow {
        name: short_path(path),
        kind: p.kind.label().to_string(),
        last: format_duration(p.last_compile),
        max: format_duration(p.max_compile),
        compiles: format_count(p.compile_count),
        hits: format_count(p.cache_hits),
        name_color,
        last_color: duration_color(p.last_compile, rgb(SECONDARY)),
        error: p.last_error.as_ref().map(|e| format!("\u{2717} {}", truncate(e, 200))),
    }
}

fn table_snapshot(world: &World) -> KeyedSnapshot {
    let snap = mperf(world, |p| p.snapshot());
    if snap.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new("(no materials resolved yet)"),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                ))
                .id()
            }),
        };
    }
    let rows: Vec<MatRow> = snap.iter().map(|(path, p)| make_row(path, p)).collect();
    let items: Vec<(u64, u64)> = snap
        .iter()
        .map(|(path, p)| {
            let mut hk = std::collections::hash_map::DefaultHasher::new();
            path.hash(&mut hk);
            let mut hh = std::collections::hash_map::DefaultHasher::new();
            (
                p.last_compile,
                p.max_compile,
                p.compile_count,
                p.cache_hits,
                p.fail_count,
                p.last_error.is_some(),
            )
                .hash(&mut hh);
            (hk.finish(), hh.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| mat_row(c, f, &rows[i])),
    }
}

fn mat_row(commands: &mut Commands, fonts: &EmberFonts, r: &MatRow) -> Entity {
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
    let c0 = cell(commands, fonts, &r.name, 200.0, r.name_color);
    let c1 = cell(commands, fonts, &r.kind, 40.0, rgb(SECONDARY));
    let c2 = cell(commands, fonts, &r.last, 60.0, r.last_color);
    let c3 = cell(commands, fonts, &r.max, 60.0, rgb(SECONDARY));
    let c4 = cell(commands, fonts, &r.compiles, 60.0, rgb(SECONDARY));
    let c5 = cell(commands, fonts, &r.hits, 50.0, rgb(SECONDARY));
    commands.entity(main).add_children(&[c0, c1, c2, c3, c4, c5]);
    let mut kids = vec![main];
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

fn failures_snapshot(world: &World) -> KeyedSnapshot {
    let fails: Vec<(String, String)> = mperf(world, |p| p.recent_failures.iter().cloned().collect());
    let items: Vec<(u64, u64)> = fails
        .iter()
        .map(|(path, err)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (path, err).hash(&mut h);
            (h.finish(), 0)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| failure_row(c, f, &fails[i].0, &fails[i].1)),
    }
}

fn failure_row(commands: &mut Commands, fonts: &EmberFonts, path: &str, err: &str) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        })
        .id();
    let name = commands
        .spawn((
            Text::new(short_path(path)),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb((220, 80, 80))),
            Node {
                padding: UiRect::left(Val::Px(4.0)),
                ..default()
            },
        ))
        .id();
    let msg = commands
        .spawn((
            Text::new(truncate(err, 200)),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(SECONDARY)),
            Node {
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(col).add_children(&[name, msg]);
    col
}

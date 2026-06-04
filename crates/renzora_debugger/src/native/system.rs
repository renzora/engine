//! Bevy-native System Profiler panel — frame-time + FPS/render grids, an
//! estimated schedule breakdown (reactive `keyed_list` of proportion bars), a
//! limitations note, and static external-profiler info.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{rgb, section_bg, text_muted, text_primary, window_bg};
use renzora_ember::widgets::{line_chart_live, ChartStyle};

use crate::state::{DiagnosticsState, RenderStats, SystemTimingState};

use super::{root, section};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const FAINT_BG: (u8, u8, u8) = (30, 30, 36);

pub(super) fn register_system_profiler(app: &mut App) {
    app.register_panel_content("system_profiler", true, build_system_profiler);
}

fn diag<R: Default>(w: &World, f: impl FnOnce(&DiagnosticsState) -> R) -> R {
    w.get_resource::<DiagnosticsState>().map(f).unwrap_or_default()
}
fn rstats<R: Default>(w: &World, f: impl FnOnce(&RenderStats) -> R) -> R {
    w.get_resource::<RenderStats>().map(f).unwrap_or_default()
}
fn timing<R: Default>(w: &World, f: impl FnOnce(&SystemTimingState) -> R) -> R {
    w.get_resource::<SystemTimingState>().map(f).unwrap_or_default()
}

fn frame_time_color(ms: f32) -> Color {
    if ms <= 16.67 {
        rgb((100, 200, 100))
    } else if ms <= 33.33 {
        rgb((200, 200, 100))
    } else {
        rgb((200, 100, 100))
    }
}
fn fps_color(fps: f32) -> Color {
    if fps >= 60.0 {
        rgb((100, 200, 100))
    } else if fps >= 30.0 {
        rgb((200, 200, 100))
    } else {
        rgb((200, 100, 100))
    }
}
fn schedule_color(name: &str) -> Color {
    match name {
        "PreUpdate" => rgb((100, 180, 220)),
        "Update" => rgb((140, 200, 140)),
        "PostUpdate" => rgb((200, 160, 100)),
        "Render" => rgb((180, 140, 200)),
        _ => rgb((150, 150, 160)),
    }
}
fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

fn frame_status(ms: f32) -> (String, Color) {
    if ms <= 16.67 {
        (
            format!("\u{2713} Under 60fps target ({:.1}ms budget)", 16.67 - ms),
            rgb((100, 200, 100)),
        )
    } else if ms <= 33.33 {
        (
            format!("\u{26a0} Between 30-60fps ({:.1}ms over 60fps target)", ms - 16.67),
            rgb((200, 180, 80)),
        )
    } else {
        (
            format!("\u{2717} Below 30fps ({:.1}ms over target)", ms - 33.33),
            rgb((200, 100, 100)),
        )
    }
}

// ── Small builders ───────────────────────────────────────────────────────────

fn big<V, C>(commands: &mut Commands, fonts: &EmberFonts, unit: &str, value: V, color: C) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
    C: Fn(&World) -> Color + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let num = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 24.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, num, value);
    bind_text_color(commands, num, color);
    let u = commands
        .spawn((
            Text::new(unit),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_children(&[num, u]);
    row
}

fn faint_box(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(2.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(FAINT_BG)),
        ))
        .id()
}

/// A `label   value` grid row (value is a binding, mono).
fn grid_row<V>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: V) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Px(80.0),
                ..default()
            },
        ))
        .id();
    let v = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 10.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, v, value);
    commands.entity(row).add_children(&[l, v]);
    row
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build_system_profiler(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    // Frame time.
    let ft_label = section(commands, fonts, "Frame Time");
    let ft_big = big(
        commands,
        fonts,
        "ms / frame",
        |w| format!("{:.2}", diag(w, |d| d.frame_time_ms) as f32),
        |w| frame_time_color(diag(w, |d| d.frame_time_ms) as f32),
    );
    let ft_status = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, ft_status, |w| frame_status(diag(w, |d| d.frame_time_ms) as f32).0);
    bind_text_color(commands, ft_status, |w| frame_status(diag(w, |d| d.frame_time_ms) as f32).1);
    let ft_chart = line_chart_live(
        commands,
        ChartStyle {
            color: rgb((100, 180, 220)),
            min: Some(0.0),
            max: None,
            target: None,
            height: 40.0,
        },
        |w| diag(w, |d| d.frame_time_history.iter().copied().collect()),
    );

    // FPS statistics.
    let fps_label = section(commands, fonts, "FPS Statistics");
    let fps_big = big(
        commands,
        fonts,
        "fps",
        |w| format!("{:.0}", diag(w, |d| d.fps) as f32),
        |w| fps_color(diag(w, |d| d.fps) as f32),
    );
    let fps_grid = faint_box(commands);
    let avg = grid_row(commands, fonts, "Avg", |w| format!("{:.0}", diag(w, |d| d.avg_fps())));
    let min = grid_row(commands, fonts, "Min", |w| format!("{:.0}", diag(w, |d| d.min_fps())));
    let max = grid_row(commands, fonts, "Max", |w| format!("{:.0}", diag(w, |d| d.max_fps())));
    let ents = grid_row(commands, fonts, "Entities", |w| format!("{}", diag(w, |d| d.entity_count)));
    commands.entity(fps_grid).add_children(&[avg, min, max, ents]);

    // Render stats (hidden until enabled).
    let rs_section = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, rs_section, |w| rstats(w, |r| r.enabled));
    let rs_label = section(commands, fonts, "Render Stats");
    let rs_grid = faint_box(commands);
    let dc = grid_row(commands, fonts, "Draw Calls", |w| format!("{}", rstats(w, |r| r.draw_calls)));
    let tr = grid_row(commands, fonts, "Triangles", |w| format_count(rstats(w, |r| r.triangles)));
    let vx = grid_row(commands, fonts, "Vertices", |w| format_count(rstats(w, |r| r.vertices)));
    let gp = grid_row(commands, fonts, "GPU Time", |w| format!("{:.2}ms", rstats(w, |r| r.gpu_time_ms)));
    commands.entity(rs_grid).add_children(&[dc, tr, vx, gp]);
    commands.entity(rs_section).add_children(&[rs_label, rs_grid]);

    // Schedule breakdown.
    let sched_label = section(commands, fonts, "Schedule Overview (Estimated)");
    let sched_box = faint_box(commands);
    keyed_list(commands, sched_box, schedule_snapshot);
    let sched_note = commands
        .spawn((
            Text::new("Note: These are rough estimates, not actual measurements"),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_display(commands, sched_note, |w| !timing(w, |t| t.schedule_timings.is_empty()));

    // Limitations box.
    let lim = limitations_box(commands, fonts);

    // External profilers (static).
    let ext_label = section(commands, fonts, "External Profilers");
    let ext = external_box(commands, fonts);

    commands.entity(root).add_children(&[
        ft_label, ft_big, ft_status, ft_chart, fps_label, fps_big, fps_grid, rs_section,
        sched_label, sched_box, sched_note, lim, ext_label, ext,
    ]);
    root
}

fn limitations_box(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bx = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
        ))
        .id();
    let head = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let warn = icon_text(commands, &fonts.phosphor, "warning", (220, 180, 80), 14.0);
    let title = commands
        .spawn((
            Text::new("Limitations"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb((220, 180, 80))),
        ))
        .id();
    commands.entity(head).add_children(&[warn, title]);
    let note = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(SECONDARY))))
        .id();
    bind_text(commands, note, |w| timing(w, |t| t.limitation_note.clone()));
    commands.entity(bx).add_children(&[head, note]);
    bx
}

fn external_box(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bx = faint_box(commands);
    let mut kids: Vec<Entity> = Vec::new();

    let tracy_head = labelled_icon(commands, fonts, "magnifying-glass", "Tracy Profiler");
    kids.push(tracy_head);
    kids.push(muted(commands, fonts, "Best for per-system timing and GPU profiling", 9.0, text_muted()));
    kids.push(muted(commands, fonts, "Launch Tracy GUI 0.11.x and connect to localhost", 9.0, (180, 180, 180)));

    kids.push(spacer(commands, 8.0));

    let chrome_head = labelled_icon(commands, fonts, "chart-bar", "Chrome Tracing");
    kids.push(chrome_head);
    kids.push(muted(commands, fonts, "Export traces to chrome://tracing", 9.0, text_muted()));
    let cmd = commands
        .spawn((
            Text::new("cargo run --features bevy/trace_chrome"),
            ui_font(&fonts.mono, 9.0),
            TextColor(rgb((100, 100, 100))),
        ))
        .id();
    kids.push(cmd);

    commands.entity(bx).add_children(&kids);
    bx
}

fn labelled_icon(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    let tx = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(row).add_children(&[ic, tx]);
    row
}

fn muted(commands: &mut Commands, fonts: &EmberFonts, text: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(&fonts.ui, size), TextColor(rgb(color))))
        .id()
}

fn spacer(commands: &mut Commands, h: f32) -> Entity {
    commands
        .spawn(Node {
            height: Val::Px(h),
            ..default()
        })
        .id()
}

// ── Schedule list ────────────────────────────────────────────────────────────

struct SchedRow {
    name: String,
    pct: String,
    ms: String,
    ratio: f32,
    color: Color,
}

fn schedule_snapshot(world: &World) -> KeyedSnapshot {
    let timings = timing(world, |t| t.schedule_timings.clone());
    if timings.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| muted(c, f, "No timing data available", 11.0, text_muted())),
        };
    }
    let total: f32 = timings.iter().map(|s| s.time_ms).sum();
    let rows: Vec<SchedRow> = timings
        .iter()
        .map(|s| SchedRow {
            name: s.name.clone(),
            pct: format!("{:.0}%", s.percentage),
            ms: format!("{:.2}ms", s.time_ms),
            ratio: if total > 0.0 { s.time_ms / total } else { 0.0 },
            color: schedule_color(&s.name),
        })
        .collect();
    let items: Vec<(u64, u64)> = timings
        .iter()
        .map(|s| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            s.name.hash(&mut h);
            (h.finish(), s.time_ms.to_bits() as u64 ^ s.percentage.to_bits() as u64)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| sched_row(c, f, &rows[i])),
    }
}

fn sched_row(commands: &mut Commands, fonts: &EmberFonts, r: &SchedRow) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(2.0),
            margin: UiRect::bottom(Val::Px(6.0)),
            ..default()
        })
        .id();
    // Header: name … pct ms
    let head = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let name = commands
        .spawn((Text::new(r.name.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    let gap = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let pct = commands
        .spawn((Text::new(r.pct.clone()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id();
    let ms = commands
        .spawn((Text::new(r.ms.clone()), ui_font(&fonts.mono, 10.0), TextColor(rgb(SECONDARY))))
        .id();
    commands.entity(head).add_children(&[name, gap, pct, ms]);
    // Proportion bar.
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(8.0),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(r.ratio.clamp(0.0, 1.0) * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(r.color),
        ))
        .id();
    commands.entity(track).add_child(fill);
    commands.entity(col).add_children(&[head, track]);
    col
}

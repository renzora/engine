//! Bevy-native (ember) debug panels — faithful bevy_ui ports of the egui
//! diagnostic panels. They read the same world resources the egui panels did
//! (kept current by the crate's `update_*` systems); every value is a
//! value-diffed binding and every graph is a reactive [`line_chart_live`], so an
//! idle panel costs nothing.

mod ecs;
mod memory;
mod system;

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_text_color};
use renzora_ember::theme::{rgb, TEXT_MUTED, TEXT_PRIMARY};
use renzora_ember::widgets::{line_chart_live, ChartStyle};

use crate::state::{DiagnosticsState, RenderStats};

/// Register the native debug panels.
pub fn register_native_debug(app: &mut App) {
    app.register_panel_content("render_stats", true, build_render_stats);
    app.register_panel_content("performance", true, build_performance);
    ecs::register_ecs_stats(app);
    memory::register_memory(app);
    system::register_system_profiler(app);
}

// ── Shared builders ─────────────────────────────────────────────────────────

pub(super) fn root(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::all(Val::Px(8.0)),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id()
}

pub(super) fn column(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(gap),
            ..default()
        })
        .id()
}

pub(super) fn section(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
        ))
        .id()
}

/// A big colored value + a muted unit suffix (e.g. `28  FPS`).
pub(super) fn big_stat<T, C>(commands: &mut Commands, fonts: &EmberFonts, unit: &str, value: T, color: C) -> Entity
where
    T: Fn(&World) -> String + Send + Sync + 'static,
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
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 28.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    bind_text(commands, num, value);
    bind_text_color(commands, num, color);
    let unit = commands
        .spawn((
            Text::new(unit),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_children(&[num, unit]);
    row
}

/// A `label   value` row (value is a binding).
pub(super) fn label_row<T>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: T) -> Entity
where
    T: Fn(&World) -> String + Send + Sync + 'static,
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
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                width: Val::Px(90.0),
                ..default()
            },
        ))
        .id();
    let v = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    bind_text(commands, v, value);
    commands.entity(row).add_children(&[l, v]);
    row
}

/// A small inline stat (e.g. `Avg: 60`).
fn small_stat<T>(commands: &mut Commands, fonts: &EmberFonts, value: T) -> Entity
where
    T: Fn(&World) -> String + Send + Sync + 'static,
{
    let t = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id();
    bind_text(commands, t, value);
    t
}

// ── Formatting / color helpers (ported from the egui panels) ─────────────────

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
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

fn frame_time_color(ms: f32) -> Color {
    if ms <= 16.67 {
        rgb((100, 200, 100))
    } else if ms <= 33.33 {
        rgb((200, 200, 100))
    } else {
        rgb((200, 100, 100))
    }
}

fn gpu_time_color(ms: f32) -> Color {
    if ms <= 8.0 {
        rgb((100, 200, 100))
    } else if ms <= 16.67 {
        rgb((200, 200, 100))
    } else {
        rgb((200, 100, 100))
    }
}

// Resource readers (avoid cloning whole histories for scalar lookups).
fn render_stats<R: Default>(w: &World, f: impl FnOnce(&RenderStats) -> R) -> R {
    w.get_resource::<RenderStats>().map(f).unwrap_or_default()
}
fn diag<R: Default>(w: &World, f: impl FnOnce(&DiagnosticsState) -> R) -> R {
    w.get_resource::<DiagnosticsState>().map(f).unwrap_or_default()
}

// ── Render Stats ─────────────────────────────────────────────────────────────

fn build_render_stats(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    // "Collecting…" placeholder until the first stats arrive.
    let collecting = commands
        .spawn((
            Text::new("Collecting render data..."),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                margin: UiRect::all(Val::Px(40.0)),
                ..default()
            },
        ))
        .id();
    bind_display(commands, collecting, |w| !render_stats(w, |r| r.enabled));

    let content = column(commands, 4.0);
    bind_display(commands, content, |w| render_stats(w, |r| r.enabled));

    let gpu_label = section(commands, fonts, "GPU Timing");
    let gpu_big = big_stat(
        commands,
        fonts,
        "ms GPU",
        |w| format!("{:.2}", render_stats(w, |r| r.gpu_time_ms)),
        |w| gpu_time_color(render_stats(w, |r| r.gpu_time_ms)),
    );
    let gpu_chart = line_chart_live(
        commands,
        ChartStyle {
            color: rgb((150, 100, 200)),
            min: Some(0.0),
            max: Some(20.0),
            target: Some(16.67),
            height: 40.0,
        },
        |w| render_stats(w, |r| r.gpu_time_history.clone()),
    );

    let pipe_label = section(commands, fonts, "Pipeline Statistics");
    let draws = label_row(commands, fonts, "Draw Calls", |w| {
        format_number(render_stats(w, |r| r.draw_calls))
    });
    let tris = label_row(commands, fonts, "Triangles", |w| {
        format_number(render_stats(w, |r| r.triangles))
    });
    let verts = label_row(commands, fonts, "Vertices", |w| {
        format_number(render_stats(w, |r| r.vertices))
    });

    commands.entity(content).add_children(&[
        gpu_label, gpu_big, gpu_chart, pipe_label, draws, tris, verts,
    ]);
    commands.entity(root).add_children(&[collecting, content]);
    root
}

// ── Performance ──────────────────────────────────────────────────────────────

fn build_performance(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    // FPS.
    let fps_label = section(commands, fonts, "Frames Per Second");
    let fps_big = big_stat(
        commands,
        fonts,
        "FPS",
        |w| format!("{:.0}", diag(w, |d| d.fps) as f32),
        |w| fps_color(diag(w, |d| d.fps) as f32),
    );
    let avg_min_max = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .id();
    let avg = small_stat(commands, fonts, |w| format!("Avg: {:.0}", diag(w, |d| d.avg_fps())));
    let min = small_stat(commands, fonts, |w| format!("Min: {:.0}", diag(w, |d| d.min_fps())));
    let max = small_stat(commands, fonts, |w| format!("Max: {:.0}", diag(w, |d| d.max_fps())));
    commands.entity(avg_min_max).add_children(&[avg, min, max]);
    let fps_chart = line_chart_live(
        commands,
        ChartStyle {
            color: rgb((100, 200, 100)),
            min: Some(0.0),
            max: Some(120.0),
            target: Some(60.0),
            height: 50.0,
        },
        |w| diag(w, |d| d.fps_history.iter().copied().collect()),
    );

    // Frame time.
    let ft_label = section(commands, fonts, "Frame Time");
    let ft_big = big_stat(
        commands,
        fonts,
        "ms",
        |w| format!("{:.2}", diag(w, |d| d.frame_time_ms) as f32),
        |w| frame_time_color(diag(w, |d| d.frame_time_ms) as f32),
    );
    let ft_chart = line_chart_live(
        commands,
        ChartStyle {
            color: rgb((100, 150, 200)),
            min: Some(0.0),
            max: Some(33.33),
            target: Some(16.67),
            height: 50.0,
        },
        |w| diag(w, |d| d.frame_time_history.iter().copied().collect()),
    );

    // Entities.
    let ent_label = section(commands, fonts, "Entities");
    let ent_big = big_stat(
        commands,
        fonts,
        "entities",
        |w| format!("{}", diag(w, |d| d.entity_count)),
        |_| rgb(TEXT_PRIMARY),
    );
    let ent_chart = line_chart_live(
        commands,
        ChartStyle {
            color: rgb((200, 150, 100)),
            min: Some(0.0),
            max: None,
            target: None,
            height: 50.0,
        },
        |w| diag(w, |d| d.entity_count_history.iter().copied().collect()),
    );

    // System (CPU / memory) — only shown when the platform reports them.
    let sys_label = section(commands, fonts, "System");
    bind_display(commands, sys_label, |w| {
        diag(w, |d| d.cpu_usage.is_some() || d.memory_usage.is_some())
    });
    let cpu = label_row(commands, fonts, "CPU:", |w| {
        format!("{:.1}%", diag(w, |d| d.cpu_usage).unwrap_or(0.0))
    });
    bind_display(commands, cpu, |w| diag(w, |d| d.cpu_usage.is_some()));
    let mem = label_row(commands, fonts, "Memory:", |w| {
        format_bytes(diag(w, |d| d.memory_usage).unwrap_or(0))
    });
    bind_display(commands, mem, |w| diag(w, |d| d.memory_usage.is_some()));

    commands.entity(root).add_children(&[
        fps_label, fps_big, avg_min_max, fps_chart, ft_label, ft_big, ft_chart, ent_label,
        ent_big, ent_chart, sys_label, cpu, mem,
    ]);
    root
}

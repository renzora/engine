//! Bevy-native Memory Profiler panel — process memory + history graph, trend,
//! estimated asset memory (horizontal bars), and allocation rate. All values are
//! value-diffed bindings; the graph is a reactive `line_chart_live`.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_text_color, bind_with};
use renzora_ember::theme::{rgb, TEXT_MUTED, TEXT_PRIMARY};
use renzora_ember::widgets::{line_chart_live, ChartStyle};

use crate::state::{MemoryProfilerState, MemoryTrend};

use super::{column, label_row, root, section};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const TRACK_BG: (u8, u8, u8) = (18, 18, 24);

pub(super) fn register_memory(app: &mut App) {
    app.register_panel_content("memory_profiler", true, build_memory);
}

fn mem<R: Default>(w: &World, f: impl FnOnce(&MemoryProfilerState) -> R) -> R {
    w.get_resource::<MemoryProfilerState>().map(f).unwrap_or_default()
}

fn format_memory(bytes: u64) -> String {
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

fn memory_color(mb: f64) -> Color {
    if mb < 512.0 {
        rgb((100, 200, 100))
    } else if mb < 1024.0 {
        rgb((200, 200, 100))
    } else if mb < 2048.0 {
        rgb((200, 150, 80))
    } else {
        rgb((200, 100, 100))
    }
}

fn trend_parts(t: MemoryTrend) -> (&'static str, Color) {
    match t {
        MemoryTrend::Increasing => ("\u{2191} Increasing", rgb((220, 100, 100))),
        MemoryTrend::Decreasing => ("\u{2193} Decreasing", rgb((100, 200, 100))),
        MemoryTrend::Stable => ("\u{2194} Stable", rgb((150, 150, 150))),
    }
}

fn alloc_parts(rate: f64) -> (String, Color) {
    let a = rate.abs();
    let (num, color) = if a < 1024.0 {
        (format!("{:.0} B/s", a), rgb((150, 150, 150)))
    } else if a < 1024.0 * 1024.0 {
        (
            format!("{:.1} KB/s", a / 1024.0),
            if rate > 0.0 {
                rgb((200, 180, 100))
            } else {
                rgb((100, 200, 100))
            },
        )
    } else {
        (
            format!("{:.2} MB/s", a / (1024.0 * 1024.0)),
            if rate > 0.0 {
                rgb((220, 100, 100))
            } else {
                rgb((100, 200, 100))
            },
        )
    };
    let prefix = if rate > 0.0 {
        "+"
    } else if rate < 0.0 {
        "-"
    } else {
        ""
    };
    (format!("{}{}", prefix, num), color)
}

fn build_memory(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    let unavailable = commands
        .spawn((
            Text::new("Memory profiling unavailable"),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                margin: UiRect::all(Val::Px(40.0)),
                ..default()
            },
        ))
        .id();
    bind_display(commands, unavailable, |w| !mem(w, |s| s.available));

    let content = column(commands, 4.0);
    bind_display(commands, content, |w| mem(w, |s| s.available));

    // Process memory.
    let proc_label = section(commands, fonts, "Process Memory");
    let proc_big = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 28.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    bind_text(commands, proc_big, |w| format_memory(mem(w, |s| s.process_memory)));
    bind_text_color(commands, proc_big, |w| {
        memory_color(mem(w, |s| s.process_memory) as f64 / (1024.0 * 1024.0))
    });
    let peak = label_row(commands, fonts, "Peak:", |w| format_memory(mem(w, |s| s.peak_memory)));
    let graph = line_chart_live(
        commands,
        ChartStyle {
            color: rgb((150, 100, 200)),
            min: None,
            max: None,
            target: None,
            height: 50.0,
        },
        |w| mem(w, |s| s.memory_history.iter().copied().collect()),
    );

    // Trend.
    let trend = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let trend_k = commands
        .spawn((
            Text::new("Trend:"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(SECONDARY)),
        ))
        .id();
    let trend_v = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    bind_text(commands, trend_v, |w| trend_parts(mem(w, |s| s.memory_trend)).0.to_string());
    bind_text_color(commands, trend_v, |w| trend_parts(mem(w, |s| s.memory_trend)).1);
    commands.entity(trend).add_children(&[trend_k, trend_v]);

    // Asset memory.
    let asset_label = section(commands, fonts, "Asset Memory (Estimated)");
    let total_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let total_v = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 20.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    bind_text(commands, total_v, |w| {
        format_memory(mem(w, |s| {
            s.asset_memory.meshes_bytes + s.asset_memory.textures_bytes + s.asset_memory.materials_bytes
        }))
    });
    let total_u = commands
        .spawn((
            Text::new("total"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                margin: UiRect::bottom(Val::Px(3.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(total_row).add_children(&[total_v, total_u]);

    let meshes = asset_bar(
        commands,
        fonts,
        (100, 180, 220),
        |w| format!("Meshes ({})", mem(w, |s| s.asset_memory.mesh_count)),
        |w| asset_ratio(w, |a| a.meshes_bytes),
        |w| format_memory(mem(w, |s| s.asset_memory.meshes_bytes)),
    );
    let textures = asset_bar(
        commands,
        fonts,
        (180, 140, 200),
        |w| format!("Textures ({})", mem(w, |s| s.asset_memory.texture_count)),
        |w| asset_ratio(w, |a| a.textures_bytes),
        |w| format_memory(mem(w, |s| s.asset_memory.textures_bytes)),
    );
    let materials = asset_bar(
        commands,
        fonts,
        (200, 160, 100),
        |w| format!("Materials ({})", mem(w, |s| s.asset_memory.material_count)),
        |w| asset_ratio(w, |a| a.materials_bytes),
        |w| format_memory(mem(w, |s| s.asset_memory.materials_bytes)),
    );

    // Allocation rate.
    let rate_label = section(commands, fonts, "Allocation Rate");
    let rate_v = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    bind_text(commands, rate_v, |w| alloc_parts(mem(w, |s| s.allocation_rate)).0);
    bind_text_color(commands, rate_v, |w| alloc_parts(mem(w, |s| s.allocation_rate)).1);
    let warn = commands
        .spawn((
            Text::new("\u{26a0} High allocation rate detected"),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb((220, 180, 80))),
        ))
        .id();
    bind_display(commands, warn, |w| {
        mem(w, |s| s.allocation_rate) > 10.0 * 1024.0 * 1024.0
    });

    commands.entity(content).add_children(&[
        proc_label, proc_big, peak, graph, trend, asset_label, total_row, meshes, textures,
        materials, rate_label, rate_v, warn,
    ]);
    commands.entity(root).add_children(&[unavailable, content]);
    root
}

/// Fill ratio (0..1) of one asset bucket against the largest bucket.
fn asset_ratio(w: &World, pick: impl Fn(&crate::state::AssetMemoryStats) -> u64) -> f32 {
    mem(w, |s| {
        let a = &s.asset_memory;
        let max = a.meshes_bytes.max(a.textures_bytes).max(a.materials_bytes).max(1);
        pick(a) as f32 / max as f32
    })
}

/// A labelled horizontal bar: `Name (count)` over a track whose fill width is a
/// live ratio binding, with the byte size to the right.
fn asset_bar<N, R, B>(commands: &mut Commands, fonts: &EmberFonts, color: (u8, u8, u8), name: N, ratio: R, bytes: B) -> Entity
where
    N: Fn(&World) -> String + Send + Sync + 'static,
    R: Fn(&World) -> f32 + Send + Sync + 'static,
    B: Fn(&World) -> String + Send + Sync + 'static,
{
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    let label = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(SECONDARY)),
        ))
        .id();
    bind_text(commands, label, name);

    let bar_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let track = commands
        .spawn((
            Node {
                width: Val::Px(120.0),
                height: Val::Px(12.0),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(TRACK_BG)),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(color)),
        ))
        .id();
    bind_with(commands, fill, ratio, |w, e, r: &f32| {
        if let Some(mut n) = w.get_mut::<Node>(e) {
            n.width = Val::Percent((r.clamp(0.0, 1.0)) * 100.0);
        }
    });
    commands.entity(track).add_child(fill);
    let size = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    bind_text(commands, size, bytes);
    commands.entity(bar_row).add_children(&[track, size]);

    commands.entity(col).add_children(&[label, bar_row]);
    col
}

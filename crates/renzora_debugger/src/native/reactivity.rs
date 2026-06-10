//! UI Reactivity panel — live profiler for ember's reactive layer.
//!
//! Surfaces what [`renzora_ember::reactive::ReactiveStats`] measures: how many
//! bindings/keyed lists exist, how much time recomputing them costs per frame,
//! how many produced new values, plus top-N tables for the most expensive
//! bindings ("cost"), the bindings whose values keep changing ("churn" — these
//! defeat value-diffing and force UI writes every frame), and every keyed
//! list's snapshot cost.
//!
//! The panel observes itself: its own bindings are registered in the same
//! registry, so opening it nudges the totals up — by design, not a bug.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, ReactiveStats};
use renzora_ember::theme::{rgb, text_muted, text_primary};
use renzora_ember::widgets::{line_chart_live, ChartStyle};

use super::camera::faint_box;
use super::{big_stat, label_row, section};

pub(super) fn register(app: &mut App) {
    app.register_panel_content("ui_reactivity", true, build);
}

fn rs<R: Default>(w: &World, f: impl FnOnce(&ReactiveStats) -> R) -> R {
    w.get_resource::<ReactiveStats>().map(f).unwrap_or_default()
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = super::root(commands);

    // Headline: binding count + recompute cost.
    let big = big_stat(
        commands,
        fonts,
        "bindings",
        |w| rs(w, |s| s.bindings_total).to_string(),
        |_| rgb(text_primary()),
    );
    let cost_big = big_stat(
        commands,
        fonts,
        "ms/frame recompute",
        |w| rs(w, |s| format!("{:.2}", (s.reactions_us + s.lists_us) / 1000.0)),
        |w| {
            let ms = rs(w, |s| (s.reactions_us + s.lists_us) / 1000.0);
            if ms > 4.0 {
                rgb((230, 110, 110))
            } else if ms > 1.0 {
                rgb((230, 200, 110))
            } else {
                rgb((120, 210, 120))
            }
        },
    );
    let chart = line_chart_live(
        commands,
        ChartStyle {
            color: rgb((110, 170, 230)),
            min: Some(0.0),
            max: None,
            target: None,
            height: 40.0,
        },
        |w| rs(w, |s| s.history_us.iter().map(|us| us / 1000.0).collect()),
    );

    // Totals grid.
    let totals_label = section(commands, fonts, "This Frame");
    let totals = faint_box(commands);
    let rows = [
        label_row(commands, fonts, "Bindings run", |w| {
            rs(w, |s| s.bindings_total).to_string()
        }),
        label_row(commands, fonts, "New values", |w| {
            rs(w, |s| s.changed_this_frame).to_string()
        }),
        label_row(commands, fonts, "Changes/sec", |w| {
            rs(w, |s| format!("{:.0}", s.changes_per_sec))
        }),
        label_row(commands, fonts, "Bindings time", |w| {
            rs(w, |s| format!("{:.2} ms", s.reactions_us / 1000.0))
        }),
        label_row(commands, fonts, "Keyed lists", |w| {
            rs(w, |s| s.lists_total).to_string()
        }),
        label_row(commands, fonts, "Lists time", |w| {
            rs(w, |s| format!("{:.2} ms", s.lists_us / 1000.0))
        }),
        label_row(commands, fonts, "Rows rebuilt", |w| {
            rs(w, |s| s.rows_rebuilt_this_frame).to_string()
        }),
    ];
    commands.entity(totals).add_children(&rows);

    // Top-N tables (fixed rows bound by rank; hidden while the rank is empty).
    let cost_label = section(commands, fonts, "Top Cost (avg µs/frame)");
    let cost_box = faint_box(commands);
    let cost_rows: Vec<Entity> = (0..ReactiveStats::TOP_N)
        .map(|i| {
            binding_row(
                commands,
                fonts,
                move |w| rs(w, |s| s.top_cost.get(i).cloned()),
                |r| format!("{:.1}", r.cost_ema_us),
            )
        })
        .collect();
    commands.entity(cost_box).add_children(&cost_rows);

    let churn_label = section(commands, fonts, "Top Churn (new values/sec)");
    let churn_box = faint_box(commands);
    let churn_rows: Vec<Entity> = (0..ReactiveStats::TOP_N)
        .map(|i| {
            binding_row(
                commands,
                fonts,
                move |w| rs(w, |s| s.top_churn.get(i).cloned()),
                |r| format!("{:.0}/s", r.change_rate),
            )
        })
        .collect();
    commands.entity(churn_box).add_children(&churn_rows);

    let lists_label = section(commands, fonts, "Keyed Lists (snapshot µs/frame)");
    let lists_box = faint_box(commands);
    let list_rows: Vec<Entity> = (0..8)
        .map(|i| list_row(commands, fonts, i))
        .collect();
    commands.entity(lists_box).add_children(&list_rows);

    commands.entity(root).add_children(&[
        big,
        cost_big,
        chart,
        totals_label,
        totals,
        cost_label,
        cost_box,
        churn_label,
        churn_box,
        lists_label,
        lists_box,
    ]);
    root
}

/// One table row for a [`BindingReport`] rank: `label  kind  metric`.
/// `get` fetches the report at this row's rank (None hides the row);
/// `metric` formats the highlighted number for this table.
fn binding_row<G, M>(commands: &mut Commands, fonts: &EmberFonts, get: G, metric: M) -> Entity
where
    G: Fn(&World) -> Option<renzora_ember::reactive::BindingReport>
        + Send
        + Sync
        + Copy
        + 'static,
    M: Fn(&renzora_ember::reactive::BindingReport) -> String + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            width: Val::Percent(100.0),
            ..default()
        })
        .id();
    bind_display(commands, row, move |w| get(w).is_some());

    let label = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                overflow: Overflow::hidden(),
                ..default()
            },
        ))
        .id();
    bind_text(commands, label, move |w| {
        get(w).map(|r| r.label).unwrap_or_default()
    });

    let kind = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 9.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Px(44.0),
                ..default()
            },
        ))
        .id();
    bind_text(commands, kind, move |w| {
        get(w).map(|r| r.kind.to_string()).unwrap_or_default()
    });

    let value = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(text_primary())),
            Node {
                width: Val::Px(56.0),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
        ))
        .id();
    bind_text(commands, value, move |w| {
        get(w).map(|r| metric(&r)).unwrap_or_default()
    });

    commands.entity(row).add_children(&[label, kind, value]);
    row
}

/// One table row for a [`ListReport`] rank: `label  rows  µs  rebuilt`.
fn list_row(commands: &mut Commands, fonts: &EmberFonts, i: usize) -> Entity {
    let get = move |w: &World| rs(w, |s| s.list_reports.get(i).cloned());

    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            width: Val::Percent(100.0),
            ..default()
        })
        .id();
    bind_display(commands, row, move |w| get(w).is_some());

    let label = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                overflow: Overflow::hidden(),
                ..default()
            },
        ))
        .id();
    bind_text(commands, label, move |w| {
        get(w).map(|r| r.label).unwrap_or_default()
    });

    let rows = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 9.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, rows, move |w| {
        get(w).map(|r| format!("{} rows", r.rows)).unwrap_or_default()
    });

    let cost = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(text_primary())),
            Node {
                width: Val::Px(56.0),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
        ))
        .id();
    bind_text(commands, cost, move |w| {
        get(w).map(|r| format!("{:.1}", r.cost_ema_us)).unwrap_or_default()
    });

    let rebuilt = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 9.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Px(70.0),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
        ))
        .id();
    bind_text(commands, rebuilt, move |w| {
        get(w)
            .map(|r| format!("{} rebuilt", r.rows_rebuilt))
            .unwrap_or_default()
    });

    commands.entity(row).add_children(&[label, rows, cost, rebuilt]);
    row
}

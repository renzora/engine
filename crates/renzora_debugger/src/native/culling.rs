//! Bevy-native Culling Debug panel — read-only breakdown/distribution bars plus
//! a Settings box (enable toggle + max-distance / fade-start sliders) two-way
//! bound to `CullingDebugState`.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, bind_text, bind_with};
use renzora_ember::theme::{rgb, TEXT_MUTED, TEXT_PRIMARY};
use renzora_ember::widgets::{checkbox, slider};

use crate::state::CullingDebugState;

use super::{root, section};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const FAINT_BG: (u8, u8, u8) = (30, 30, 36);
const TRACK_BG: (u8, u8, u8) = (18, 18, 24);

pub(super) fn register_culling(app: &mut App) {
    app.register_panel_content("culling_debug", true, build);
}

fn cull<R: Default>(w: &World, f: impl FnOnce(&CullingDebugState) -> R) -> R {
    w.get_resource::<CullingDebugState>().map(f).unwrap_or_default()
}

fn set_field(w: &mut World, f: impl FnOnce(&mut CullingDebugState)) {
    if let Some(mut s) = w.get_resource_mut::<CullingDebugState>() {
        f(&mut s);
    }
}

fn visible(s: &CullingDebugState) -> u32 {
    s.frustum_visible.saturating_sub(s.distance_culled)
}

fn faint_box(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(FAINT_BG)),
        ))
        .id()
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    // Overview header.
    let head = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let big = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 28.0), TextColor(rgb(TEXT_PRIMARY))))
        .id();
    bind_text(commands, big, |w| cull(w, visible).to_string());
    let total = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(TEXT_MUTED)),
            Node { margin: UiRect::bottom(Val::Px(4.0)), ..default() }))
        .id();
    bind_text(commands, total, |w| format!("/ {} visible", cull(w, |s| s.total_entities)));
    commands.entity(head).add_children(&[big, total]);
    let pct = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(TEXT_MUTED))))
        .id();
    // Only shown when total > 0 (bind_display below), so a max(1) divisor is safe.
    bind_text(commands, pct, |w| {
        cull(w, |s| format!("{}% of mesh entities visible", visible(s) * 100 / s.total_entities.max(1)))
    });
    bind_display(commands, pct, |w| cull(w, |s| s.total_entities) > 0);

    // Culling breakdown.
    let bd_label = section(commands, fonts, "Culling Breakdown");
    let bd = faint_box(commands);
    let r_vis = bar_row(commands, fonts, "Visible", (100, 200, 100), |w| cull(w, visible), |w| frac(w, visible, |s| s.total_entities));
    let r_fc = bar_row(commands, fonts, "Frustum Culled", (200, 150, 80), |w| cull(w, |s| s.frustum_culled), |w| frac(w, |s| s.frustum_culled, |s| s.total_entities));
    let r_dc = bar_row(commands, fonts, "Distance Culled", (200, 100, 100), |w| cull(w, |s| s.distance_culled), |w| frac(w, |s| s.distance_culled, |s| s.total_entities));
    let r_fade = bar_row(commands, fonts, "Fading", (180, 180, 100), |w| cull(w, |s| s.distance_faded), |w| frac(w, |s| s.distance_faded, |s| s.total_entities));
    bind_display(commands, r_fade, |w| cull(w, |s| s.distance_faded) > 0);
    commands.entity(bd).add_children(&[r_vis, r_fc, r_dc, r_fade]);

    // Distance distribution.
    let dd_label = section(commands, fonts, "Distance Distribution");
    let dd = faint_box(commands);
    const LABELS: [&str; 5] = ["0-50m", "50-100m", "100-200m", "200-500m", "500m+"];
    let mut rows = Vec::new();
    for (i, label) in LABELS.iter().enumerate() {
        let ci = (i as f32 / 4.0 * 0.6 + 0.4).min(1.0);
        let color = ((80.0 + 120.0 * ci) as u8, (160.0 - 60.0 * ci) as u8, 200u8);
        rows.push(bar_row(
            commands,
            fonts,
            label,
            color,
            move |w| cull(w, |s| s.distance_buckets[i]),
            move |w| {
                cull(w, |s| {
                    let max = s.distance_buckets.iter().copied().max().unwrap_or(1).max(1);
                    s.distance_buckets[i] as f32 / max as f32
                })
            },
        ));
    }
    commands.entity(dd).add_children(&rows);

    // Settings.
    let set_label = section(commands, fonts, "Settings");
    let set_box = faint_box(commands);
    let enable = checkbox_row(commands, fonts, "Enable Distance Culling");
    let max_dist = slider_row(
        commands,
        fonts,
        "Max Distance",
        10.0,
        2000.0,
        |v| format!("{:.0}m", v),
        |w| cull(w, |s| s.max_distance),
        |w, v| set_field(w, move |s| s.max_distance = v),
    );
    let fade = slider_row(
        commands,
        fonts,
        "Fade Start",
        0.5,
        1.0,
        |v| format!("{:.2}", v),
        |w| cull(w, |s| s.fade_start_fraction),
        |w, v| set_field(w, move |s| s.fade_start_fraction = v),
    );
    commands.entity(set_box).add_children(&[enable, max_dist, fade]);

    commands.entity(root).add_children(&[
        head, pct, bd_label, bd, dd_label, dd, set_label, set_box,
    ]);
    root
}

fn frac(w: &World, count: impl Fn(&CullingDebugState) -> u32, total: impl Fn(&CullingDebugState) -> u32) -> f32 {
    cull(w, |s| {
        let t = total(s).max(1) as f32;
        (count(s) as f32 / t).clamp(0.0, 1.0)
    })
}

/// A `label …… count` header over a thin proportion bar (fill width is a live
/// fraction binding).
fn bar_row<C, F>(commands: &mut Commands, fonts: &EmberFonts, label: &str, color: (u8, u8, u8), count: C, frac: F) -> Entity
where
    C: Fn(&World) -> u32 + Send + Sync + 'static,
    F: Fn(&World) -> f32 + Send + Sync + 'static,
{
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let header = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 10.0), TextColor(rgb(SECONDARY))))
        .id();
    let gap = commands
        .spawn(Node { flex_grow: 1.0, ..default() })
        .id();
    let c = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 10.0), TextColor(rgb(TEXT_PRIMARY))))
        .id();
    bind_text(commands, c, move |w| count(w).to_string());
    commands.entity(header).add_children(&[l, gap, c]);

    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
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
    bind_with(commands, fill, frac, |w, e, v: &f32| {
        if let Some(mut n) = w.get_mut::<Node>(e) {
            n.width = Val::Percent(v.clamp(0.0, 1.0) * 100.0);
        }
    });
    commands.entity(track).add_child(fill);
    commands.entity(col).add_children(&[header, track]);
    col
}

fn checkbox_row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let cb = checkbox(commands, false);
    bind_2way(
        commands,
        cb,
        |w| cull(w, |s| s.enabled),
        |w, v| set_field(w, move |s| s.enabled = *v),
    );
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(TEXT_PRIMARY))))
        .id();
    commands.entity(row).add_children(&[cb, l]);
    row
}

fn slider_row<G, S>(commands: &mut Commands, fonts: &EmberFonts, label: &str, min: f32, max: f32, fmt: fn(f32) -> String, get: G, set: S) -> Entity
where
    G: Fn(&World) -> f32 + Copy + Send + Sync + 'static,
    S: Fn(&mut World, f32) + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 10.0), TextColor(rgb(SECONDARY)),
            Node { width: Val::Px(80.0), ..default() }))
        .id();
    let sl = slider(commands, 0.0);
    let range = (max - min).max(1e-4);
    bind_2way(
        commands,
        sl,
        move |w| ((get(w) - min) / range).clamp(0.0, 1.0),
        move |w, v| set(w, min + *v * range),
    );
    let val = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 10.0), TextColor(rgb(TEXT_PRIMARY))))
        .id();
    bind_text(commands, val, move |w| fmt(get(w)));
    commands.entity(row).add_children(&[l, sl, val]);
    row
}

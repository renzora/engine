//! Bevy-native Lumen diagnostics panel — CPU bake timing/throttle, coverage
//! counts, and per-camera voxel-cache view flags. Pure reader over
//! `LumenDiagState`; every value is a binding, the camera list is a `keyed_list`.

use std::time::Duration;

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;

use crate::panels::lumen::{LumenCameraEntry, LumenDiagState};

use super::{root, section};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const AMBER: (u8, u8, u8) = (230, 180, 80);

pub(super) fn register_lumen(app: &mut App) {
    app.register_panel_content("lumen_diag", true, build_lumen);
}

fn lumen<R: Default>(w: &World, f: impl FnOnce(&LumenDiagState) -> R) -> R {
    w.get_resource::<LumenDiagState>().map(f).unwrap_or_default()
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

fn duration_color(d: Duration) -> Color {
    let ms = d.as_micros() as f64 / 1_000.0;
    if ms >= 5.0 {
        rgb((230, 110, 110))
    } else if ms >= 1.0 {
        rgb(AMBER)
    } else {
        rgb(text_primary())
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

fn yesno(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn saturated(s: &LumenDiagState) -> bool {
    s.bake.bake_budget_per_frame > 0 && s.bake.bakes_last_frame >= s.bake.bake_budget_per_frame
}

/// A `label …… value` row (label left, mono value right-aligned).
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

fn build_lumen(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    // CPU bake.
    let bake_label = section(commands, fonts, &renzora::lang::t("lumen.cpu_bake"));
    let dur_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let dur = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 22.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, dur, |w| format_duration(lumen(w, |s| s.bake.last_bake_dur)));
    bind_text_color(commands, dur, |w| duration_color(lumen(w, |s| s.bake.last_bake_dur)));
    let dur_note = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(SECONDARY)),
            Node {
                margin: UiRect::bottom(Val::Px(3.0)),
                ..default()
            },
        ))
        .id();
    bind_text(commands, dur_note, |w| {
        lumen(w, |s| {
            format!(
                "last frame  \u{b7}  avg {}  \u{b7}  max {}",
                format_duration(s.bake.avg_bake_dur),
                format_duration(s.bake.max_bake_dur),
            )
        })
    });
    commands.entity(dur_row).add_children(&[dur, dur_note]);

    let bakes = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(SECONDARY))))
        .id();
    bind_text(commands, bakes, |w| {
        lumen(w, |s| {
            format!(
                "{}/{} bakes this frame",
                s.bake.bakes_last_frame,
                s.bake.bake_budget_per_frame.max(1)
            )
        })
    });
    bind_text_color(commands, bakes, |w| {
        if lumen(w, saturated) {
            rgb(AMBER)
        } else {
            rgb(SECONDARY)
        }
    });
    let warn = commands
        .spawn((
            Text::new(format!("\u{26a0} {}", renzora::lang::t("lumen.throttle_saturated"))),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(AMBER)),
        ))
        .id();
    bind_display(commands, warn, |w| lumen(w, saturated));

    let lifetime_bakes = stat_row(commands, fonts, &renzora::lang::t("lumen.lifetime_bakes"), |w| {
        format_count(lumen(w, |s| s.bake.total_bakes))
    });
    let lifetime_samples = stat_row(commands, fonts, &renzora::lang::t("lumen.lifetime_samples"), |w| {
        format_count(lumen(w, |s| s.bake.total_samples_baked))
    });

    // Coverage.
    let cov_label = section(commands, fonts, &renzora::lang::t("lumen.coverage"));
    let cov_entities = stat_row(commands, fonts, &renzora::lang::t("lumen.entities_voxel_samples"), |w| {
        lumen(w, |s| s.mesh_voxel_samples_entities).to_string()
    });
    let cov_sky = stat_row(commands, fonts, &renzora::lang::t("lumen.sky_cubemap_bound"), |w| {
        yesno(lumen(w, |s| s.has_sky_cubemap)).to_string()
    });

    // Cameras.
    let cam_label = section(commands, fonts, &renzora::lang::t("lumen.voxel_cache_views"));
    let cam_list = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    keyed_list(commands, cam_list, cameras_snapshot);

    commands.entity(root).add_children(&[
        bake_label, dur_row, bakes, warn, lifetime_bakes, lifetime_samples, cov_label,
        cov_entities, cov_sky, cam_label, cam_list,
    ]);
    root
}

fn cameras_snapshot(world: &World) -> KeyedSnapshot {
    let cams = lumen(world, |s| s.cameras.clone());
    if cams.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new(renzora::lang::t("lumen.no_cameras")),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                ))
                .id()
            }),
        };
    }
    let items: Vec<(u64, u64)> = cams
        .iter()
        .map(|c| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&c.camera_name, &mut h);
            let flags = (c.inject_active as u64) | ((c.debug_active as u64) << 1);
            (std::hash::Hasher::finish(&h), flags)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| camera_row(c, f, &cams[i])),
    }
}

fn camera_row(commands: &mut Commands, fonts: &EmberFonts, cam: &LumenCameraEntry) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    let name = commands
        .spawn((
            Text::new(cam.camera_name.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let sub = commands
        .spawn((
            Text::new(format!(
                "inject: {}  \u{b7}  debug: {}",
                yesno(cam.inject_active),
                yesno(cam.debug_active)
            )),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(SECONDARY)),
            Node {
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(col).add_children(&[name, sub]);
    col
}

//! The Ray Tracing panel: Solari's internals as numbers and pictures, so its
//! quality can be judged and improved by measurement rather than by eye alone.
//!
//! The panel writes a [`SolariDiagnosticsRequest`] and reads back
//! [`SolariDiagnostics`]; `renzora_solari` does the rest (see its
//! `diagnostics.rs`). Measuring costs GPU time, so it runs only while the panel
//! is the visible tab. A visualization, and an accumulating reference, carry on
//! when the panel is hidden, since they are what the user is looking at.

use bevy::prelude::*;
use bevy::solari::realtime::HISTOGRAM_BINS;

use renzora::{SolariDebugMode, SolariDiagnostics, SolariDiagnosticsRequest, SolariFrameStats};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, bind_text_color, bind_with};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, collapsible, dropdown, line_chart_live, slider_ranged, toggle_switch, ChartStyle};
use renzora_ember::UiInteraction;

pub(crate) const PANEL: &str = "solari_debug";

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const GOOD: (u8, u8, u8) = (120, 200, 120);
const WARN: (u8, u8, u8) = (230, 190, 110);
const BAD: (u8, u8, u8) = (230, 120, 110);
const RAW_BAR: (u8, u8, u8) = (150, 150, 158);

/// Rolling window of total rays per frame, for the chart.
const RAY_HISTORY: usize = 32;

/// Solari's passes as Bevy's render diagnostics name them, and what to call
/// them. A pass missing from this list is not shown; the total still counts it.
const PASSES: [(&str, &str); 9] = [
    ("presample_light_tiles", "Light tiles"),
    ("world_cache", "World cache"),
    ("initial", "First paths"),
    ("spatial_and_shade", "ReSTIR reuse and shading"),
    ("denoise_temporal", "Denoiser: temporal"),
    ("denoise_atrous", "Denoiser: blur"),
    ("denoise_composite", "Denoiser: composite"),
    ("see_through", "Glass and see-through"),
    ("diagnostics", "Diagnostics (this panel)"),
];

/// Set by a system that runs only while the panel is visible, read and cleared
/// by one that always runs: the pair is how `stats` follows visibility.
#[derive(Resource, Default)]
pub(crate) struct PanelVisible(bool);

/// Totals of the last [`RAY_HISTORY`] frames measured, in millions.
#[derive(Resource, Default)]
pub(crate) struct RayHistory {
    values: Vec<f32>,
    /// The last frame added, so one frame is not added twice.
    last: Option<(u64, u64)>,
}

#[derive(Component)]
pub(crate) struct ResetReferenceButton;

pub(crate) fn mark_visible(mut visible: ResMut<PanelVisible>) {
    visible.0 = true;
}

/// Measure while the panel is visible, and stop when it is not.
pub(crate) fn follow_visibility(
    mut visible: ResMut<PanelVisible>,
    mut request: ResMut<SolariDiagnosticsRequest>,
) {
    if request.stats != visible.0 {
        request.stats = visible.0;
    }
    visible.0 = false;
}

pub(crate) fn reset_reference_pressed(
    pressed: Query<&UiInteraction, (Changed<UiInteraction>, With<ResetReferenceButton>)>,
    mut request: ResMut<SolariDiagnosticsRequest>,
) {
    if pressed.iter().any(|i| *i == UiInteraction::Pressed) {
        request.reset_reference = true;
    }
}

pub(crate) fn record_ray_history(diagnostics: Res<SolariDiagnostics>, mut history: ResMut<RayHistory>) {
    let Some(frame) = &diagnostics.frame else { return };
    let key = (frame.lighting_rays(), frame.covered_pixels);
    if history.last == Some(key) {
        return;
    }
    history.last = Some(key);
    history.values.push(frame.lighting_rays() as f32 / 1.0e6);
    if history.values.len() > RAY_HISTORY {
        history.values.remove(0);
    }
}

// ── Formatting ──────────────────────────────────────────────────────────────

fn count(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.2} M", n as f64 / 1e6),
        n if n >= 10_000 => format!("{:.0} k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

fn percent(f: f32) -> String {
    format!("{:.1}%", f * 100.0)
}

fn frame<R: Default>(w: &Rx, f: impl FnOnce(&SolariFrameStats) -> R) -> R {
    w.get_resource::<SolariDiagnostics>()
        .and_then(|d| d.frame.as_ref())
        .map(f)
        .unwrap_or_default()
}

fn has_frame(w: &Rx) -> bool {
    w.get_resource::<SolariDiagnostics>().is_some_and(|d| d.frame.is_some())
}

fn request(w: &Rx) -> SolariDiagnosticsRequest {
    w.get_resource::<SolariDiagnosticsRequest>().copied().unwrap_or_default()
}

fn set_request(world: &mut World, change: impl FnOnce(&mut SolariDiagnosticsRequest)) {
    if let Some(mut request) = world.get_resource_mut::<SolariDiagnosticsRequest>() {
        change(&mut request);
    }
}

// ── Rows ────────────────────────────────────────────────────────────────────

fn row(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: Entity) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            column_gap: Val::Px(8.0),
            min_height: Val::Px(20.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 12.0), TextColor(rgb(SECONDARY))))
        .id();
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    commands.entity(row).add_children(&[l, gap, value]);
    row
}

fn stat_row<V>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: V) -> Entity
where
    V: Fn(&Rx) -> String + Send + Sync + 'static,
{
    let v = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 12.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, v, value);
    row(commands, fonts, label, v)
}

/// A [`stat_row`] coloured by how good the figure is.
fn rated_row<V, C>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: V, color: C) -> Entity
where
    V: Fn(&Rx) -> String + Send + Sync + 'static,
    C: Fn(&Rx) -> (u8, u8, u8) + Send + Sync + 'static,
{
    let v = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 12.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, v, value);
    bind_text_color(commands, v, move |w| rgb(color(w)));
    row(commands, fonts, label, v)
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(SECONDARY)),
            Node {
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
        ))
        .id()
}

fn bound_note<V>(commands: &mut Commands, fonts: &EmberFonts, value: V) -> Entity
where
    V: Fn(&Rx) -> String + Send + Sync + 'static,
{
    let n = note(commands, fonts, "");
    bind_text(commands, n, value);
    n
}

fn section(commands: &mut Commands, fonts: &EmberFonts, root: Entity, icon: &str, title: &str, children: &[Entity]) {
    let (s, body) = collapsible(commands, fonts, Some(icon), title, true);
    commands.entity(body).add_children(children);
    commands.entity(root).add_child(s);
}

// ── Histogram ───────────────────────────────────────────────────────────────

/// One bar per bin, its height bound to that bin's share of the tallest bin of
/// either histogram, so the two rows share a scale and can be compared.
fn histogram_row(commands: &mut Commands, raw: bool) -> Entity {
    let bins = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(44.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(1.0),
                padding: UiRect::all(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let color = if raw { rgb(RAW_BAR) } else { rgb(accent()) };
    for bin in 0..HISTOGRAM_BINS {
        let bar = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(0.0),
                    ..default()
                },
                BackgroundColor(color),
            ))
            .id();
        bind_with(
            commands,
            bar,
            move |w| {
                frame(w, |f| {
                    let tallest = f
                        .histogram_raw
                        .iter()
                        .chain(&f.histogram_denoised)
                        .copied()
                        .max()
                        .unwrap_or(0)
                        .max(1);
                    let bins = if raw { &f.histogram_raw } else { &f.histogram_denoised };
                    // Whole percent, so a frame that moves a bin by a hair
                    // does not relayout the panel.
                    (bins.get(bin).copied().unwrap_or(0) as u64 * 100 / tallest as u64) as u32
                })
            },
            |world, e, height: &u32| {
                if let Some(mut node) = world.get_mut::<Node>(e) {
                    node.height = Val::Percent(*height as f32);
                }
            },
        );
        commands.entity(bins).add_child(bar);
    }
    bins
}

// ── Panel ───────────────────────────────────────────────────────────────────

pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::all(Val::Px(8.0)),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();

    let waiting = bound_note(commands, fonts, |w| {
        if has_frame(w) {
            String::new()
        } else {
            "Waiting for Solari: it measures the focused viewport while Solari Ray-Traced GI is on.".into()
        }
    });
    bind_display(commands, waiting, |w| !has_frame(w));
    commands.entity(root).add_child(waiting);

    // ── View ──
    let view = dropdown(commands, fonts, &SolariDebugMode::LABELS, 0);
    bind_2way(
        commands,
        view,
        |w| request(w).view as usize,
        |world, i: &usize| {
            if let Some(mode) = SolariDebugMode::ALL.get(*i).copied() {
                set_request(world, |r| r.view = mode);
            }
        },
    );
    let view_row = row(commands, fonts, "Show", view);
    let legend = bound_note(commands, fonts, |w| request(w).view.legend().to_string());
    let split = slider_ranged(commands, 0.5, 0.0, 1.0);
    bind_2way(commands, split, |w| request(w).split, |world, v: &f32| set_request(world, |r| r.split = *v));
    let split_row = row(commands, fonts, "Divider", split);
    bind_display(commands, split_row, |w| request(w).view == SolariDebugMode::RawSplit);
    let dlss_note = note(commands, fonts, "DLSS steps aside while a view is shown, so the built-in denoiser runs.");
    bind_display(commands, dlss_note, |w| request(w).view != SolariDebugMode::None);
    section(commands, fonts, root, "eye", "View", &[view_row, legend, split_row, dlss_note]);

    // ── Reference ──
    let accumulate = toggle_switch(commands, false);
    bind_2way(commands, accumulate, |w| request(w).reference, |world, on: &bool| set_request(world, |r| r.reference = *on));
    let accumulate_row = row(commands, fonts, "Accumulate", accumulate);
    let reset = button(commands, &fonts.ui, "Start over");
    commands.entity(reset).insert(ResetReferenceButton);
    let reset_row = row(commands, fonts, "", reset);
    let samples = stat_row(commands, fonts, "Samples per pixel", |w| frame(w, |f| f.reference_samples.to_string()));
    let error = rated_row(
        commands,
        fonts,
        "Error vs reference",
        |w| frame(w, |f| f.reference_error.map(percent).unwrap_or_else(|| "-".into())),
        |w| {
            frame(w, |f| match f.reference_error {
                Some(e) if e < 0.1 => GOOD,
                Some(e) if e < 0.25 => WARN,
                Some(_) => BAD,
                None => SECONDARY,
            })
        },
    );
    let reference_note = note(
        commands,
        fonts,
        "A conventional path tracer converging beside the live image. It starts over when the camera moves. \
         The error compares 3x3 neighbourhoods and falls as the reference converges, settling at the live \
         image's own error.",
    );
    section(commands, fonts, root, "target", "Reference", &[accumulate_row, reset_row, samples, error, reference_note]);

    // ── Rays ──
    let total = stat_row(commands, fonts, "Lighting rays per frame", |w| frame(w, |f| count(f.lighting_rays())));
    let per_pixel = stat_row(commands, fonts, "Per pixel", |w| {
        frame(w, |f| format!("{:.2}", f.lighting_rays() as f64 / f.covered_pixels.max(1) as f64))
    });
    let initial = stat_row(commands, fonts, "First paths (shadow)", |w| {
        frame(w, |f| format!("{} ({})", count(f.rays_initial), count(f.shadow_rays_initial)))
    });
    let reuse = stat_row(commands, fonts, "ReSTIR reuse (shadow)", |w| {
        frame(w, |f| format!("{} ({})", count(f.rays_reuse), count(f.shadow_rays_reuse)))
    });
    let cache = stat_row(commands, fonts, "World cache (shadow)", |w| {
        frame(w, |f| format!("{} ({})", count(f.rays_world_cache), count(f.shadow_rays_world_cache)))
    });
    let debug_rays = stat_row(commands, fonts, "Diagnostics", |w| frame(w, |f| count(f.rays_debug)));
    let chart = line_chart_live(
        commands,
        ChartStyle {
            min: Some(0.0),
            height: 40.0,
            ..default()
        },
        |w| w.get_resource::<RayHistory>().map(|h| h.values.clone()).unwrap_or_default(),
    );
    let ray_note = note(commands, fonts, "Millions of lighting rays per frame, most recent on the right.");
    section(commands, fonts, root, "flashlight", "Rays", &[total, per_pixel, initial, reuse, cache, debug_rays, chart, ray_note]);

    // ── Denoiser ──
    let dlss = note(
        commands,
        fonts,
        "DLSS is denoising. Its result is not measured here: these figures describe the built-in \
         denoiser, and come back when it runs.",
    );
    fn dlss_running(w: &Rx) -> bool {
        w.get_resource::<SolariDiagnostics>().is_some_and(|d| d.dlss_running)
    }
    bind_display(commands, dlss, dlss_running);
    let history = rated_row(
        commands,
        fonts,
        "Average history",
        |w| frame(w, |f| if f.denoiser_active { format!("{:.1} frames", f.mean_history) } else { "off".into() }),
        |w| frame(w, |f| if f.mean_history >= 24.0 { GOOD } else if f.mean_history >= 8.0 { WARN } else { BAD }),
    );
    let short = stat_row(commands, fonts, "Under 4 frames", |w| frame(w, |f| percent(f.short_history_fraction)));
    let rejected = rated_row(
        commands,
        fonts,
        "Rejected this frame",
        |w| frame(w, |f| percent(f.rejected_fraction)),
        |w| frame(w, |f| if f.rejected_fraction < 0.02 { GOOD } else if f.rejected_fraction < 0.1 { WARN } else { BAD }),
    );
    let grain = rated_row(
        commands,
        fonts,
        "Grain, raw to denoised",
        |w| frame(w, |f| format!("{:.3} -> {:.3}", f.grain_raw, f.grain_denoised)),
        |w| frame(w, |f| if f.grain_denoised < 0.02 { GOOD } else if f.grain_denoised < 0.05 { WARN } else { BAD }),
    );
    let reduction = stat_row(commands, fonts, "Grain removed", |w| {
        frame(w, |f| format!("{:.0}x", f.grain_raw / f.grain_denoised.max(1e-4)))
    });
    let grain_note = note(
        commands,
        fonts,
        "Grain is each pixel's difference from its neighbours, relative to their brightness. A still \
         camera should reach a full history (32 frames) with almost nothing rejected.",
    );
    // Hidden while DLSS denoises: the built-in denoiser is not running then, and
    // the grain is measured before DLSS, so "0.42 -> 0.42, 1x" read as DLSS
    // removing nothing.
    for row in [history, short, rejected, grain, reduction, grain_note] {
        bind_display(commands, row, move |w| !dlss_running(w));
    }
    section(commands, fonts, root, "drop", "Denoiser", &[dlss, history, short, rejected, grain, reduction, grain_note]);

    // ── ReSTIR ──
    let confidence = stat_row(commands, fonts, "Average confidence", |w| {
        frame(w, |f| {
            if f.restir {
                format!("{:.1} of {:.0}", f.mean_confidence, 2.0 * f.confidence_cap + 1.0)
            } else {
                "off (Quality: Low)".into()
            }
        })
    });
    let confidence_note = note(commands, fonts, "How many samples each pixel's reservoir has gathered from past frames and neighbours.");
    section(commands, fonts, root, "arrows-clockwise", "ReSTIR", &[confidence, confidence_note]);

    // ── Brightness ──
    let raw_label = note(commands, fonts, "Raw");
    let raw_bars = histogram_row(commands, true);
    let denoised_label = note(commands, fonts, "Denoised");
    let denoised_bars = histogram_row(commands, false);
    let range = bound_note(commands, fonts, |w| {
        frame(w, |f| {
            let top = f.histogram_min_log2 + f.histogram_stops_per_bin * f.histogram_raw.len() as f32;
            format!(
                "Exposed brightness, 2^{:.0} (left) to 2^{:.0} (right), half a stop per bar. Denoising should narrow the spread.",
                f.histogram_min_log2, top
            )
        })
    });
    section(commands, fonts, root, "chart-bar", "Brightness", &[raw_label, raw_bars, denoised_label, denoised_bars, range]);

    // ── Ray-traced scene ──
    let coverage = |pick: fn(&renzora::SolariCoverage) -> usize| {
        move |w: &Rx| {
            w.get_resource::<SolariDiagnostics>()
                .map(|d| pick(&d.coverage).to_string())
                .unwrap_or_default()
        }
    };
    let mirrored = stat_row(commands, fonts, "Meshes traced", coverage(|c| c.mirrored));
    let stripped = stat_row(commands, fonts, "Through stripped copies", coverage(|c| c.stripped_copies));
    let stand_ins = stat_row(commands, fonts, "With material stand-ins", coverage(|c| c.stand_ins));
    let skipped = stat_row(commands, fonts, "Cannot be traced", coverage(|c| c.skipped));
    let excluded = stat_row(commands, fonts, "Held back (hidden, off-layer)", coverage(|c| c.excluded));
    let not_traced = rated_row(
        commands,
        fonts,
        "Pixels not matching",
        |w| frame(w, |f| percent(f.not_traced_fraction)),
        |w| frame(w, |f| if f.not_traced_fraction < 0.005 { GOOD } else if f.not_traced_fraction < 0.05 { WARN } else { BAD }),
    );
    let cells = stat_row(commands, fonts, "World cache cells", |w| {
        w.get_resource::<SolariDiagnostics>()
            .and_then(|d| d.world_cache_cells)
            .map(|c| count(c as u64))
            .unwrap_or_else(|| "-".into())
    });
    let scene_note = note(
        commands,
        fonts,
        "Pixels not matching: what was drawn is missing from, or different in, the ray-traced scene, \
         so it casts and catches light wrongly. See it with Show: Not in ray-traced scene.",
    );
    section(commands, fonts, root, "cube", "Ray-traced scene", &[mirrored, stripped, stand_ins, skipped, excluded, not_traced, cells, scene_note]);

    // ── GPU time ──
    let mut rows: Vec<Entity> = PASSES
        .iter()
        .map(|&(pass, label)| {
            stat_row(commands, fonts, label, move |w| {
                w.get_resource::<SolariDiagnostics>()
                    .and_then(|d| d.gpu_ms.iter().find(|(p, _)| p == pass).map(|(_, ms)| format!("{ms:.2} ms")))
                    .unwrap_or_else(|| "-".into())
            })
        })
        .collect();
    rows.push(stat_row(commands, fonts, "Total", |w| {
        w.get_resource::<SolariDiagnostics>()
            .map(|d| format!("{:.2} ms", d.gpu_ms.iter().map(|(_, ms)| ms).sum::<f64>()))
            .unwrap_or_default()
    }));
    rows.push(note(commands, fonts, "GPU time per pass, smoothed. Needs a GPU that reports timestamps."));
    section(commands, fonts, root, "timer", "GPU time", &rows);

    root
}

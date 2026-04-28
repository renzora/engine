//! Loading overlay — the phase between project selection and the editor.
//!
//! Plugins register `LoadingTask`s that the overlay renders with a progress
//! bar. When every task reports complete (`completed >= total`), the state
//! advances to [`SplashState::Editor`][super::SplashState::Editor] and the
//! overlay disappears.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::SplashState;

/// Centered letterbox dimensions for the loading overlay (width, height).
const LETTERBOX_SIZE: (f32, f32) = (600.0, 180.0);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LoadingTaskHandle(u32);

#[derive(Clone, Debug)]
pub struct LoadingTask {
    pub label: String,
    pub total: u32,
    pub completed: u32,
    pub detail: Option<String>,
}

impl LoadingTask {
    pub fn is_done(&self) -> bool {
        self.completed >= self.total
    }
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            (self.completed as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }
}

#[derive(Resource, Default)]
pub struct LoadingTasks {
    next_id: u32,
    tasks: Vec<(LoadingTaskHandle, LoadingTask)>,
    pub(crate) min_frames_remaining: u32,
}

impl LoadingTasks {
    pub fn register(&mut self, label: impl Into<String>, total: u32) -> LoadingTaskHandle {
        let h = LoadingTaskHandle(self.next_id);
        self.next_id += 1;
        self.tasks.push((
            h,
            LoadingTask {
                label: label.into(),
                total,
                completed: 0,
                detail: None,
            },
        ));
        // Minimum display time to prevent flicker when tasks register with zero work.
        self.min_frames_remaining = self.min_frames_remaining.max(30);
        h
    }

    pub fn advance(&mut self, handle: LoadingTaskHandle, by: u32) {
        if let Some(task) = self.task_mut(handle) {
            task.completed = task.completed.saturating_add(by).min(task.total);
        }
    }

    pub fn set_detail(&mut self, handle: LoadingTaskHandle, detail: impl Into<String>) {
        if let Some(task) = self.task_mut(handle) {
            task.detail = Some(detail.into());
        }
    }

    pub fn complete(&mut self, handle: LoadingTaskHandle) {
        if let Some(task) = self.task_mut(handle) {
            task.completed = task.total;
            task.detail = None;
        }
    }

    pub fn tasks(&self) -> &[(LoadingTaskHandle, LoadingTask)] {
        &self.tasks
    }

    pub fn all_done(&self) -> bool {
        self.tasks.iter().all(|(_, t)| t.is_done())
    }

    /// Drop every registered task. Used when transitioning between
    /// loading sessions (e.g. splash → editor, or one editor overlay
    /// session ending so the next can start) — without this, the
    /// fraction calculation would still see the previous session's
    /// completed totals and start the next bar partway full.
    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    /// Drop only finished tasks; in-flight ones survive. Lets the editor
    /// overlay tear down its session at the end while still being safe
    /// to call mid-session if anything's still ticking.
    pub fn clear_completed(&mut self) {
        self.tasks.retain(|(_, t)| !t.is_done());
    }

    pub fn tick_and_can_advance(&mut self) -> bool {
        if self.min_frames_remaining > 0 {
            self.min_frames_remaining -= 1;
        }
        self.min_frames_remaining == 0 && self.all_done()
    }

    fn task_mut(&mut self, handle: LoadingTaskHandle) -> Option<&mut LoadingTask> {
        self.tasks
            .iter_mut()
            .find(|(h, _)| *h == handle)
            .map(|(_, t)| t)
    }
}

pub(crate) fn loading_ui_system(
    mut contexts: EguiContexts,
    tasks: Res<LoadingTasks>,
    time: Res<Time>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        bevy::log::warn!("[loading] egui context unavailable");
        return;
    };

    // Request continuous repaints — the animated shimmer & elapsed-time
    // animations need a new frame even when no user input arrives.
    ctx.request_repaint();

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(10, 10, 14)))
        .show(ctx, |ui| {
            let full_rect = ui.max_rect();
            paint_loading_overlay(ui, &tasks, full_rect, time.elapsed_secs_f64());
        });
}

/// Paint the loading overlay widget (rosette + letterbox + animated bar)
/// inside `paint_rect`. Caller decides where the overlay lives — the
/// splash uses a fullscreen `CentralPanel`; the editor's modal uses a
/// foreground-order `Area` so it floats over the editor without
/// disturbing its panel layout.
///
/// Pulled out of `loading_ui_system` so both consumers render the same
/// widget without copy-pasting 90 lines of egui calls.
pub fn paint_loading_overlay(
    ui: &mut egui::Ui,
    tasks: &LoadingTasks,
    paint_rect: egui::Rect,
    time_secs: f64,
) {
    // Compute aggregate progress from registered loading tasks. Each task
    // contributes its `completed/total` to the bar.
    let (total_completed, total_total): (u64, u64) = tasks
        .tasks
        .iter()
        .fold((0, 0), |(sc, st), (_, t)| {
            (sc + t.completed as u64, st + t.total as u64)
        });
    let fraction = if total_total == 0 {
        1.0
    } else {
        (total_completed as f32 / total_total as f32).clamp(0.0, 1.0)
    };
    let percent = (fraction * 100.0).round() as u32;

    let phase_label = "Loading";
    let current = tasks
        .tasks
        .iter()
        .find(|(_, t)| !t.is_done())
        .map(|(_, t)| match &t.detail {
            Some(d) => format!("{} — {}", t.label, d),
            None => t.label.clone(),
        })
        .unwrap_or_default();

    // Centered letterbox — the loading UI fits in a compact strip
    // instead of swallowing the whole window. Clamp so it never
    // exceeds the available viewport on tiny displays.
    let target = egui::vec2(
        LETTERBOX_SIZE.0.min(paint_rect.width() - 16.0),
        LETTERBOX_SIZE.1.min(paint_rect.height() - 16.0),
    );
    let letterbox = egui::Rect::from_center_size(paint_rect.center(), target);

    // Translucent fill so the strip lifts off the surrounding window
    // — slightly bluer than the backdrop and semi-opaque so the
    // rosette underneath stays visible through it.
    ui.painter().rect_filled(
        letterbox,
        egui::CornerRadius::same(6),
        egui::Color32::from_rgba_unmultiplied(20, 22, 32, 215),
    );

    // Spirograph rosette painted inside the letterbox only.
    draw_neon_grid(ui.painter(), letterbox, time_secs);

    // Soft outline framing the strip — drawn last so it stays clean
    // over the rosette's edges.
    ui.painter().rect_stroke(
        letterbox,
        egui::CornerRadius::same(6),
        egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 50, 72)),
        egui::StrokeKind::Inside,
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(letterbox), |ui| {
        ui.add_space(18.0);

        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(phase_label)
                        .size(18.0)
                        .strong()
                        .color(egui::Color32::from_rgb(240, 240, 248)),
                );
                ui.add_space(2.0);
                if !current.is_empty() {
                    ui.label(
                        egui::RichText::new(&current)
                            .size(12.0)
                            .color(egui::Color32::from_rgb(170, 175, 190)),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(" ")
                            .size(12.0)
                            .color(egui::Color32::TRANSPARENT),
                    );
                }
            });
        });

        ui.add_space(14.0);

        // Progress bar row: bar + percentage, anchored to the letterbox.
        let bar_rect = {
            let h = 12.0;
            let margin = 18.0;
            let pct_width = 44.0;
            let spacing = 10.0;
            let bar_width = letterbox.width() - margin * 2.0 - pct_width - spacing;
            let origin = egui::pos2(letterbox.left() + margin, ui.cursor().top());
            egui::Rect::from_min_size(origin, egui::vec2(bar_width, h))
        };

        draw_animated_bar(ui, bar_rect, fraction, time_secs);

        // Percentage label aligned right of the bar.
        let pct_rect = egui::Rect::from_min_size(
            egui::pos2(bar_rect.right() + 10.0, bar_rect.top() - 2.0),
            egui::vec2(44.0, 16.0),
        );
        ui.painter().text(
            pct_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            format!("{}%", percent),
            egui::FontId::monospace(14.0),
            egui::Color32::from_rgb(220, 225, 240),
        );
    });
}

/// Paint the editor's modal loading overlay. Renders the same widget
/// the splash uses, but on a foreground-order `Area` with a fullscreen
/// dimmed backdrop that absorbs pointer events — so the editor can't
/// be clicked while a tab's assets are decoding.
///
/// Caller is responsible for deciding when to show this — the runner
/// system in `renzora_scene` ticks `LoadingTasks` and the
/// `EditorLoadingOverlayActive` resource that gates this UI.
pub(crate) fn editor_loading_overlay_ui_system(
    mut contexts: EguiContexts,
    tasks: Res<LoadingTasks>,
    time: Res<Time>,
    active: Res<EditorLoadingOverlayActive>,
) {
    if !active.0 {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    ctx.request_repaint();

    let screen = ctx.screen_rect();

    egui::Area::new(egui::Id::new("editor_loading_overlay"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(0.0, 0.0))
        .interactable(true)
        .show(ctx, |ui| {
            // Fullscreen dim absorbs pointer events. Allocating a
            // click-and-drag-sense response over the whole screen is
            // what makes this modal — egui won't let any widget
            // beneath this layer get hovered or clicked while we hold
            // the pointer here.
            let resp = ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
            ui.painter().rect_filled(
                resp.rect,
                egui::CornerRadius::ZERO,
                egui::Color32::from_rgba_unmultiplied(8, 10, 16, 180),
            );

            paint_loading_overlay(ui, &tasks, screen, time.elapsed_secs_f64());
        });
}

/// Resource toggled by `renzora_scene::tick_editor_load_progress`.
/// While `true`, [`editor_loading_overlay_ui_system`] paints the modal
/// over the editor; while `false` (the steady state) the system is a
/// no-op.
#[derive(Resource, Default)]
pub struct EditorLoadingOverlayActive(pub bool);

fn draw_animated_bar(ui: &mut egui::Ui, rect: egui::Rect, fraction: f32, time_secs: f64) {
    let painter = ui.painter();
    let radius = egui::CornerRadius::same(4);

    // Dim track behind the filled region (semi-opaque over the plasma).
    painter.rect_filled(
        rect,
        radius,
        egui::Color32::from_rgba_unmultiplied(6, 6, 12, 220),
    );
    // Track border so it reads as a channel even when plasma is quiet.
    painter.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 50, 72)),
        egui::StrokeKind::Inside,
    );

    let filled_w = (rect.width() * fraction).max(0.0);
    if filled_w > 0.0 {
        let filled_rect = egui::Rect::from_min_size(rect.min, egui::vec2(filled_w, rect.height()));
        // Filled region: bright cyan→violet gradient that pulses with time.
        let pulse = 0.5 + 0.5 * ((time_secs * 3.2) as f32).sin();
        let r = (90.0 + 40.0 * pulse) as u8;
        let g = (180.0 - 30.0 * pulse) as u8;
        let b = 255u8;
        painter.rect_filled(filled_rect, radius, egui::Color32::from_rgb(r, g, b));
        draw_shimmer_stripes(painter, filled_rect, time_secs);

        // Leading-edge glow — a brighter vertical band at the fill tip.
        let glow_x = filled_rect.right();
        for i in 0..6 {
            let a = 120 - i as u8 * 18;
            let w = 2.0 + i as f32 * 1.5;
            let r = egui::Rect::from_min_size(
                egui::pos2(glow_x - w, filled_rect.top()),
                egui::vec2(w * 2.0, filled_rect.height()),
            );
            painter.rect_filled(
                r,
                egui::CornerRadius::same(3),
                egui::Color32::from_rgba_unmultiplied(180, 220, 255, a),
            );
        }
    }
}

fn draw_shimmer_stripes(painter: &egui::Painter, rect: egui::Rect, time_secs: f64) {
    let stripe_period: f32 = 28.0;
    let phase = ((time_secs * 60.0) as f32) % stripe_period;
    let stripe_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28);

    // Clip shimmer to the filled region.
    let mut x = rect.left() - stripe_period + phase;
    while x < rect.right() {
        // Each stripe is a parallelogram shifted +h/2 along x at the bottom.
        let h = rect.height();
        let slant = h * 0.6;
        let pts = [
            egui::pos2(x, rect.top()),
            egui::pos2(x + 8.0, rect.top()),
            egui::pos2(x + 8.0 + slant, rect.bottom()),
            egui::pos2(x + slant, rect.bottom()),
        ];
        // Clip manually by only drawing stripes whose midpoint falls within rect x-range.
        let mid_x = x + slant * 0.5 + 4.0;
        if mid_x >= rect.left() && mid_x <= rect.right() {
            painter.add(egui::Shape::convex_polygon(
                pts.to_vec(),
                stripe_color,
                egui::Stroke::NONE,
            ));
        }
        x += stripe_period;
    }
}

/// Neon rosette — a family of rotated ellipses layered to form an envelope
/// with a star-shaped core. Each line segment is colored by its angular
/// position from the rosette's center, giving the fixed yellow→cyan→blue→
/// magenta quadrant gradient shown in the reference art. The whole pattern
/// rotates slowly so the curves appear to swim.
fn draw_neon_grid(painter: &egui::Painter, rect: egui::Rect, t: f64) {
    let center = rect.center();
    let radius = rect.width().min(rect.height() * 2.0) * 0.55;

    // How many ellipses make the envelope. More = denser weave, but costs more
    // line segments per frame. 32 hits a sweet spot at the loader's resolution.
    let n_ellipses: u32 = 32;
    let n_points: u32 = 72;

    // Primary slow clockwise rotation of the whole rosette.
    let base_rot = (t * 0.08) as f32;
    // Second slow oscillation so the inner star "breathes" rather than just spinning.
    let aspect_mod = 0.35 + 0.12 * ((t * 0.22) as f32).sin();

    for i in 0..n_ellipses {
        let ring_theta =
            base_rot + (i as f32 / n_ellipses as f32) * std::f32::consts::TAU;
        let (sin_r, cos_r) = ring_theta.sin_cos();

        // Each ellipse: major axis = full radius, minor axis oscillates (breathing).
        let a = radius * 1.02;
        let b = radius * aspect_mod;

        let mut prev: Option<egui::Pos2> = None;
        let mut prev_color: Option<egui::Color32> = None;
        for s in 0..=n_points {
            let alpha = (s as f32 / n_points as f32) * std::f32::consts::TAU;
            // Parametric ellipse in local frame, then rotate into place.
            let lx = a * alpha.cos();
            let ly = b * alpha.sin();
            let x = lx * cos_r - ly * sin_r;
            let y = lx * sin_r + ly * cos_r;
            let point = egui::pos2(center.x + x, center.y + y);

            // Color: angular position of the point relative to the rosette
            // center. Anchored so yellow stays roughly top, which matches the
            // reference image's orientation.
            let ang = y.atan2(x);
            let hue = ((ang / std::f32::consts::TAU) + 0.5).rem_euclid(1.0);
            let (rr, gg, bb) = hsv_to_rgb(hue, 0.95, 1.0);
            let color = egui::Color32::from_rgba_unmultiplied(rr, gg, bb, 70);

            if let (Some(pp), Some(_)) = (prev, prev_color) {
                // Thin core stroke + a faint thicker glow underlay for neon feel.
                painter.line_segment(
                    [pp, point],
                    egui::Stroke::new(
                        2.5,
                        egui::Color32::from_rgba_unmultiplied(rr, gg, bb, 22),
                    ),
                );
                painter.line_segment([pp, point], egui::Stroke::new(1.0, color));
            }
            prev = Some(point);
            prev_color = Some(color);
        }
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(1.0);
    let c = v * s;
    let h6 = h * 6.0;
    let x = c * (1.0 - ((h6 % 2.0) - 1.0).abs());
    let (r, g, b) = match h6 as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    (
        (((r + m) * 255.0).clamp(0.0, 255.0)) as u8,
        (((g + m) * 255.0).clamp(0.0, 255.0)) as u8,
        (((b + m) * 255.0).clamp(0.0, 255.0)) as u8,
    )
}

pub(crate) fn auto_advance_to_editor(
    mut tasks: ResMut<LoadingTasks>,
    mut next_state: ResMut<NextState<SplashState>>,
) {
    if tasks.tick_and_can_advance() {
        next_state.set(SplashState::Editor);
    }
}

pub(crate) fn log_loading_entered() {
    bevy::log::info!("[loading] entered Loading state");
}

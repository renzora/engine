//! Loading overlay — the phase between project selection and the editor.
//!
//! Plugins register `LoadingTask`s that the overlay renders with a progress
//! bar. When every task reports complete (`completed >= total`), the state
//! advances to [`SplashState::Editor`][super::SplashState::Editor] and the
//! overlay disappears.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::launcher::{LoadPhase, LoadProgress};
use crate::SplashState;

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
    progress: Option<Res<LoadProgress>>,
    time: Res<Time>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        bevy::log::warn!("[loading] egui context unavailable");
        return;
    };

    // Request continuous repaints — the animated shimmer & elapsed-time
    // animations need a new frame even when no user input arrives.
    ctx.request_repaint();

    let snapshot = progress.map(|p| p.snapshot()).unwrap_or_default();
    let phase_label = snapshot.phase.label();
    let fraction = snapshot.fraction();
    let percent = (fraction * 100.0).round() as u32;

    let current = if snapshot.current.is_empty() {
        String::new()
    } else {
        match snapshot.phase {
            LoadPhase::Plugins => format!("Loading plugin: {}", snapshot.current),
            LoadPhase::Thumbnails => format!("Rendering: {}", snapshot.current),
            _ => snapshot.current.clone(),
        }
    };

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(10, 10, 14)))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let t = time.elapsed_secs_f64();

            draw_neon_grid(ui.painter(), rect, t);

            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.add_space(22.0);

                ui.horizontal(|ui| {
                    ui.add_space(22.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(phase_label)
                                .size(20.0)
                                .strong()
                                .color(egui::Color32::from_rgb(240, 240, 248)),
                        );
                        ui.add_space(3.0);
                        if !current.is_empty() {
                            ui.label(
                                egui::RichText::new(&current)
                                    .size(14.0)
                                    .color(egui::Color32::from_rgb(170, 175, 190)),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(" ")
                                    .size(14.0)
                                    .color(egui::Color32::TRANSPARENT),
                            );
                        }
                    });
                });

                ui.add_space(18.0);

                // Progress bar row: bar + percentage.
                let bar_rect = {
                    let h = 14.0;
                    let available = ui.available_width();
                    let margin = 18.0;
                    let pct_width = 48.0;
                    let spacing = 10.0;
                    let bar_width = available - margin * 2.0 - pct_width - spacing;
                    let origin = egui::pos2(
                        ui.min_rect().left() + margin,
                        ui.cursor().top(),
                    );
                    egui::Rect::from_min_size(origin, egui::vec2(bar_width, h))
                };

                draw_animated_bar(ui, bar_rect, fraction, time.elapsed_secs_f64());

                // Percentage label aligned right of the bar.
                let pct_rect = egui::Rect::from_min_size(
                    egui::pos2(bar_rect.right() + 10.0, bar_rect.top() - 2.0),
                    egui::vec2(48.0, 18.0),
                );
                ui.painter().text(
                    pct_rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    format!("{}%", percent),
                    egui::FontId::monospace(15.0),
                    egui::Color32::from_rgb(220, 225, 240),
                );

                // Advance cursor past the bar so any follow-up layout (tasks below) stacks cleanly.
                ui.allocate_space(egui::vec2(ui.available_width(), bar_rect.height() + 8.0));
            });
        });
}

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

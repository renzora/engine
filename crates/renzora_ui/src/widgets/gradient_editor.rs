//! Gradient editor — RGB + alpha color stops along a normalized 0..1 track.
//!
//! Click below the gradient to add a stop, drag to move, double-click to
//! remove. Selected stop's color is editable via a popup picker.

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// A single gradient stop.
#[derive(Clone, Copy, Debug)]
pub struct GradientStop {
    pub t: f32,
    pub color: [f32; 4], // rgba
}

/// Configuration for `gradient_editor`.
#[derive(Clone, Copy, Debug)]
pub struct GradientEditorConfig {
    pub height: f32,
    pub with_alpha: bool,
}

impl Default for GradientEditorConfig {
    fn default() -> Self {
        Self { height: 28.0, with_alpha: true }
    }
}

/// Render an interactive gradient editor. Returns the index of the currently
/// selected stop (click-to-select), or `None`.
pub fn gradient_editor(
    ui: &mut egui::Ui,
    stops: &mut Vec<GradientStop>,
    cfg: GradientEditorConfig,
    theme: &Theme,
) -> (egui::Response, Option<usize>) {
    let w = ui.available_width().max(120.0);
    let full_h = cfg.height + 16.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, full_h), Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    let gradient_rect = egui::Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width(), cfg.height),
    );
    let handle_rect = egui::Rect::from_min_size(
        Pos2::new(rect.min.x, rect.min.y + cfg.height + 2.0),
        Vec2::new(rect.width(), 12.0),
    );

    // Draw gradient by sampling pixels
    stops.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    let steps = rect.width() as usize;
    for i in 0..steps {
        let t = i as f32 / (steps - 1).max(1) as f32;
        let c = sample_gradient(stops, t);
        let x = gradient_rect.min.x + i as f32;
        painter.line_segment(
            [Pos2::new(x, gradient_rect.min.y), Pos2::new(x, gradient_rect.max.y)],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(
                (c[0] * 255.0) as u8,
                (c[1] * 255.0) as u8,
                (c[2] * 255.0) as u8,
                if cfg.with_alpha { (c[3] * 255.0) as u8 } else { 255 },
            )),
        );
    }
    painter.rect_stroke(
        gradient_rect,
        3.0,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );

    // Draw stop handles
    let selected = ui.memory(|m| m.data.get_temp::<Option<usize>>(response.id).flatten());
    for (i, s) in stops.iter().enumerate() {
        let x = handle_rect.min.x + handle_rect.width() * s.t;
        let y = handle_rect.center().y;
        let color = Color32::from_rgb(
            (s.color[0] * 255.0) as u8,
            (s.color[1] * 255.0) as u8,
            (s.color[2] * 255.0) as u8,
        );
        painter.circle_filled(Pos2::new(x, y), 5.0, color);
        let stroke_color = if Some(i) == selected {
            theme.widgets.active_bg.to_color32()
        } else {
            Color32::WHITE
        };
        painter.circle_stroke(Pos2::new(x, y), 5.0, Stroke::new(1.5, stroke_color));
    }

    // Interaction
    let mut new_selected = selected;
    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            let idx = stops.iter().position(|s| {
                let px = handle_rect.min.x + handle_rect.width() * s.t;
                (pos - Pos2::new(px, handle_rect.center().y)).length() < 8.0
            });
            if idx.is_some() {
                new_selected = idx;
            } else if gradient_rect.contains(pos) || handle_rect.contains(pos) {
                let t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                let color = sample_gradient(stops, t);
                stops.push(GradientStop { t, color });
                new_selected = Some(stops.len() - 1);
            }
        }
    }
    if response.dragged() {
        if let Some(idx) = new_selected {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(s) = stops.get_mut(idx) {
                    s.t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                }
            }
        }
    }
    if response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(idx) = stops.iter().position(|s| {
                let px = handle_rect.min.x + handle_rect.width() * s.t;
                (pos - Pos2::new(px, handle_rect.center().y)).length() < 8.0
            }) {
                if stops.len() > 2 {
                    stops.remove(idx);
                    new_selected = None;
                }
            }
        }
    }
    ui.memory_mut(|m| m.data.insert_temp::<Option<usize>>(response.id, new_selected));

    (response, new_selected)
}

fn sample_gradient(stops: &[GradientStop], t: f32) -> [f32; 4] {
    if stops.is_empty() {
        return [0.0; 4];
    }
    if t <= stops[0].t {
        return stops[0].color;
    }
    if t >= stops[stops.len() - 1].t {
        return stops[stops.len() - 1].color;
    }
    for w in stops.windows(2) {
        if t >= w[0].t && t <= w[1].t {
            let span = (w[1].t - w[0].t).max(1e-6);
            let local = (t - w[0].t) / span;
            return [
                w[0].color[0] + (w[1].color[0] - w[0].color[0]) * local,
                w[0].color[1] + (w[1].color[1] - w[0].color[1]) * local,
                w[0].color[2] + (w[1].color[2] - w[0].color[2]) * local,
                w[0].color[3] + (w[1].color[3] - w[0].color[3]) * local,
            ];
        }
    }
    stops[0].color
}

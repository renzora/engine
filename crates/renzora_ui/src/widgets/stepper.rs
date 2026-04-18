//! Stepper / wizard progress — numbered steps with a current position.

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

pub struct StepperConfig {
    pub height: f32,
    pub allow_navigation: bool,
}

impl Default for StepperConfig {
    fn default() -> Self {
        Self { height: 48.0, allow_navigation: true }
    }
}

/// Render a horizontal stepper. Returns a picked index if the user clicks
/// on a step label (and navigation is allowed), otherwise `None`.
pub fn stepper(
    ui: &mut egui::Ui,
    steps: &[&str],
    current: usize,
    cfg: StepperConfig,
    theme: &Theme,
) -> Option<usize> {
    let n = steps.len().max(1);
    let w = ui.available_width().max(200.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, cfg.height), Sense::hover());
    let painter = ui.painter_at(rect);
    let mut picked = None;

    let step_cx = |i: usize| -> Pos2 {
        Pos2::new(
            rect.min.x + rect.width() * (i as f32 + 0.5) / n as f32,
            rect.min.y + 16.0,
        )
    };

    // Connector line
    let line_y = step_cx(0).y;
    painter.line_segment(
        [
            Pos2::new(rect.min.x + rect.width() * 0.05, line_y),
            Pos2::new(rect.max.x - rect.width() * 0.05, line_y),
        ],
        Stroke::new(2.0, theme.widgets.border.to_color32()),
    );

    for (i, label) in steps.iter().enumerate() {
        let c = step_cx(i);
        let active = i == current;
        let complete = i < current;
        let color = if active || complete {
            theme.widgets.active_bg.to_color32()
        } else {
            theme.widgets.border.to_color32()
        };
        painter.circle_filled(c, 10.0, color);
        painter.text(
            c,
            egui::Align2::CENTER_CENTER,
            format!("{}", i + 1),
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );
        let label_pos = Pos2::new(c.x, c.y + 16.0);
        painter.text(
            label_pos,
            egui::Align2::CENTER_TOP,
            *label,
            egui::FontId::proportional(10.0),
            if active {
                theme.text.primary.to_color32()
            } else {
                theme.text.muted.to_color32()
            },
        );

        if cfg.allow_navigation {
            let hit = egui::Rect::from_center_size(c, Vec2::new(48.0, 36.0));
            let resp = ui.interact(hit, ui.id().with(("step", i)), Sense::click());
            if resp.clicked() {
                picked = Some(i);
            }
        }
    }

    picked
}

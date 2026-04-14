//! Timeline scrubber + track lanes.
//!
//! Foundation for animation timeline, audio arrangement, cinemachine dolly,
//! any feature that needs a horizontal time axis with draggable keys per
//! track.

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// A single track lane.
pub struct TrackLane<'a> {
    pub label: &'a str,
    /// Keys on this lane, in seconds.
    pub keys: &'a mut Vec<f32>,
    pub color: Color32,
}

/// Configuration for the timeline widget.
#[derive(Clone, Copy, Debug)]
pub struct TimelineConfig {
    pub lane_height: f32,
    pub label_width: f32,
    pub duration: f32,
    pub minor_divisions: u32,
    pub major_divisions: u32,
}

impl Default for TimelineConfig {
    fn default() -> Self {
        Self {
            lane_height: 22.0,
            label_width: 100.0,
            duration: 10.0,
            minor_divisions: 10,
            major_divisions: 2,
        }
    }
}

/// Render the timeline header + lanes. `playhead` is the current time in
/// seconds; clicking on the ruler area seeks it. Returns the response over
/// the whole timeline so callers can react to clicks/drags.
pub fn timeline(
    ui: &mut egui::Ui,
    lanes: &mut [TrackLane<'_>],
    playhead: &mut f32,
    cfg: TimelineConfig,
    theme: &Theme,
) -> egui::Response {
    let total_h = 18.0 + cfg.lane_height * lanes.len() as f32;
    let w = ui.available_width().max(200.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, total_h), Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    let track_start_x = rect.min.x + cfg.label_width;
    let track_rect = egui::Rect::from_min_max(
        Pos2::new(track_start_x, rect.min.y),
        rect.max,
    );
    let to_x = |t: f32| track_rect.min.x + track_rect.width() * (t / cfg.duration);
    let to_t = |x: f32| ((x - track_rect.min.x) / track_rect.width()) * cfg.duration;

    // Header ruler
    let ruler_rect = egui::Rect::from_min_size(
        Pos2::new(track_start_x, rect.min.y),
        Vec2::new(track_rect.width(), 18.0),
    );
    painter.rect_filled(ruler_rect, 0.0, theme.surfaces.faint.to_color32());
    let major_step = cfg.duration / cfg.major_divisions.max(1) as f32;
    let minor_step = cfg.duration / cfg.minor_divisions.max(1) as f32;
    let muted = theme.text.muted.to_color32();
    let mut t = 0.0;
    while t <= cfg.duration + 1e-4 {
        let x = to_x(t);
        painter.line_segment(
            [Pos2::new(x, ruler_rect.max.y - 4.0), Pos2::new(x, ruler_rect.max.y)],
            Stroke::new(1.0, muted),
        );
        t += minor_step;
    }
    let mut t = 0.0;
    while t <= cfg.duration + 1e-4 {
        let x = to_x(t);
        painter.line_segment(
            [Pos2::new(x, ruler_rect.min.y + 2.0), Pos2::new(x, ruler_rect.max.y)],
            Stroke::new(1.0, theme.text.primary.to_color32()),
        );
        painter.text(
            Pos2::new(x + 2.0, ruler_rect.min.y),
            egui::Align2::LEFT_TOP,
            format!("{:.2}s", t),
            egui::FontId::proportional(9.0),
            muted,
        );
        t += major_step;
    }

    // Lanes
    for (i, lane) in lanes.iter_mut().enumerate() {
        let y = rect.min.y + 18.0 + i as f32 * cfg.lane_height;
        let lane_rect = egui::Rect::from_min_size(
            Pos2::new(rect.min.x, y),
            Vec2::new(rect.width(), cfg.lane_height),
        );
        let bg = if i % 2 == 0 {
            theme.panels.inspector_row_even.to_color32()
        } else {
            theme.panels.inspector_row_odd.to_color32()
        };
        painter.rect_filled(lane_rect, 0.0, bg);
        painter.text(
            Pos2::new(lane_rect.min.x + 6.0, lane_rect.center().y),
            egui::Align2::LEFT_CENTER,
            lane.label,
            egui::FontId::proportional(11.0),
            theme.text.primary.to_color32(),
        );
        for k in lane.keys.iter() {
            let x = to_x(*k);
            painter.rect_filled(
                egui::Rect::from_center_size(
                    Pos2::new(x, lane_rect.center().y),
                    Vec2::new(8.0, cfg.lane_height - 8.0),
                ),
                2.0,
                lane.color,
            );
        }
    }

    // Playhead interaction (click/drag on ruler sets playhead)
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if pos.x >= track_start_x {
                *playhead = to_t(pos.x).clamp(0.0, cfg.duration);
            }
        }
    }

    // Playhead line
    let px = to_x(*playhead);
    painter.line_segment(
        [Pos2::new(px, rect.min.y), Pos2::new(px, rect.max.y)],
        Stroke::new(1.5, theme.widgets.active_bg.to_color32()),
    );

    response
}

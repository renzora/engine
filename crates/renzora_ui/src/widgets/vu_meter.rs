//! VU / level meter — vertical bar with green→yellow→red gradient and peak hold.

use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

/// Configuration for a VU meter.
pub struct VuMeterConfig {
    /// Width of a single meter bar.
    pub width: f32,
    /// Height of the meter.
    pub height: f32,
    /// Whether to render stereo (two bars side by side).
    pub stereo: bool,
    /// Gap between stereo bars.
    pub gap: f32,
}

impl Default for VuMeterConfig {
    fn default() -> Self {
        Self {
            width: 8.0,
            height: 120.0,
            stereo: false,
            gap: 2.0,
        }
    }
}

/// Current level and peak-hold value for one channel.
pub struct VuMeterValue {
    /// Current level, 0.0–1.0.
    pub level: f32,
    /// Peak hold level, 0.0–1.0.
    pub peak: f32,
}

impl Default for VuMeterValue {
    fn default() -> Self {
        Self {
            level: 0.0,
            peak: 0.0,
        }
    }
}

/// Color for a given vertical position on the meter.
///
/// - Bottom 60%: green
/// - 60%–85%: yellow/warning
/// - 85%–100%: red/error
fn meter_color(t: f32, green: Color32, yellow: Color32, red: Color32) -> Color32 {
    if t < 0.60 {
        green
    } else if t < 0.85 {
        let blend = (t - 0.60) / 0.25;
        lerp_color(green, yellow, blend)
    } else {
        let blend = (t - 0.85) / 0.15;
        lerp_color(yellow, red, blend)
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

/// Paint a VU meter. Returns the `Response` for the allocated area.
pub fn vu_meter(
    ui: &mut egui::Ui,
    left: &VuMeterValue,
    right: Option<&VuMeterValue>,
    config: &VuMeterConfig,
) -> egui::Response {
    let total_width = if config.stereo && right.is_some() {
        config.width * 2.0 + config.gap
    } else {
        config.width
    };

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(total_width, config.height),
        Sense::hover(),
    );

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let green = Color32::from_rgb(89, 191, 115);
        let yellow = Color32::from_rgb(242, 166, 64);
        let red = Color32::from_rgb(230, 89, 89);

        let draw_bar = |bar_rect: Rect, val: &VuMeterValue| {
            // Dark background
            painter.rect_filled(bar_rect, 2.0, Color32::from_rgb(12, 12, 15));

            // Segmented filled bar
            let level = val.level.clamp(0.0, 1.0);
            if level > 0.005 {
                let segments = 20;
                let seg_height = bar_rect.height() / segments as f32;
                for i in 0..segments {
                    let seg_t = i as f32 / segments as f32;
                    if seg_t >= level {
                        break;
                    }
                    let seg_bottom = bar_rect.bottom() - seg_t * bar_rect.height();
                    let seg_top = seg_bottom - seg_height + 1.0; // 1px gap
                    let seg_rect = Rect::from_min_max(
                        Pos2::new(bar_rect.left() + 1.0, seg_top),
                        Pos2::new(bar_rect.right() - 1.0, seg_bottom),
                    );
                    let color = meter_color(seg_t, green, yellow, red);
                    painter.rect_filled(seg_rect, 0.0, color);
                }
            }

            // Peak hold line
            let peak = val.peak.clamp(0.0, 1.0);
            if peak > 0.01 {
                let peak_y = bar_rect.bottom() - peak * bar_rect.height();
                painter.line_segment(
                    [
                        Pos2::new(bar_rect.left() + 1.0, peak_y),
                        Pos2::new(bar_rect.right() - 1.0, peak_y),
                    ],
                    Stroke::new(2.0, Color32::WHITE),
                );
            }
        };

        // Left channel
        let left_rect = Rect::from_min_size(rect.min, Vec2::new(config.width, config.height));
        draw_bar(left_rect, left);

        // Right channel (stereo)
        if config.stereo {
            if let Some(r) = right {
                let right_rect = Rect::from_min_size(
                    Pos2::new(rect.min.x + config.width + config.gap, rect.min.y),
                    Vec2::new(config.width, config.height),
                );
                draw_bar(right_rect, r);
            }
        }
    }

    response
}

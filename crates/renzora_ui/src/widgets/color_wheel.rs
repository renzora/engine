//! HSV color wheel — hue ring + S/V triangle. Complement to `color_picker_with_hex`.

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Interactive HSV color wheel. Mutates `rgb` in-place when user picks.
pub fn color_wheel(ui: &mut egui::Ui, rgb: &mut [f32; 3], size: f32, _theme: &Theme) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let outer = size * 0.5 - 2.0;
    let inner = outer * 0.75;

    // Hue ring — draw as many small arcs approximated by triangles
    let segments = 72;
    for i in 0..segments {
        let a0 = i as f32 / segments as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let h = i as f32 / segments as f32;
        let c = hsv_to_rgb(h, 1.0, 1.0);
        let color = Color32::from_rgb(
            (c[0] * 255.0) as u8,
            (c[1] * 255.0) as u8,
            (c[2] * 255.0) as u8,
        );
        let p0 = center + Vec2::angled(a0) * inner;
        let p1 = center + Vec2::angled(a0) * outer;
        let p2 = center + Vec2::angled(a1) * outer;
        let p3 = center + Vec2::angled(a1) * inner;
        painter.add(egui::Shape::convex_polygon(vec![p0, p1, p2, p3], color, Stroke::NONE));
    }

    // SV square inside the ring
    let (h, mut s, mut v) = rgb_to_hsv(*rgb);
    let square_size = inner * 1.3;
    let square = egui::Rect::from_center_size(center, Vec2::splat(square_size));
    let steps = 24;
    for yi in 0..steps {
        for xi in 0..steps {
            let sx = xi as f32 / (steps - 1) as f32;
            let sy = yi as f32 / (steps - 1) as f32;
            let c = hsv_to_rgb(h, sx, 1.0 - sy);
            let color = Color32::from_rgb(
                (c[0] * 255.0) as u8,
                (c[1] * 255.0) as u8,
                (c[2] * 255.0) as u8,
            );
            let px = square.min.x + square.width() * sx;
            let py = square.min.y + square.height() * sy;
            let cell = Vec2::new(square.width() / steps as f32, square.height() / steps as f32);
            painter.rect_filled(egui::Rect::from_min_size(Pos2::new(px, py), cell), 0.0, color);
        }
    }

    // Interaction
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let from_center = pos - center;
            let dist = from_center.length();
            if dist > inner && dist < outer {
                let angle = from_center.y.atan2(from_center.x);
                let h_new = (angle / std::f32::consts::TAU).rem_euclid(1.0);
                *rgb = hsv_to_rgb(h_new, s.max(0.01), v.max(0.01));
            } else if square.contains(pos) {
                s = ((pos.x - square.min.x) / square.width()).clamp(0.0, 1.0);
                v = 1.0 - ((pos.y - square.min.y) / square.height()).clamp(0.0, 1.0);
                *rgb = hsv_to_rgb(h, s, v);
            }
        }
    }

    // Current-value indicators
    let hue_angle = h * std::f32::consts::TAU;
    let hue_dot = center + Vec2::angled(hue_angle) * ((inner + outer) * 0.5);
    painter.circle_stroke(hue_dot, 4.0, Stroke::new(2.0, Color32::WHITE));
    let sv_pos = Pos2::new(
        square.min.x + square.width() * s,
        square.min.y + square.height() * (1.0 - v),
    );
    painter.circle_stroke(sv_pos, 4.0, Stroke::new(2.0, Color32::WHITE));

    response
}

fn rgb_to_hsv(rgb: [f32; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let v = max;
    let s = if max <= 0.0 { 0.0 } else { d / max };
    let h = if d <= 1e-6 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, v)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let c = v * s;
    let h6 = h * 6.0;
    let x = c * (1.0 - (h6.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match h6 as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r1 + m, g1 + m, b1 + m]
}

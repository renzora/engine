//! Rendering helpers for the node graph widget.
//!
//! All functions take an egui `Painter` and draw onto its clip rect.

use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, Vec2};

use super::types::*;

// ── Coordinate helpers ─────────────────────────────────────────────────────

/// Convert canvas-space position to screen-space position.
pub fn canvas_to_screen(pos: [f32; 2], offset: [f32; 2], zoom: f32, rect: Rect) -> Pos2 {
    let center = rect.center();
    Pos2::new(
        center.x + (pos[0] + offset[0]) * zoom,
        center.y + (pos[1] + offset[1]) * zoom,
    )
}

/// Convert screen-space position to canvas-space position.
pub fn screen_to_canvas(screen: Pos2, offset: [f32; 2], zoom: f32, rect: Rect) -> [f32; 2] {
    let center = rect.center();
    let rel = screen - center;
    [rel.x / zoom - offset[0], rel.y / zoom - offset[1]]
}

// ── Grid ───────────────────────────────────────────────────────────────────

/// Draw a dot grid on the canvas background.
pub fn draw_grid(painter: &egui::Painter, rect: Rect, offset: [f32; 2], zoom: f32, config: &NodeGraphConfig) {
    let spacing = config.grid_spacing * zoom;
    let dot_size = 1.5 * zoom.sqrt();

    let ox = (offset[0] * zoom) % spacing;
    let oy = (offset[1] * zoom) % spacing;

    let mut x = rect.left() + ox;
    while x < rect.right() {
        let mut y = rect.top() + oy;
        while y < rect.bottom() {
            painter.circle_filled(Pos2::new(x, y), dot_size, config.grid_dot);
            y += spacing;
        }
        x += spacing;
    }
}

// ── Nodes ──────────────────────────────────────────────────────────────────

/// Pin position record returned by [`draw_node`].
pub struct PinPos {
    pub id: PinId,
    pub screen_pos: Pos2,
    pub color: Color32,
    /// Hit-test rectangle covering the pin dot + label.
    pub hit_rect: Rect,
}

/// Draw a single node. Returns (node screen rect, list of pin positions).
pub fn draw_node(
    painter: &egui::Painter,
    node: &NodeDef,
    offset: [f32; 2],
    zoom: f32,
    canvas_rect: Rect,
    is_selected: bool,
    is_hovered: bool,
    config: &NodeGraphConfig,
) -> (Rect, Vec<PinPos>) {
    let screen_pos = canvas_to_screen(node.position, offset, zoom, canvas_rect);

    let input_count = node.pins.iter().filter(|p| p.direction == PinDirection::Input).count();
    let output_count = node.pins.iter().filter(|p| p.direction == PinDirection::Output).count();
    let max_pins = input_count.max(output_count);

    // Thumbnail adds extra height between header and pins
    let thumb_height = if node.thumbnail.is_some() { config.node_width * 0.5 } else { 0.0 };
    let node_height = config.header_height + thumb_height + (max_pins as f32 * config.pin_height) + 8.0;
    let scaled_w = config.node_width * zoom;
    let scaled_h = node_height * zoom;

    let node_rect = Rect::from_min_size(screen_pos, Vec2::new(scaled_w, scaled_h));

    // Cull off-screen nodes
    if !canvas_rect.intersects(node_rect) {
        return (node_rect, Vec::new());
    }

    let cr = (config.corner_radius * zoom) as u8;

    // Body
    painter.rect_filled(node_rect, CornerRadius::same(cr), config.node_bg);

    // Header
    let header_rect = Rect::from_min_size(screen_pos, Vec2::new(scaled_w, config.header_height * zoom));
    painter.rect_filled(
        header_rect,
        CornerRadius { nw: cr, ne: cr, sw: 0, se: 0 },
        node.header_color,
    );

    // Title
    let font_size = 12.0 * zoom;
    painter.text(
        header_rect.center(),
        egui::Align2::CENTER_CENTER,
        &node.title,
        egui::FontId::proportional(font_size),
        Color32::WHITE,
    );

    // Thumbnail (between header and pins)
    let thumb_offset = if let Some(tex_id) = node.thumbnail {
        let th = thumb_height * zoom;
        let pad = 4.0 * zoom;
        let thumb_rect = Rect::from_min_size(
            Pos2::new(screen_pos.x + pad, screen_pos.y + config.header_height * zoom),
            Vec2::new(scaled_w - pad * 2.0, th),
        );
        painter.image(tex_id, thumb_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        th
    } else {
        0.0
    };

    // Border
    let (border_color, border_width) = if is_selected {
        (config.selected_border, 2.0 * zoom)
    } else if is_hovered {
        // Slightly brighter border on hover
        let c = config.node_border;
        (Color32::from_rgb(c.r().saturating_add(40), c.g().saturating_add(40), c.b().saturating_add(40)), 1.5 * zoom)
    } else {
        (config.node_border, 1.5 * zoom)
    };
    painter.rect_stroke(
        node_rect,
        CornerRadius::same(cr),
        Stroke::new(border_width, border_color),
        StrokeKind::Outside,
    );

    // Pins
    let pin_start_y = screen_pos.y + config.header_height * zoom + thumb_offset + 4.0 * zoom;
    let pin_spacing = config.pin_height * zoom;
    let pin_radius = config.pin_radius * zoom;
    let label_font = egui::FontId::proportional(10.0 * zoom);

    let mut pin_positions: Vec<PinPos> = Vec::new();

    // Fixed pin hit-area width (pin dot + label zone)
    let half_row = pin_spacing / 2.0;
    let pin_zone_w = 60.0 * zoom;

    // Input pins (left side)
    let mut iy = 0;
    for pin in node.pins.iter().filter(|p| p.direction == PinDirection::Input) {
        let py = pin_start_y + (iy as f32 + 0.5) * pin_spacing;
        let pos = Pos2::new(screen_pos.x, py);

        let hit_rect = Rect::from_min_max(
            Pos2::new(pos.x - pin_radius, py - half_row),
            Pos2::new(pos.x + pin_zone_w, py + half_row),
        );

        draw_pin_shape(painter, pos, pin.shape, pin.color, pin_radius, zoom);

        if !pin.label.is_empty() {
            painter.text(
                Pos2::new(pos.x + pin_radius * 2.0, pos.y),
                egui::Align2::LEFT_CENTER,
                &pin.label,
                label_font.clone(),
                config.text_muted,
            );
        }

        pin_positions.push(PinPos {
            id: PinId { node: node.id, name: pin.name.clone(), direction: PinDirection::Input },
            screen_pos: pos,
            color: pin.color,
            hit_rect,
        });
        iy += 1;
    }

    // Output pins (right side)
    let mut oy_idx = 0;
    for pin in node.pins.iter().filter(|p| p.direction == PinDirection::Output) {
        let py = pin_start_y + (oy_idx as f32 + 0.5) * pin_spacing;
        let pos = Pos2::new(screen_pos.x + scaled_w, py);

        let hit_rect = Rect::from_min_max(
            Pos2::new(pos.x - pin_zone_w, py - half_row),
            Pos2::new(pos.x + pin_radius, py + half_row),
        );

        draw_pin_shape(painter, pos, pin.shape, pin.color, pin_radius, zoom);

        if !pin.label.is_empty() {
            painter.text(
                Pos2::new(pos.x - pin_radius * 2.0, pos.y),
                egui::Align2::RIGHT_CENTER,
                &pin.label,
                label_font.clone(),
                config.text_muted,
            );
        }

        pin_positions.push(PinPos {
            id: PinId { node: node.id, name: pin.name.clone(), direction: PinDirection::Output },
            screen_pos: pos,
            color: pin.color,
            hit_rect,
        });
        oy_idx += 1;
    }

    (node_rect, pin_positions)
}

/// Draw a pin shape (circle or triangle).
fn draw_pin_shape(painter: &egui::Painter, pos: Pos2, shape: PinShape, color: Color32, radius: f32, zoom: f32) {
    match shape {
        PinShape::Circle => {
            painter.circle_filled(pos, radius, color);
            painter.circle_stroke(pos, radius, Stroke::new(1.0 * zoom, Color32::from_rgb(80, 80, 85)));
        }
        PinShape::Triangle => {
            let s = radius * 0.8;
            let points = vec![
                Pos2::new(pos.x - s, pos.y - s),
                Pos2::new(pos.x + s, pos.y),
                Pos2::new(pos.x - s, pos.y + s),
            ];
            painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
        }
    }
}

/// Draw a highlight behind a pin row on hover.
pub fn draw_pin_highlight(painter: &egui::Painter, pin: &PinPos, _zoom: f32) {
    let color = Color32::from_rgba_premultiplied(
        pin.color.r() / 4, pin.color.g() / 4, pin.color.b() / 4, 60,
    );
    painter.rect_filled(pin.hit_rect, 0.0, color);
}

// ── Connections ────────────────────────────────────────────────────────────

/// Draw a cubic bezier connection between two screen-space positions.
pub fn draw_connection(
    painter: &egui::Painter,
    from: Pos2,
    to: Pos2,
    color: Color32,
    zoom: f32,
    config: &NodeGraphConfig,
) {
    let control_dist = ((to.x - from.x).abs() * 0.5).max(50.0 * zoom);
    let cp1 = Pos2::new(from.x + control_dist, from.y);
    let cp2 = Pos2::new(to.x - control_dist, to.y);

    let bezier = egui::epaint::CubicBezierShape::from_points_stroke(
        [from, cp1, cp2, to],
        false,
        Color32::TRANSPARENT,
        Stroke::new(config.connection_width * zoom, color),
    );
    painter.add(bezier);
}

/// Draw a pending (in-progress) connection from a pin to the cursor.
pub fn draw_pending_connection(
    painter: &egui::Painter,
    from: Pos2,
    to: Pos2,
    color: Color32,
    zoom: f32,
    config: &NodeGraphConfig,
) {
    draw_connection(painter, from, to, color, zoom, config);
}

// ── Cable hit-testing ──────────────────────────────────────────────────────

/// Evaluate a cubic bezier at parameter `t` (0..1).
fn bezier_point(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let uu = u * u;
    let tt = t * t;
    Pos2::new(
        uu * u * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + tt * t * p3.x,
        uu * u * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + tt * t * p3.y,
    )
}

/// Check if `point` is within `threshold` pixels of the bezier cable between `from` and `to`.
pub fn point_near_cable(from: Pos2, to: Pos2, point: Pos2, zoom: f32, threshold: f32) -> bool {
    let control_dist = ((to.x - from.x).abs() * 0.5).max(50.0 * zoom);
    let cp1 = Pos2::new(from.x + control_dist, from.y);
    let cp2 = Pos2::new(to.x - control_dist, to.y);

    // Sample 20 points along the curve
    let steps = 20;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let bp = bezier_point(from, cp1, cp2, to, t);
        if point.distance(bp) <= threshold {
            return true;
        }
    }
    false
}

// ── Box selection ──────────────────────────────────────────────────────────

/// Draw the rubber-band selection rectangle.
pub fn draw_box_selection(painter: &egui::Painter, start: Pos2, end: Pos2, config: &NodeGraphConfig) {
    let rect = Rect::from_two_pos(start, end);
    painter.rect_filled(rect, CornerRadius::ZERO, config.selection_fill);
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, config.selection_stroke),
        StrokeKind::Outside,
    );
}

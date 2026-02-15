//! Render Pipeline node graph visualization panel
//!
//! Displays Bevy's render graph as an interactive node graph with live timing overlays.
//! Nodes are draggable, canvas supports pan/zoom.

use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Stroke, Vec2};

use crate::core::resources::render_pipeline::{
    RenderGraphEdge, RenderGraphNode, RenderPipelineGraphData,
};
use crate::core::resources::diagnostics::RenderStats;
use renzora_theme::Theme;

/// Node dimensions in canvas units
const NODE_WIDTH: f32 = 180.0;
const NODE_HEIGHT: f32 = 60.0;
/// Header height within node
const HEADER_HEIGHT: f32 = 22.0;
/// Background bump grid spacing
const BUMP_SPACING: f32 = 24.0;

/// Render the render pipeline panel content
pub fn render_render_pipeline_content(
    ui: &mut egui::Ui,
    graph_data: &mut RenderPipelineGraphData,
    render_stats: &RenderStats,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            render_toolbar(ui, graph_data, theme);

            let available = ui.available_rect_before_wrap();
            if available.width() < 10.0 || available.height() < 10.0 {
                return;
            }

            render_canvas(ui, available, graph_data, render_stats, theme);
        });
}

fn render_toolbar(
    ui: &mut egui::Ui,
    graph_data: &mut RenderPipelineGraphData,
    theme: &Theme,
) {
    let toolbar_height = 28.0;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), toolbar_height),
        egui::Sense::hover(),
    );

    ui.painter().rect_filled(rect, 0.0, theme.surfaces.extreme.to_color32());
    ui.painter().line_segment(
        [Pos2::new(rect.min.x, rect.max.y), Pos2::new(rect.max.x, rect.max.y)],
        Stroke::new(1.0, theme.widgets.border.to_color32()),
    );

    let mut cursor_x = rect.min.x + 8.0;
    let btn_height = 20.0;
    let btn_y = rect.min.y + (toolbar_height - btn_height) / 2.0;

    // Timing toggle
    cursor_x = toolbar_button(
        ui, rect, cursor_x, btn_y, btn_height,
        &mut graph_data.show_timing, "Timing", "timing_toggle",
        Color32::from_rgba_unmultiplied(80, 190, 120, 40), theme,
    );

    // Sub-graph toggle
    cursor_x = toolbar_button(
        ui, rect, cursor_x, btn_y, btn_height,
        &mut graph_data.show_sub_graphs, "Groups", "sg_toggle",
        Color32::from_rgba_unmultiplied(80, 140, 220, 40), theme,
    );

    // Vertical toggle
    let vert_label = if graph_data.vertical { "Vertical" } else { "Horizontal" };
    let vert_rect = Rect::from_min_size(Pos2::new(cursor_x, btn_y), Vec2::new(80.0, btn_height));
    let vert_resp = ui.interact(vert_rect, ui.id().with("vert_toggle"), egui::Sense::click());
    let vert_bg = if vert_resp.hovered() {
        theme.panels.tab_hover.to_color32()
    } else {
        Color32::from_rgba_unmultiplied(160, 140, 200, 40)
    };
    ui.painter().rect_filled(vert_rect, 3.0, vert_bg);
    ui.painter().text(vert_rect.center(), egui::Align2::CENTER_CENTER, vert_label, egui::FontId::proportional(10.0), theme.text.secondary.to_color32());
    if vert_resp.clicked() {
        graph_data.vertical = !graph_data.vertical;
        crate::core::resources::render_pipeline::auto_layout(graph_data);
        fit_graph_to_view(graph_data);
    }
    cursor_x += 88.0;

    // Fit View
    let fit_rect = Rect::from_min_size(Pos2::new(cursor_x, btn_y), Vec2::new(70.0, btn_height));
    let fit_resp = ui.interact(fit_rect, ui.id().with("fit_view"), egui::Sense::click());
    let fit_bg = if fit_resp.hovered() { theme.panels.tab_hover.to_color32() } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(fit_rect, 3.0, fit_bg);
    ui.painter().text(fit_rect.center(), egui::Align2::CENTER_CENTER, "Fit View", egui::FontId::proportional(10.0), theme.text.secondary.to_color32());
    if fit_resp.clicked() {
        fit_graph_to_view(graph_data);
    }
    let _cursor_x = cursor_x + 78.0;

    // Re-layout
    let relayout_rect = Rect::from_min_size(Pos2::new(cursor_x + 78.0, btn_y), Vec2::new(70.0, btn_height));
    let relayout_resp = ui.interact(relayout_rect, ui.id().with("relayout"), egui::Sense::click());
    let relayout_bg = if relayout_resp.hovered() { theme.panels.tab_hover.to_color32() } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(relayout_rect, 3.0, relayout_bg);
    ui.painter().text(relayout_rect.center(), egui::Align2::CENTER_CENTER, "Re-layout", egui::FontId::proportional(10.0), theme.text.secondary.to_color32());
    if relayout_resp.clicked() {
        crate::core::resources::render_pipeline::auto_layout(graph_data);
        fit_graph_to_view(graph_data);
    }

    // Node count label (right side)
    let count_text = format!("{} nodes  {} edges", graph_data.nodes.len(), graph_data.edges.len());
    ui.painter().text(
        Pos2::new(rect.max.x - 8.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        count_text,
        egui::FontId::proportional(10.0),
        theme.text.disabled.to_color32(),
    );
}

// Helper: render a toolbar toggle button. Returns new cursor_x.
fn toolbar_button(
    ui: &mut egui::Ui,
    _toolbar_rect: Rect,
    cursor_x: f32,
    btn_y: f32,
    btn_height: f32,
    value: &mut bool,
    label: &str,
    id_str: &str,
    active_color: Color32,
    theme: &Theme,
) -> f32 {
    let text = if *value { format!("{}: ON", label) } else { format!("{}: OFF", label) };
    let btn_rect = Rect::from_min_size(Pos2::new(cursor_x, btn_y), Vec2::new(80.0, btn_height));
    let resp = ui.interact(btn_rect, ui.id().with(id_str), egui::Sense::click());
    let bg = if resp.hovered() {
        theme.panels.tab_hover.to_color32()
    } else if *value {
        active_color
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(btn_rect, 3.0, bg);
    ui.painter().text(btn_rect.center(), egui::Align2::CENTER_CENTER, &text, egui::FontId::proportional(10.0), theme.text.secondary.to_color32());
    if resp.clicked() {
        *value = !*value;
    }
    cursor_x + 88.0
}

fn fit_graph_to_view(graph_data: &mut RenderPipelineGraphData) {
    if graph_data.nodes.is_empty() {
        return;
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for node in &graph_data.nodes {
        min_x = min_x.min(node.position[0]);
        min_y = min_y.min(node.position[1]);
        max_x = max_x.max(node.position[0] + NODE_WIDTH);
        max_y = max_y.max(node.position[1] + NODE_HEIGHT);
    }
    graph_data.canvas.offset = [-(min_x + max_x) / 2.0, -(min_y + max_y) / 2.0];
    graph_data.canvas.zoom = 1.0;
}

fn render_canvas(
    ui: &mut egui::Ui,
    rect: Rect,
    graph_data: &mut RenderPipelineGraphData,
    _render_stats: &RenderStats,
    theme: &Theme,
) {
    let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 0.0, theme.surfaces.panel.to_color32());

    let pointer = ui.input(|i| i.pointer.clone());
    let pointer_pos = pointer.hover_pos();
    let zoom = graph_data.canvas.zoom;
    let canvas_center = [rect.center().x, rect.center().y];

    // ── Node dragging ────────────────────────────────────────────────────
    let primary_down = pointer.button_down(egui::PointerButton::Primary);
    let primary_pressed = pointer.button_pressed(egui::PointerButton::Primary);
    let primary_released = pointer.button_released(egui::PointerButton::Primary);

    if primary_pressed {
        // Check if we clicked on a node
        if let Some(cursor) = pointer_pos {
            if rect.contains(cursor) {
                let mut hit_node: Option<(usize, [f32; 2])> = None;
                for node in graph_data.nodes.iter().rev() {
                    let screen = canvas_to_screen(node.position, &graph_data.canvas.offset, canvas_center, zoom);
                    let node_rect = Rect::from_min_size(
                        Pos2::new(screen[0], screen[1]),
                        Vec2::new(NODE_WIDTH * zoom, NODE_HEIGHT * zoom),
                    );
                    if node_rect.contains(cursor) {
                        // Offset = cursor canvas pos - node canvas pos
                        let cursor_canvas = screen_to_canvas([cursor.x, cursor.y], &graph_data.canvas.offset, canvas_center, zoom);
                        hit_node = Some((node.id, [
                            cursor_canvas[0] - node.position[0],
                            cursor_canvas[1] - node.position[1],
                        ]));
                        break;
                    }
                }
                if let Some((id, offset)) = hit_node {
                    graph_data.dragged_node = Some(id);
                    graph_data.drag_offset = offset;
                }
            }
        }
    }

    if primary_released {
        graph_data.dragged_node = None;
    }

    // Move dragged node
    if primary_down {
        if let Some(dragged_id) = graph_data.dragged_node {
            if let Some(cursor) = pointer_pos {
                let cursor_canvas = screen_to_canvas([cursor.x, cursor.y], &graph_data.canvas.offset, canvas_center, zoom);
                let new_pos = [
                    cursor_canvas[0] - graph_data.drag_offset[0],
                    cursor_canvas[1] - graph_data.drag_offset[1],
                ];
                if let Some(&idx) = graph_data.node_index.get(&dragged_id) {
                    graph_data.nodes[idx].position = new_pos;
                }
            }
        }
    }

    // ── Pan (middle or right drag, but only if NOT dragging a node) ──────
    let is_dragging_node = graph_data.dragged_node.is_some();
    if !is_dragging_node {
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            let delta = response.drag_delta();
            graph_data.canvas.offset[0] += delta.x / zoom;
            graph_data.canvas.offset[1] += delta.y / zoom;
        }
    }

    // ── Zoom (scroll wheel) ─────────────────────────────────────────────
    if rect.contains(pointer_pos.unwrap_or_default()) {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.1 {
            let hover = pointer_pos.unwrap_or(rect.center());
            graph_data.canvas.zoom_at(
                [hover.x, hover.y],
                canvas_center,
                scroll,
            );
        }
    }

    // Re-read after potential changes
    let zoom = graph_data.canvas.zoom;
    let offset = graph_data.canvas.offset;

    // ── Background bumps ─────────────────────────────────────────────────
    draw_background_bumps(&painter, rect, &offset, zoom, theme);

    // ── Sub-graph groups ─────────────────────────────────────────────────
    if graph_data.show_sub_graphs {
        for sub_graph in &graph_data.sub_graphs.clone() {
            draw_sub_graph_group(&painter, graph_data, sub_graph, canvas_center, zoom, theme);
        }
    }

    // ── Detect hovered node first (so edges can use it) ─────────────────
    let node_count = graph_data.nodes.len();
    let mut new_hovered: Option<usize> = None;

    for i in 0..node_count {
        let node = &graph_data.nodes[i];
        let screen_pos = canvas_to_screen(node.position, &offset, canvas_center, zoom);
        let node_rect = Rect::from_min_size(
            Pos2::new(screen_pos[0], screen_pos[1]),
            Vec2::new(NODE_WIDTH * zoom, NODE_HEIGHT * zoom),
        );

        if let Some(hover_pos) = pointer_pos {
            if node_rect.contains(hover_pos) && rect.contains(hover_pos) {
                new_hovered = Some(node.id);
            }
        }
    }

    graph_data.hovered_node = new_hovered;
    let hovered = graph_data.hovered_node;

    // ── Edges ────────────────────────────────────────────────────────────
    let edges_clone: Vec<RenderGraphEdge> = graph_data.edges.clone();
    for edge in &edges_clone {
        let highlighted = hovered.map_or(false, |h| edge.from == h || edge.to == h);
        draw_edge(&painter, graph_data, edge, canvas_center, zoom, highlighted, theme);
    }

    // ── Nodes ────────────────────────────────────────────────────────────
    for i in 0..node_count {
        let node = &graph_data.nodes[i];
        let screen_pos = canvas_to_screen(node.position, &offset, canvas_center, zoom);
        let node_rect = Rect::from_min_size(
            Pos2::new(screen_pos[0], screen_pos[1]),
            Vec2::new(NODE_WIDTH * zoom, NODE_HEIGHT * zoom),
        );

        let is_hovered = hovered == Some(node.id);
        let is_dragged = graph_data.dragged_node == Some(node.id);
        draw_node(&painter, node, node_rect, zoom, is_hovered, is_dragged, graph_data.show_timing, theme);
    }

    // ── Tooltip ──────────────────────────────────────────────────────────
    if graph_data.dragged_node.is_none() {
        if let Some(hovered_id) = graph_data.hovered_node {
            if let Some(node) = graph_data.get_node(hovered_id) {
                let name = node.display_name.clone();
                let type_name = node.type_name.clone();
                let category = node.category.label().to_string();
                let gpu_ms = node.gpu_time_ms;
                let sub = node.sub_graph.clone();

                if let Some(hover_pos) = ui.ctx().pointer_hover_pos() {
                    egui::Area::new(ui.id().with("rp_node_tooltip"))
                        .fixed_pos(hover_pos + Vec2::new(12.0, 12.0))
                        .order(egui::Order::Tooltip)
                        .interactable(false)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.label(RichText::new(&name).strong().size(12.0));
                                ui.label(RichText::new(format!("Type: {}", type_name)).size(9.0).color(theme.text.disabled.to_color32()));
                                ui.label(RichText::new(format!("Category: {}", category)).size(10.0));
                                ui.label(RichText::new(format!("Sub-graph: {}", sub)).size(10.0));
                                if gpu_ms > 0.0 {
                                    ui.label(RichText::new(format!("GPU: {:.2} ms", gpu_ms)).size(10.0));
                                }
                            });
                        });
                }
            }
        }
    }

    // ── Not initialized overlay ──────────────────────────────────────────
    if !graph_data.initialized {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Render graph not yet initialized",
            egui::FontId::proportional(14.0),
            theme.text.muted.to_color32(),
        );
    }
}

// =============================================================================
// Coordinate conversions
// =============================================================================

fn canvas_to_screen(
    canvas_pos: [f32; 2],
    offset: &[f32; 2],
    canvas_center: [f32; 2],
    zoom: f32,
) -> [f32; 2] {
    [
        canvas_center[0] + (canvas_pos[0] + offset[0]) * zoom,
        canvas_center[1] + (canvas_pos[1] + offset[1]) * zoom,
    ]
}

fn screen_to_canvas(
    screen_pos: [f32; 2],
    offset: &[f32; 2],
    canvas_center: [f32; 2],
    zoom: f32,
) -> [f32; 2] {
    [
        (screen_pos[0] - canvas_center[0]) / zoom - offset[0],
        (screen_pos[1] - canvas_center[1]) / zoom - offset[1],
    ]
}

// =============================================================================
// Background bumps
// =============================================================================

fn draw_background_bumps(
    painter: &egui::Painter,
    rect: Rect,
    offset: &[f32; 2],
    zoom: f32,
    theme: &Theme,
) {
    let spacing = BUMP_SPACING * zoom;
    if spacing < 6.0 {
        return;
    }

    let base = theme.text.disabled.to_color32();

    // Two layers of bumps for texture
    let bump_light = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 12);
    let bump_dark = Color32::from_rgba_unmultiplied(0, 0, 0, 8);

    let offset_x = (offset[0] * zoom) % spacing;
    let offset_y = (offset[1] * zoom) % spacing;

    let center_x = rect.center().x;
    let center_y = rect.center().y;

    let start_x = center_x + offset_x - ((center_x - rect.min.x) / spacing).ceil() * spacing;
    let start_y = center_y + offset_y - ((center_y - rect.min.y) / spacing).ceil() * spacing;

    let r_large = (1.5 * zoom).max(0.8);
    let r_small = (0.8 * zoom).max(0.4);

    let mut x = start_x;
    while x < rect.max.x + spacing {
        let mut y = start_y;
        while y < rect.max.y + spacing {
            let pos = Pos2::new(x, y);
            if rect.contains(pos) {
                // Main bump (raised highlight)
                painter.circle_filled(pos, r_large, bump_light);
                // Shadow offset (gives the "bump" relief feel)
                let shadow_pos = Pos2::new(x + 0.5 * zoom, y + 0.5 * zoom);
                painter.circle_filled(shadow_pos, r_small, bump_dark);
            }
            y += spacing;
        }
        x += spacing;
    }
}

// =============================================================================
// Sub-graph groups
// =============================================================================

fn draw_sub_graph_group(
    painter: &egui::Painter,
    graph_data: &RenderPipelineGraphData,
    sub_graph: &str,
    canvas_center: [f32; 2],
    zoom: f32,
    theme: &Theme,
) {
    let offset = &graph_data.canvas.offset;

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut count = 0;

    for node in &graph_data.nodes {
        if node.sub_graph == sub_graph {
            min_x = min_x.min(node.position[0]);
            min_y = min_y.min(node.position[1]);
            max_x = max_x.max(node.position[0] + NODE_WIDTH);
            max_y = max_y.max(node.position[1] + NODE_HEIGHT);
            count += 1;
        }
    }

    if count == 0 {
        return;
    }

    let padding = 20.0;
    let label_space = 24.0;
    min_x -= padding;
    min_y -= padding + label_space;
    max_x += padding;
    max_y += padding;

    let tl = canvas_to_screen([min_x, min_y], offset, canvas_center, zoom);
    let br = canvas_to_screen([max_x, max_y], offset, canvas_center, zoom);

    let group_rect = Rect::from_min_max(Pos2::new(tl[0], tl[1]), Pos2::new(br[0], br[1]));

    painter.rect_filled(group_rect, 6.0 * zoom, Color32::from_rgba_unmultiplied(60, 80, 120, 15));
    painter.rect_stroke(group_rect, 6.0 * zoom, Stroke::new(1.0, Color32::from_rgba_unmultiplied(80, 110, 160, 40)), egui::StrokeKind::Inside);

    painter.text(
        Pos2::new(group_rect.min.x + 8.0 * zoom, group_rect.min.y + 4.0 * zoom),
        egui::Align2::LEFT_TOP,
        sub_graph,
        egui::FontId::proportional(11.0 * zoom),
        theme.text.muted.to_color32(),
    );
}

// =============================================================================
// Edges
// =============================================================================

fn draw_edge(
    painter: &egui::Painter,
    graph_data: &RenderPipelineGraphData,
    edge: &RenderGraphEdge,
    canvas_center: [f32; 2],
    zoom: f32,
    highlighted: bool,
    theme: &Theme,
) {
    let offset = &graph_data.canvas.offset;

    let from_node = match graph_data.get_node(edge.from) { Some(n) => n, None => return };
    let to_node = match graph_data.get_node(edge.to) { Some(n) => n, None => return };

    let vertical = graph_data.vertical;

    let (from_pos, to_pos) = if vertical {
        // Vertical: exit from bottom center, enter at top center
        let fp = canvas_to_screen(
            [from_node.position[0] + NODE_WIDTH / 2.0, from_node.position[1] + NODE_HEIGHT],
            offset, canvas_center, zoom,
        );
        let tp = canvas_to_screen(
            [to_node.position[0] + NODE_WIDTH / 2.0, to_node.position[1]],
            offset, canvas_center, zoom,
        );
        (fp, tp)
    } else {
        // Horizontal: exit from right center, enter at left center
        let fp = canvas_to_screen(
            [from_node.position[0] + NODE_WIDTH, from_node.position[1] + NODE_HEIGHT / 2.0],
            offset, canvas_center, zoom,
        );
        let tp = canvas_to_screen(
            [to_node.position[0], to_node.position[1] + NODE_HEIGHT / 2.0],
            offset, canvas_center, zoom,
        );
        (fp, tp)
    };

    let from = Pos2::new(from_pos[0], from_pos[1]);
    let to = Pos2::new(to_pos[0], to_pos[1]);

    let (cp1, cp2) = if vertical {
        let control_dist = ((to.y - from.y).abs() * 0.4).max(30.0 * zoom);
        (Pos2::new(from.x, from.y + control_dist), Pos2::new(to.x, to.y - control_dist))
    } else {
        let control_dist = ((to.x - from.x).abs() * 0.4).max(30.0 * zoom);
        (Pos2::new(from.x + control_dist, from.y), Pos2::new(to.x - control_dist, to.y))
    };

    let (edge_color, stroke_width) = if highlighted {
        (Color32::from_rgb(120, 180, 255), 2.5 * zoom)
    } else {
        let base = theme.text.disabled.to_color32();
        (Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 120), 1.5 * zoom)
    };

    let bezier = egui::epaint::CubicBezierShape::from_points_stroke(
        [from, cp1, cp2, to],
        false,
        Color32::TRANSPARENT,
        Stroke::new(stroke_width, edge_color),
    );
    painter.add(bezier);

    // Arrowhead
    let arrow_size = (if highlighted { 8.0 } else { 6.0 }) * zoom;
    let dir_x = to.x - cp2.x;
    let dir_y = to.y - cp2.y;
    let len = (dir_x * dir_x + dir_y * dir_y).sqrt().max(0.001);
    let nx = dir_x / len;
    let ny = dir_y / len;

    painter.add(egui::Shape::convex_polygon(
        vec![
            to,
            Pos2::new(to.x - arrow_size * nx + arrow_size * 0.5 * ny, to.y - arrow_size * ny - arrow_size * 0.5 * nx),
            Pos2::new(to.x - arrow_size * nx - arrow_size * 0.5 * ny, to.y - arrow_size * ny + arrow_size * 0.5 * nx),
        ],
        edge_color,
        Stroke::NONE,
    ));
}

// =============================================================================
// Nodes
// =============================================================================

fn draw_node(
    painter: &egui::Painter,
    node: &RenderGraphNode,
    rect: Rect,
    zoom: f32,
    is_hovered: bool,
    is_dragged: bool,
    show_timing: bool,
    theme: &Theme,
) {
    let [cr, cg, cb] = node.category.color();

    // Node body
    let body_color = if is_dragged {
        Color32::from_rgb(55, 58, 68)
    } else if is_hovered {
        Color32::from_rgb(50, 54, 62)
    } else {
        Color32::from_rgb(40, 43, 50)
    };
    let corner = 4.0 * zoom;
    painter.rect_filled(rect, corner, body_color);

    // Border / glow
    if is_dragged {
        painter.rect_stroke(rect, corner, Stroke::new(2.0 * zoom, Color32::from_rgb(cr, cg, cb)), egui::StrokeKind::Outside);
    } else if is_hovered {
        painter.rect_stroke(rect, corner, Stroke::new(1.5 * zoom, Color32::from_rgb(cr, cg, cb)), egui::StrokeKind::Outside);
    } else {
        painter.rect_stroke(rect, corner, Stroke::new(0.5, Color32::from_rgba_unmultiplied(100, 100, 110, 60)), egui::StrokeKind::Inside);
    }

    // Drop shadow when dragging
    if is_dragged {
        let shadow_rect = rect.translate(Vec2::new(3.0, 3.0));
        painter.rect_filled(shadow_rect, corner, Color32::from_rgba_unmultiplied(0, 0, 0, 40));
    }

    // Category-colored header bar
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEADER_HEIGHT * zoom));
    let header_color = Color32::from_rgba_unmultiplied(cr, cg, cb, 180);
    painter.rect_filled(header_rect, corner, header_color);
    let bottom_cover = Rect::from_min_size(
        Pos2::new(header_rect.min.x, header_rect.max.y - corner),
        Vec2::new(header_rect.width(), corner),
    );
    painter.rect_filled(bottom_cover, 0.0, header_color);

    // Node name
    painter.text(
        Pos2::new(rect.min.x + 8.0 * zoom, rect.min.y + HEADER_HEIGHT * zoom / 2.0),
        egui::Align2::LEFT_CENTER,
        &node.display_name,
        egui::FontId::proportional(10.0 * zoom),
        Color32::WHITE,
    );

    // Timing bar / category label in body
    if show_timing && node.gpu_time_ms > 0.0 {
        let body_top = rect.min.y + HEADER_HEIGHT * zoom + 4.0 * zoom;
        let bar_height = 8.0 * zoom;
        let bar_max_width = rect.width() - 16.0 * zoom;
        let fill_frac = (node.gpu_time_ms / 16.0).clamp(0.0, 1.0);

        let bar_bg = Rect::from_min_size(Pos2::new(rect.min.x + 8.0 * zoom, body_top), Vec2::new(bar_max_width, bar_height));
        painter.rect_filled(bar_bg, 2.0, Color32::from_rgb(30, 32, 38));
        painter.rect_filled(
            Rect::from_min_size(bar_bg.min, Vec2::new(bar_max_width * fill_frac, bar_height)),
            2.0,
            timing_color(node.gpu_time_ms),
        );

        let label_y = body_top + bar_height + 4.0 * zoom;
        if label_y + 10.0 * zoom < rect.max.y {
            painter.text(
                Pos2::new(rect.min.x + 8.0 * zoom, label_y),
                egui::Align2::LEFT_TOP,
                format!("{:.2} ms", node.gpu_time_ms),
                egui::FontId::proportional(8.0 * zoom),
                theme.text.muted.to_color32(),
            );
        }
    } else if !show_timing {
        let body_center_y = rect.min.y + HEADER_HEIGHT * zoom + (rect.height() - HEADER_HEIGHT * zoom) / 2.0;
        painter.text(
            Pos2::new(rect.min.x + 8.0 * zoom, body_center_y),
            egui::Align2::LEFT_CENTER,
            node.category.label(),
            egui::FontId::proportional(9.0 * zoom),
            theme.text.disabled.to_color32(),
        );
    }
}

fn timing_color(ms: f32) -> Color32 {
    if ms < 2.0 {
        Color32::from_rgb(80, 190, 120)
    } else if ms < 5.0 {
        let t = (ms - 2.0) / 3.0;
        Color32::from_rgb((80.0 + 140.0 * t) as u8, (190.0 - 30.0 * t) as u8, (120.0 - 60.0 * t) as u8)
    } else if ms < 10.0 {
        let t = (ms - 5.0) / 5.0;
        Color32::from_rgb((220.0 + 10.0 * t) as u8, (160.0 - 100.0 * t) as u8, (60.0 - 40.0 * t) as u8)
    } else {
        Color32::from_rgb(230, 60, 20)
    }
}

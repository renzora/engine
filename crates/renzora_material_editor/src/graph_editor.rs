//! Syncs the MaterialGraph data model with the renzora_ui NodeGraphState widget,
//! handles user interactions (move, connect, delete, context menu).

use std::collections::HashMap;

use bevy_egui::egui::{self, Align2, Color32, CursorIcon, Pos2, Sense, Stroke, Vec2, Ui};
use renzora_shader::material::graph::*;
use renzora_shader::material::nodes::{self, MaterialNodeDef};
use renzora_theme::Theme;
use renzora_ui::widgets::node_graph::{
    node_graph, ConnectionDef, NodeDef, NodeGraphConfig, NodeGraphState,
    PinDef, PinDirection, PinShape,
};

// ── Coordinate helpers ──────────────────────────────────────────────────────

pub fn screen_to_canvas(screen: Pos2, offset: [f32; 2], zoom: f32, rect: egui::Rect) -> [f32; 2] {
    let center = rect.center();
    let rel = screen - center;
    [rel.x / zoom - offset[0], rel.y / zoom - offset[1]]
}

// ── Visual mapping ──────────────────────────────────────────────────────────

fn pin_color(pin_type: PinType) -> Color32 {
    match pin_type {
        PinType::Float => Color32::from_rgb(0, 212, 170),
        PinType::Vec2 => Color32::from_rgb(127, 204, 25),
        PinType::Vec3 => Color32::from_rgb(255, 215, 0),
        PinType::Vec4 => Color32::from_rgb(255, 102, 255),
        PinType::Color => Color32::from_rgb(255, 200, 60),
        PinType::Bool => Color32::from_rgb(255, 68, 68),
        PinType::Texture2D | PinType::Sampler => Color32::from_rgb(200, 150, 120),
    }
}

fn header_color(def: &MaterialNodeDef) -> Color32 {
    Color32::from_rgb(def.color[0], def.color[1], def.color[2])
}

pub fn category_icon(category: &str) -> &'static str {
    match category {
        "Input" => egui_phosphor::regular::SIGN_IN,
        "Texture" => egui_phosphor::regular::IMAGE,
        "Math" => egui_phosphor::regular::CALCULATOR,
        "Vector" => egui_phosphor::regular::ARROWS_OUT_CARDINAL,
        "Color" => egui_phosphor::regular::PALETTE,
        "Procedural" => egui_phosphor::regular::WAVES,
        "Animation" => egui_phosphor::regular::TIMER,
        "Utility" => egui_phosphor::regular::WRENCH,
        "Output" => egui_phosphor::regular::SIGN_OUT,
        _ => egui_phosphor::regular::CIRCLE,
    }
}

// ── Sync data model → widget ────────────────────────────────────────────────

/// Map from texture asset path → egui TextureId for node thumbnails.
pub type TextureThumbnailMap = HashMap<String, egui::TextureId>;

pub fn sync_graph_to_widget(graph: &MaterialGraph, thumbnails: &TextureThumbnailMap) -> NodeGraphState {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| {
            let def = nodes::node_def(&node.node_type);
            let pins = def
                .map(|d| (d.pins)())
                .unwrap_or_default()
                .iter()
                .map(|p| PinDef {
                    name: p.name.clone(),
                    label: p.label.clone(),
                    color: pin_color(p.pin_type),
                    shape: PinShape::Circle,
                    direction: match p.direction {
                        PinDir::Input => PinDirection::Input,
                        PinDir::Output => PinDirection::Output,
                    },
                })
                .collect();

            let hdr = def
                .map(|d| header_color(d))
                .unwrap_or(Color32::from_rgb(100, 100, 100));

            // Look up texture thumbnail for texture nodes
            let thumbnail = node.input_values.get("texture").and_then(|v| {
                if let PinValue::TexturePath(ref path) = v {
                    if !path.is_empty() {
                        return thumbnails.get(path).copied();
                    }
                }
                None
            });

            NodeDef {
                id: node.id,
                title: def
                    .map(|d| d.display_name.to_string())
                    .unwrap_or_else(|| node.node_type.clone()),
                header_color: hdr,
                position: node.position,
                pins,
                thumbnail,
            }
        })
        .collect();

    let connections = graph
        .connections
        .iter()
        .map(|c| ConnectionDef {
            from_node: c.from_node,
            from_pin: c.from_pin.clone(),
            to_node: c.to_node,
            to_pin: c.to_pin.clone(),
            color: None,
        })
        .collect();

    NodeGraphState {
        nodes,
        connections,
        offset: [0.0, 0.0],
        zoom: 1.0,
        selected: Vec::new(),
        dragging: None,
        connecting: None,
        box_select: None,
    }
}

// ── Editor state ────────────────────────────────────────────────────────────

pub struct GraphEditorState {
    pub widget_state: NodeGraphState,
    pub needs_sync: bool,
    pub context_menu_open: bool,
    pub context_menu_pos: Pos2,
    pub context_menu_age: u32,
    pub open_submenu: Option<usize>,
    pub submenu_rect: Option<egui::Rect>,
    pub canvas_rect: Option<egui::Rect>,
}

impl Default for GraphEditorState {
    fn default() -> Self {
        Self {
            widget_state: NodeGraphState::default(),
            needs_sync: true,
            context_menu_open: false,
            context_menu_pos: Pos2::ZERO,
            context_menu_age: 0,
            open_submenu: None,
            submenu_rect: None,
            canvas_rect: None,
        }
    }
}

// ── Main render function ────────────────────────────────────────────────────

/// Render the graph editor and return true if the graph was modified.
pub fn render_graph_editor(
    ui: &mut Ui,
    graph: &mut MaterialGraph,
    state: &mut GraphEditorState,
    theme: &Theme,
    thumbnails: &TextureThumbnailMap,
) -> bool {
    let mut modified = false;

    // Detect external graph change
    if state.widget_state.nodes.len() != graph.nodes.len() {
        state.needs_sync = true;
    }

    // Initial / forced sync
    if state.needs_sync {
        state.widget_state = sync_graph_to_widget(graph, thumbnails);
        state.needs_sync = false;

        // Auto-fit view
        if !graph.nodes.is_empty() {
            let node_w = 140.0_f32;
            let node_h_est = 120.0_f32;
            let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
            let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
            for node in &graph.nodes {
                min_x = min_x.min(node.position[0]);
                max_x = max_x.max(node.position[0] + node_w);
                min_y = min_y.min(node.position[1]);
                max_y = max_y.max(node.position[1] + node_h_est);
            }
            state.widget_state.offset = [-(min_x + max_x) / 2.0, -(min_y + max_y) / 2.0];

            if let Some(cr) = state.canvas_rect {
                let graph_w = max_x - min_x + 100.0;
                let graph_h = max_y - min_y + 100.0;
                let fit_zoom = (cr.width() / graph_w).min(cr.height() / graph_h).clamp(0.25, 1.5);
                state.widget_state.zoom = fit_zoom;
            }
        }
    }

    // ── Render widget ──
    let config = NodeGraphConfig {
        grid_spacing: 20.0,
        node_width: 140.0,
        header_height: 24.0,
        pin_height: 22.0,
        pin_radius: 5.0,
        corner_radius: 4.0,
        node_bg: theme.surfaces.panel.to_color32(),
        node_border: theme.widgets.border.to_color32(),
        selected_border: Color32::from_rgb(100, 150, 255),
        grid_dot: Color32::from_rgb(35, 35, 35),
        canvas_bg: Color32::from_rgb(25, 25, 25),
        text_color: theme.text.primary.to_color32(),
        text_muted: theme.text.muted.to_color32(),
        connection_width: 2.0,
        selection_fill: Color32::from_rgba_premultiplied(100, 150, 255, 30),
        selection_stroke: Color32::from_rgba_premultiplied(100, 150, 255, 128),
    };

    let resp = node_graph(
        ui,
        egui::Id::new("material_node_graph"),
        &mut state.widget_state,
        &config,
    );

    if let Some(ref canvas_resp) = resp.canvas_response {
        state.canvas_rect = Some(canvas_resp.rect);
    }

    // ── Sync widget changes back to data model ──

    if let Some(node_id) = resp.node_moved {
        if let Some(wn) = state.widget_state.nodes.iter().find(|n| n.id == node_id) {
            if let Some(dn) = graph.get_node_mut(node_id) {
                dn.position = wn.position;
                modified = true;
            }
        }
    }

    if let Some((out_pin, in_pin)) = resp.connection_made {
        graph.connect(out_pin.node, &out_pin.name, in_pin.node, &in_pin.name);
        modified = true;
    }

    if let Some((node_id, pin_name)) = resp.connection_removed {
        graph.disconnect(node_id, &pin_name);
        // Resync connections
        state.widget_state.connections = graph
            .connections
            .iter()
            .map(|c| ConnectionDef {
                from_node: c.from_node,
                from_pin: c.from_pin.clone(),
                to_node: c.to_node,
                to_pin: c.to_pin.clone(),
                color: None,
            })
            .collect();
        modified = true;
    }

    if !resp.nodes_deleted.is_empty() {
        for id in &resp.nodes_deleted {
            graph.remove_node(*id);
        }
        state.needs_sync = true;
        modified = true;
    }

    // ── Context menu (right-click on canvas) ──

    if let Some(ref canvas_resp) = resp.canvas_response {
        if canvas_resp.secondary_clicked() && !resp.right_click_handled {
            state.context_menu_open = true;
            state.context_menu_pos = canvas_resp.interact_pointer_pos().unwrap_or(Pos2::ZERO);
            state.context_menu_age = 0;
        }
    }

    if state.context_menu_open {
        modified |= render_context_menu(ui, graph, state, theme);
    }

    modified
}

// ── Context menu ────────────────────────────────────────────────────────────

fn render_context_menu(
    ui: &mut Ui,
    graph: &mut MaterialGraph,
    state: &mut GraphEditorState,
    theme: &Theme,
) -> bool {
    let mut modified = false;
    let spawn_pos = state.context_menu_pos;
    let categories = nodes::categories();
    let row_height = 26.0_f32;

    let menu_frame = egui::Frame::new()
        .fill(theme.surfaces.popup.to_color32())
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(4))
        .stroke(Stroke::new(1.0, theme.widgets.border.to_color32()));

    // ── Main menu: category list ──
    let mut cat_rects: Vec<egui::Rect> = Vec::new();
    let area_resp = egui::Area::new(egui::Id::new("mat_node_ctx_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(spawn_pos)
        .constrain(true)
        .interactable(true)
        .show(ui.ctx(), |ui| {
            menu_frame.show(ui, |ui| {
                ui.set_width(180.0);
                ui.spacing_mut().item_spacing.y = 0.0;

                for (cat_idx, &category) in categories.iter().enumerate() {
                    // Skip output nodes from the add menu
                    if category == "Output" {
                        continue;
                    }

                    let icon = category_icon(category);
                    let is_open = state.open_submenu == Some(cat_idx);

                    let (row_rect, row_resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        Sense::click(),
                    );
                    cat_rects.push(row_rect);
                    let painter = ui.painter();

                    let bg = if cat_idx % 2 == 0 {
                        theme.panels.inspector_row_even.to_color32()
                    } else {
                        theme.panels.inspector_row_odd.to_color32()
                    };
                    painter.rect_filled(row_rect, 0.0, bg);

                    if row_resp.hovered() || is_open {
                        painter.rect_filled(
                            row_rect, 0.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 15),
                        );
                    }
                    if row_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    let base_x = row_rect.min.x + 6.0;
                    let cy = row_rect.center().y;

                    let node_def = nodes::nodes_in_category(category);
                    let accent = if let Some(first) = node_def.first() {
                        header_color(first)
                    } else {
                        Color32::GRAY
                    };

                    painter.text(
                        Pos2::new(base_x, cy), Align2::LEFT_CENTER,
                        icon, egui::FontId::proportional(14.0), accent,
                    );
                    painter.text(
                        Pos2::new(base_x + 20.0, cy), Align2::LEFT_CENTER,
                        category, egui::FontId::proportional(13.0),
                        theme.text.primary.to_color32(),
                    );
                    painter.text(
                        Pos2::new(row_rect.max.x - 14.0, cy), Align2::CENTER_CENTER,
                        egui_phosphor::regular::CARET_RIGHT,
                        egui::FontId::proportional(12.0),
                        Color32::from_rgb(120, 120, 130),
                    );

                    if row_resp.hovered() {
                        state.open_submenu = Some(cat_idx);
                    }
                }
            });
        });

    let main_rect = area_resp.response.rect;

    // ── Submenu: nodes in hovered category ──
    let mut submenu_rect_this_frame: Option<egui::Rect> = None;

    if let Some(cat_idx) = state.open_submenu {
        let filtered_categories: Vec<&&str> = categories.iter().filter(|c| **c != "Output").collect();
        if let Some(&&category) = filtered_categories.get(cat_idx) {
            let node_defs = nodes::nodes_in_category(category);

            let sub_y = cat_rects.get(cat_idx).map_or(spawn_pos.y, |r| r.min.y);
            let sub_pos = Pos2::new(main_rect.max.x + 2.0, sub_y);

            let sub_resp = egui::Area::new(egui::Id::new("mat_node_ctx_submenu"))
                .order(egui::Order::Foreground)
                .fixed_pos(sub_pos)
                .constrain(true)
                .interactable(true)
                .show(ui.ctx(), |ui| {
                    menu_frame.show(ui, |ui| {
                        ui.set_width(180.0);
                        ui.spacing_mut().item_spacing.y = 0.0;

                        for (i_idx, def) in node_defs.iter().enumerate() {
                            let (row_rect, row_resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), row_height),
                                Sense::click(),
                            );
                            let painter = ui.painter();

                            let bg = if i_idx % 2 == 0 {
                                theme.panels.inspector_row_even.to_color32()
                            } else {
                                theme.panels.inspector_row_odd.to_color32()
                            };
                            painter.rect_filled(row_rect, 0.0, bg);

                            if row_resp.hovered() {
                                painter.rect_filled(
                                    row_rect, 0.0,
                                    Color32::from_rgba_unmultiplied(255, 255, 255, 15),
                                );
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }

                            let item_x = row_rect.min.x + 6.0;
                            let cy = row_rect.center().y;
                            let accent = header_color(def);

                            painter.text(
                                Pos2::new(item_x, cy), Align2::LEFT_CENTER,
                                category_icon(category),
                                egui::FontId::proportional(13.0), accent,
                            );
                            painter.text(
                                Pos2::new(item_x + 20.0, cy), Align2::LEFT_CENTER,
                                def.display_name,
                                egui::FontId::proportional(12.0),
                                theme.text.primary.to_color32(),
                            );

                            if row_resp.clicked() {
                                let canvas_pos = if let Some(cr) = state.canvas_rect {
                                    screen_to_canvas(
                                        spawn_pos,
                                        state.widget_state.offset,
                                        state.widget_state.zoom,
                                        cr,
                                    )
                                } else {
                                    [0.0, 0.0]
                                };
                                graph.add_node(def.node_type, canvas_pos);
                                state.needs_sync = true;
                                modified = true;
                                state.context_menu_open = false;
                            }
                        }
                    });
                });

            submenu_rect_this_frame = Some(sub_resp.response.rect);
        }
    }

    state.submenu_rect = submenu_rect_this_frame;
    state.context_menu_age += 1;

    // Close logic
    let pointer_in_menus = ui.input(|i| {
        if let Some(pos) = i.pointer.hover_pos() {
            if main_rect.contains(pos) {
                return true;
            }
            if let Some(sub_rect) = submenu_rect_this_frame {
                if sub_rect.contains(pos) {
                    return true;
                }
                let bridge = egui::Rect::from_min_max(
                    Pos2::new(main_rect.max.x, sub_rect.min.y.min(main_rect.min.y)),
                    Pos2::new(sub_rect.min.x, sub_rect.max.y.max(main_rect.max.y)),
                );
                if bridge.contains(pos) {
                    return true;
                }
            }
            false
        } else {
            false
        }
    });

    if state.context_menu_age > 3 && !pointer_in_menus {
        state.open_submenu = None;
    }

    if state.context_menu_age > 3 {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.context_menu_open = false;
            state.open_submenu = None;
        } else if ui.input(|i| i.pointer.any_click()) && !pointer_in_menus {
            state.context_menu_open = false;
            state.open_submenu = None;
        }
    }

    modified
}

use bevy_egui::egui::{self, Align2, Color32, CursorIcon, Pos2, Sense, Stroke, Ui, Vec2};
use renzora_hanabi::node_graph::*;
use renzora_theme::Theme;
use renzora_ui::widgets::node_graph::{
    node_graph, ConnectionDef, NodeDef, NodeGraphConfig, NodeGraphState,
    PinDef, PinDirection, PinShape,
};

/// Convert screen-space position to canvas-space (mirrors render::screen_to_canvas).
pub fn screen_to_canvas_pub(screen: Pos2, offset: [f32; 2], zoom: f32, rect: egui::Rect) -> [f32; 2] {
    screen_to_canvas(screen, offset, zoom, rect)
}

fn screen_to_canvas(screen: Pos2, offset: [f32; 2], zoom: f32, rect: egui::Rect) -> [f32; 2] {
    let center = rect.center();
    let rel = screen - center;
    [rel.x / zoom - offset[0], rel.y / zoom - offset[1]]
}

/// Pin color by type
fn pin_color(t: PinType) -> Color32 {
    match t {
        PinType::Float => Color32::from_rgb(0, 212, 170),
        PinType::Vec2 => Color32::from_rgb(127, 204, 25),
        PinType::Vec3 => Color32::from_rgb(255, 215, 0),
        PinType::Vec4 => Color32::from_rgb(255, 102, 255),
        PinType::Bool => Color32::from_rgb(255, 68, 68),
    }
}

/// Header color by category
fn header_color(category: &str) -> Color32 {
    match category {
        "Emitter" => Color32::from_rgb(60, 60, 60),
        "Spawn" => Color32::from_rgb(50, 100, 200),
        "Init" => Color32::from_rgb(50, 150, 50),
        "Update" => Color32::from_rgb(200, 100, 50),
        "Render" => Color32::from_rgb(150, 50, 200),
        "Math" => Color32::from_rgb(100, 100, 100),
        "Constants" => Color32::from_rgb(80, 80, 80),
        _ => Color32::from_rgb(100, 100, 100),
    }
}

/// Icon for a node category
pub fn category_icon(category: &str) -> &'static str {
    match category {
        "Spawn" => egui_phosphor::regular::SPARKLE,
        "Init" => egui_phosphor::regular::ARROWS_OUT,
        "Update" => egui_phosphor::regular::WIND,
        "Render" => egui_phosphor::regular::PALETTE,
        "Math" => egui_phosphor::regular::CALCULATOR,
        "Constants" => egui_phosphor::regular::HASH,
        _ => egui_phosphor::regular::CIRCLE,
    }
}

/// Convert our PinDir to widget PinDirection
fn pin_direction(dir: PinDir) -> PinDirection {
    match dir {
        PinDir::Input => PinDirection::Input,
        PinDir::Output => PinDirection::Output,
    }
}

/// Sync the ParticleNodeGraph data model to the widget's NodeGraphState
pub fn sync_graph_to_widget(graph: &ParticleNodeGraph) -> NodeGraphState {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| {
            let pins = node
                .node_type
                .pins()
                .iter()
                .map(|p| PinDef {
                    name: p.name.clone(),
                    label: p.label.clone(),
                    color: pin_color(p.pin_type),
                    shape: PinShape::Circle,
                    direction: pin_direction(p.direction),
                })
                .collect();

            NodeDef {
                id: node.id,
                title: node.node_type.display_name().to_string(),
                header_color: header_color(node.node_type.category()),
                position: node.position,
                pins,
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

/// State for the graph editor (persists across frames)
pub struct GraphEditorState {
    pub widget_state: NodeGraphState,
    pub needs_sync: bool,
    pub context_menu_open: bool,
    pub context_menu_pos: egui::Pos2,
    pub context_menu_age: u32,
    /// Which category submenu is currently open (by index)
    pub open_submenu: Option<usize>,
    /// Rect of the submenu Area (for hover-gap bridging)
    pub submenu_rect: Option<egui::Rect>,
    /// Canvas rect from the node graph widget (for coordinate conversion)
    pub canvas_rect: Option<egui::Rect>,
}

impl Default for GraphEditorState {
    fn default() -> Self {
        Self {
            widget_state: NodeGraphState::default(),
            needs_sync: true,
            context_menu_open: false,
            context_menu_pos: egui::Pos2::ZERO,
            context_menu_age: 0,
            open_submenu: None,
            submenu_rect: None,
            canvas_rect: None,
        }
    }
}

/// Render the graph editor and return true if the graph was modified
pub fn render_graph_editor(
    ui: &mut Ui,
    graph: &mut ParticleNodeGraph,
    state: &mut GraphEditorState,
    theme: &renzora_theme::Theme,
) -> bool {
    let mut modified = false;

    // Detect if graph data changed externally (e.g. mode switch regenerated it)
    if state.widget_state.nodes.len() != graph.nodes.len() {
        state.needs_sync = true;
    }

    // Initial sync
    if state.needs_sync {
        state.widget_state = sync_graph_to_widget(graph);
        state.needs_sync = false;

        // Auto-fit: center view on all nodes and adjust zoom to fit
        if !graph.nodes.is_empty() {
            let node_w = 180.0_f32;
            let node_h_est = 120.0_f32; // rough average node height
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_y = f32::MAX;
            let mut max_y = f32::MIN;
            for node in &graph.nodes {
                min_x = min_x.min(node.position[0]);
                max_x = max_x.max(node.position[0] + node_w);
                min_y = min_y.min(node.position[1]);
                max_y = max_y.max(node.position[1] + node_h_est);
            }
            let center_x = (min_x + max_x) / 2.0;
            let center_y = (min_y + max_y) / 2.0;
            state.widget_state.offset = [-center_x, -center_y];

            // Fit zoom: try to fit the bounding box into the available canvas
            if let Some(cr) = state.canvas_rect {
                let graph_w = max_x - min_x + 100.0; // padding
                let graph_h = max_y - min_y + 100.0;
                let zoom_x = cr.width() / graph_w;
                let zoom_y = cr.height() / graph_h;
                let fit_zoom = zoom_x.min(zoom_y).clamp(0.25, 1.5);
                state.widget_state.zoom = fit_zoom;
            }
        }
    }

    // Render node graph widget
    let config = NodeGraphConfig {
        grid_spacing: 20.0,
        node_width: 180.0,
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

    let graph_response = node_graph(
        ui,
        egui::Id::new("particle_node_graph"),
        &mut state.widget_state,
        &config,
    );

    // Store canvas rect for coordinate conversion
    if let Some(ref canvas_resp) = graph_response.canvas_response {
        state.canvas_rect = Some(canvas_resp.rect);
    }

    // Handle graph response - sync widget changes back to data model
    if let Some(node_id) = graph_response.node_moved {
        // Sync position back
        if let Some(widget_node) = state.widget_state.nodes.iter().find(|n| n.id == node_id) {
            if let Some(data_node) = graph.get_node_mut(node_id) {
                data_node.position = widget_node.position;
                modified = true;
            }
        }
    }

    if let Some((out_pin, in_pin)) = graph_response.connection_made {
        graph.connect(out_pin.node, &out_pin.name, in_pin.node, &in_pin.name);
        modified = true;
    }

    if let Some((node_id, pin_name)) = graph_response.connection_removed {
        graph.disconnect(node_id, &pin_name);
        // Sync connections back
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

    if !graph_response.nodes_deleted.is_empty() {
        for id in &graph_response.nodes_deleted {
            // Don't allow deleting the Emitter node
            if graph
                .get_node(*id)
                .map_or(false, |n| n.node_type == ParticleNodeType::Emitter)
            {
                continue;
            }
            graph.remove_node(*id);
        }
        state.needs_sync = true;
        modified = true;
    }

    // Context menu — right-click on empty canvas opens, tracks via bool flag
    if let Some(ref canvas_resp) = graph_response.canvas_response {
        if canvas_resp.secondary_clicked() && !graph_response.right_click_handled {
            state.context_menu_open = true;
            state.context_menu_pos = canvas_resp.interact_pointer_pos().unwrap_or(egui::Pos2::ZERO);
            state.context_menu_age = 0;
        }
    }

    if state.context_menu_open {
        let spawn_pos = state.context_menu_pos;
        let categories = ParticleNodeType::categories();
        let row_height = 26.0_f32;
        let menu_frame = egui::Frame::new()
            .fill(theme.surfaces.popup.to_color32())
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(4))
            .stroke(Stroke::new(1.0, theme.widgets.border.to_color32()));

        // ── Main menu: category list ──
        let mut cat_rects: Vec<egui::Rect> = Vec::new();
        let area_resp = egui::Area::new(egui::Id::new("node_add_ctx_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(spawn_pos)
            .constrain(true)
            .interactable(true)
            .show(ui.ctx(), |ui| {
                menu_frame.show(ui, |ui| {
                    ui.set_width(180.0);
                    ui.spacing_mut().item_spacing.y = 0.0;

                    for (cat_idx, &category) in categories.iter().enumerate() {
                        let icon = category_icon(category);
                        let accent = header_color(category);
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

                        // Icon + category label
                        painter.text(
                            Pos2::new(base_x, cy), Align2::LEFT_CENTER,
                            icon, egui::FontId::proportional(14.0), accent,
                        );
                        painter.text(
                            Pos2::new(base_x + 20.0, cy), Align2::LEFT_CENTER,
                            category, egui::FontId::proportional(13.0),
                            theme.text.primary.to_color32(),
                        );

                        // Right arrow indicator
                        painter.text(
                            Pos2::new(row_rect.max.x - 14.0, cy), Align2::CENTER_CENTER,
                            egui_phosphor::regular::CARET_RIGHT,
                            egui::FontId::proportional(12.0),
                            Color32::from_rgb(120, 120, 130),
                        );

                        // Open submenu on hover
                        if row_resp.hovered() {
                            state.open_submenu = Some(cat_idx);
                        }
                    }
                });
            });

        let main_rect = area_resp.response.rect;

        // ── Submenu: node list for hovered category ──
        let mut submenu_rect_this_frame: Option<egui::Rect> = None;

        if let Some(cat_idx) = state.open_submenu {
            if let Some(&category) = categories.get(cat_idx) {
                let icon = category_icon(category);
                let accent = header_color(category);
                let nodes = ParticleNodeType::nodes_in_category(category);

                // Position submenu to the right of the main menu, aligned with the category row
                let sub_y = cat_rects.get(cat_idx).map_or(spawn_pos.y, |r| r.min.y);
                let sub_pos = Pos2::new(main_rect.max.x + 2.0, sub_y);

                let sub_resp = egui::Area::new(egui::Id::new("node_add_ctx_submenu"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(sub_pos)
                    .constrain(true)
                    .interactable(true)
                    .show(ui.ctx(), |ui| {
                        menu_frame.show(ui, |ui| {
                            ui.set_width(180.0);
                            ui.spacing_mut().item_spacing.y = 0.0;

                            for (i_idx, node_type) in nodes.iter().enumerate() {
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

                                painter.text(
                                    Pos2::new(item_x, cy), Align2::LEFT_CENTER,
                                    icon, egui::FontId::proportional(13.0), accent,
                                );
                                painter.text(
                                    Pos2::new(item_x + 20.0, cy), Align2::LEFT_CENTER,
                                    node_type.display_name(),
                                    egui::FontId::proportional(12.0),
                                    theme.text.primary.to_color32(),
                                );

                                if row_resp.clicked() {
                                    let canvas_pos = if let Some(cr) = state.canvas_rect {
                                        screen_to_canvas(spawn_pos, state.widget_state.offset, state.widget_state.zoom, cr)
                                    } else {
                                        [0.0, 0.0]
                                    };
                                    let new_id = graph.add_node(*node_type, canvas_pos);
                                    let emitter_pin = match node_type.category() {
                                        "Spawn" => Some("spawn"),
                                        "Init" => Some("init"),
                                        "Update" => Some("update"),
                                        "Render" => Some("render"),
                                        _ => None,
                                    };
                                    if let Some(pin) = emitter_pin {
                                        if let Some(emitter) = graph.nodes.iter().find(|n| n.node_type == ParticleNodeType::Emitter) {
                                            let eid = emitter.id;
                                            graph.connect(new_id, "module", eid, pin);
                                        }
                                    }
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

        // Check if pointer is in the main menu, submenu, or the gap between them
        let pointer_in_menus = ui.input(|i| {
            if let Some(pos) = i.pointer.hover_pos() {
                if main_rect.contains(pos) {
                    return true;
                }
                if let Some(sub_rect) = submenu_rect_this_frame {
                    if sub_rect.contains(pos) {
                        return true;
                    }
                    // Bridge the gap: allow pointer in the horizontal strip between menus
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

        // Close submenu if pointer left all menu areas
        if state.context_menu_age > 3 && !pointer_in_menus {
            state.open_submenu = None;
        }

        // Close on click outside or escape
        if state.context_menu_age > 3 {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                state.context_menu_open = false;
                state.open_submenu = None;
            } else if ui.input(|i| i.pointer.any_click()) && !pointer_in_menus {
                state.context_menu_open = false;
                state.open_submenu = None;
            }
        }
    }

    modified
}

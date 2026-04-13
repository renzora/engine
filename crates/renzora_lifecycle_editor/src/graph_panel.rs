//! Lifecycle Graph Panel — the main editor panel with toolbar + node graph canvas.

use std::cell::RefCell;

use bevy::prelude::*;
use renzora::bevy_egui::egui::{self, Align2, Color32, CursorIcon, Pos2, RichText, Sense, Stroke, Vec2};
use renzora::egui_phosphor::regular::{
    CROSSHAIR, FLOPPY_DISK, FLOW_ARROW, MAGNIFYING_GLASS_MINUS, MAGNIFYING_GLASS_PLUS, PLUS,
};

use renzora_blueprint::graph::{BlueprintConnection, BlueprintNodeDef, PinDir, PinType};
use renzora::editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_lifecycle::graph::LifecycleGraph;
use renzora_lifecycle::nodes;
use renzora::theme::{Theme, ThemeManager};
use renzora_ui::widgets::node_graph::{
    node_graph, ConnectionDef, NodeDef, NodeGraphConfig, NodeGraphState, PinDef, PinDirection,
    PinShape,
};

// ── State ───────────────────────────────────────────────────────────────────

pub struct GraphEditorState {
    pub widget_state: NodeGraphState,
    pub needs_sync: bool,
    pub canvas_rect: Option<egui::Rect>,
    pub context_menu_open: bool,
    pub context_menu_pos: Pos2,
    pub context_menu_age: u32,
    pub open_submenu: Option<usize>,
    pub submenu_rect: Option<egui::Rect>,
}

impl Default for GraphEditorState {
    fn default() -> Self {
        Self {
            widget_state: NodeGraphState::default(),
            needs_sync: true,
            canvas_rect: None,
            context_menu_open: false,
            context_menu_pos: Pos2::ZERO,
            context_menu_age: 0,
            open_submenu: None,
            submenu_rect: None,
        }
    }
}

thread_local! {
    static GRAPH_EDITOR_STATE: RefCell<GraphEditorState> = RefCell::new(GraphEditorState::default());
}

// ── Visual helpers ──────────────────────────────────────────────────────────

fn pin_color(pin_type: PinType) -> Color32 {
    match pin_type {
        PinType::Exec => Color32::WHITE,
        PinType::Float => Color32::from_rgb(0, 212, 170),
        PinType::Int => Color32::from_rgb(68, 206, 246),
        PinType::Bool => Color32::from_rgb(255, 68, 68),
        PinType::String => Color32::from_rgb(255, 100, 255),
        PinType::Vec2 => Color32::from_rgb(127, 204, 25),
        PinType::Vec3 => Color32::from_rgb(255, 215, 0),
        PinType::Color => Color32::from_rgb(255, 200, 60),
        PinType::Entity => Color32::from_rgb(80, 200, 180),
        PinType::Any => Color32::from_rgb(180, 180, 180),
    }
}

fn pin_shape_for(pin_type: PinType) -> PinShape {
    match pin_type {
        PinType::Exec => PinShape::Triangle,
        _ => PinShape::Circle,
    }
}

fn header_color(def: &BlueprintNodeDef) -> Color32 {
    Color32::from_rgb(def.color[0], def.color[1], def.color[2])
}

fn screen_to_canvas(screen: Pos2, offset: [f32; 2], zoom: f32, rect: egui::Rect) -> [f32; 2] {
    let center = rect.center();
    let rel = screen - center;
    [rel.x / zoom - offset[0], rel.y / zoom - offset[1]]
}

fn is_exec_connection(graph: &LifecycleGraph, conn: &BlueprintConnection) -> bool {
    if let Some(node) = graph.get_node(conn.from_node) {
        if let Some(def) = nodes::node_def(&node.node_type) {
            return (def.pins)()
                .iter()
                .any(|p| p.name == conn.from_pin && p.pin_type == PinType::Exec);
        }
    }
    false
}

fn sync_graph_to_widget(graph: &LifecycleGraph) -> NodeGraphState {
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
                    shape: pin_shape_for(p.pin_type),
                    direction: match p.direction {
                        PinDir::Input => PinDirection::Input,
                        PinDir::Output => PinDirection::Output,
                    },
                })
                .collect();

            let hdr = def
                .map(|d| header_color(d))
                .unwrap_or(Color32::from_rgb(100, 100, 100));

            NodeDef {
                id: node.id,
                title: def
                    .map(|d| d.display_name.to_string())
                    .unwrap_or_else(|| node.node_type.clone()),
                header_color: hdr,
                position: node.position,
                pins,
                thumbnail: None,
            }
        })
        .collect();

    let connections = graph
        .connections
        .iter()
        .map(|c| {
            let color = if is_exec_connection(graph, c) {
                Some(Color32::WHITE)
            } else {
                None
            };
            ConnectionDef {
                from_node: c.from_node,
                from_pin: c.from_pin.clone(),
                to_node: c.to_node,
                to_pin: c.to_pin.clone(),
                color,
            }
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

fn resync_connections(graph: &LifecycleGraph) -> Vec<ConnectionDef> {
    graph
        .connections
        .iter()
        .map(|c| {
            let color = if is_exec_connection(graph, c) {
                Some(Color32::WHITE)
            } else {
                None
            };
            ConnectionDef {
                from_node: c.from_node,
                from_pin: c.from_pin.clone(),
                to_node: c.to_node,
                to_pin: c.to_pin.clone(),
                color,
            }
        })
        .collect()
}

fn category_icon(category: &str) -> &'static str {
    match category {
        "Lifecycle" => renzora::egui_phosphor::regular::ARROWS_CLOCKWISE,
        "Flow" => renzora::egui_phosphor::regular::FLOW_ARROW,
        "Math" => renzora::egui_phosphor::regular::CALCULATOR,
        "String" => renzora::egui_phosphor::regular::TEXT_AA,
        "Convert" => renzora::egui_phosphor::regular::SWAP,
        _ => renzora::egui_phosphor::regular::CIRCLE,
    }
}

// ── Panel ───────────────────────────────────────────────────────────────────

pub struct LifecycleGraphPanel;

impl EditorPanel for LifecycleGraphPanel {
    fn id(&self) -> &str {
        "lifecycle_graph"
    }

    fn title(&self) -> &str {
        "Lifecycle Graph"
    }

    fn icon(&self) -> Option<&str> {
        Some(FLOW_ARROW)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let cmds = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };
        let Some(lifecycle_graph) = world.get_resource::<LifecycleGraph>() else {
            return;
        };

        let mut graph = lifecycle_graph.clone();

        // ── Toolbar ──
        let toolbar_modified = GRAPH_EDITOR_STATE.with(|cell| {
            let mut gs = cell.borrow_mut();
            render_toolbar(ui, &mut graph, &mut gs, cmds, &theme, world)
        });

        ui.separator();

        // ── Graph canvas ──
        let graph_modified = GRAPH_EDITOR_STATE.with(|cell| {
            let mut gs = cell.borrow_mut();

            if gs.widget_state.nodes.len() != graph.nodes.len() {
                gs.needs_sync = true;
            }
            if gs.needs_sync {
                gs.widget_state = sync_graph_to_widget(&graph);
                gs.needs_sync = false;
            }

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

            let resp = node_graph(
                ui,
                egui::Id::new("lifecycle_node_graph"),
                &mut gs.widget_state,
                &config,
            );

            if let Some(ref canvas_resp) = resp.canvas_response {
                gs.canvas_rect = Some(canvas_resp.rect);
            }

            let mut modified = false;

            // Node moves
            if let Some(node_id) = resp.node_moved {
                if let Some(wn) = gs.widget_state.nodes.iter().find(|n| n.id == node_id) {
                    if let Some(dn) = graph.get_node_mut(node_id) {
                        dn.position = wn.position;
                        modified = true;
                    }
                }
            }

            // New connections
            if let Some((out_pin, in_pin)) = resp.connection_made {
                graph.connect(out_pin.node, &out_pin.name, in_pin.node, &in_pin.name);
                modified = true;
            }

            // Removed connections
            if let Some((node_id, pin_name)) = resp.connection_removed {
                graph.disconnect(node_id, &pin_name);
                gs.widget_state.connections = resync_connections(&graph);
                modified = true;
            }

            // Deleted nodes
            if !resp.nodes_deleted.is_empty() {
                for id in &resp.nodes_deleted {
                    graph.remove_node(*id);
                }
                gs.needs_sync = true;
                modified = true;
            }

            // Context menu (right-click on canvas)
            if let Some(ref canvas_resp) = resp.canvas_response {
                if canvas_resp.secondary_clicked() && !resp.right_click_handled {
                    gs.context_menu_open = true;
                    gs.context_menu_pos = canvas_resp.interact_pointer_pos().unwrap_or(Pos2::ZERO);
                    gs.context_menu_age = 0;
                }
            }

            if gs.context_menu_open {
                modified |= render_context_menu(ui, &mut graph, &mut *gs, &theme);
            }

            modified
        });

        // Sync selection
        let selected = GRAPH_EDITOR_STATE.with(|cell| {
            let gs = cell.borrow();
            if gs.widget_state.selected.len() == 1 {
                Some(gs.widget_state.selected[0])
            } else {
                None
            }
        });

        let editor_state = world.get_resource::<crate::LifecycleEditorState>();
        let prev_selected = editor_state.map(|s| s.selected_node).unwrap_or(None);
        if selected != prev_selected {
            let sel = selected;
            cmds.push(move |world: &mut World| {
                world.resource_mut::<crate::LifecycleEditorState>().selected_node = sel;
            });
        }

        if graph_modified || toolbar_modified {
            cmds.push(move |world: &mut World| {
                world.insert_resource(graph);
            });
        }
    }

    fn closable(&self) -> bool {
        true
    }
}

// ── Context menu ────────────────────────────────────────────────────────────

fn render_context_menu(
    ui: &mut egui::Ui,
    graph: &mut LifecycleGraph,
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
    let area_resp = egui::Area::new(egui::Id::new("lc_node_ctx_menu"))
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

                    let node_defs = nodes::nodes_in_category(category);
                    let accent = if let Some(first) = node_defs.first() {
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
                        renzora::egui_phosphor::regular::CARET_RIGHT,
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
        if let Some(&&category) = categories.iter().collect::<Vec<_>>().get(cat_idx) {
            let node_defs = nodes::nodes_in_category(category);

            let sub_y = cat_rects.get(cat_idx).map_or(spawn_pos.y, |r| r.min.y);
            let sub_pos = Pos2::new(main_rect.max.x + 2.0, sub_y);

            let sub_resp = egui::Area::new(egui::Id::new("lc_node_ctx_submenu"))
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
                // Bridge rect between main and submenu
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

// ── Toolbar ─────────────────────────────────────────────────────────────────

fn render_toolbar(
    ui: &mut egui::Ui,
    graph: &mut LifecycleGraph,
    state: &mut GraphEditorState,
    cmds: &EditorCommands,
    theme: &Theme,
    world: &World,
) -> bool {
    let mut modified = false;
    let text_color = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.style_mut().spacing.button_padding = egui::vec2(6.0, 2.0);

        // ── Save ──
        if ui
            .add(egui::Button::new(
                RichText::new(format!("{FLOPPY_DISK} Save"))
                    .size(12.0)
                    .color(text_color),
            ))
            .clicked()
        {
            if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
                let path = project.path.join("lifecycle.json");
                let _ = renzora_lifecycle::io::save_lifecycle(&path, graph);
            }
        }

        ui.separator();

        // ── Add Node ──
        let add_btn = ui.add(egui::Button::new(
            RichText::new(format!("{PLUS} Add Node"))
                .size(12.0)
                .color(accent),
        ));
        let add_id = ui.make_persistent_id("lc_add_node_popup");
        if add_btn.clicked() {
            ui.memory_mut(|m| m.toggle_popup(add_id));
        }
        egui::popup_below_widget(
            ui,
            add_id,
            &add_btn,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(160.0);
                for &category in &nodes::categories() {
                    let icon = category_icon(category);
                    ui.menu_button(format!("{icon} {category}"), |ui| {
                        let defs = nodes::nodes_in_category(category);
                        for def in &defs {
                            if ui.button(format!("{icon} {}", def.display_name)).clicked() {
                                let canvas_pos = if let Some(cr) = state.canvas_rect {
                                    screen_to_canvas(
                                        cr.center(),
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
                                ui.memory_mut(|m| m.toggle_popup(add_id));
                            }
                        }
                    });
                }
            },
        );

        ui.separator();

        // ── Center ──
        if ui
            .add(egui::Button::new(
                RichText::new(format!("{CROSSHAIR}"))
                    .size(12.0)
                    .color(text_muted),
            ))
            .on_hover_text("Center Graph")
            .clicked()
        {
            if !graph.nodes.is_empty() {
                let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
                let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
                for node in &graph.nodes {
                    min_x = min_x.min(node.position[0]);
                    max_x = max_x.max(node.position[0]);
                    min_y = min_y.min(node.position[1]);
                    max_y = max_y.max(node.position[1]);
                }
                state.widget_state.offset = [-(min_x + max_x) / 2.0, -(min_y + max_y) / 2.0];
            } else {
                state.widget_state.offset = [0.0, 0.0];
            }
        }

        // ── Zoom ──
        if ui
            .add(egui::Button::new(
                RichText::new(format!("{MAGNIFYING_GLASS_MINUS}"))
                    .size(12.0)
                    .color(text_muted),
            ))
            .on_hover_text("Zoom Out")
            .clicked()
        {
            state.widget_state.zoom = (state.widget_state.zoom * 0.8).max(0.25);
        }

        let zoom_pct = (state.widget_state.zoom * 100.0) as u32;
        ui.label(RichText::new(format!("{zoom_pct}%")).size(11.0).color(text_muted));

        if ui
            .add(egui::Button::new(
                RichText::new(format!("{MAGNIFYING_GLASS_PLUS}"))
                    .size(12.0)
                    .color(text_muted),
            ))
            .on_hover_text("Zoom In")
            .clicked()
        {
            state.widget_state.zoom = (state.widget_state.zoom * 1.25).min(4.0);
        }
    });

    modified
}

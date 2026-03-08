//! Reusable node graph widget.
//!
//! Call [`node_graph()`] each frame, passing a persistent [`NodeGraphState`]
//! and a [`NodeGraphConfig`] for theming.  The function returns a
//! [`NodeGraphResponse`] describing any user actions that occurred.

pub mod types;
mod render;

pub use types::*;
use render::*;

use bevy_egui::egui::{self, Key, Pos2, Rect, Sense, Vec2};

/// Render an interactive node graph canvas.
///
/// The caller owns `state` (persist it across frames) and provides visual
/// tuning via `config`.  Returns what happened this frame.
pub fn node_graph(
    ui: &mut egui::Ui,
    _id: egui::Id,
    state: &mut NodeGraphState,
    config: &NodeGraphConfig,
) -> NodeGraphResponse {
    let mut response = NodeGraphResponse::default();

    // ── Allocate canvas ────────────────────────────────────────────────
    let available = ui.available_rect_before_wrap();
    let canvas_resp = ui.allocate_rect(available, Sense::click_and_drag());
    let canvas_rect = canvas_resp.rect;

    // Background
    ui.painter().rect_filled(canvas_rect, 0.0, config.canvas_bg);
    let painter = ui.painter_at(canvas_rect);

    let mouse_pos = ui.input(|i| i.pointer.hover_pos()).unwrap_or(Pos2::ZERO);

    // ── Zoom (scroll wheel) ────────────────────────────────────────────
    if canvas_resp.hovered() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll.abs() > 0.0 {
            let old_zoom = state.zoom;
            state.zoom = (state.zoom * (1.0 + scroll * 0.001)).clamp(0.25, 4.0);

            // Keep mouse position stationary while zooming
            if (state.zoom - old_zoom).abs() > 0.0001 {
                let center = canvas_rect.center();
                let rel = mouse_pos - center;
                let before = [rel.x / old_zoom - state.offset[0], rel.y / old_zoom - state.offset[1]];
                let after  = [rel.x / state.zoom - state.offset[0], rel.y / state.zoom - state.offset[1]];
                state.offset[0] -= before[0] - after[0];
                state.offset[1] -= before[1] - after[1];
            }
        }
    }

    // ── Pan (middle-mouse drag) ────────────────────────────────────────
    let middle_down = ui.input(|i| i.pointer.middle_down());
    if middle_down && canvas_resp.hovered() {
        let delta = ui.input(|i| i.pointer.delta());
        state.offset[0] += delta.x / state.zoom;
        state.offset[1] += delta.y / state.zoom;
    }

    // ── Draw grid ──────────────────────────────────────────────────────
    draw_grid(&painter, canvas_rect, state.offset, state.zoom, config);

    // ── Draw connections ───────────────────────────────────────────────
    // We'll collect pin screen positions while drawing nodes, but we need
    // connections drawn *behind* nodes, so we do a pre-pass to compute
    // pin positions, then draw connections, then draw nodes on top.

    // Pre-pass: compute node rects and pin positions
    let mut all_pin_positions: Vec<PinPos> = Vec::new();
    let mut node_rects: Vec<(u64, Rect)> = Vec::new();

    for node in &state.nodes {
        let (nrect, pins) = draw_node_layout(node, state.offset, state.zoom, canvas_rect, config);
        node_rects.push((node.id, nrect));
        all_pin_positions.extend(pins);
    }

    // ── Hit-testing (using layout pre-pass) ──────────────────────────
    // Pin hit area always covers the full row (dot + label text)
    let hovered_pin_idx = all_pin_positions.iter().position(|p| {
        p.hit_rect.contains(mouse_pos)
    });

    // Snapshot the hovered pin's ID, screen pos, and color (releases borrow)
    let hovered_pin_info = hovered_pin_idx.map(|i| {
        let p = &all_pin_positions[i];
        (p.id.clone(), p.screen_pos, p.color, p.hit_rect)
    });

    // Find node under mouse (reverse order = top-most first)
    let hovered_node_id = node_rects.iter().rev()
        .find(|(_, rect)| rect.contains(mouse_pos))
        .map(|&(id, _)| id);

    // Node body = on a node but not on a pin row
    let hovered_node_body = if hovered_pin_info.is_none() {
        hovered_node_id
    } else {
        None
    };

    // ── Cursors ────────────────────────────────────────────────────────
    if canvas_resp.hovered() {
        if hovered_pin_info.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        } else if hovered_node_body.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    // ── Draw connections ───────────────────────────────────────────────
    for conn in &state.connections {
        let from_pos = all_pin_positions.iter().find(|p| {
            p.id.node == conn.from_node && p.id.name == conn.from_pin && p.id.direction == PinDirection::Output
        });
        let to_pos = all_pin_positions.iter().find(|p| {
            p.id.node == conn.to_node && p.id.name == conn.to_pin && p.id.direction == PinDirection::Input
        });

        if let (Some(from), Some(to)) = (from_pos, to_pos) {
            let color = conn.color.unwrap_or(from.color);
            draw_connection(&painter, from.screen_pos, to.screen_pos, color, state.zoom, config);
        }
    }

    // ── Draw nodes (on top of connections) ─────────────────────────────
    all_pin_positions.clear();
    node_rects.clear();

    for node in &state.nodes {
        let is_sel = state.selected.contains(&node.id);
        let is_hov = hovered_node_body == Some(node.id);
        let (nrect, pins) = render::draw_node(
            &painter, node, state.offset, state.zoom, canvas_rect, is_sel, is_hov, config,
        );
        node_rects.push((node.id, nrect));
        all_pin_positions.extend(pins);
    }

    // ── Draw pending connection ────────────────────────────────────────
    if let Some(ref conn_state) = state.connecting {
        let from_screen = canvas_to_screen(conn_state.from_pos, state.offset, state.zoom, canvas_rect);
        let color = all_pin_positions.iter()
            .find(|p| p.id == conn_state.from)
            .map(|p| p.color)
            .unwrap_or(config.selection_stroke);
        draw_pending_connection(&painter, from_screen, mouse_pos, color, state.zoom, config);
    }

    // ── Draw pin row highlight ─────────────────────────────────────────
    if let Some((ref pin_id, _, pin_color, hit_rect)) = hovered_pin_info {
        // When connecting: highlight drop targets (skip origin pin)
        // When idle: subtle highlight on hover
        let should_highlight = if let Some(ref conn_state) = state.connecting {
            *pin_id != conn_state.from
        } else {
            true
        };
        if should_highlight {
            let highlight_pin = PinPos {
                id: pin_id.clone(),
                screen_pos: Pos2::ZERO,
                color: pin_color,
                hit_rect,
            };
            draw_pin_highlight(&painter, &highlight_pin, state.zoom);
        }
    }

    // ── Draw box selection ─────────────────────────────────────────────
    if let Some(ref bs) = state.box_select {
        draw_box_selection(
            &painter,
            Pos2::new(bs.start[0], bs.start[1]),
            Pos2::new(bs.end[0], bs.end[1]),
            config,
        );
    }

    // ── Keyboard shortcuts ─────────────────────────────────────────────
    if canvas_resp.hovered() && !ui.ctx().wants_keyboard_input() {
        if ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            if !state.selected.is_empty() {
                response.nodes_deleted = state.selected.clone();
                state.connections.retain(|c| {
                    !state.selected.contains(&c.from_node) && !state.selected.contains(&c.to_node)
                });
                state.nodes.retain(|n| !state.selected.contains(&n.id));
                state.selected.clear();
                response.selection_changed = true;
            }
        }

        if ui.input(|i| i.key_pressed(Key::Escape)) {
            if state.connecting.is_some() {
                state.connecting = None;
            } else {
                state.selected.clear();
                response.selection_changed = true;
            }
        }

        let modifiers = ui.input(|i| i.modifiers);
        if modifiers.ctrl && ui.input(|i| i.key_pressed(Key::A)) {
            state.selected = state.nodes.iter().map(|n| n.id).collect();
            response.selection_changed = true;
        }
    }

    // ── Interaction ────────────────────────────────────────────────────
    let modifiers = ui.input(|i| i.modifiers);

    // ── Right-click — disconnect pin or cut cable ──────────────────────
    let mut right_click_handled = false;
    if canvas_resp.secondary_clicked() {
        if let Some((ref pin_id, _, _, _)) = hovered_pin_info {
            // Right-click on pin: remove all connections on that pin
            let before = state.connections.len();
            let pid = pin_id.clone();
            state.connections.retain(|c| {
                let hits_from = c.from_node == pid.node && c.from_pin == pid.name;
                let hits_to   = c.to_node == pid.node && c.to_pin == pid.name;
                !(hits_from || hits_to)
            });
            if state.connections.len() != before {
                response.connection_removed = Some((pid.node, pid.name.clone()));
            }
            state.connecting = None;
            right_click_handled = true;
        } else {
            // Right-click on empty space: check if mouse is near a cable
            let cable_threshold = (5.0 * state.zoom).max(4.0);
            let mut cut_idx = None;
            for (i, conn) in state.connections.iter().enumerate() {
                let from_pos = all_pin_positions.iter().find(|p| {
                    p.id.node == conn.from_node && p.id.name == conn.from_pin && p.id.direction == PinDirection::Output
                });
                let to_pos = all_pin_positions.iter().find(|p| {
                    p.id.node == conn.to_node && p.id.name == conn.to_pin && p.id.direction == PinDirection::Input
                });
                if let (Some(from), Some(to)) = (from_pos, to_pos) {
                    if point_near_cable(from.screen_pos, to.screen_pos, mouse_pos, state.zoom, cable_threshold) {
                        cut_idx = Some(i);
                        break;
                    }
                }
            }
            if let Some(idx) = cut_idx {
                let removed = state.connections.remove(idx);
                response.connection_removed = Some((removed.from_node, removed.from_pin));
                right_click_handled = true;
            }
        }
    }
    // Always expose canvas response for context_menu() — caller checks right_click_handled
    response.canvas_response = Some(canvas_resp.clone());
    response.right_click_handled = right_click_handled;

    // ── Click on pin ───────────────────────────────────────────────────
    if canvas_resp.clicked() || canvas_resp.drag_started() {
        if let Some((ref pin_id, pin_screen, _, _)) = hovered_pin_info {
            if let Some(ref conn_state) = state.connecting.clone() {
                // Complete connection
                let from = &conn_state.from;
                let to = pin_id;

                // Only connect output→input across different nodes
                let valid = from.node != to.node
                    && ((from.direction == PinDirection::Output && to.direction == PinDirection::Input)
                        || (from.direction == PinDirection::Input && to.direction == PinDirection::Output));

                if valid {
                    let (out_pin, in_pin) = if from.direction == PinDirection::Output {
                        (from.clone(), to.clone())
                    } else {
                        (to.clone(), from.clone())
                    };

                    state.connections.push(ConnectionDef {
                        from_node: out_pin.node,
                        from_pin: out_pin.name.clone(),
                        to_node: in_pin.node,
                        to_pin: in_pin.name.clone(),
                        color: None,
                    });
                    response.connection_made = Some((out_pin, in_pin));
                }
                state.connecting = None;
            } else {
                // Start connection
                state.connecting = Some(ConnectingState {
                    from: pin_id.clone(),
                    from_pos: screen_to_canvas(pin_screen, state.offset, state.zoom, canvas_rect),
                });
            }
            return response;
        }
    }

    // ── Click / drag on node ───────────────────────────────────────────
    if canvas_resp.drag_started() {
        if let Some(node_id) = hovered_node_id {
            // Select if not already
            if !state.selected.contains(&node_id) {
                if !modifiers.ctrl {
                    state.selected.clear();
                }
                state.selected.push(node_id);
                response.selection_changed = true;
            }
            // Start dragging
            if let Some(node) = state.nodes.iter().find(|n| n.id == node_id) {
                let node_screen = canvas_to_screen(node.position, state.offset, state.zoom, canvas_rect);
                state.dragging = Some(DragState {
                    node: node_id,
                    offset: [mouse_pos.x - node_screen.x, mouse_pos.y - node_screen.y],
                });
            }
        } else if hovered_pin_info.is_none() && !middle_down {
            // Start box selection on empty space (not middle-mouse, that's pan)
            state.box_select = Some(BoxSelectState {
                start: [mouse_pos.x, mouse_pos.y],
                end: [mouse_pos.x, mouse_pos.y],
            });
        }
    }

    if canvas_resp.clicked() {
        // Cancel pending connection if clicking anywhere that isn't a pin
        if hovered_pin_info.is_none() && state.connecting.is_some() {
            state.connecting = None;
        }

        if hovered_pin_info.is_none() {
            if let Some(node_id) = hovered_node_id {
                if modifiers.ctrl {
                    // Toggle selection
                    if let Some(idx) = state.selected.iter().position(|&id| id == node_id) {
                        state.selected.remove(idx);
                    } else {
                        state.selected.push(node_id);
                    }
                } else if !state.selected.contains(&node_id) {
                    state.selected.clear();
                    state.selected.push(node_id);
                }
                response.selection_changed = true;
            } else {
                // Clicked empty space — clear selection & cancel connection
                if !modifiers.ctrl {
                    state.selected.clear();
                    state.connecting = None;
                    response.selection_changed = true;
                }
            }
        }
    }

    // ── Dragging nodes ─────────────────────────────────────────────────
    if canvas_resp.dragged() {
        if let Some(ref drag) = state.dragging.as_ref().map(|d| (d.node, d.offset)) {
            let (drag_node, drag_off) = *drag;
            let target_screen = Pos2::new(mouse_pos.x - drag_off[0], mouse_pos.y - drag_off[1]);
            let target_canvas = screen_to_canvas(target_screen, state.offset, state.zoom, canvas_rect);

            // Find delta from dragged node's current position
            if let Some(dn) = state.nodes.iter().find(|n| n.id == drag_node) {
                let dx = target_canvas[0] - dn.position[0];
                let dy = target_canvas[1] - dn.position[1];

                let selected = state.selected.clone();
                for node in &mut state.nodes {
                    if selected.contains(&node.id) {
                        node.position[0] += dx;
                        node.position[1] += dy;
                    }
                }
                response.node_moved = Some(drag_node);
            }
        }

        // Update box selection
        if let Some(ref mut bs) = state.box_select {
            bs.end = [mouse_pos.x, mouse_pos.y];
        }
    }

    // ── Drag stopped ───────────────────────────────────────────────────
    if canvas_resp.drag_stopped() {
        // Finalize pending connection: complete if over a valid pin, cancel otherwise
        if let Some(conn_state) = state.connecting.take() {
            if let Some((ref pin_id, _, _, _)) = hovered_pin_info {
                let from = &conn_state.from;
                let to = pin_id;
                let valid = from.node != to.node
                    && ((from.direction == PinDirection::Output && to.direction == PinDirection::Input)
                        || (from.direction == PinDirection::Input && to.direction == PinDirection::Output));
                if valid {
                    let (out_pin, in_pin) = if from.direction == PinDirection::Output {
                        (from.clone(), to.clone())
                    } else {
                        (to.clone(), from.clone())
                    };
                    state.connections.push(ConnectionDef {
                        from_node: out_pin.node,
                        from_pin: out_pin.name.clone(),
                        to_node: in_pin.node,
                        to_pin: in_pin.name.clone(),
                        color: None,
                    });
                    response.connection_made = Some((out_pin, in_pin));
                }
            }
            // If not over a valid pin, connection is simply dropped (cancelled)
        }

        // Finalize box selection
        if let Some(bs) = state.box_select.take() {
            let sel_rect = Rect::from_two_pos(
                Pos2::new(bs.start[0], bs.start[1]),
                Pos2::new(bs.end[0], bs.end[1]),
            );
            if !modifiers.ctrl {
                state.selected.clear();
            }
            for &(node_id, node_rect) in &node_rects {
                if sel_rect.intersects(node_rect) && !state.selected.contains(&node_id) {
                    state.selected.push(node_id);
                }
            }
            response.selection_changed = true;
        }
        state.dragging = None;
    }

    response
}

// ── Layout-only pass (no painting) ─────────────────────────────────────────

/// Compute node rect and pin positions without actually painting.
fn draw_node_layout(
    node: &NodeDef,
    offset: [f32; 2],
    zoom: f32,
    canvas_rect: Rect,
    config: &NodeGraphConfig,
) -> (Rect, Vec<PinPos>) {
    let screen_pos = canvas_to_screen(node.position, offset, zoom, canvas_rect);

    let input_count = node.pins.iter().filter(|p| p.direction == PinDirection::Input).count();
    let output_count = node.pins.iter().filter(|p| p.direction == PinDirection::Output).count();
    let max_pins = input_count.max(output_count);

    let node_height = config.header_height + (max_pins as f32 * config.pin_height) + 8.0;
    let scaled_w = config.node_width * zoom;
    let scaled_h = node_height * zoom;
    let node_rect = Rect::from_min_size(screen_pos, Vec2::new(scaled_w, scaled_h));

    let pin_start_y = screen_pos.y + config.header_height * zoom + 4.0 * zoom;
    let pin_spacing = config.pin_height * zoom;
    let pin_radius = config.pin_radius * zoom;
    let half_row = pin_spacing / 2.0;
    let pin_zone_w = 60.0 * zoom;

    let mut pin_positions = Vec::new();

    let mut iy = 0;
    for pin in node.pins.iter().filter(|p| p.direction == PinDirection::Input) {
        let py = pin_start_y + (iy as f32 + 0.5) * pin_spacing;
        let pos = Pos2::new(screen_pos.x, py);
        let hit_rect = Rect::from_min_max(
            Pos2::new(pos.x - pin_radius, py - half_row),
            Pos2::new(pos.x + pin_zone_w, py + half_row),
        );
        pin_positions.push(PinPos {
            id: PinId { node: node.id, name: pin.name.clone(), direction: PinDirection::Input },
            screen_pos: pos,
            color: pin.color,
            hit_rect,
        });
        iy += 1;
    }

    let mut oi = 0;
    for pin in node.pins.iter().filter(|p| p.direction == PinDirection::Output) {
        let py = pin_start_y + (oi as f32 + 0.5) * pin_spacing;
        let pos = Pos2::new(screen_pos.x + scaled_w, py);
        let hit_rect = Rect::from_min_max(
            Pos2::new(pos.x - pin_zone_w, py - half_row),
            Pos2::new(pos.x + pin_radius, py + half_row),
        );
        pin_positions.push(PinPos {
            id: PinId { node: node.id, name: pin.name.clone(), direction: PinDirection::Output },
            screen_pos: pos,
            color: pin.color,
            hit_rect,
        });
        oi += 1;
    }

    (node_rect, pin_positions)
}

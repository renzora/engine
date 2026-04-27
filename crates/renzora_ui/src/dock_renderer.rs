//! Recursive dock-tree renderer
//!
//! Walks the `DockTree`, splits space, draws tab bars and resize handles, and
//! dispatches to `PanelRegistry` for content.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, Id, Pos2, Rect, Sense, Stroke, Vec2};
use renzora_theme::Theme;

use crate::dock_tree::{DockTree, SplitDirection};
use crate::drag_drop::{self, DragState, DropTarget};
use crate::panel::PanelRegistry;

// ── Constants ────────────────────────────────────────────────────────────────

const TAB_BAR_HEIGHT: f32 = 28.0;
const RESIZE_HANDLE_VISUAL: f32 = 1.0;
const RESIZE_INTERACT_SIZE: f32 = 6.0;
const MIN_PANEL_SIZE: f32 = 50.0;

// ── Render result ────────────────────────────────────────────────────────────

/// Deferred mutations collected during rendering.
#[derive(Default)]
pub struct DockRenderResult {
    /// A tab close button was clicked.
    pub panel_to_close: Option<String>,
    /// A non-active tab was clicked: `(panel_in_same_leaf, clicked_panel)`.
    pub new_active_tab: Option<(String, String)>,
    /// A resize handle was dragged: `(tree_path, new_ratio)`.
    pub ratio_update: Option<(Vec<bool>, f32)>,
    /// A tab drag was initiated: `(panel_id)`.
    pub drag_started: Option<String>,
    /// Current frame's drop target (computed during drag).
    pub drop_target: Option<DropTarget>,
    /// A panel was added from the "+" menu: `(sibling_panel_id, new_panel_id)`.
    pub panel_to_add: Option<(String, String)>,
    /// A tab was Ctrl+dragged to undock it as a floating window.
    pub ctrl_drag_undock: Option<String>,
    /// A tab was right-click → "Undock" to float it.
    pub context_menu_undock: Option<String>,
}

// ── Public entry point ───────────────────────────────────────────────────────

/// Render the full dock tree into `rect`.
pub fn render_dock_tree(
    ui: &mut egui::Ui,
    tree: &DockTree,
    rect: Rect,
    registry: &PanelRegistry,
    world: &World,
    base_id: Id,
    theme: &Theme,
    drag: Option<&DragState>,
) -> DockRenderResult {
    let mut result = DockRenderResult::default();
    render_node(ui, tree, rect, registry, world, base_id, &[], &mut result, theme, drag);
    result
}

// ── Recursive dispatch ───────────────────────────────────────────────────────

fn render_node(
    ui: &mut egui::Ui,
    node: &DockTree,
    rect: Rect,
    registry: &PanelRegistry,
    world: &World,
    base_id: Id,
    path: &[bool],
    result: &mut DockRenderResult,
    theme: &Theme,
    drag: Option<&DragState>,
) {
    match node {
        DockTree::Split { direction, ratio, first, second } => {
            render_split(ui, *direction, *ratio, first, second, rect, registry, world, base_id, path, result, theme, drag);
        }
        DockTree::Leaf { tabs, active_tab } => {
            render_leaf(ui, tabs, *active_tab, rect, registry, world, base_id, path, result, theme, drag);
        }
        DockTree::Empty => {
            ui.painter().rect_filled(rect, 0.0, theme.surfaces.extreme.to_color32());
        }
    }
}

// ── Split node ───────────────────────────────────────────────────────────────

fn calculate_split_rects(direction: SplitDirection, ratio: f32, rect: Rect) -> (Rect, Rect, Rect) {
    // The resize handle's interaction zone is wider than the visible 1px line.
    // To stop it from overlapping the child panels (which causes cursor flicker
    // when hovering near a panel's scrollbar), we inset each child by half the
    // overhang on the side facing the handle. The handle's hit area then sits
    // entirely in the gap between children.
    let inset = ((RESIZE_INTERACT_SIZE - RESIZE_HANDLE_VISUAL) * 0.5).max(0.0);
    match direction {
        SplitDirection::Horizontal => {
            let avail = rect.width() - RESIZE_HANDLE_VISUAL;
            let first_w = (avail * ratio).max(MIN_PANEL_SIZE);
            let second_w = (avail - first_w).max(MIN_PANEL_SIZE);

            let first_visible_w = (first_w - inset).max(0.0);
            let second_x = rect.min.x + first_w + RESIZE_HANDLE_VISUAL + inset;
            let second_visible_w = (second_w - inset).max(0.0);

            let first_rect = Rect::from_min_size(
                rect.min,
                Vec2::new(first_visible_w, rect.height()),
            );
            let handle_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + first_w, rect.min.y),
                Vec2::new(RESIZE_HANDLE_VISUAL, rect.height()),
            );
            let second_rect = Rect::from_min_size(
                Pos2::new(second_x, rect.min.y),
                Vec2::new(second_visible_w, rect.height()),
            );
            (first_rect, handle_rect, second_rect)
        }
        SplitDirection::Vertical => {
            let avail = rect.height() - RESIZE_HANDLE_VISUAL;
            let first_h = (avail * ratio).max(MIN_PANEL_SIZE);
            let second_h = (avail - first_h).max(MIN_PANEL_SIZE);

            let first_visible_h = (first_h - inset).max(0.0);
            let second_y = rect.min.y + first_h + RESIZE_HANDLE_VISUAL + inset;
            let second_visible_h = (second_h - inset).max(0.0);

            let first_rect = Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width(), first_visible_h),
            );
            let handle_rect = Rect::from_min_size(
                Pos2::new(rect.min.x, rect.min.y + first_h),
                Vec2::new(rect.width(), RESIZE_HANDLE_VISUAL),
            );
            let second_rect = Rect::from_min_size(
                Pos2::new(rect.min.x, second_y),
                Vec2::new(rect.width(), second_visible_h),
            );
            (first_rect, handle_rect, second_rect)
        }
    }
}

fn render_split(
    ui: &mut egui::Ui,
    direction: SplitDirection,
    ratio: f32,
    first: &DockTree,
    second: &DockTree,
    rect: Rect,
    registry: &PanelRegistry,
    world: &World,
    base_id: Id,
    path: &[bool],
    result: &mut DockRenderResult,
    theme: &Theme,
    drag: Option<&DragState>,
) {
    let (first_rect, handle_rect, second_rect) = calculate_split_rects(direction, ratio, rect);

    // Render children
    let mut first_path = path.to_vec();
    first_path.push(false);
    render_node(ui, first, first_rect, registry, world, base_id, &first_path, result, theme, drag);

    let mut second_path = path.to_vec();
    second_path.push(true);
    render_node(ui, second, second_rect, registry, world, base_id, &second_path, result, theme, drag);

    // Interactive resize area (wider than the visual line)
    let interact_rect = match direction {
        SplitDirection::Horizontal => Rect::from_center_size(
            handle_rect.center(),
            Vec2::new(RESIZE_INTERACT_SIZE, handle_rect.height()),
        ),
        SplitDirection::Vertical => Rect::from_center_size(
            handle_rect.center(),
            Vec2::new(handle_rect.width(), RESIZE_INTERACT_SIZE),
        ),
    };

    let handle_id = base_id.with(("resize", path));
    let response = ui.interact(interact_rect, handle_id, Sense::drag());

    if response.hovered() || response.dragged() {
        let cursor = match direction {
            SplitDirection::Horizontal => CursorIcon::ResizeHorizontal,
            SplitDirection::Vertical => CursorIcon::ResizeVertical,
        };
        ui.ctx().set_cursor_icon(cursor);
    }

    // Drag → update ratio
    if response.dragged() {
        let delta = response.drag_delta();
        let total = match direction {
            SplitDirection::Horizontal => rect.width() - RESIZE_HANDLE_VISUAL,
            SplitDirection::Vertical => rect.height() - RESIZE_HANDLE_VISUAL,
        };
        let delta_ratio = match direction {
            SplitDirection::Horizontal => delta.x / total,
            SplitDirection::Vertical => delta.y / total,
        };
        let new_ratio = (ratio + delta_ratio).clamp(0.1, 0.9);
        if (new_ratio - ratio).abs() > 0.001 {
            result.ratio_update = Some((path.to_vec(), new_ratio));
        }
    }

    // Double-click → reset to 50/50
    if response.double_clicked() {
        result.ratio_update = Some((path.to_vec(), 0.5));
    }

    // Fill the gap between children with the surface color so the inset
    // pixels around the handle don't show through to whatever was painted
    // last frame. Then draw the 1px visual separator line on top.
    ui.painter().rect_filled(
        interact_rect,
        0.0,
        theme.surfaces.extreme.to_color32(),
    );
    let line_color = theme.widgets.border.to_color32();
    match direction {
        SplitDirection::Horizontal => {
            let x = handle_rect.center().x;
            ui.painter().line_segment(
                [Pos2::new(x, handle_rect.min.y), Pos2::new(x, handle_rect.max.y)],
                Stroke::new(1.0, line_color),
            );
        }
        SplitDirection::Vertical => {
            let y = handle_rect.center().y;
            ui.painter().line_segment(
                [Pos2::new(handle_rect.min.x, y), Pos2::new(handle_rect.max.x, y)],
                Stroke::new(1.0, line_color),
            );
        }
    }
}

// ── Leaf node (tab bar + panel content) ──────────────────────────────────────

fn render_leaf(
    ui: &mut egui::Ui,
    tabs: &[String],
    active_tab: usize,
    rect: Rect,
    registry: &PanelRegistry,
    world: &World,
    base_id: Id,
    path: &[bool],
    result: &mut DockRenderResult,
    theme: &Theme,
    drag: Option<&DragState>,
) {
    if tabs.is_empty() {
        return;
    }

    let is_dragging = drag.map(|d| d.is_detached).unwrap_or(false);

    // --- Tab bar background ---
    let tab_bar_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_BAR_HEIGHT));
    ui.painter().rect_filled(tab_bar_rect, 0.0, theme.panels.tab_inactive.to_color32());

    // --- Draw tabs ---
    let tab_height = TAB_BAR_HEIGHT - 4.0;
    let tab_padding = 6.0;
    let close_button_width = 14.0;
    let mut tab_x = rect.min.x + 2.0;
    let mut tab_rects_collected: Vec<Rect> = Vec::new();

    for (idx, panel_id) in tabs.iter().enumerate() {
        let is_active = idx == active_tab;
        let panel = registry.get(panel_id);
        let title = panel.map(|p| p.title()).unwrap_or(panel_id.as_str());
        let icon = panel.and_then(|p| p.icon());
        let can_close = panel.map(|p| p.closable()).unwrap_or(true);

        // Is this the tab being dragged?
        let is_being_dragged = drag
            .map(|d| d.is_detached && d.panel_id == *panel_id)
            .unwrap_or(false);

        // Measure tab width
        let icon_width: f32 = if icon.is_some() { 14.0 } else { 0.0 };
        let icon_gap: f32 = if icon.is_some() { 4.0 } else { 0.0 };
        let text_width = title.len() as f32 * 7.0; // rough approximation
        let close_space = if can_close { close_button_width } else { 0.0 };
        let tab_width = tab_padding + icon_width + icon_gap + text_width + close_space + tab_padding;

        let tab_rect = Rect::from_min_size(
            Pos2::new(tab_x, rect.min.y + 2.0),
            Vec2::new(tab_width, tab_height),
        );
        tab_rects_collected.push(tab_rect);

        let tab_id = base_id.with(("tab", path, idx));
        // Use click_and_drag to support drag initiation
        let tab_response = ui.interact(tab_rect, tab_id, Sense::click_and_drag());

        if tab_response.hovered() && !is_dragging {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        // Background — reduce opacity if this tab is being dragged out
        let alpha = if is_being_dragged { 100 } else { 255 };
        let bg = if is_active {
            let c = theme.panels.tab_active.to_color32();
            Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
        } else if tab_response.hovered() && !is_dragging {
            theme.panels.tab_hover.to_color32()
        } else {
            theme.panels.tab_inactive.to_color32()
        };
        ui.painter().rect_filled(tab_rect, egui::CornerRadius::ZERO, bg);

        // Icon
        let text_alpha = if is_being_dragged { 100 } else { 255 };
        let mut text_x = tab_rect.min.x + tab_padding;
        if let Some(icon_str) = icon {
            let ic = if is_active { theme.text.primary.to_color32() } else { theme.text.muted.to_color32() };
            let ic = Color32::from_rgba_unmultiplied(ic.r(), ic.g(), ic.b(), text_alpha);
            ui.painter().text(
                Pos2::new(text_x, tab_rect.center().y),
                egui::Align2::LEFT_CENTER,
                icon_str,
                egui::FontId::proportional(12.0),
                ic,
            );
            text_x += icon_width + icon_gap;
        }

        // Title
        let tc = if is_active { Color32::WHITE } else { theme.text.secondary.to_color32() };
        let tc = Color32::from_rgba_unmultiplied(tc.r(), tc.g(), tc.b(), text_alpha);
        ui.painter().text(
            Pos2::new(text_x, tab_rect.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(11.0),
            tc,
        );

        // Close button (only if not dragging)
        if can_close && !is_dragging {
            let close_rect = Rect::from_min_size(
                Pos2::new(
                    tab_rect.max.x - close_button_width - 2.0,
                    tab_rect.min.y + (tab_height - 12.0) / 2.0,
                ),
                Vec2::new(12.0, 12.0),
            );
            let close_id = base_id.with(("close", path, idx));
            let close_resp = ui.interact(close_rect, close_id, Sense::click());

            let close_color = if close_resp.hovered() {
                theme.panels.close_hover.to_color32()
            } else if is_active || tab_response.hovered() {
                theme.text.muted.to_color32()
            } else {
                theme.text.disabled.to_color32()
            };

            // Draw X
            let cx = close_rect.center();
            let s = 3.0;
            ui.painter().line_segment(
                [Pos2::new(cx.x - s, cx.y - s), Pos2::new(cx.x + s, cx.y + s)],
                Stroke::new(1.2, close_color),
            );
            ui.painter().line_segment(
                [Pos2::new(cx.x + s, cx.y - s), Pos2::new(cx.x - s, cx.y + s)],
                Stroke::new(1.2, close_color),
            );

            if close_resp.clicked() {
                result.panel_to_close = Some(panel_id.clone());
            }
        }

        // Tab click → switch active (only when not dragging)
        if tab_response.clicked() && !is_active && !is_dragging {
            if let Some(first) = tabs.first() {
                result.new_active_tab = Some((first.clone(), panel_id.clone()));
            }
        }

        // Drag initiation — Ctrl+drag undocks immediately, normal drag rearranges
        if tab_response.drag_started() && result.drag_started.is_none() {
            let ctrl_held = ui.input(|i| i.modifiers.ctrl);
            if ctrl_held {
                result.ctrl_drag_undock = Some(panel_id.clone());
            } else {
                result.drag_started = Some(panel_id.clone());
            }
        }

        // Right-click context menu
        tab_response.context_menu(|ui| {
            if can_close {
                if ui.button("Close").clicked() {
                    result.panel_to_close = Some(panel_id.clone());
                    ui.close();
                }
            }
            if ui.button("Undock").clicked() {
                result.context_menu_undock = Some(panel_id.clone());
                ui.close();
            }
        });

        tab_x += tab_width + 1.0;
    }

    // "+" button to add a panel as a new tab
    if !is_dragging {
        let plus_size = Vec2::new(22.0, tab_height);
        let plus_rect = Rect::from_min_size(
            Pos2::new(tab_x + 2.0, rect.min.y + 2.0),
            plus_size,
        );
        let plus_id = base_id.with(("plus_tab", path));
        let plus_resp = ui.interact(plus_rect, plus_id, Sense::click());

        if plus_resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        // Draw "+" button
        if ui.is_rect_visible(plus_rect) {
            let bg = if plus_resp.hovered() {
                theme.panels.tab_hover.to_color32()
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(plus_rect, egui::CornerRadius::ZERO, bg);
            ui.painter().text(
                plus_rect.center(),
                egui::Align2::CENTER_CENTER,
                "+",
                egui::FontId::proportional(14.0),
                theme.text.muted.to_color32(),
            );
        }

        if plus_resp.clicked() {
            #[allow(deprecated)]
            ui.memory_mut(|mem| mem.toggle_popup(plus_id.with("popup")));
        }

        // Panel picker popup
        #[allow(deprecated)]
        egui::popup_below_widget(
            ui,
            plus_id.with("popup"),
            &plus_resp,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(180.0);
                ui.set_max_height(400.0);
                ui.style_mut().spacing.item_spacing.y = 1.0;

                let already_here: std::collections::HashSet<&str> =
                    tabs.iter().map(|s| s.as_str()).collect();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for panel in registry.iter() {
                        let pid = panel.id();
                        let is_here = already_here.contains(pid);
                        let label = if let Some(icon) = panel.icon() {
                            format!("{} {}", icon, panel.title())
                        } else {
                            panel.title().to_string()
                        };

                        let btn = ui.add_enabled(
                            !is_here,
                            egui::Button::new(&label)
                                .fill(Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(2))
                                .min_size(Vec2::new(ui.available_width(), 0.0)),
                        );
                        if btn.hovered() && !is_here {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if btn.clicked() {
                            if let Some(sibling) = tabs.first() {
                                result.panel_to_add = Some((sibling.clone(), pid.to_string()));
                            }
                            ui.close();
                        }
                    }
                });
            },
        );
    }

    // Separator line under tab bar
    ui.painter().line_segment(
        [
            Pos2::new(rect.min.x, rect.min.y + TAB_BAR_HEIGHT),
            Pos2::new(rect.max.x, rect.min.y + TAB_BAR_HEIGHT),
        ],
        Stroke::new(1.0, theme.widgets.border.to_color32()),
    );

    // --- Detect drop target (during active drag) — draw overlay after content ---
    let mut pending_drop_target: Option<DropTarget> = None;
    if let Some(d) = drag {
        if d.is_detached {
            if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                if let Some(target) = drag_drop::detect_drop_target(
                    pointer,
                    rect,
                    tabs,
                    &tab_rects_collected,
                    &d.panel_id,
                ) {
                    pending_drop_target = Some(target);
                }
            }
        }
    }

    // --- Panel content area ---
    let content_rect = Rect::from_min_max(
        Pos2::new(rect.min.x, rect.min.y + TAB_BAR_HEIGHT),
        rect.max,
    );

    // Background fill for content area
    ui.painter().rect_filled(content_rect, 0.0, theme.surfaces.panel.to_color32());

    let active_id = tabs.get(active_tab).cloned().unwrap_or_default();
    if let Some(panel) = registry.get(&active_id) {
        // Render registered panel with unique ID scope
        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .id_salt(&active_id),
        );
        child_ui.set_clip_rect(content_rect);
        panel.ui(&mut child_ui, world);
    } else {
        // Placeholder for unregistered panels
        let center = content_rect.center();
        let label = format!("\"{}\" — panel not registered", active_id);
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            theme.text.muted.to_color32(),
        );
    }

    // --- Draw drop target overlay ON TOP of content ---
    if let Some(target) = pending_drop_target {
        match target.zone {
            crate::dock_tree::DropZone::Tab(_) => {
                drag_drop::draw_tab_insert_marker(ui, target.visual_rect, theme);
            }
            _ => {
                drag_drop::draw_zone_overlay(ui, target.visual_rect, theme);
            }
        }
        if result.drop_target.is_none() {
            result.drop_target = Some(target);
        }
    }
}

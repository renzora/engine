//! Dock tree rendering
//!
//! Renders the dock tree layout recursively, handling splits, tabs, and resize handles.

use super::dock_tree::{DockTree, DropZone, PanelId, SplitDirection};
use super::drag_drop::{
    detect_drop_zone, draw_tab_insert_indicator,
    detect_tab_insert_position, DragState, DropTarget,
};
use bevy_egui::egui::{self, Color32, CursorIcon, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use crate::theming::Theme;

/// Information about an active drop preview for animating panel positions
#[derive(Clone)]
struct DropPreview {
    /// The panel being hovered over
    target_panel: PanelId,
    /// The zone where the drop will occur
    zone: DropZone,
    /// Animation progress (0.0 to 1.0)
    animation_progress: f32,
}

/// Height of the tab bar in dock leaves
pub const TAB_BAR_HEIGHT: f32 = 28.0;
/// Visual gap between splits (minimal)
pub const RESIZE_HANDLE_SIZE: f32 = 1.0;
/// Interactive area for resize (extends into panels)
const RESIZE_INTERACT_SIZE: f32 = 8.0;
/// Minimum size for any panel dimension
pub const MIN_PANEL_SIZE: f32 = 50.0;

/// Result of rendering the dock tree
#[derive(Default)]
pub struct DockRenderResult {
    /// Panel that should be removed (user clicked close)
    pub panel_to_close: Option<PanelId>,
    /// New active tab set by user click
    pub new_active_tab: Option<(PanelId, PanelId)>, // (leaf containing, new active)
    /// Ratio update for a split (path, new_ratio)
    pub ratio_update: Option<(Vec<bool>, f32)>,
    /// Tab drag started
    pub drag_started: Option<PanelId>,
    /// Tab drag ended with drop
    pub drop_completed: Option<DropTarget>,
    /// Rectangles of each leaf (for drop zone detection)
    pub leaf_rects: Vec<(PanelId, Rect)>,
    /// Tab bar rectangles for each leaf (for tab insertion detection)
    pub tab_bar_rects: Vec<(PanelId, Rect)>,
    /// Tab positions within each leaf (for precise insertion indicator)
    pub tab_positions: Vec<(PanelId, Vec<(Rect, PanelId)>)>,
    /// Whether a resize handle is being hovered or dragged
    pub resize_handle_active: bool,
    /// Panel to add as a tab in a specific leaf (leaf_first_panel, new_panel)
    pub panel_to_add: Option<(PanelId, PanelId)>,
}

/// Context passed to panel render functions
#[allow(dead_code)]
pub struct PanelRenderContext<'a> {
    /// The rectangle allocated for this panel's content (excluding tab bar)
    pub rect: Rect,
    /// Whether this panel is the active tab in its group
    pub is_active: bool,
    /// ID for egui state
    pub id: Id,
    /// Reference to drag state if dragging
    pub drag_state: Option<&'a DragState>,
}

/// Render the complete dock tree
pub fn render_dock_tree(
    ui: &mut Ui,
    tree: &DockTree,
    available_rect: Rect,
    drag_state: Option<&DragState>,
    base_id: Id,
    theme: &Theme,
) -> DockRenderResult {
    let mut result = DockRenderResult::default();

    // First pass: collect leaf rects without drop preview to detect drop targets
    collect_leaf_rects(tree, available_rect, &mut result);

    // Detect drop target if dragging
    let drop_preview = if let Some(drag) = drag_state {
        if let Some(preview) = detect_current_drop_target(ui, drag, &result) {
            // Check if target changed - if so, reset animation
            let target_changed = drag.last_target.as_ref()
                .map(|(p, z)| p != &preview.target_panel || *z != preview.zone)
                .unwrap_or(true);

            let animation = if target_changed { 0.0 } else { drag.animation_progress };

            Some(DropPreview {
                target_panel: preview.target_panel,
                zone: preview.zone,
                animation_progress: animation,
            })
        } else {
            None
        }
    } else {
        None
    };

    // Update result with drop target
    if let Some(ref preview) = drop_preview {
        result.drop_completed = Some(DropTarget {
            target_panel: preview.target_panel.clone(),
            zone: preview.zone,
            rect: Rect::NOTHING, // Will be updated during render
        });
    }

    // Clear leaf rects - they'll be recollected during render with adjusted positions
    result.leaf_rects.clear();
    result.tab_bar_rects.clear();
    result.tab_positions.clear();

    // Second pass: render with animated positions
    render_node(ui, tree, available_rect, drag_state, base_id, &[], &mut result, theme, drop_preview.as_ref());

    // Draw tab insertion indicator for tab drops (panels moving handles split zones)
    if let Some(ref preview) = drop_preview {
        if preview.zone == DropZone::Tab {
            if let Some((_, rect)) = result.leaf_rects.iter().find(|(p, _)| p == &preview.target_panel) {
                let tab_bar_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_BAR_HEIGHT));
                if let Some(cursor_pos) = ui.ctx().pointer_hover_pos() {
                    if let Some((_, tab_positions)) = result.tab_positions.iter().find(|(p, _)| p == &preview.target_panel) {
                        if let Some(insert_info) = detect_tab_insert_position(cursor_pos, tab_bar_rect, tab_positions) {
                            draw_tab_insert_indicator(ui, &insert_info, tab_bar_rect, theme);
                        }
                    }
                }
            }
        }
        // Split zones are handled by panel movement in render_leaf - no overlay needed
    }

    result
}

/// Collect leaf rects without rendering (for drop detection)
fn collect_leaf_rects(node: &DockTree, rect: Rect, result: &mut DockRenderResult) {
    match node {
        DockTree::Split { direction, ratio, first, second, .. } => {
            let (first_rect, _, second_rect) = calculate_split_rects(*direction, *ratio, rect);
            collect_leaf_rects(first, first_rect, result);
            collect_leaf_rects(second, second_rect, result);
        }
        DockTree::Leaf { tabs, .. } => {
            if let Some(first_tab) = tabs.first() {
                result.leaf_rects.push((first_tab.clone(), rect));
            }
        }
        DockTree::Empty => {}
    }
}

/// Calculate split rectangles for a given direction and ratio
fn calculate_split_rects(direction: SplitDirection, ratio: f32, rect: Rect) -> (Rect, Rect, Rect) {
    match direction {
        SplitDirection::Horizontal => {
            let available_width = rect.width() - RESIZE_HANDLE_SIZE;
            let first_width = (available_width * ratio).max(MIN_PANEL_SIZE);
            let second_width = (available_width - first_width).max(MIN_PANEL_SIZE);

            let first_rect = Rect::from_min_size(rect.min, Vec2::new(first_width, rect.height()));
            let handle_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + first_width, rect.min.y),
                Vec2::new(RESIZE_HANDLE_SIZE, rect.height()),
            );
            let second_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + first_width + RESIZE_HANDLE_SIZE, rect.min.y),
                Vec2::new(second_width, rect.height()),
            );

            (first_rect, handle_rect, second_rect)
        }
        SplitDirection::Vertical => {
            let available_height = rect.height() - RESIZE_HANDLE_SIZE;
            let first_height = (available_height * ratio).max(MIN_PANEL_SIZE);
            let second_height = (available_height - first_height).max(MIN_PANEL_SIZE);

            let first_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), first_height));
            let handle_rect = Rect::from_min_size(
                Pos2::new(rect.min.x, rect.min.y + first_height),
                Vec2::new(rect.width(), RESIZE_HANDLE_SIZE),
            );
            let second_rect = Rect::from_min_size(
                Pos2::new(rect.min.x, rect.min.y + first_height + RESIZE_HANDLE_SIZE),
                Vec2::new(rect.width(), second_height),
            );

            (first_rect, handle_rect, second_rect)
        }
    }
}

/// Detected drop target info (without animation)
struct DetectedTarget {
    target_panel: PanelId,
    zone: DropZone,
}

/// Detect the current drop target based on cursor position
fn detect_current_drop_target(
    ui: &Ui,
    drag: &DragState,
    result: &DockRenderResult,
) -> Option<DetectedTarget> {
    let cursor_pos = ui.ctx().pointer_hover_pos()?;

    for (panel, rect) in &result.leaf_rects {
        // Don't show drop zone on the panel being dragged
        if panel == &drag.panel {
            continue;
        }

        // Check if cursor is in this panel's rect
        if !rect.contains(cursor_pos) {
            continue;
        }

        // First check if we're in the tab bar area for precise tab insertion
        let tab_bar_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_BAR_HEIGHT));

        if tab_bar_rect.contains(cursor_pos) {
            return Some(DetectedTarget {
                target_panel: panel.clone(),
                zone: DropZone::Tab,
            });
        }

        // Check regular drop zones (content area)
        if let Some(zone) = detect_drop_zone(cursor_pos, *rect) {
            return Some(DetectedTarget {
                target_panel: panel.clone(),
                zone,
            });
        }
    }

    None
}

fn render_node(
    ui: &mut Ui,
    node: &DockTree,
    rect: Rect,
    drag_state: Option<&DragState>,
    base_id: Id,
    path: &[bool],
    result: &mut DockRenderResult,
    theme: &Theme,
    drop_preview: Option<&DropPreview>,
) {
    match node {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            render_split(ui, *direction, *ratio, first, second, rect, drag_state, base_id, path, result, theme, drop_preview);
        }
        DockTree::Leaf { tabs, active_tab } => {
            render_leaf(ui, tabs, *active_tab, rect, drag_state, base_id, path, result, theme, drop_preview);
        }
        DockTree::Empty => {
            // Draw placeholder for empty node
            ui.painter().rect_filled(rect, 0.0, theme.surfaces.extreme.to_color32());
        }
    }
}

fn render_split(
    ui: &mut Ui,
    direction: SplitDirection,
    ratio: f32,
    first: &DockTree,
    second: &DockTree,
    rect: Rect,
    drag_state: Option<&DragState>,
    base_id: Id,
    path: &[bool],
    result: &mut DockRenderResult,
    theme: &Theme,
    drop_preview: Option<&DropPreview>,
) {
    let (first_rect, handle_rect, second_rect) = calculate_split_rects(direction, ratio, rect);

    // Render children
    let mut first_path = path.to_vec();
    first_path.push(false);
    render_node(ui, first, first_rect, drag_state, base_id, &first_path, result, theme, drop_preview);

    let mut second_path = path.to_vec();
    second_path.push(true);
    render_node(ui, second, second_rect, drag_state, base_id, &second_path, result, theme, drop_preview);

    // Create larger interactive area for easier grabbing
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

    // Render resize handle (invisible but interactive)
    let handle_id = base_id.with(("resize_handle", path));
    let handle_response = ui.interact(interact_rect, handle_id, Sense::drag());

    // Set cursor and track resize handle interaction
    if handle_response.hovered() || handle_response.dragged() {
        let cursor = match direction {
            SplitDirection::Horizontal => CursorIcon::ResizeHorizontal,
            SplitDirection::Vertical => CursorIcon::ResizeVertical,
        };
        ui.ctx().set_cursor_icon(cursor);
        result.resize_handle_active = true;
    }

    // Handle drag to resize
    if handle_response.dragged() {
        let delta = handle_response.drag_delta();
        let total_size = match direction {
            SplitDirection::Horizontal => rect.width() - RESIZE_HANDLE_SIZE,
            SplitDirection::Vertical => rect.height() - RESIZE_HANDLE_SIZE,
        };

        let delta_ratio = match direction {
            SplitDirection::Horizontal => delta.x / total_size,
            SplitDirection::Vertical => delta.y / total_size,
        };

        let new_ratio = (ratio + delta_ratio).clamp(0.1, 0.9);
        if (new_ratio - ratio).abs() > 0.001 {
            result.ratio_update = Some((path.to_vec(), new_ratio));
        }
    }

    // Double-click to equalize
    if handle_response.double_clicked() {
        result.ratio_update = Some((path.to_vec(), 0.5));
    }
}

fn render_leaf(
    ui: &mut Ui,
    tabs: &[PanelId],
    active_tab: usize,
    rect: Rect,
    drag_state: Option<&DragState>,
    base_id: Id,
    path: &[bool],
    result: &mut DockRenderResult,
    theme: &Theme,
    drop_preview: Option<&DropPreview>,
) {
    if tabs.is_empty() {
        return;
    }

    // Check if this leaf is the drop target and calculate adjusted rect
    let first_tab = tabs.first().cloned();
    let is_drop_target = drop_preview
        .as_ref()
        .map(|p| first_tab.as_ref() == Some(&p.target_panel))
        .unwrap_or(false);

    // Calculate the adjusted rect if this is the drop target
    let (panel_rect, preview_rect) = if is_drop_target {
        let preview = drop_preview.unwrap();
        calculate_animated_rects(rect, preview.zone, preview.animation_progress)
    } else {
        (rect, None)
    };

    // Draw the preview area (empty space for the new panel) if applicable
    if let Some(preview_r) = preview_rect {
        let accent = theme.semantic.accent.to_color32();
        let preview = drop_preview.unwrap();
        let t = ease_out_cubic(preview.animation_progress);

        // Draw the empty space with a solid blue fill (no border)
        let fill_alpha = (140.0 * t) as u8;
        let fill_color = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), fill_alpha);

        ui.painter().rect_filled(preview_r, 4.0, fill_color);
    }

    // Use the adjusted rect for rendering
    let rect = panel_rect;

    // Store leaf rect for drop detection
    let leaf_id = if let Some(first_tab) = tabs.first() {
        result.leaf_rects.push((first_tab.clone(), rect));
        first_tab.clone()
    } else {
        return;
    };

    // Calculate tab bar and content areas
    let tab_bar_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_BAR_HEIGHT));
    let _content_rect = Rect::from_min_max(
        Pos2::new(rect.min.x, rect.min.y + TAB_BAR_HEIGHT),
        rect.max,
    );

    // Store tab bar rect for tab insertion detection
    result.tab_bar_rects.push((leaf_id.clone(), tab_bar_rect));

    // Draw tab bar background
    ui.painter().rect_filled(tab_bar_rect, 0.0, theme.panels.tab_inactive.to_color32());

    // Track tab positions for insertion indicator
    let mut tab_positions: Vec<(Rect, PanelId)> = Vec::new();

    // Draw tabs
    let mut tab_x = rect.min.x + 2.0;
    let tab_height = TAB_BAR_HEIGHT - 4.0;
    let tab_padding = 6.0; // Compact padding
    let close_button_width = 14.0;

    for (idx, panel) in tabs.iter().enumerate() {
        let is_active = idx == active_tab;
        let is_being_dragged = drag_state.as_ref().map(|d| &d.panel == panel).unwrap_or(false);
        let can_close = panel.can_close();

        // Calculate tab width based on content
        let icon_width = 14.0;
        let title = panel.title();
        let text_width = ui.fonts_mut(|f| {
            f.glyph_width(&egui::FontId::proportional(11.0), 'M') * title.len() as f32 * 0.65
        });
        let close_space = if can_close { close_button_width } else { 0.0 };
        let tab_width = tab_padding + icon_width + 4.0 + text_width + close_space + tab_padding;

        let tab_rect = Rect::from_min_size(
            Pos2::new(tab_x, rect.min.y + 2.0),
            Vec2::new(tab_width, tab_height),
        );

        // Store tab position for insertion indicator (even for dragged tabs)
        tab_positions.push((tab_rect, panel.clone()));

        // Skip rendering if this tab is being dragged (show ghost)
        if is_being_dragged {
            let ghost_color = theme.panels.tab_inactive.to_color32();
            let [r, g, b, _] = ghost_color.to_array();
            ui.painter().rect_filled(
                tab_rect,
                egui::CornerRadius::ZERO,
                Color32::from_rgba_unmultiplied(r, g, b, 100),
            );
            tab_x += tab_width + 1.0;
            continue;
        }

        let tab_id = base_id.with(("tab", path, idx));
        let tab_response = ui.interact(tab_rect, tab_id, Sense::click_and_drag());

        // Show pointer cursor on tab hover
        if tab_response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        // Tab background - no border
        let bg_color = if is_active {
            theme.panels.tab_active.to_color32()
        } else if tab_response.hovered() {
            theme.panels.tab_hover.to_color32()
        } else {
            theme.panels.tab_inactive.to_color32()
        };

        ui.painter().rect_filled(tab_rect, egui::CornerRadius::ZERO, bg_color);

        // Icon - positioned at left edge with padding
        let icon_x = tab_rect.min.x + tab_padding;
        ui.painter().text(
            Pos2::new(icon_x, tab_rect.center().y),
            egui::Align2::LEFT_CENTER,
            panel.icon(),
            egui::FontId::proportional(12.0),
            if is_active {
                theme.text.primary.to_color32()
            } else {
                theme.text.muted.to_color32()
            },
        );

        // Title - positioned after icon
        let text_x = icon_x + icon_width + 4.0;
        ui.painter().text(
            Pos2::new(text_x, tab_rect.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(11.0),
            if is_active {
                Color32::WHITE
            } else {
                theme.text.secondary.to_color32()
            },
        );

        // Close button - always visible for closeable tabs
        if can_close {
            let close_rect = Rect::from_min_size(
                Pos2::new(tab_rect.max.x - close_button_width - 2.0, tab_rect.min.y + (tab_height - 12.0) / 2.0),
                Vec2::new(12.0, 12.0),
            );
            let close_id = base_id.with(("close", path, idx));
            let close_response = ui.interact(close_rect, close_id, Sense::click());
            if close_response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }

            let close_color = if close_response.hovered() {
                theme.panels.close_hover.to_color32()
            } else if is_active || tab_response.hovered() {
                theme.text.muted.to_color32()
            } else {
                theme.text.disabled.to_color32()
            };

            // Draw X
            let cx = close_rect.center();
            let x_size = 3.0;
            ui.painter().line_segment(
                [Pos2::new(cx.x - x_size, cx.y - x_size), Pos2::new(cx.x + x_size, cx.y + x_size)],
                Stroke::new(1.2, close_color),
            );
            ui.painter().line_segment(
                [Pos2::new(cx.x + x_size, cx.y - x_size), Pos2::new(cx.x - x_size, cx.y + x_size)],
                Stroke::new(1.2, close_color),
            );

            if close_response.clicked() {
                result.panel_to_close = Some(panel.clone());
            }
        }

        // Handle tab click (switch active)
        if tab_response.clicked() && !is_active {
            if let Some(first) = tabs.first() {
                result.new_active_tab = Some((first.clone(), panel.clone()));
            }
        }

        // Handle tab drag start
        if tab_response.drag_started() {
            result.drag_started = Some(panel.clone());
        }

        tab_x += tab_width + 1.0;
    }

    // "+" button to add a panel as a new tab
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

            let categories: &[(&str, &[PanelId])] = &[
                ("General", &[
                    PanelId::Hierarchy,
                    PanelId::Inspector,
                    PanelId::Assets,
                    PanelId::Console,
                    PanelId::Viewport,
                    PanelId::Settings,
                    PanelId::History,
                ]),
                ("Content", &[
                    PanelId::CodeEditor,
                    PanelId::Blueprint,
                    PanelId::NodeLibrary,
                    PanelId::ScriptVariables,
                    PanelId::Animation,
                    PanelId::Timeline,
                ]),
                ("Preview", &[
                    PanelId::StudioPreview,
                    PanelId::MaterialPreview,
                    PanelId::ShaderPreview,
                    PanelId::ImagePreview,
                    PanelId::NodeExplorer,
                ]),
                ("Tools", &[
                    PanelId::LevelTools,
                    PanelId::TextureEditor,
                    PanelId::ParticleEditor,
                    PanelId::ParticlePreview,
                    PanelId::VideoEditor,
                    PanelId::DAW,
                ]),
                ("Debug", &[
                    PanelId::Performance,
                    PanelId::RenderStats,
                    PanelId::RenderPipeline,
                    PanelId::EcsStats,
                    PanelId::MemoryProfiler,
                    PanelId::PhysicsDebug,
                    PanelId::CameraDebug,
                    PanelId::SystemProfiler,
                    PanelId::Gamepad,
                ]),
            ];

            let already_here: Vec<_> = tabs.to_vec();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, (category, panels)) in categories.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(4.0);
                    }
                    ui.label(
                        egui::RichText::new(*category)
                            .size(10.0)
                            .color(theme.text.muted.to_color32()),
                    );
                    ui.add_space(1.0);

                    for panel in *panels {
                        let is_here = already_here.contains(panel);
                        let label = format!("{} {}", panel.icon(), panel.title());

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
                            result.panel_to_add = Some((leaf_id.clone(), panel.clone()));
                            ui.close();
                        }
                    }
                }
            });
        },
    );

    // Store tab positions for insertion indicator
    result.tab_positions.push((leaf_id, tab_positions));

    // Draw subtle separator line under tab bar
    ui.painter().line_segment(
        [
            Pos2::new(rect.min.x, rect.min.y + TAB_BAR_HEIGHT),
            Pos2::new(rect.max.x, rect.min.y + TAB_BAR_HEIGHT),
        ],
        Stroke::new(1.0, theme.widgets.border.to_color32()),
    );
}

/// Get the active panel for a leaf at the given path
#[allow(dead_code)]
pub fn get_active_panel_at_rect(tree: &DockTree, rect: Rect, pos: Pos2) -> Option<PanelId> {
    get_active_panel_recursive(tree, rect, pos)
}

#[allow(dead_code)]
fn get_active_panel_recursive(node: &DockTree, rect: Rect, pos: Pos2) -> Option<PanelId> {
    match node {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (first_rect, second_rect) = match direction {
                SplitDirection::Horizontal => {
                    let available_width = rect.width() - RESIZE_HANDLE_SIZE;
                    let first_width = available_width * ratio;

                    let first_rect = Rect::from_min_size(rect.min, Vec2::new(first_width, rect.height()));
                    let second_rect = Rect::from_min_size(
                        Pos2::new(rect.min.x + first_width + RESIZE_HANDLE_SIZE, rect.min.y),
                        Vec2::new(available_width - first_width, rect.height()),
                    );

                    (first_rect, second_rect)
                }
                SplitDirection::Vertical => {
                    let available_height = rect.height() - RESIZE_HANDLE_SIZE;
                    let first_height = available_height * ratio;

                    let first_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), first_height));
                    let second_rect = Rect::from_min_size(
                        Pos2::new(rect.min.x, rect.min.y + first_height + RESIZE_HANDLE_SIZE),
                        Vec2::new(rect.width(), available_height - first_height),
                    );

                    (first_rect, second_rect)
                }
            };

            if first_rect.contains(pos) {
                get_active_panel_recursive(first, first_rect, pos)
            } else if second_rect.contains(pos) {
                get_active_panel_recursive(second, second_rect, pos)
            } else {
                None
            }
        }
        DockTree::Leaf { tabs, active_tab } => {
            tabs.get(*active_tab).cloned()
        }
        DockTree::Empty => None,
    }
}

/// Calculate the rect for a specific panel's content area
#[allow(dead_code)]
pub fn get_panel_content_rect(tree: &DockTree, panel: &PanelId, available_rect: Rect) -> Option<Rect> {
    get_panel_rect_recursive(tree, panel, available_rect)
}

/// Calculate all panel rectangles from the dock tree
/// Returns a map of panel ID to (leaf_rect, is_active)
pub fn calculate_panel_rects(tree: &DockTree, available_rect: Rect) -> Vec<(PanelId, Rect, bool)> {
    let mut result = Vec::new();
    collect_panel_rects(tree, available_rect, &mut result);
    result
}

/// Calculate all panel rectangles, using adjusted rects from render result where available
/// This ensures panel content renders at the same animated positions as the dock tree structure
pub fn calculate_panel_rects_with_adjustments(
    tree: &DockTree,
    available_rect: Rect,
    adjusted_leaf_rects: &[(PanelId, Rect)],
) -> Vec<(PanelId, Rect, bool)> {
    // Build a map of first-tab panel ID to adjusted rect
    let adjustments: std::collections::HashMap<&PanelId, &Rect> = adjusted_leaf_rects
        .iter()
        .map(|(id, rect)| (id, rect))
        .collect();

    let mut result = Vec::new();
    collect_panel_rects_with_adjustments(tree, available_rect, &adjustments, &mut result);
    result
}

fn collect_panel_rects_with_adjustments(
    node: &DockTree,
    rect: Rect,
    adjustments: &std::collections::HashMap<&PanelId, &Rect>,
    result: &mut Vec<(PanelId, Rect, bool)>,
) {
    match node {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (first_rect, _, second_rect) = calculate_split_rects(*direction, *ratio, rect);
            collect_panel_rects_with_adjustments(first, first_rect, adjustments, result);
            collect_panel_rects_with_adjustments(second, second_rect, adjustments, result);
        }
        DockTree::Leaf { tabs, active_tab } => {
            // Check if the first tab has an adjusted rect - all tabs in this leaf use it
            let adjusted_rect = tabs
                .first()
                .and_then(|first_tab| adjustments.get(first_tab).copied())
                .cloned()
                .unwrap_or(rect);

            for (idx, panel) in tabs.iter().enumerate() {
                let is_active = idx == *active_tab;
                result.push((panel.clone(), adjusted_rect, is_active));
            }
        }
        DockTree::Empty => {}
    }
}

fn collect_panel_rects(node: &DockTree, rect: Rect, result: &mut Vec<(PanelId, Rect, bool)>) {
    match node {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (first_rect, second_rect) = match direction {
                SplitDirection::Horizontal => {
                    let available_width = rect.width() - RESIZE_HANDLE_SIZE;
                    let first_width = (available_width * ratio).max(MIN_PANEL_SIZE);

                    let first_rect = Rect::from_min_size(rect.min, Vec2::new(first_width, rect.height()));
                    let second_rect = Rect::from_min_size(
                        Pos2::new(rect.min.x + first_width + RESIZE_HANDLE_SIZE, rect.min.y),
                        Vec2::new((available_width - first_width).max(MIN_PANEL_SIZE), rect.height()),
                    );

                    (first_rect, second_rect)
                }
                SplitDirection::Vertical => {
                    let available_height = rect.height() - RESIZE_HANDLE_SIZE;
                    let first_height = (available_height * ratio).max(MIN_PANEL_SIZE);

                    let first_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), first_height));
                    let second_rect = Rect::from_min_size(
                        Pos2::new(rect.min.x, rect.min.y + first_height + RESIZE_HANDLE_SIZE),
                        Vec2::new(rect.width(), (available_height - first_height).max(MIN_PANEL_SIZE)),
                    );

                    (first_rect, second_rect)
                }
            };

            collect_panel_rects(first, first_rect, result);
            collect_panel_rects(second, second_rect, result);
        }
        DockTree::Leaf { tabs, active_tab } => {
            for (idx, panel) in tabs.iter().enumerate() {
                let is_active = idx == *active_tab;
                result.push((panel.clone(), rect, is_active));
            }
        }
        DockTree::Empty => {}
    }
}

/// Get legacy layout values from dock tree for backward compatibility
/// Returns (hierarchy_width, inspector_width, assets_height) or None if not a standard layout
pub fn get_legacy_layout_values(tree: &DockTree, available_rect: Rect) -> Option<(f32, f32, f32)> {
    // Try to detect standard layout pattern: Hierarchy | Center+Bottom | Inspector
    // This provides backward compatibility with the old fixed layout system

    let panels = calculate_panel_rects(tree, available_rect);

    let mut hierarchy_width = None;
    let mut inspector_width = None;
    let mut assets_height = None;

    for (panel, rect, is_active) in panels {
        if !is_active {
            continue;
        }

        match panel {
            PanelId::Hierarchy => {
                hierarchy_width = Some(rect.width());
            }
            PanelId::Inspector => {
                inspector_width = Some(rect.width());
            }
            PanelId::Assets | PanelId::Console | PanelId::Animation => {
                // Bottom panel - use height
                if assets_height.is_none() || rect.height() < assets_height.unwrap() {
                    assets_height = Some(rect.height());
                }
            }
            _ => {}
        }
    }

    match (hierarchy_width, inspector_width, assets_height) {
        (Some(h), Some(i), Some(a)) => Some((h, i, a)),
        (Some(h), Some(i), None) => Some((h, i, 200.0)), // Default assets height
        _ => None,
    }
}

#[allow(dead_code)]
fn get_panel_rect_recursive(node: &DockTree, panel: &PanelId, rect: Rect) -> Option<Rect> {
    match node {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (first_rect, second_rect) = match direction {
                SplitDirection::Horizontal => {
                    let available_width = rect.width() - RESIZE_HANDLE_SIZE;
                    let first_width = available_width * ratio;

                    let first_rect = Rect::from_min_size(rect.min, Vec2::new(first_width, rect.height()));
                    let second_rect = Rect::from_min_size(
                        Pos2::new(rect.min.x + first_width + RESIZE_HANDLE_SIZE, rect.min.y),
                        Vec2::new(available_width - first_width, rect.height()),
                    );

                    (first_rect, second_rect)
                }
                SplitDirection::Vertical => {
                    let available_height = rect.height() - RESIZE_HANDLE_SIZE;
                    let first_height = available_height * ratio;

                    let first_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), first_height));
                    let second_rect = Rect::from_min_size(
                        Pos2::new(rect.min.x, rect.min.y + first_height + RESIZE_HANDLE_SIZE),
                        Vec2::new(rect.width(), available_height - first_height),
                    );

                    (first_rect, second_rect)
                }
            };

            get_panel_rect_recursive(first, panel, first_rect)
                .or_else(|| get_panel_rect_recursive(second, panel, second_rect))
        }
        DockTree::Leaf { tabs, active_tab } => {
            if tabs.get(*active_tab) == Some(panel) {
                // Return content rect (below tab bar)
                Some(Rect::from_min_max(
                    Pos2::new(rect.min.x, rect.min.y + TAB_BAR_HEIGHT),
                    rect.max,
                ))
            } else {
                None
            }
        }
        DockTree::Empty => None,
    }
}

/// Calculate animated rects for a panel when it's a drop target
/// Returns (adjusted_panel_rect, optional_preview_rect)
fn calculate_animated_rects(rect: Rect, zone: DropZone, animation_progress: f32) -> (Rect, Option<Rect>) {
    let t = ease_out_cubic(animation_progress);

    match zone {
        DropZone::Tab => {
            // For tab drops, don't change the panel size
            (rect, None)
        }
        DropZone::Left => {
            // Panel moves right and shrinks to make room on the left
            let preview_width = rect.width() * 0.5 * t;
            let panel_width = rect.width() - preview_width;

            let preview_rect = Rect::from_min_size(
                rect.min,
                Vec2::new(preview_width, rect.height()),
            );
            let panel_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + preview_width, rect.min.y),
                Vec2::new(panel_width, rect.height()),
            );

            (panel_rect, Some(preview_rect))
        }
        DropZone::Right => {
            // Panel shrinks to make room on the right
            let preview_width = rect.width() * 0.5 * t;
            let panel_width = rect.width() - preview_width;

            let panel_rect = Rect::from_min_size(
                rect.min,
                Vec2::new(panel_width, rect.height()),
            );
            let preview_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + panel_width, rect.min.y),
                Vec2::new(preview_width, rect.height()),
            );

            (panel_rect, Some(preview_rect))
        }
        DropZone::Top => {
            // Panel moves down and shrinks to make room on top
            let preview_height = rect.height() * 0.5 * t;
            let panel_height = rect.height() - preview_height;

            let preview_rect = Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width(), preview_height),
            );
            let panel_rect = Rect::from_min_size(
                Pos2::new(rect.min.x, rect.min.y + preview_height),
                Vec2::new(rect.width(), panel_height),
            );

            (panel_rect, Some(preview_rect))
        }
        DropZone::Bottom => {
            // Panel shrinks to make room on the bottom
            let preview_height = rect.height() * 0.5 * t;
            let panel_height = rect.height() - preview_height;

            let panel_rect = Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width(), panel_height),
            );
            let preview_rect = Rect::from_min_size(
                Pos2::new(rect.min.x, rect.min.y + panel_height),
                Vec2::new(rect.width(), preview_height),
            );

            (panel_rect, Some(preview_rect))
        }
    }
}

/// Cubic ease-out for smooth animation
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

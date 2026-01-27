use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Stroke, StrokeKind, Vec2};

use crate::core::{DockingState, SceneManagerState, SceneTab, TabKind};
use crate::theming::Theme;

use egui_phosphor::regular::{FILM_SCRIPT, SCROLL, CUBE, TREE_STRUCTURE, CODE};

const TAB_HEIGHT: f32 = 28.0;
const TAB_PADDING: f32 = 12.0;
const TAB_GAP: f32 = 2.0;
const TOP_MARGIN: f32 = 4.0;

/// Types of documents that can be created
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewDocumentType {
    Scene,
    Blueprint,
    Script,
    Material,
    Shader,
}

impl NewDocumentType {
    pub fn label(&self) -> &'static str {
        match self {
            NewDocumentType::Scene => "Scene",
            NewDocumentType::Blueprint => "Blueprint",
            NewDocumentType::Script => "Script",
            NewDocumentType::Material => "Material",
            NewDocumentType::Shader => "Shader",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            NewDocumentType::Scene => FILM_SCRIPT,
            NewDocumentType::Blueprint => TREE_STRUCTURE,
            NewDocumentType::Script => SCROLL,
            NewDocumentType::Material => CUBE,
            NewDocumentType::Shader => CODE,
        }
    }

    /// Layout to switch to when this document type is activated
    pub fn layout(&self) -> &'static str {
        match self {
            NewDocumentType::Scene => "Default",
            NewDocumentType::Blueprint => "Blueprints",
            NewDocumentType::Script => "Scripting",
            NewDocumentType::Material => "Materials",
            NewDocumentType::Shader => "Scripting",
        }
    }
}

/// Drag state for tab reordering
#[derive(Clone)]
struct TabDragState {
    dragging: Option<usize>, // Index in tab_order being dragged
    drop_target: Option<usize>, // Index where to drop
}

pub fn render_document_tabs(
    ctx: &egui::Context,
    scene_state: &mut SceneManagerState,
    docking_state: &mut DockingState,
    left_panel_width: f32,
    right_panel_width: f32,
    top_y: f32,
    theme: &Theme,
) -> f32 {
    let screen_rect = ctx.screen_rect();
    let available_width = screen_rect.width() - left_panel_width - right_panel_width;

    let tab_bar_rect = egui::Rect::from_min_size(
        Pos2::new(left_panel_width, top_y),
        Vec2::new(available_width, TAB_HEIGHT + TOP_MARGIN),
    );

    let bg_color = theme.surfaces.extreme.to_color32();
    let tab_bg = theme.widgets.inactive_bg.to_color32();
    let tab_active_bg = theme.widgets.active_bg.to_color32();
    let tab_hover_bg = theme.widgets.hovered_bg.to_color32();
    let text_color = theme.text.secondary.to_color32();
    let text_active_color = theme.text.primary.to_color32();
    let scene_accent_color = theme.semantic.accent.to_color32();
    let script_accent_color = theme.categories.scripting.accent.to_color32();
    let drop_indicator_color = theme.semantic.accent.to_color32();

    let mut layout_to_switch: Option<&'static str> = None;

    // Ensure tab_order is in sync with actual tabs
    sync_tab_order(scene_state);

    // Get drag state from egui memory
    let drag_id = egui::Id::new("document_tab_drag");
    let mut drag_state = ctx.memory(|mem| {
        mem.data.get_temp::<TabDragState>(drag_id).unwrap_or(TabDragState {
            dragging: None,
            drop_target: None,
        })
    });

    egui::Area::new(egui::Id::new("document_tabs_area"))
        .fixed_pos(tab_bar_rect.min)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            // Draw background
            ui.painter().rect_filled(tab_bar_rect, CornerRadius::ZERO, bg_color);

            // Bottom border (dark)
            ui.painter().line_segment(
                [
                    Pos2::new(tab_bar_rect.min.x, tab_bar_rect.max.y),
                    Pos2::new(tab_bar_rect.max.x, tab_bar_rect.max.y),
                ],
                Stroke::new(1.0, theme.widgets.border.to_color32()),
            );

            let mut x_offset = left_panel_width + 8.0;
            let mut tab_to_close: Option<TabKind> = None;
            let mut tab_to_activate: Option<TabKind> = None;

            // Store tab rects for drag-drop detection
            let mut tab_rects: Vec<(usize, egui::Rect)> = Vec::new();

            // Render all tabs in unified order
            for (order_idx, tab_kind) in scene_state.tab_order.iter().enumerate() {
                let (tab_text, is_active, icon, accent_color, icon_inactive_color) = match tab_kind {
                    TabKind::Scene(idx) => {
                        let tab = &scene_state.scene_tabs[*idx];
                        let text = if tab.is_modified {
                            format!("{}*", tab.name)
                        } else {
                            tab.name.clone()
                        };
                        let active = scene_state.active_script_tab.is_none() && *idx == scene_state.active_scene_tab;
                        // Muted version of scene accent
                        let [r, g, b, _] = scene_accent_color.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 75);
                        (text, active, FILM_SCRIPT, scene_accent_color, inactive)
                    }
                    TabKind::Script(idx) => {
                        let script = &scene_state.open_scripts[*idx];
                        let text = if script.is_modified {
                            format!("{}*", script.name)
                        } else {
                            script.name.clone()
                        };
                        let active = scene_state.active_script_tab == Some(*idx);
                        // Muted version of script accent
                        let [r, g, b, _] = script_accent_color.to_array();
                        let inactive = Color32::from_rgb(r / 2 + 50, g / 2 + 50, b / 2 + 55);
                        (text, active, SCROLL, script_accent_color, inactive)
                    }
                };

                // Calculate tab width based on text
                let text_width = ui.fonts(|f| {
                    f.glyph_width(&egui::FontId::proportional(12.0), 'M') * tab_text.len() as f32
                });
                let tab_width = text_width + TAB_PADDING * 2.0 + 36.0;

                let tab_rect = egui::Rect::from_min_size(
                    Pos2::new(x_offset, top_y + TOP_MARGIN + 2.0),
                    Vec2::new(tab_width, TAB_HEIGHT - 2.0),
                );

                tab_rects.push((order_idx, tab_rect));

                // Tab interaction - use drag sense
                let tab_response = ui.allocate_rect(tab_rect, egui::Sense::click_and_drag());
                let is_hovered = tab_response.hovered();
                let is_being_dragged = drag_state.dragging == Some(order_idx);

                // Start drag
                if tab_response.drag_started() {
                    drag_state.dragging = Some(order_idx);
                }

                // Draw tab background (dimmed if being dragged)
                let bg = if is_being_dragged {
                    Color32::from_rgb(60, 60, 75)
                } else if is_active {
                    tab_active_bg
                } else if is_hovered {
                    tab_hover_bg
                } else {
                    tab_bg
                };

                ui.painter().rect(
                    tab_rect,
                    CornerRadius::ZERO,
                    bg,
                    Stroke::NONE,
                    StrokeKind::Outside,
                );

                // Active indicator line at top
                if is_active && !is_being_dragged {
                    ui.painter().line_segment(
                        [
                            Pos2::new(tab_rect.min.x, tab_rect.min.y),
                            Pos2::new(tab_rect.max.x, tab_rect.min.y),
                        ],
                        Stroke::new(2.0, accent_color),
                    );
                }

                // Tab icon
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 8.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    icon,
                    egui::FontId::proportional(12.0),
                    if is_active { accent_color } else { icon_inactive_color },
                );

                // Tab text
                let txt_color = if is_active { text_active_color } else { text_color };
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 24.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &tab_text,
                    egui::FontId::proportional(12.0),
                    txt_color,
                );

                // Close button (x)
                let close_rect = egui::Rect::from_min_size(
                    Pos2::new(tab_rect.max.x - 20.0, tab_rect.min.y + 6.0),
                    Vec2::new(14.0, 14.0),
                );

                let close_response = ui.allocate_rect(close_rect, egui::Sense::click());
                let close_hovered = close_response.hovered();

                // Only show close button for scenes if there's more than one tab
                let can_close = match tab_kind {
                    TabKind::Scene(_) => scene_state.scene_tabs.len() > 1,
                    TabKind::Script(_) => true,
                };

                if can_close {
                    let close_color = if close_hovered {
                        theme.semantic.error.to_color32()
                    } else if is_hovered || is_active {
                        theme.text.muted.to_color32()
                    } else {
                        theme.text.disabled.to_color32()
                    };

                    // Draw X
                    let x_center = close_rect.center();
                    let x_size = 4.0;
                    ui.painter().line_segment(
                        [
                            Pos2::new(x_center.x - x_size, x_center.y - x_size),
                            Pos2::new(x_center.x + x_size, x_center.y + x_size),
                        ],
                        Stroke::new(1.5, close_color),
                    );
                    ui.painter().line_segment(
                        [
                            Pos2::new(x_center.x + x_size, x_center.y - x_size),
                            Pos2::new(x_center.x - x_size, x_center.y + x_size),
                        ],
                        Stroke::new(1.5, close_color),
                    );
                }

                // Handle clicks
                if close_response.clicked() && can_close {
                    tab_to_close = Some(*tab_kind);
                } else if tab_response.clicked() && !tab_response.dragged() {
                    tab_to_activate = Some(*tab_kind);
                }

                x_offset += tab_width + TAB_GAP;
            }

            // Add document button (+) with dropdown
            let add_btn_pos = Pos2::new(x_offset, top_y + TOP_MARGIN + 4.0);
            let add_btn_size = Vec2::new(24.0, 22.0);

            let _menu_area = ui.allocate_ui_at_rect(
                egui::Rect::from_min_size(add_btn_pos, add_btn_size),
                |ui| {
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = tab_bg;
                    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = tab_hover_bg;
                    ui.style_mut().visuals.widgets.active.weak_bg_fill = tab_hover_bg;

                    let _menu_response = egui::menu::menu_button(ui, "+", |ui| {
                        ui.set_min_width(140.0);

                        let doc_types = [
                            NewDocumentType::Scene,
                            NewDocumentType::Blueprint,
                            NewDocumentType::Script,
                            NewDocumentType::Material,
                            NewDocumentType::Shader,
                        ];

                        for doc_type in doc_types {
                            let label = format!("{} {}", doc_type.icon(), doc_type.label());
                            if ui.button(label).clicked() {
                                match doc_type {
                                    NewDocumentType::Scene => {
                                        let new_tab_num = scene_state.scene_tabs.len() + 1;
                                        let new_idx = scene_state.scene_tabs.len();
                                        scene_state.scene_tabs.push(SceneTab {
                                            name: format!("Untitled {}", new_tab_num),
                                            ..Default::default()
                                        });
                                        scene_state.tab_order.push(TabKind::Scene(new_idx));
                                        scene_state.pending_tab_switch = Some(new_idx);
                                        scene_state.active_script_tab = None;
                                        layout_to_switch = Some("Default");
                                    }
                                    NewDocumentType::Blueprint => {
                                        let new_tab_num = scene_state.scene_tabs.len() + 1;
                                        let new_idx = scene_state.scene_tabs.len();
                                        scene_state.scene_tabs.push(SceneTab {
                                            name: format!("Blueprint {}", new_tab_num),
                                            ..Default::default()
                                        });
                                        scene_state.tab_order.push(TabKind::Scene(new_idx));
                                        scene_state.pending_tab_switch = Some(new_idx);
                                        scene_state.active_script_tab = None;
                                        layout_to_switch = Some("Blueprints");
                                    }
                                    NewDocumentType::Script => {
                                        let script_num = scene_state.open_scripts.len() + 1;
                                        let new_idx = scene_state.open_scripts.len();
                                        scene_state.open_scripts.push(crate::core::OpenScript {
                                            name: format!("Script {}", script_num),
                                            path: std::path::PathBuf::new(),
                                            content: String::new(),
                                            is_modified: false,
                                            error: None,
                                            last_checked_content: String::new(),
                                        });
                                        scene_state.tab_order.push(TabKind::Script(new_idx));
                                        scene_state.active_script_tab = Some(new_idx);
                                        layout_to_switch = Some("Scripting");
                                    }
                                    NewDocumentType::Material => {
                                        layout_to_switch = Some("Materials");
                                    }
                                    NewDocumentType::Shader => {
                                        layout_to_switch = Some("Scripting");
                                    }
                                }
                                ui.close_menu();
                            }
                        }
                    });
                },
            );

            // Handle drag-drop for reordering
            if drag_state.dragging.is_some() {
                let pointer_pos = ui.input(|i| i.pointer.hover_pos());

                if let Some(pos) = pointer_pos {
                    // Find drop target
                    drag_state.drop_target = None;
                    for (order_idx, rect) in &tab_rects {
                        if pos.x < rect.center().x {
                            drag_state.drop_target = Some(*order_idx);
                            break;
                        }
                    }
                    // If past all tabs, drop at end
                    if drag_state.drop_target.is_none() && !tab_rects.is_empty() {
                        drag_state.drop_target = Some(tab_rects.len());
                    }

                    // Draw drop indicator
                    if let Some(drop_idx) = drag_state.drop_target {
                        let indicator_x = if drop_idx < tab_rects.len() {
                            tab_rects[drop_idx].1.min.x - 1.0
                        } else if let Some((_, last_rect)) = tab_rects.last() {
                            last_rect.max.x + 1.0
                        } else {
                            left_panel_width + 8.0
                        };

                        ui.painter().line_segment(
                            [
                                Pos2::new(indicator_x, top_y + TOP_MARGIN + 4.0),
                                Pos2::new(indicator_x, top_y + TOP_MARGIN + TAB_HEIGHT - 4.0),
                            ],
                            Stroke::new(2.0, drop_indicator_color),
                        );
                    }
                }

                // End drag on release
                if ui.input(|i| i.pointer.any_released()) {
                    if let (Some(from_idx), Some(to_idx)) = (drag_state.dragging, drag_state.drop_target) {
                        if from_idx != to_idx && from_idx + 1 != to_idx {
                            // Perform the reorder
                            let tab = scene_state.tab_order.remove(from_idx);
                            let insert_idx = if to_idx > from_idx { to_idx - 1 } else { to_idx };
                            scene_state.tab_order.insert(insert_idx, tab);
                        }
                    }
                    drag_state.dragging = None;
                    drag_state.drop_target = None;
                }
            }

            // Process tab actions
            if let Some(tab_kind) = tab_to_close {
                close_tab(scene_state, tab_kind);
            }

            if let Some(tab_kind) = tab_to_activate {
                activate_tab(scene_state, tab_kind, &mut layout_to_switch);
            }
        });

    // Store drag state back to memory
    ctx.memory_mut(|mem| {
        mem.data.insert_temp(drag_id, drag_state);
    });

    // Switch layout if requested
    if let Some(layout_name) = layout_to_switch {
        docking_state.switch_layout(layout_name);
    }

    TAB_HEIGHT + TOP_MARGIN
}

/// Ensure tab_order is in sync with actual tabs (handles external additions/removals)
fn sync_tab_order(scene_state: &mut SceneManagerState) {
    // Remove invalid entries
    scene_state.tab_order.retain(|kind| match kind {
        TabKind::Scene(idx) => *idx < scene_state.scene_tabs.len(),
        TabKind::Script(idx) => *idx < scene_state.open_scripts.len(),
    });

    // Add any missing scene tabs
    for idx in 0..scene_state.scene_tabs.len() {
        if !scene_state.tab_order.contains(&TabKind::Scene(idx)) {
            scene_state.tab_order.push(TabKind::Scene(idx));
        }
    }

    // Add any missing script tabs
    for idx in 0..scene_state.open_scripts.len() {
        if !scene_state.tab_order.contains(&TabKind::Script(idx)) {
            scene_state.tab_order.push(TabKind::Script(idx));
        }
    }
}

fn close_tab(scene_state: &mut SceneManagerState, tab_kind: TabKind) {
    // Find position in tab_order
    let order_pos = scene_state.tab_order.iter().position(|k| *k == tab_kind);

    match tab_kind {
        TabKind::Scene(idx) => {
            if scene_state.scene_tabs.len() > 1 {
                // Determine which tab to switch to
                let is_active = scene_state.active_script_tab.is_none() && idx == scene_state.active_scene_tab;

                // Remove from tab_order first
                if let Some(pos) = order_pos {
                    scene_state.tab_order.remove(pos);
                }

                // Update tab_order indices for remaining scene tabs
                for kind in &mut scene_state.tab_order {
                    if let TabKind::Scene(scene_idx) = kind {
                        if *scene_idx > idx {
                            *scene_idx -= 1;
                        }
                    }
                }

                // Set pending close and switch
                if is_active {
                    let new_active = if idx + 1 < scene_state.scene_tabs.len() {
                        idx
                    } else {
                        idx.saturating_sub(1)
                    };
                    scene_state.pending_tab_switch = Some(new_active);
                } else if idx < scene_state.active_scene_tab {
                    scene_state.pending_tab_switch = Some(scene_state.active_scene_tab - 1);
                }
                scene_state.pending_tab_close = Some(idx);
            }
        }
        TabKind::Script(idx) => {
            // Remove from tab_order first
            if let Some(pos) = order_pos {
                scene_state.tab_order.remove(pos);
            }

            // Update tab_order indices for remaining script tabs
            for kind in &mut scene_state.tab_order {
                if let TabKind::Script(script_idx) = kind {
                    if *script_idx > idx {
                        *script_idx -= 1;
                    }
                }
            }

            // Remove the script
            scene_state.open_scripts.remove(idx);

            // Update active script tab
            if scene_state.open_scripts.is_empty() {
                scene_state.active_script_tab = None;
            } else if let Some(active) = scene_state.active_script_tab {
                if active >= scene_state.open_scripts.len() {
                    scene_state.active_script_tab = Some(scene_state.open_scripts.len() - 1);
                } else if active > idx {
                    scene_state.active_script_tab = Some(active - 1);
                }
            }
        }
    }
}

fn activate_tab(scene_state: &mut SceneManagerState, tab_kind: TabKind, layout_to_switch: &mut Option<&'static str>) {
    match tab_kind {
        TabKind::Scene(idx) => {
            if scene_state.active_script_tab.is_some() || idx != scene_state.active_scene_tab {
                scene_state.active_script_tab = None;
                scene_state.pending_tab_switch = Some(idx);
                *layout_to_switch = Some("Default");
            }
        }
        TabKind::Script(idx) => {
            scene_state.active_script_tab = Some(idx);
            *layout_to_switch = Some("Scripting");
        }
    }
}

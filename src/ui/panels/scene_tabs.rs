use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Stroke, StrokeKind, Vec2};

use crate::core::{EditorState, SceneTab};

use egui_phosphor::regular::{FILM_SCRIPT, SCROLL};

const TAB_HEIGHT: f32 = 28.0;
const TAB_PADDING: f32 = 12.0;
const TAB_GAP: f32 = 2.0;

pub fn render_scene_tabs(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    left_panel_width: f32,
    right_panel_width: f32,
    top_y: f32,
) -> f32 {
    let screen_rect = ctx.screen_rect();
    let available_width = screen_rect.width() - left_panel_width - right_panel_width;

    let tab_bar_rect = egui::Rect::from_min_size(
        Pos2::new(left_panel_width, top_y),
        Vec2::new(available_width, TAB_HEIGHT),
    );

    let bg_color = Color32::from_rgb(30, 30, 38);
    let tab_bg = Color32::from_rgb(40, 40, 50);
    let tab_active_bg = Color32::from_rgb(50, 50, 62);
    let tab_hover_bg = Color32::from_rgb(45, 45, 55);
    let text_color = Color32::from_rgb(180, 180, 190);
    let text_active_color = Color32::WHITE;
    let border_color = Color32::from_rgb(60, 60, 70);
    let scene_accent_color = Color32::from_rgb(100, 160, 255);
    let script_accent_color = Color32::from_rgb(140, 217, 191);

    egui::Area::new(egui::Id::new("scene_tabs_area"))
        .fixed_pos(tab_bar_rect.min)
        .show(ctx, |ui| {
            // Draw background
            ui.painter().rect_filled(tab_bar_rect, CornerRadius::ZERO, bg_color);

            // Bottom border
            ui.painter().line_segment(
                [
                    Pos2::new(tab_bar_rect.min.x, tab_bar_rect.max.y),
                    Pos2::new(tab_bar_rect.max.x, tab_bar_rect.max.y),
                ],
                Stroke::new(1.0, border_color),
            );

            let mut x_offset = left_panel_width + 8.0;
            let mut scene_tab_to_close: Option<usize> = None;
            let mut scene_tab_to_activate: Option<usize> = None;
            let mut script_tab_to_close: Option<usize> = None;
            let mut script_tab_to_activate: Option<usize> = None;

            // Render scene tabs
            for (idx, tab) in editor_state.scene_tabs.iter().enumerate() {
                let is_active = editor_state.active_script_tab.is_none() && idx == editor_state.active_scene_tab;

                // Calculate tab width based on text
                let tab_text = if tab.is_modified {
                    format!("{}*", tab.name)
                } else {
                    tab.name.clone()
                };

                let text_width = ui.fonts(|f| {
                    f.glyph_width(&egui::FontId::proportional(12.0), 'M') * tab_text.len() as f32
                });
                let tab_width = text_width + TAB_PADDING * 2.0 + 36.0; // Extra space for icon and close button

                let tab_rect = egui::Rect::from_min_size(
                    Pos2::new(x_offset, top_y + 2.0),
                    Vec2::new(tab_width, TAB_HEIGHT - 2.0),
                );

                // Tab interaction
                let tab_response = ui.allocate_rect(tab_rect, egui::Sense::click());
                let is_hovered = tab_response.hovered();

                // Draw tab background
                let bg = if is_active {
                    tab_active_bg
                } else if is_hovered {
                    tab_hover_bg
                } else {
                    tab_bg
                };

                ui.painter().rect(
                    tab_rect,
                    CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 },
                    bg,
                    Stroke::NONE,
                    StrokeKind::Outside,
                );

                // Active indicator line at top
                if is_active {
                    ui.painter().line_segment(
                        [
                            Pos2::new(tab_rect.min.x + 2.0, tab_rect.min.y),
                            Pos2::new(tab_rect.max.x - 2.0, tab_rect.min.y),
                        ],
                        Stroke::new(2.0, scene_accent_color),
                    );
                }

                // Scene icon
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 8.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    FILM_SCRIPT,
                    egui::FontId::proportional(12.0),
                    if is_active { scene_accent_color } else { Color32::from_rgb(100, 140, 200) },
                );

                // Tab text
                let text_color = if is_active { text_active_color } else { text_color };
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 24.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &tab_text,
                    egui::FontId::proportional(12.0),
                    text_color,
                );

                // Close button (x)
                let close_rect = egui::Rect::from_min_size(
                    Pos2::new(tab_rect.max.x - 20.0, tab_rect.min.y + 6.0),
                    Vec2::new(14.0, 14.0),
                );

                let close_response = ui.allocate_rect(close_rect, egui::Sense::click());
                let close_hovered = close_response.hovered();

                let close_color = if close_hovered {
                    Color32::from_rgb(255, 100, 100)
                } else if is_hovered || is_active {
                    Color32::from_rgb(140, 140, 150)
                } else {
                    Color32::from_rgb(80, 80, 90)
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

                // Handle clicks
                if close_response.clicked() && editor_state.scene_tabs.len() > 1 {
                    scene_tab_to_close = Some(idx);
                } else if tab_response.clicked() {
                    scene_tab_to_activate = Some(idx);
                }

                x_offset += tab_width + TAB_GAP;
            }

            // Add scene tab button (+)
            let add_btn_rect = egui::Rect::from_min_size(
                Pos2::new(x_offset, top_y + 4.0),
                Vec2::new(24.0, 22.0),
            );

            let add_response = ui.allocate_rect(add_btn_rect, egui::Sense::click());
            let add_hovered = add_response.hovered();

            let add_bg = if add_hovered { tab_hover_bg } else { tab_bg };
            ui.painter().rect(
                add_btn_rect,
                CornerRadius::same(4),
                add_bg,
                Stroke::NONE,
                StrokeKind::Outside,
            );

            // Draw +
            let plus_color = if add_hovered { text_active_color } else { text_color };
            let plus_center = add_btn_rect.center();
            let plus_size = 5.0;
            ui.painter().line_segment(
                [
                    Pos2::new(plus_center.x - plus_size, plus_center.y),
                    Pos2::new(plus_center.x + plus_size, plus_center.y),
                ],
                Stroke::new(1.5, plus_color),
            );
            ui.painter().line_segment(
                [
                    Pos2::new(plus_center.x, plus_center.y - plus_size),
                    Pos2::new(plus_center.x, plus_center.y + plus_size),
                ],
                Stroke::new(1.5, plus_color),
            );

            if add_response.clicked() {
                // Add new tab
                let new_tab_num = editor_state.scene_tabs.len() + 1;
                editor_state.scene_tabs.push(SceneTab {
                    name: format!("Untitled {}", new_tab_num),
                    ..Default::default()
                });
                // Request switch to the new tab (this will save current scene first)
                editor_state.pending_tab_switch = Some(editor_state.scene_tabs.len() - 1);
            }

            x_offset += 24.0 + TAB_GAP;

            // Separator between scene tabs and script tabs
            if !editor_state.open_scripts.is_empty() {
                x_offset += 8.0;
                ui.painter().line_segment(
                    [
                        Pos2::new(x_offset, top_y + 6.0),
                        Pos2::new(x_offset, top_y + TAB_HEIGHT - 6.0),
                    ],
                    Stroke::new(1.0, border_color),
                );
                x_offset += 12.0;
            }

            // Render script tabs
            for (idx, script) in editor_state.open_scripts.iter().enumerate() {
                let is_active = editor_state.active_script_tab == Some(idx);

                let tab_text = if script.is_modified {
                    format!("{}*", script.name)
                } else {
                    script.name.clone()
                };

                let text_width = ui.fonts(|f| {
                    f.glyph_width(&egui::FontId::proportional(12.0), 'M') * tab_text.len() as f32
                });
                let tab_width = text_width + TAB_PADDING * 2.0 + 36.0;

                let tab_rect = egui::Rect::from_min_size(
                    Pos2::new(x_offset, top_y + 2.0),
                    Vec2::new(tab_width, TAB_HEIGHT - 2.0),
                );

                let tab_response = ui.allocate_rect(tab_rect, egui::Sense::click());
                let is_hovered = tab_response.hovered();

                let bg = if is_active {
                    tab_active_bg
                } else if is_hovered {
                    tab_hover_bg
                } else {
                    tab_bg
                };

                ui.painter().rect(
                    tab_rect,
                    CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 },
                    bg,
                    Stroke::NONE,
                    StrokeKind::Outside,
                );

                if is_active {
                    ui.painter().line_segment(
                        [
                            Pos2::new(tab_rect.min.x + 2.0, tab_rect.min.y),
                            Pos2::new(tab_rect.max.x - 2.0, tab_rect.min.y),
                        ],
                        Stroke::new(2.0, script_accent_color),
                    );
                }

                // Script icon
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 8.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    SCROLL,
                    egui::FontId::proportional(12.0),
                    if is_active { script_accent_color } else { Color32::from_rgb(100, 180, 160) },
                );

                let text_color = if is_active { text_active_color } else { text_color };
                ui.painter().text(
                    Pos2::new(tab_rect.min.x + 24.0, tab_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &tab_text,
                    egui::FontId::proportional(12.0),
                    text_color,
                );

                // Close button
                let close_rect = egui::Rect::from_min_size(
                    Pos2::new(tab_rect.max.x - 20.0, tab_rect.min.y + 6.0),
                    Vec2::new(14.0, 14.0),
                );

                let close_response = ui.allocate_rect(close_rect, egui::Sense::click());
                let close_hovered = close_response.hovered();

                let close_color = if close_hovered {
                    Color32::from_rgb(255, 100, 100)
                } else if is_hovered || is_active {
                    Color32::from_rgb(140, 140, 150)
                } else {
                    Color32::from_rgb(80, 80, 90)
                };

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

                if close_response.clicked() {
                    script_tab_to_close = Some(idx);
                } else if tab_response.clicked() {
                    script_tab_to_activate = Some(idx);
                }

                x_offset += tab_width + TAB_GAP;
            }

            // Process scene tab actions
            if let Some(idx) = scene_tab_to_close {
                if editor_state.scene_tabs.len() > 1 {
                    editor_state.pending_tab_close = Some(idx);
                }
            }

            if let Some(idx) = scene_tab_to_activate {
                if editor_state.active_script_tab.is_some() || idx != editor_state.active_scene_tab {
                    editor_state.active_script_tab = None; // Deactivate script tab
                    editor_state.pending_tab_switch = Some(idx);
                }
            }

            // Process script tab actions
            if let Some(idx) = script_tab_to_close {
                close_script_tab(editor_state, idx);
            }

            if let Some(idx) = script_tab_to_activate {
                editor_state.active_script_tab = Some(idx);
            }
        });

    TAB_HEIGHT
}

fn close_script_tab(editor_state: &mut EditorState, idx: usize) {
    editor_state.open_scripts.remove(idx);

    if editor_state.open_scripts.is_empty() {
        editor_state.active_script_tab = None;
    } else if let Some(active) = editor_state.active_script_tab {
        if active >= editor_state.open_scripts.len() {
            editor_state.active_script_tab = Some(editor_state.open_scripts.len() - 1);
        } else if active > idx {
            editor_state.active_script_tab = Some(active - 1);
        }
    }
}

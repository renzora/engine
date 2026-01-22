use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Sense, Vec2, TextEdit, FontId, FontFamily, TextBuffer};
use std::path::PathBuf;
use rhai::Engine;

use crate::core::{SceneManagerState, OpenScript, ScriptError};
use super::syntax_highlight::highlight_rhai;

use egui_phosphor::regular::{FLOPPY_DISK, WARNING};

/// Check script for compilation errors
fn check_script_errors(content: &str) -> Option<ScriptError> {
    let engine = Engine::new();
    match engine.compile(content) {
        Ok(_) => None,
        Err(err) => {
            let pos = err.position();
            Some(ScriptError {
                message: err.to_string(),
                line: if pos.is_none() { None } else { pos.line() },
                column: if pos.is_none() { None } else { pos.position() },
            })
        }
    }
}

/// Render the script editor panel (when a script tab is active)
pub fn render_script_editor(
    ctx: &egui::Context,
    scene_state: &mut SceneManagerState,
    left_panel_width: f32,
    right_panel_width: f32,
    top_y: f32,
    available_height: f32,
) -> bool {
    // Only show if a script tab is active
    let Some(active_idx) = scene_state.active_script_tab else {
        return false;
    };

    let Some(script) = scene_state.open_scripts.get_mut(active_idx) else {
        return false;
    };

    // Check for errors if content changed
    if script.content != script.last_checked_content {
        script.error = check_script_errors(&script.content);
        script.last_checked_content = script.content.clone();
    }

    let has_error = script.error.is_some();

    let screen_rect = ctx.screen_rect();
    let panel_width = screen_rect.width() - left_panel_width - right_panel_width;

    let panel_rect = egui::Rect::from_min_size(
        egui::pos2(left_panel_width, top_y),
        Vec2::new(panel_width, available_height),
    );

    // Toolbar height
    let toolbar_height = 32.0;

    egui::Area::new(egui::Id::new("script_editor_area"))
        .fixed_pos(panel_rect.min)
        .show(ctx, |ui| {
            ui.set_clip_rect(panel_rect);

            // Background
            ui.painter().rect_filled(panel_rect, 0.0, Color32::from_rgb(25, 25, 30));

            // Toolbar
            let toolbar_rect = egui::Rect::from_min_size(
                panel_rect.min,
                Vec2::new(panel_width, toolbar_height),
            );

            ui.painter().rect_filled(toolbar_rect, 0.0, Color32::from_rgb(35, 35, 42));

            // Bottom border on toolbar
            ui.painter().line_segment(
                [
                    egui::pos2(toolbar_rect.min.x, toolbar_rect.max.y),
                    egui::pos2(toolbar_rect.max.x, toolbar_rect.max.y),
                ],
                egui::Stroke::new(1.0, Color32::from_rgb(50, 50, 60)),
            );

            // Save button
            let save_btn_rect = egui::Rect::from_min_size(
                egui::pos2(toolbar_rect.min.x + 8.0, toolbar_rect.min.y + 4.0),
                Vec2::new(70.0, 24.0),
            );

            let save_response = ui.allocate_rect(save_btn_rect, Sense::click());
            let save_hovered = save_response.hovered();

            let save_bg = if save_hovered {
                Color32::from_rgb(50, 50, 60)
            } else {
                Color32::from_rgb(40, 40, 50)
            };

            ui.painter().rect_filled(save_btn_rect, 4.0, save_bg);

            ui.painter().text(
                egui::pos2(save_btn_rect.min.x + 8.0, save_btn_rect.center().y),
                egui::Align2::LEFT_CENTER,
                FLOPPY_DISK,
                FontId::proportional(14.0),
                Color32::from_rgb(180, 180, 190),
            );

            ui.painter().text(
                egui::pos2(save_btn_rect.min.x + 26.0, save_btn_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "Save",
                FontId::proportional(12.0),
                Color32::from_rgb(180, 180, 190),
            );

            // File path display
            ui.painter().text(
                egui::pos2(save_btn_rect.max.x + 16.0, toolbar_rect.center().y),
                egui::Align2::LEFT_CENTER,
                script.path.display().to_string(),
                FontId::proportional(11.0),
                Color32::from_rgb(100, 100, 110),
            );

            // Check for save (button or Ctrl+S)
            let should_save = save_response.clicked() ||
                ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S));

            if should_save {
                save_script(script);
            }
        });

    // Error panel height (only if there's an error)
    let error_panel_height = if has_error { 60.0 } else { 0.0 };

    // Editor content area (separate area for proper scrolling)
    let editor_rect = egui::Rect::from_min_max(
        egui::pos2(left_panel_width, top_y + toolbar_height),
        egui::pos2(left_panel_width + panel_width, top_y + available_height - error_panel_height),
    );

    egui::Area::new(egui::Id::new("script_editor_content"))
        .fixed_pos(editor_rect.min)
        .show(ctx, |ui| {
            ui.set_clip_rect(editor_rect);
            ui.set_min_size(Vec2::new(editor_rect.width(), editor_rect.height()));

            let content_width = editor_rect.width();
            let content_height = editor_rect.height();

            egui::Frame::new()
                .fill(Color32::from_rgb(25, 25, 30))
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(content_width, content_height));

                    egui::ScrollArea::both()
                        .max_width(content_width)
                        .max_height(content_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_width(content_width);

                            // Set the style for code editor
                            ui.style_mut().visuals.extreme_bg_color = Color32::from_rgb(25, 25, 30);
                            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(25, 25, 30);
                            ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(30, 30, 38);

                            // Get script mutably again for the text edit
                            if let Some(script) = scene_state.open_scripts.get_mut(active_idx) {
                                let font_size = 16.0;
                                let mut layouter = |ui: &egui::Ui, text: &dyn TextBuffer, _wrap_width: f32| {
                                    let layout_job = highlight_rhai(text.as_str(), font_size);
                                    ui.fonts(|f| f.layout_job(layout_job))
                                };

                                let response = ui.add_sized(
                                    Vec2::new(content_width, content_height),
                                    TextEdit::multiline(&mut script.content)
                                        .font(FontId::new(font_size, FontFamily::Monospace))
                                        .code_editor()
                                        .desired_width(content_width)
                                        .lock_focus(true)
                                        .layouter(&mut layouter)
                                );

                                if response.changed() {
                                    script.is_modified = true;
                                }
                            }
                        });
                });
        });

    // Error panel (if there's an error)
    if let Some(script) = scene_state.open_scripts.get(active_idx) {
        if let Some(ref error) = script.error {
            let error_rect = egui::Rect::from_min_max(
                egui::pos2(left_panel_width, top_y + available_height - error_panel_height),
                egui::pos2(left_panel_width + panel_width, top_y + available_height),
            );

            egui::Area::new(egui::Id::new("script_error_panel"))
                .fixed_pos(error_rect.min)
                .show(ctx, |ui| {
                    ui.set_clip_rect(error_rect);

                    // Error background
                    ui.painter().rect_filled(error_rect, 0.0, Color32::from_rgb(60, 30, 30));

                    // Top border
                    ui.painter().line_segment(
                        [
                            egui::pos2(error_rect.min.x, error_rect.min.y),
                            egui::pos2(error_rect.max.x, error_rect.min.y),
                        ],
                        egui::Stroke::new(2.0, Color32::from_rgb(200, 80, 80)),
                    );

                    // Warning icon
                    ui.painter().text(
                        egui::pos2(error_rect.min.x + 12.0, error_rect.min.y + 20.0),
                        egui::Align2::LEFT_CENTER,
                        WARNING,
                        FontId::proportional(18.0),
                        Color32::from_rgb(255, 120, 120),
                    );

                    // Error location
                    let location = match (error.line, error.column) {
                        (Some(line), Some(col)) => format!("Line {}, Column {}", line, col),
                        (Some(line), None) => format!("Line {}", line),
                        _ => "Unknown location".to_string(),
                    };

                    ui.painter().text(
                        egui::pos2(error_rect.min.x + 36.0, error_rect.min.y + 16.0),
                        egui::Align2::LEFT_CENTER,
                        &location,
                        FontId::proportional(11.0),
                        Color32::from_rgb(255, 150, 150),
                    );

                    // Error message (truncate if too long)
                    let max_chars = ((panel_width - 50.0) / 7.0) as usize;
                    let message = if error.message.len() > max_chars {
                        format!("{}...", &error.message[..max_chars.saturating_sub(3)])
                    } else {
                        error.message.clone()
                    };

                    ui.painter().text(
                        egui::pos2(error_rect.min.x + 36.0, error_rect.min.y + 38.0),
                        egui::Align2::LEFT_CENTER,
                        &message,
                        FontId::new(12.0, FontFamily::Monospace),
                        Color32::from_rgb(220, 180, 180),
                    );
                });
        }
    }

    true
}

fn save_script(script: &mut OpenScript) {
    match std::fs::write(&script.path, &script.content) {
        Ok(_) => {
            script.is_modified = false;
            info!("Saved script: {}", script.path.display());
        }
        Err(e) => {
            error!("Failed to save script: {}", e);
        }
    }
}

/// Open a script file in the editor
pub fn open_script(scene_state: &mut SceneManagerState, path: PathBuf) {
    // Check if already open
    for (idx, script) in scene_state.open_scripts.iter().enumerate() {
        if script.path == path {
            scene_state.active_script_tab = Some(idx);
            return;
        }
    }

    // Read the file
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read script: {}", e);
            return;
        }
    };

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let content_clone = content.clone();
    scene_state.open_scripts.push(OpenScript {
        path,
        name,
        content,
        is_modified: false,
        error: None,
        last_checked_content: content_clone,
    });

    scene_state.active_script_tab = Some(scene_state.open_scripts.len() - 1);
}

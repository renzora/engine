#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Sense, Vec2, TextEdit, FontId, FontFamily, TextBuffer};
use std::path::PathBuf;
use std::process::Command;
use rhai::Engine;

use crate::core::{SceneManagerState, OpenScript, ScriptError, BuildState, BuildError};
use crate::project::CurrentProject;
use super::syntax_highlight::{highlight_rhai, highlight_rust};

use egui_phosphor::regular::{FLOPPY_DISK, WARNING, HAMMER, SPINNER, CHECK_CIRCLE};

/// Check if a file is a Rust source file
fn is_rust_file(path: &PathBuf) -> bool {
    path.extension()
        .map(|ext| ext == "rs")
        .unwrap_or(false)
}

/// Find the Cargo.toml for a Rust file (walk up directories)
fn find_cargo_toml(file_path: &PathBuf) -> Option<PathBuf> {
    let mut current = file_path.parent()?;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            return Some(cargo_toml);
        }
        current = current.parent()?;
    }
}

/// Get the crate name from Cargo.toml
fn get_crate_name(cargo_toml: &PathBuf) -> Option<String> {
    let content = std::fs::read_to_string(cargo_toml).ok()?;
    // Simple parsing - look for name = "..."
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("name") {
            if let Some(start) = line.find('"') {
                if let Some(end) = line[start + 1..].find('"') {
                    return Some(line[start + 1..start + 1 + end].to_string());
                }
            }
        }
    }
    None
}

/// Build a Rust plugin and return errors if any
fn build_rust_plugin(cargo_toml: &PathBuf) -> Result<String, Vec<BuildError>> {
    let crate_dir = cargo_toml.parent().unwrap();

    // Run cargo build --release with JSON output for better error parsing
    let output = Command::new("cargo")
        .args(["build", "--release", "--message-format=json"])
        .current_dir(crate_dir)
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if output.status.success() {
                // Get crate name for the DLL
                let crate_name = get_crate_name(cargo_toml)
                    .unwrap_or_else(|| "plugin".to_string());
                Ok(crate_name)
            } else {
                // Parse errors from JSON output
                let mut errors = Vec::new();

                for line in stdout.lines() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                        if json.get("reason").and_then(|r| r.as_str()) == Some("compiler-message") {
                            if let Some(message) = json.get("message") {
                                let level = message.get("level").and_then(|l| l.as_str()).unwrap_or("");
                                if level == "error" {
                                    let msg = message.get("message")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Unknown error")
                                        .to_string();

                                    let (file, line, column) = if let Some(spans) = message.get("spans").and_then(|s| s.as_array()) {
                                        if let Some(span) = spans.first() {
                                            let file = span.get("file_name").and_then(|f| f.as_str()).map(|s| s.to_string());
                                            let line = span.get("line_start").and_then(|l| l.as_u64()).map(|l| l as usize);
                                            let col = span.get("column_start").and_then(|c| c.as_u64()).map(|c| c as usize);
                                            (file, line, col)
                                        } else {
                                            (None, None, None)
                                        }
                                    } else {
                                        (None, None, None)
                                    };

                                    errors.push(BuildError {
                                        message: msg,
                                        file,
                                        line,
                                        column,
                                    });
                                }
                            }
                        }
                    }
                }

                // If no JSON errors parsed, use stderr
                if errors.is_empty() && !stderr.is_empty() {
                    errors.push(BuildError {
                        message: stderr.to_string(),
                        file: None,
                        line: None,
                        column: None,
                    });
                }

                if errors.is_empty() {
                    errors.push(BuildError {
                        message: "Build failed with unknown error".to_string(),
                        file: None,
                        line: None,
                        column: None,
                    });
                }

                Err(errors)
            }
        }
        Err(e) => {
            Err(vec![BuildError {
                message: format!("Failed to run cargo: {}", e),
                file: None,
                line: None,
                column: None,
            }])
        }
    }
}

/// Copy built DLL to project plugins folder
fn copy_dll_to_plugins(crate_dir: &PathBuf, crate_name: &str, plugins_dir: &PathBuf) -> Result<(), String> {
    let dll_name = if cfg!(windows) {
        format!("{}.dll", crate_name.replace('-', "_"))
    } else if cfg!(target_os = "macos") {
        format!("lib{}.dylib", crate_name.replace('-', "_"))
    } else {
        format!("lib{}.so", crate_name.replace('-', "_"))
    };

    let source = crate_dir.join("target").join("release").join(&dll_name);
    let dest = plugins_dir.join(&dll_name);

    // Create plugins directory if it doesn't exist
    if !plugins_dir.exists() {
        std::fs::create_dir_all(plugins_dir)
            .map_err(|e| format!("Failed to create plugins directory: {}", e))?;
    }

    std::fs::copy(&source, &dest)
        .map_err(|e| format!("Failed to copy DLL from {} to {}: {}", source.display(), dest.display(), e))?;

    info!("Copied {} to {}", source.display(), dest.display());
    Ok(())
}

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
/// NOTE: This function is kept for reference but is no longer used.
/// The script editor now uses render_script_editor_content via the docking system.
#[allow(dead_code)]
fn render_script_editor(
    ctx: &egui::Context,
    scene_state: &mut SceneManagerState,
    current_project: Option<&CurrentProject>,
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

    let is_rust = is_rust_file(&script.path);

    // Check for errors if content changed (only for Rhai scripts, not Rust)
    if !is_rust && script.content != script.last_checked_content {
        script.error = check_script_errors(&script.content);
        script.last_checked_content = script.content.clone();
    }

    // For Rust files, show build errors instead of script errors
    let has_error = if is_rust {
        matches!(scene_state.build_state, BuildState::Failed(_))
    } else {
        script.error.is_some()
    };

    let screen_rect = ctx.content_rect();
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

            // Build button (only for Rust files)
            let mut next_x = save_btn_rect.max.x + 8.0;
            let mut should_build = false;

            if is_rust {
                let build_btn_rect = egui::Rect::from_min_size(
                    egui::pos2(next_x, toolbar_rect.min.y + 4.0),
                    Vec2::new(75.0, 24.0),
                );
                next_x = build_btn_rect.max.x + 8.0;

                let is_building = matches!(scene_state.build_state, BuildState::Building);
                let build_response = ui.allocate_rect(build_btn_rect, if is_building { Sense::hover() } else { Sense::click() });
                let build_hovered = build_response.hovered();

                let build_bg = if is_building {
                    Color32::from_rgb(50, 70, 90)
                } else if build_hovered {
                    Color32::from_rgb(50, 50, 60)
                } else {
                    Color32::from_rgb(40, 40, 50)
                };

                ui.painter().rect_filled(build_btn_rect, 4.0, build_bg);

                let (build_icon, build_text, icon_color) = match &scene_state.build_state {
                    BuildState::Building => (SPINNER, "Building...", Color32::from_rgb(100, 180, 255)),
                    BuildState::Success(_) => (CHECK_CIRCLE, "Build", Color32::from_rgb(100, 200, 100)),
                    BuildState::Failed(_) => (HAMMER, "Build", Color32::from_rgb(255, 120, 120)),
                    BuildState::Idle => (HAMMER, "Build", Color32::from_rgb(180, 180, 190)),
                };

                ui.painter().text(
                    egui::pos2(build_btn_rect.min.x + 8.0, build_btn_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    build_icon,
                    FontId::proportional(14.0),
                    icon_color,
                );

                ui.painter().text(
                    egui::pos2(build_btn_rect.min.x + 26.0, build_btn_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    build_text,
                    FontId::proportional(12.0),
                    Color32::from_rgb(180, 180, 190),
                );

                // Check for build (button or Ctrl+B)
                should_build = !is_building && (build_response.clicked() ||
                    ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::B)));
            }

            // File path display
            ui.painter().text(
                egui::pos2(next_x + 8.0, toolbar_rect.center().y),
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

            // Handle build
            if should_build {
                if let Some(cargo_toml) = find_cargo_toml(&script.path) {
                    scene_state.build_state = BuildState::Building;

                    // Clone paths for the build thread
                    let cargo_toml_clone = cargo_toml.clone();
                    let plugins_dir = current_project.map(|p| p.path.join("plugins"));

                    // Perform build (synchronous for now - could be async in the future)
                    match build_rust_plugin(&cargo_toml_clone) {
                        Ok(crate_name) => {
                            info!("Build successful: {}", crate_name);

                            // Copy DLL to plugins folder if we have a project
                            if let Some(plugins_dir) = plugins_dir {
                                let crate_dir = cargo_toml_clone.parent().unwrap().to_path_buf();
                                match copy_dll_to_plugins(&crate_dir, &crate_name, &plugins_dir) {
                                    Ok(()) => {
                                        info!("Plugin {} installed, hot reload should pick it up", crate_name);
                                        scene_state.build_state = BuildState::Success(crate_name);
                                    }
                                    Err(e) => {
                                        error!("Failed to install plugin: {}", e);
                                        scene_state.build_state = BuildState::Failed(vec![BuildError {
                                            message: e,
                                            file: None,
                                            line: None,
                                            column: None,
                                        }]);
                                    }
                                }
                            } else {
                                scene_state.build_state = BuildState::Success(crate_name);
                            }
                        }
                        Err(errors) => {
                            for err in &errors {
                                error!("Build error: {}", err.message);
                            }
                            scene_state.build_state = BuildState::Failed(errors);
                        }
                    }
                } else {
                    error!("Could not find Cargo.toml for this Rust file");
                    scene_state.build_state = BuildState::Failed(vec![BuildError {
                        message: "Could not find Cargo.toml for this file".to_string(),
                        file: None,
                        line: None,
                        column: None,
                    }]);
                }
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

                    // Get line count for gutter width calculation
                    let line_count = scene_state.open_scripts.get(active_idx)
                        .map(|s| s.content.lines().count().max(1))
                        .unwrap_or(1);
                    let font_size = 16.0;
                    let line_height = font_size * 1.3; // Approximate line height

                    // Calculate gutter width based on number of digits
                    let digit_count = (line_count as f32).log10().floor() as usize + 1;
                    let gutter_width = (digit_count.max(2) as f32 * 10.0) + 24.0; // min 2 digits + padding

                    egui::ScrollArea::both()
                        .max_width(content_width)
                        .max_height(content_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // Set the style for code editor
                            ui.style_mut().visuals.extreme_bg_color = Color32::from_rgb(25, 25, 30);
                            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(25, 25, 30);
                            ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(30, 30, 38);

                            ui.horizontal_top(|ui| {
                                // Line numbers gutter
                                ui.vertical(|ui| {
                                    ui.set_min_width(gutter_width);
                                    ui.set_max_width(gutter_width);

                                    // Gutter background
                                    let gutter_rect = ui.available_rect_before_wrap();
                                    ui.painter().rect_filled(
                                        egui::Rect::from_min_size(
                                            gutter_rect.min,
                                            Vec2::new(gutter_width, line_count as f32 * line_height + 20.0)
                                        ),
                                        0.0,
                                        Color32::from_rgb(30, 30, 35)
                                    );

                                    // Draw line numbers
                                    for line_num in 1..=line_count {
                                        let y_offset = (line_num - 1) as f32 * line_height + 2.0;
                                        ui.painter().text(
                                            egui::pos2(gutter_rect.min.x + gutter_width - 12.0, gutter_rect.min.y + y_offset),
                                            egui::Align2::RIGHT_TOP,
                                            format!("{}", line_num),
                                            FontId::new(font_size, FontFamily::Monospace),
                                            Color32::from_rgb(100, 100, 120),
                                        );
                                    }

                                    // Reserve space for the line numbers
                                    ui.allocate_space(Vec2::new(gutter_width, line_count as f32 * line_height));
                                });

                                // Separator line
                                let sep_rect = ui.available_rect_before_wrap();
                                ui.painter().vline(
                                    sep_rect.min.x,
                                    egui::Rangef::new(sep_rect.min.y, sep_rect.min.y + line_count as f32 * line_height + 20.0),
                                    egui::Stroke::new(1.0, Color32::from_rgb(50, 50, 60))
                                );

                                // Code editor
                                if let Some(script) = scene_state.open_scripts.get_mut(active_idx) {
                                    let use_rust_highlight = is_rust_file(&script.path);
                                    let mut layouter = |ui: &egui::Ui, text: &dyn TextBuffer, _wrap_width: f32| {
                                        let layout_job = if use_rust_highlight {
                                            highlight_rust(text.as_str(), font_size)
                                        } else {
                                            highlight_rhai(text.as_str(), font_size)
                                        };
                                        ui.fonts_mut(|f| f.layout_job(layout_job))
                                    };

                                    let editor_width = content_width - gutter_width - 4.0;
                                    let response = ui.add_sized(
                                        Vec2::new(editor_width, content_height),
                                        TextEdit::multiline(&mut script.content)
                                            .font(FontId::new(font_size, FontFamily::Monospace))
                                            .code_editor()
                                            .desired_width(editor_width)
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
        });

    // Error panel (if there's an error)
    if has_error {
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

                // Get error info based on file type
                let (location, message) = if is_rust {
                    // Rust build errors
                    if let BuildState::Failed(ref errors) = scene_state.build_state {
                        if let Some(first_error) = errors.first() {
                            let loc = match (&first_error.file, first_error.line, first_error.column) {
                                (Some(file), Some(line), Some(col)) => format!("{}:{}:{}", file, line, col),
                                (Some(file), Some(line), None) => format!("{}:{}", file, line),
                                (Some(file), None, None) => file.clone(),
                                _ => "Build error".to_string(),
                            };
                            let msg = if errors.len() > 1 {
                                format!("{} (+{} more errors)", first_error.message, errors.len() - 1)
                            } else {
                                first_error.message.clone()
                            };
                            (loc, msg)
                        } else {
                            ("Build error".to_string(), "Unknown error".to_string())
                        }
                    } else {
                        ("".to_string(), "".to_string())
                    }
                } else {
                    // Rhai script errors
                    if let Some(script) = scene_state.open_scripts.get(active_idx) {
                        if let Some(ref error) = script.error {
                            let loc = match (error.line, error.column) {
                                (Some(line), Some(col)) => format!("Line {}, Column {}", line, col),
                                (Some(line), None) => format!("Line {}", line),
                                _ => "Unknown location".to_string(),
                            };
                            (loc, error.message.clone())
                        } else {
                            ("".to_string(), "".to_string())
                        }
                    } else {
                        ("".to_string(), "".to_string())
                    }
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
                let message = if message.len() > max_chars {
                    format!("{}...", &message[..max_chars.saturating_sub(3)])
                } else {
                    message
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

/// Render script editor content into a docking panel
pub fn render_script_editor_content(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    scene_state: &mut SceneManagerState,
    current_project: Option<&crate::project::CurrentProject>,
) {
    // Check if a script tab is active
    let Some(active_idx) = scene_state.active_script_tab else {
        // No script open - show placeholder
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                egui::RichText::new(egui_phosphor::regular::CODE)
                    .size(48.0)
                    .color(Color32::from_gray(60)),
            );
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("No Script Open")
                    .size(16.0)
                    .color(Color32::from_gray(100)),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Double-click a script file in Assets to open it")
                    .size(12.0)
                    .color(Color32::from_gray(70)),
            );
        });
        return;
    };

    let Some(script) = scene_state.open_scripts.get_mut(active_idx) else {
        return;
    };

    let is_rust = is_rust_file(&script.path);

    // Check for errors if content changed (only for Rhai scripts, not Rust)
    if !is_rust && script.content != script.last_checked_content {
        script.error = check_script_errors(&script.content);
        script.last_checked_content = script.content.clone();
    }

    // For Rust files, show build errors instead of script errors
    let has_error = if is_rust {
        matches!(scene_state.build_state, BuildState::Failed(_))
    } else {
        script.error.is_some()
    };

    let available_rect = ui.available_rect_before_wrap();
    let panel_width = available_rect.width();
    let toolbar_height = 32.0;
    let error_panel_height = if has_error { 60.0 } else { 0.0 };

    // Draw toolbar background
    let toolbar_rect = egui::Rect::from_min_size(
        available_rect.min,
        Vec2::new(panel_width, toolbar_height),
    );
    ui.painter().rect_filled(toolbar_rect, 0.0, Color32::from_rgb(35, 35, 42));
    ui.painter().line_segment(
        [
            egui::pos2(toolbar_rect.min.x, toolbar_rect.max.y),
            egui::pos2(toolbar_rect.max.x, toolbar_rect.max.y),
        ],
        egui::Stroke::new(1.0, Color32::from_rgb(50, 50, 60)),
    );

    // Toolbar buttons
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

    // Build button (only for Rust files)
    let mut next_x = save_btn_rect.max.x + 8.0;
    let mut should_build = false;

    if is_rust {
        let build_btn_rect = egui::Rect::from_min_size(
            egui::pos2(next_x, toolbar_rect.min.y + 4.0),
            Vec2::new(75.0, 24.0),
        );
        next_x = build_btn_rect.max.x + 8.0;

        let is_building = matches!(scene_state.build_state, BuildState::Building);
        let build_response = ui.allocate_rect(build_btn_rect, if is_building { Sense::hover() } else { Sense::click() });
        let build_hovered = build_response.hovered();

        let build_bg = if is_building {
            Color32::from_rgb(50, 70, 90)
        } else if build_hovered {
            Color32::from_rgb(50, 50, 60)
        } else {
            Color32::from_rgb(40, 40, 50)
        };

        ui.painter().rect_filled(build_btn_rect, 4.0, build_bg);

        let (build_icon, build_text, icon_color) = match &scene_state.build_state {
            BuildState::Building => (SPINNER, "Building...", Color32::from_rgb(100, 180, 255)),
            BuildState::Success(_) => (CHECK_CIRCLE, "Build", Color32::from_rgb(100, 200, 100)),
            BuildState::Failed(_) => (HAMMER, "Build", Color32::from_rgb(255, 120, 120)),
            BuildState::Idle => (HAMMER, "Build", Color32::from_rgb(180, 180, 190)),
        };

        ui.painter().text(
            egui::pos2(build_btn_rect.min.x + 8.0, build_btn_rect.center().y),
            egui::Align2::LEFT_CENTER,
            build_icon,
            FontId::proportional(14.0),
            icon_color,
        );

        ui.painter().text(
            egui::pos2(build_btn_rect.min.x + 26.0, build_btn_rect.center().y),
            egui::Align2::LEFT_CENTER,
            build_text,
            FontId::proportional(12.0),
            Color32::from_rgb(180, 180, 190),
        );

        should_build = !is_building && (build_response.clicked() ||
            ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::B)));
    }

    // File path display
    let script_path = scene_state.open_scripts.get(active_idx)
        .map(|s| s.path.display().to_string())
        .unwrap_or_default();
    ui.painter().text(
        egui::pos2(next_x + 8.0, toolbar_rect.center().y),
        egui::Align2::LEFT_CENTER,
        &script_path,
        FontId::proportional(11.0),
        Color32::from_rgb(100, 100, 110),
    );

    // Check for save (button or Ctrl+S)
    let should_save = save_response.clicked() ||
        ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S));

    if should_save {
        if let Some(script) = scene_state.open_scripts.get_mut(active_idx) {
            save_script(script);
        }
    }

    // Handle build
    if should_build {
        if let Some(script) = scene_state.open_scripts.get(active_idx) {
            if let Some(cargo_toml) = find_cargo_toml(&script.path) {
                scene_state.build_state = BuildState::Building;
                let cargo_toml_clone = cargo_toml.clone();
                let plugins_dir = current_project.map(|p| p.path.join("plugins"));

                match build_rust_plugin(&cargo_toml_clone) {
                    Ok(crate_name) => {
                        info!("Build successful: {}", crate_name);
                        if let Some(plugins_dir) = plugins_dir {
                            let crate_dir = cargo_toml_clone.parent().unwrap().to_path_buf();
                            match copy_dll_to_plugins(&crate_dir, &crate_name, &plugins_dir) {
                                Ok(()) => {
                                    info!("Plugin {} installed", crate_name);
                                    scene_state.build_state = BuildState::Success(crate_name);
                                }
                                Err(e) => {
                                    error!("Failed to install plugin: {}", e);
                                    scene_state.build_state = BuildState::Failed(vec![BuildError {
                                        message: e,
                                        file: None,
                                        line: None,
                                        column: None,
                                    }]);
                                }
                            }
                        } else {
                            scene_state.build_state = BuildState::Success(crate_name);
                        }
                    }
                    Err(errors) => {
                        for err in &errors {
                            error!("Build error: {}", err.message);
                        }
                        scene_state.build_state = BuildState::Failed(errors);
                    }
                }
            } else {
                error!("Could not find Cargo.toml for this Rust file");
                scene_state.build_state = BuildState::Failed(vec![BuildError {
                    message: "Could not find Cargo.toml for this file".to_string(),
                    file: None,
                    line: None,
                    column: None,
                }]);
            }
        }
    }

    // Editor content area
    let editor_rect = egui::Rect::from_min_max(
        egui::pos2(available_rect.min.x, available_rect.min.y + toolbar_height),
        egui::pos2(available_rect.max.x, available_rect.max.y - error_panel_height),
    );

    let content_width = editor_rect.width();
    let content_height = editor_rect.height();

    // Get line count for gutter
    let line_count = scene_state.open_scripts.get(active_idx)
        .map(|s| s.content.lines().count().max(1))
        .unwrap_or(1);
    let font_size = 16.0;
    let line_height = font_size * 1.3;
    let digit_count = (line_count as f32).log10().floor() as usize + 1;
    let gutter_width = (digit_count.max(2) as f32 * 10.0) + 24.0;

    // Allocate space for editor area
    ui.allocate_space(Vec2::new(0.0, toolbar_height));

    egui::ScrollArea::both()
        .max_width(content_width)
        .max_height(content_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().visuals.extreme_bg_color = Color32::from_rgb(25, 25, 30);
            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(25, 25, 30);
            ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(30, 30, 38);

            ui.horizontal_top(|ui| {
                // Line numbers gutter
                ui.vertical(|ui| {
                    ui.set_min_width(gutter_width);
                    ui.set_max_width(gutter_width);

                    let gutter_rect = ui.available_rect_before_wrap();
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            gutter_rect.min,
                            Vec2::new(gutter_width, line_count as f32 * line_height + 20.0)
                        ),
                        0.0,
                        Color32::from_rgb(30, 30, 35)
                    );

                    for line_num in 1..=line_count {
                        let y_offset = (line_num - 1) as f32 * line_height + 2.0;
                        ui.painter().text(
                            egui::pos2(gutter_rect.min.x + gutter_width - 12.0, gutter_rect.min.y + y_offset),
                            egui::Align2::RIGHT_TOP,
                            format!("{}", line_num),
                            FontId::new(font_size, FontFamily::Monospace),
                            Color32::from_rgb(100, 100, 120),
                        );
                    }

                    ui.allocate_space(Vec2::new(gutter_width, line_count as f32 * line_height));
                });

                // Separator line
                let sep_rect = ui.available_rect_before_wrap();
                ui.painter().vline(
                    sep_rect.min.x,
                    egui::Rangef::new(sep_rect.min.y, sep_rect.min.y + line_count as f32 * line_height + 20.0),
                    egui::Stroke::new(1.0, Color32::from_rgb(50, 50, 60))
                );

                // Code editor
                if let Some(script) = scene_state.open_scripts.get_mut(active_idx) {
                    let use_rust_highlight = is_rust_file(&script.path);
                    let mut layouter = |ui: &egui::Ui, text: &dyn TextBuffer, _wrap_width: f32| {
                        let layout_job = if use_rust_highlight {
                            highlight_rust(text.as_str(), font_size)
                        } else {
                            highlight_rhai(text.as_str(), font_size)
                        };
                        ui.fonts_mut(|f| f.layout_job(layout_job))
                    };

                    let editor_width = content_width - gutter_width - 4.0;
                    let response = ui.add_sized(
                        Vec2::new(editor_width, content_height),
                        TextEdit::multiline(&mut script.content)
                            .font(FontId::new(font_size, FontFamily::Monospace))
                            .code_editor()
                            .desired_width(editor_width)
                            .lock_focus(true)
                            .layouter(&mut layouter)
                    );

                    if response.changed() {
                        script.is_modified = true;
                    }
                }
            });
        });

    // Error panel
    if has_error {
        let error_rect = egui::Rect::from_min_max(
            egui::pos2(available_rect.min.x, available_rect.max.y - error_panel_height),
            available_rect.max,
        );

        ui.painter().rect_filled(error_rect, 0.0, Color32::from_rgb(60, 30, 30));
        ui.painter().line_segment(
            [
                egui::pos2(error_rect.min.x, error_rect.min.y),
                egui::pos2(error_rect.max.x, error_rect.min.y),
            ],
            egui::Stroke::new(2.0, Color32::from_rgb(200, 80, 80)),
        );

        ui.painter().text(
            egui::pos2(error_rect.min.x + 12.0, error_rect.min.y + 20.0),
            egui::Align2::LEFT_CENTER,
            WARNING,
            FontId::proportional(18.0),
            Color32::from_rgb(255, 120, 120),
        );

        let (location, message) = if is_rust {
            if let BuildState::Failed(ref errors) = scene_state.build_state {
                if let Some(first_error) = errors.first() {
                    let loc = match (&first_error.file, first_error.line, first_error.column) {
                        (Some(file), Some(line), Some(col)) => format!("{}:{}:{}", file, line, col),
                        (Some(file), Some(line), None) => format!("{}:{}", file, line),
                        (Some(file), None, None) => file.clone(),
                        _ => "Build error".to_string(),
                    };
                    let msg = if errors.len() > 1 {
                        format!("{} (+{} more errors)", first_error.message, errors.len() - 1)
                    } else {
                        first_error.message.clone()
                    };
                    (loc, msg)
                } else {
                    ("Build error".to_string(), "Unknown error".to_string())
                }
            } else {
                ("".to_string(), "".to_string())
            }
        } else {
            if let Some(script) = scene_state.open_scripts.get(active_idx) {
                if let Some(ref error) = script.error {
                    let loc = match (error.line, error.column) {
                        (Some(line), Some(col)) => format!("Line {}, Column {}", line, col),
                        (Some(line), None) => format!("Line {}", line),
                        _ => "Unknown location".to_string(),
                    };
                    (loc, error.message.clone())
                } else {
                    ("".to_string(), "".to_string())
                }
            } else {
                ("".to_string(), "".to_string())
            }
        };

        ui.painter().text(
            egui::pos2(error_rect.min.x + 36.0, error_rect.min.y + 16.0),
            egui::Align2::LEFT_CENTER,
            &location,
            FontId::proportional(11.0),
            Color32::from_rgb(255, 150, 150),
        );

        let max_chars = ((panel_width - 50.0) / 7.0) as usize;
        let message = if message.len() > max_chars {
            format!("{}...", &message[..max_chars.saturating_sub(3)])
        } else {
            message
        };

        ui.painter().text(
            egui::pos2(error_rect.min.x + 36.0, error_rect.min.y + 38.0),
            egui::Align2::LEFT_CENTER,
            &message,
            FontId::new(12.0, FontFamily::Monospace),
            Color32::from_rgb(220, 180, 180),
        );
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

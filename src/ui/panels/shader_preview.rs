#![allow(dead_code)]

use bevy_egui::egui::{self, Color32, Pos2, Rect, TextureId, Vec2};

use crate::core::SceneManagerState;
use crate::shader_preview::{ShaderPreviewState, ShaderCompileStatus, ShaderType, detect_shader_type, parse_workgroup_size};
use renzora_theme::Theme;

use egui_phosphor::regular::{ARROW_CLOCKWISE, CHECK_CIRCLE, CLIPBOARD, WARNING, X_CIRCLE};

/// Render the shader preview panel content
pub fn render_shader_preview_content(
    ui: &mut egui::Ui,
    preview_state: &mut ShaderPreviewState,
    scene_state: &SceneManagerState,
    theme: &Theme,
    preview_texture_id: Option<TextureId>,
) {
    let available = ui.available_rect_before_wrap();
    let toolbar_height = 32.0;
    let status_bar_height = 28.0;

    // Toolbar
    let toolbar_rect = egui::Rect::from_min_size(
        available.min,
        Vec2::new(available.width(), toolbar_height),
    );
    ui.painter().rect_filled(toolbar_rect, 0.0, theme.surfaces.panel.to_color32());
    ui.painter().line_segment(
        [
            egui::pos2(toolbar_rect.min.x, toolbar_rect.max.y),
            egui::pos2(toolbar_rect.max.x, toolbar_rect.max.y),
        ],
        egui::Stroke::new(1.0, theme.surfaces.window_stroke.to_color32()),
    );

    // Toolbar buttons
    let btn_rect = egui::Rect::from_min_size(
        egui::pos2(toolbar_rect.min.x + 8.0, toolbar_rect.min.y + 4.0),
        Vec2::new(90.0, 24.0),
    );

    let recompile_response = ui.allocate_rect(btn_rect, egui::Sense::click());
    let btn_hovered = recompile_response.hovered();
    if btn_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let btn_bg = if btn_hovered {
        theme.surfaces.popup.to_color32()
    } else {
        theme.surfaces.faint.to_color32()
    };
    ui.painter().rect_filled(btn_rect, 4.0, btn_bg);
    ui.painter().text(
        egui::pos2(btn_rect.min.x + 8.0, btn_rect.center().y),
        egui::Align2::LEFT_CENTER,
        ARROW_CLOCKWISE,
        egui::FontId::proportional(14.0),
        theme.text.secondary.to_color32(),
    );
    ui.painter().text(
        egui::pos2(btn_rect.min.x + 26.0, btn_rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Recompile",
        egui::FontId::proportional(12.0),
        theme.text.secondary.to_color32(),
    );

    if recompile_response.clicked() {
        preview_state.needs_recompile = true;
    }

    // Auto-recompile toggle
    let toggle_x = btn_rect.max.x + 16.0;
    let toggle_rect = egui::Rect::from_min_size(
        egui::pos2(toggle_x, toolbar_rect.min.y + 6.0),
        Vec2::new(120.0, 20.0),
    );

    let toggle_response = ui.allocate_rect(toggle_rect, egui::Sense::click());
    if toggle_response.clicked() {
        preview_state.auto_recompile = !preview_state.auto_recompile;
    }

    let check_color = if preview_state.auto_recompile {
        theme.semantic.accent.to_color32()
    } else {
        theme.text.disabled.to_color32()
    };
    let check_box = egui::Rect::from_min_size(
        egui::pos2(toggle_x, toolbar_rect.min.y + 8.0),
        Vec2::splat(14.0),
    );
    ui.painter().rect_stroke(check_box, 2.0, egui::Stroke::new(1.0, check_color), egui::StrokeKind::Outside);
    if preview_state.auto_recompile {
        ui.painter().rect_filled(check_box.shrink(2.0), 1.0, check_color);
    }
    ui.painter().text(
        egui::pos2(toggle_x + 20.0, toolbar_rect.min.y + 15.0),
        egui::Align2::LEFT_CENTER,
        "Auto-recompile",
        egui::FontId::proportional(11.0),
        theme.text.muted.to_color32(),
    );

    // Auto-recompile: check if active script is WGSL and content changed
    if preview_state.auto_recompile {
        if let Some(active_idx) = scene_state.active_script_tab {
            if let Some(script) = scene_state.open_scripts.get(active_idx) {
                let is_wgsl = script.path.extension()
                    .map(|ext| ext == "wgsl")
                    .unwrap_or(false);
                if is_wgsl && script.content != preview_state.last_validated_source {
                    preview_state.needs_recompile = true;
                }
            }
        }
    }

    // Perform recompile if requested
    if preview_state.needs_recompile {
        preview_state.needs_recompile = false;
        if let Some(active_idx) = scene_state.active_script_tab {
            if let Some(script) = scene_state.open_scripts.get(active_idx) {
                let is_wgsl = script.path.extension()
                    .map(|ext| ext == "wgsl")
                    .unwrap_or(false);
                if is_wgsl {
                    preview_state.active_shader_path = Some(script.path.display().to_string());
                    preview_state.last_validated_source = script.content.clone();

                    // Detect shader type and parse workgroup size
                    if let Some(shader_type) = detect_shader_type(&script.content) {
                        preview_state.shader_type = shader_type;
                        if shader_type == ShaderType::Compute {
                            preview_state.workgroup_size = parse_workgroup_size(&script.content);
                        }

                        // Push shader to GPU immediately — Bevy's pipeline cache
                        // handles validation with full #import resolution.
                        // PBR shaders use ExtendedMaterial<StandardMaterial, ...>
                        // for proper bind group layout.
                        preview_state.compile_status = ShaderCompileStatus::Compiling;
                        preview_state.needs_gpu_update = true;
                    } else {
                        preview_state.compile_status = ShaderCompileStatus::Error(
                            "Vertex-only shaders cannot be previewed.\n\
                             The shader preview supports @fragment and @compute shaders."
                                .to_string(),
                        );
                    }
                } else {
                    preview_state.compile_status = ShaderCompileStatus::Error(
                        "Active file is not a .wgsl shader".to_string()
                    );
                }
            } else {
                preview_state.compile_status = ShaderCompileStatus::Error(
                    "No file open in the code editor".to_string()
                );
            }
        } else {
            preview_state.compile_status = ShaderCompileStatus::Error(
                "No file open in the code editor".to_string()
            );
        }
    }

    // Allocate toolbar space
    ui.allocate_space(Vec2::new(0.0, toolbar_height));

    // Preview content area
    let content_rect = egui::Rect::from_min_max(
        egui::pos2(available.min.x, available.min.y + toolbar_height),
        egui::pos2(available.max.x, available.max.y - status_bar_height),
    );

    // Show preview content based on state
    match &preview_state.compile_status {
        ShaderCompileStatus::Idle => {
            // Show texture if available (default animated shader), otherwise placeholder text
            if let Some(texture_id) = preview_texture_id {
                render_preview_texture(ui, texture_id, content_rect, theme);
            } else {
                ui.painter().text(
                    content_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No Shader Loaded\n\nOpen a .wgsl file and click Recompile",
                    egui::FontId::proportional(14.0),
                    theme.text.disabled.to_color32(),
                );
            }
        }
        ShaderCompileStatus::Compiling => {
            // Show previous texture (if any) with a "Compiling..." overlay
            if let Some(texture_id) = preview_texture_id {
                render_preview_texture(ui, texture_id, content_rect, theme);
            }
            ui.painter().rect_filled(
                content_rect,
                0.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 120),
            );
            ui.painter().text(
                content_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Compiling...",
                egui::FontId::proportional(16.0),
                theme.text.secondary.to_color32(),
            );
        }
        ShaderCompileStatus::Compiled => {
            // Show the rendered shader preview
            if let Some(texture_id) = preview_texture_id {
                render_preview_texture(ui, texture_id, content_rect, theme);

                // Overlay shader name
                let shader_name = preview_state.active_shader_path.as_deref().unwrap_or("default");
                let name_short = std::path::Path::new(shader_name)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| shader_name.to_string());
                ui.painter().text(
                    Pos2::new(content_rect.min.x + 8.0, content_rect.min.y + 8.0),
                    egui::Align2::LEFT_TOP,
                    name_short,
                    egui::FontId::proportional(10.0),
                    Color32::from_rgba_unmultiplied(200, 200, 200, 180),
                );
            } else {
                // Fallback text when texture not ready
                let shader_name = preview_state.active_shader_path.as_deref().unwrap_or("unknown");
                ui.painter().text(
                    egui::pos2(content_rect.center().x, content_rect.center().y - 20.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{} Shader Valid", CHECK_CIRCLE),
                    egui::FontId::proportional(18.0),
                    theme.semantic.success.to_color32(),
                );
                ui.painter().text(
                    egui::pos2(content_rect.center().x, content_rect.center().y + 10.0),
                    egui::Align2::CENTER_CENTER,
                    shader_name,
                    egui::FontId::proportional(11.0),
                    theme.text.muted.to_color32(),
                );
            }
        }
        ShaderCompileStatus::Error(msg) => {
            ui.painter().text(
                egui::pos2(content_rect.center().x, content_rect.min.y + 30.0),
                egui::Align2::CENTER_CENTER,
                format!("{} Compilation Error", X_CIRCLE),
                egui::FontId::proportional(16.0),
                theme.semantic.error.to_color32(),
            );

            // Copy error button
            let copy_btn_rect = egui::Rect::from_min_size(
                egui::pos2(content_rect.max.x - 80.0, content_rect.min.y + 20.0),
                Vec2::new(68.0, 22.0),
            );
            let copy_response = ui.allocate_rect(copy_btn_rect, egui::Sense::click());
            let copy_hovered = copy_response.hovered();
            if copy_hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let copy_bg = if copy_hovered {
                theme.surfaces.popup.to_color32()
            } else {
                theme.surfaces.faint.to_color32()
            };
            ui.painter().rect_filled(copy_btn_rect, 4.0, copy_bg);
            ui.painter().text(
                egui::pos2(copy_btn_rect.min.x + 6.0, copy_btn_rect.center().y),
                egui::Align2::LEFT_CENTER,
                CLIPBOARD,
                egui::FontId::proportional(12.0),
                theme.text.secondary.to_color32(),
            );
            ui.painter().text(
                egui::pos2(copy_btn_rect.min.x + 22.0, copy_btn_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "Copy",
                egui::FontId::proportional(11.0),
                theme.text.secondary.to_color32(),
            );
            if copy_response.clicked() {
                ui.ctx().copy_text(msg.clone());
            }

            let error_area = egui::Rect::from_min_max(
                egui::pos2(content_rect.min.x + 12.0, content_rect.min.y + 50.0),
                egui::pos2(content_rect.max.x - 12.0, content_rect.max.y - 4.0),
            );

            ui.painter().rect_filled(
                error_area,
                4.0,
                Color32::from_rgb(40, 25, 25),
            );

            let old_clip = ui.clip_rect();
            ui.set_clip_rect(error_area);
            ui.painter().text(
                egui::pos2(error_area.min.x + 8.0, error_area.min.y + 8.0),
                egui::Align2::LEFT_TOP,
                msg,
                egui::FontId::new(11.0, egui::FontFamily::Monospace),
                theme.semantic.error.to_color32(),
            );
            ui.set_clip_rect(old_clip);
        }
    }

    // Allocate remaining space
    let remaining = (available.height() - toolbar_height - status_bar_height).max(0.0);
    ui.allocate_space(Vec2::new(0.0, remaining));

    // Status bar at bottom
    let status_rect = egui::Rect::from_min_size(
        egui::pos2(available.min.x, available.max.y - status_bar_height),
        Vec2::new(available.width(), status_bar_height),
    );

    let (status_bg, status_icon, status_text, status_color) = match &preview_state.compile_status {
        ShaderCompileStatus::Idle => (
            theme.surfaces.faint.to_color32(),
            "",
            "No shader loaded",
            theme.text.disabled.to_color32(),
        ),
        ShaderCompileStatus::Compiling => (
            Color32::from_rgb(35, 35, 50),
            ARROW_CLOCKWISE,
            "Compiling...",
            Color32::from_rgb(160, 180, 255),
        ),
        ShaderCompileStatus::Compiled => (
            Color32::from_rgb(25, 50, 30),
            CHECK_CIRCLE,
            "Compiled successfully",
            Color32::from_rgb(100, 200, 100),
        ),
        ShaderCompileStatus::Error(_) => (
            Color32::from_rgb(50, 25, 25),
            WARNING,
            "Compilation failed",
            Color32::from_rgb(255, 120, 120),
        ),
    };

    ui.painter().rect_filled(status_rect, 0.0, status_bg);
    ui.painter().line_segment(
        [
            egui::pos2(status_rect.min.x, status_rect.min.y),
            egui::pos2(status_rect.max.x, status_rect.min.y),
        ],
        egui::Stroke::new(1.0, theme.surfaces.window_stroke.to_color32()),
    );

    if !status_icon.is_empty() {
        ui.painter().text(
            egui::pos2(status_rect.min.x + 10.0, status_rect.center().y),
            egui::Align2::LEFT_CENTER,
            status_icon,
            egui::FontId::proportional(14.0),
            status_color,
        );
    }

    ui.painter().text(
        egui::pos2(status_rect.min.x + 28.0, status_rect.center().y),
        egui::Align2::LEFT_CENTER,
        status_text,
        egui::FontId::proportional(11.0),
        status_color,
    );

    // Shader type badge on right side of status bar
    let type_label = match preview_state.shader_type {
        ShaderType::Fragment => "Fragment",
        ShaderType::PbrFragment => "PBR",
        ShaderType::Compute => "Compute",
    };
    let type_color = match preview_state.shader_type {
        ShaderType::Fragment => Color32::from_rgb(120, 160, 220),
        ShaderType::PbrFragment => Color32::from_rgb(180, 140, 220),
        ShaderType::Compute => Color32::from_rgb(220, 160, 80),
    };
    ui.painter().text(
        egui::pos2(status_rect.max.x - 10.0, status_rect.center().y),
        egui::Align2::RIGHT_CENTER,
        type_label,
        egui::FontId::proportional(10.0),
        type_color,
    );
}

/// Render the preview texture into the content area, maintaining aspect ratio
fn render_preview_texture(
    ui: &mut egui::Ui,
    texture_id: TextureId,
    content_rect: Rect,
    theme: &Theme,
) {
    // Fill background
    ui.painter().rect_filled(content_rect, 0.0, Color32::BLACK);

    // Calculate display rect maintaining 1:1 aspect (texture is 512x512)
    let panel_aspect = content_rect.width() / content_rect.height();
    let (display_width, display_height) = if panel_aspect > 1.0 {
        (content_rect.height(), content_rect.height())
    } else {
        (content_rect.width(), content_rect.width())
    };

    let display_rect = Rect::from_center_size(
        content_rect.center(),
        Vec2::new(display_width, display_height),
    );

    // Draw the preview image
    ui.painter().image(
        texture_id,
        display_rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );

    // Draw border
    ui.painter().rect_stroke(
        display_rect,
        0.0,
        egui::Stroke::new(1.0, theme.surfaces.window_stroke.to_color32()),
        egui::StrokeKind::Inside,
    );
}

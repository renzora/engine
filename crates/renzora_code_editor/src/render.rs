use renzora::bevy_egui::egui::{self, CursorIcon, FontFamily, FontId, RichText, Sense};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use renzora::theme::Theme;

use std::path::PathBuf;

use renzora::egui_phosphor::regular::{CODE, FILE_PLUS, FLOPPY_DISK, WARNING, X};

use crate::state::CodeEditorState;

fn syntax_for_extension(ext: &str) -> Syntax {
    match ext {
        "lua" => Syntax::lua(),
        "rs" => Syntax::rust(),
        "py" => Syntax::python(),
        "sh" | "bash" => Syntax::shell(),
        "sql" => Syntax::sql(),
        "wgsl" => Syntax::new("wgsl")
            .with_comment("//")
            .with_comment_multiline(["/*", "*/"])
            .with_keywords([
                "fn", "let", "var", "const", "struct", "if", "else", "for", "while", "loop",
                "return", "discard", "switch", "case", "default", "break", "continue",
                "enable", "override", "alias",
            ])
            .with_types([
                "bool", "i32", "u32", "f32", "f16",
                "vec2", "vec3", "vec4", "vec2i", "vec3i", "vec4i",
                "vec2u", "vec3u", "vec4u", "vec2f", "vec3f", "vec4f",
                "mat2x2", "mat2x3", "mat2x4", "mat3x2", "mat3x3", "mat3x4",
                "mat4x2", "mat4x3", "mat4x4",
                "mat2x2f", "mat3x3f", "mat4x4f",
                "texture_2d", "texture_3d", "texture_cube",
                "sampler", "sampler_comparison", "array", "atomic", "ptr",
            ])
            .with_special([
                "textureSample", "textureSampleLevel", "textureLoad", "textureStore",
                "dot", "cross", "normalize", "length", "distance",
                "mix", "clamp", "smoothstep", "step",
                "abs", "ceil", "floor", "round", "fract",
                "cos", "sin", "tan", "exp", "log", "pow", "sqrt",
                "min", "max", "saturate", "select",
                "transpose", "determinant",
            ]),
        "rhai" => Syntax::new("rhai")
            .with_comment("//")
            .with_comment_multiline(["/*", "*/"])
            .with_keywords([
                "let", "const", "fn", "if", "else", "while", "for", "in", "loop",
                "break", "continue", "return", "throw", "try", "catch", "switch",
                "import", "export", "as", "private", "this", "do", "until",
            ])
            .with_special([
                "print", "debug", "type_of", "is",
            ]),
        _ => Syntax::lua(),
    }
}

/// Render the code editor panel content.
pub fn render_code_editor_content(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    theme: &Theme,
    scripts_dir: Option<PathBuf>,
) {
    let muted = theme.text.muted.to_color32();
    let disabled = theme.text.disabled.to_color32();
    let secondary = theme.text.secondary.to_color32();
    let error_color = theme.semantic.error.to_color32();
    let surface_panel = theme.surfaces.panel.to_color32();

    // Empty state
    if state.open_files.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new(CODE).size(48.0).color(disabled));
            ui.add_space(12.0);
            ui.label(RichText::new("No file open").size(16.0).color(muted));
            ui.add_space(8.0);
            ui.label(
                RichText::new("Double-click a script in the Asset Browser to open it")
                    .size(12.0)
                    .color(disabled),
            );
            ui.add_space(16.0);
            if let Some(ref dir) = scripts_dir {
                if ui
                    .button(RichText::new(format!("{} New Script", FILE_PLUS)).size(13.0))
                    .clicked()
                {
                    state.create_new_script(dir.clone());
                }
            }
        });
        return;
    }

    // --- Tab bar ---
    ui.horizontal(|ui| {
        let mut switch_to = None;
        let mut close_tab = None;

        for (idx, file) in state.open_files.iter().enumerate() {
            let is_active = state.active_tab == Some(idx);
            let tab_bg = if is_active {
                surface_panel
            } else {
                theme.surfaces.faint.to_color32()
            };

            let label = if file.is_modified {
                format!("{} *", file.name)
            } else {
                file.name.clone()
            };

            egui::Frame::new()
                .fill(tab_bg)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let tab_resp = ui.add(
                            egui::Label::new(RichText::new(&label).size(11.0)).sense(Sense::click()),
                        );
                        if tab_resp.clicked() {
                            switch_to = Some(idx);
                        }

                        let close_resp = ui.add(
                            egui::Button::new(RichText::new(X).size(10.0).color(muted))
                                .frame(false),
                        );
                        if close_resp.clicked() {
                            close_tab = Some(idx);
                        }
                        if close_resp.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                    });
                });
        }

        if let Some(idx) = switch_to {
            state.active_tab = Some(idx);
        }
        if let Some(idx) = close_tab {
            state.close_tab(idx);
        }
    });

    ui.separator();

    let Some(active_idx) = state.active_tab else {
        return;
    };
    if active_idx >= state.open_files.len() {
        return;
    }

    let file_ext = state.open_files[active_idx]
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    let has_error = state.open_files[active_idx].error.is_some();

    // --- Toolbar ---
    egui::Frame::new()
        .fill(surface_panel)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let save_btn =
                    ui.button(RichText::new(format!("{} Save", FLOPPY_DISK)).size(12.0));
                if save_btn.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }

                if let Some(ref dir) = scripts_dir {
                    let new_btn = ui
                        .button(RichText::new(FILE_PLUS).size(12.0))
                        .on_hover_text("New Script");
                    if new_btn.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if new_btn.clicked() {
                        state.create_new_script(dir.clone());
                    }
                }

                ui.separator();

                // Zoom controls
                let zoom_pct = (state.font_size / 16.0 * 100.0).round() as i32;
                if ui
                    .small_button(RichText::new("\u{2212}").size(12.0))
                    .clicked()
                {
                    state.zoom_out();
                }
                let zoom_label = ui.add(
                    egui::Label::new(
                        RichText::new(format!("{}%", zoom_pct))
                            .size(11.0)
                            .color(muted),
                    )
                    .sense(Sense::click()),
                );
                if zoom_label.on_hover_text("Reset zoom").clicked() {
                    state.zoom_reset();
                }
                if ui
                    .small_button(RichText::new("+").size(12.0))
                    .clicked()
                {
                    state.zoom_in();
                }

                // File path (right-aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let path_str = state.open_files[active_idx].path.display().to_string();
                    ui.label(RichText::new(path_str).size(11.0).color(disabled));
                });

                // Handle Ctrl+S
                let should_save = save_btn.clicked()
                    || ui
                        .ctx()
                        .input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S));
                if should_save {
                    state.save_active();
                }
            });
        });

    // Handle Ctrl+scroll zoom
    let panel_hovered = ui.rect_contains_pointer(ui.max_rect());
    let zoom_delta = if panel_hovered {
        ui.ctx().input(|i| {
            if i.modifiers.ctrl {
                let scroll = i.raw_scroll_delta.y;
                if scroll.abs() > 0.5 {
                    return scroll.signum();
                }
            }
            0.0
        })
    } else {
        0.0
    };
    if zoom_delta > 0.0 {
        state.zoom_in();
    } else if zoom_delta < 0.0 {
        state.zoom_out();
    }

    // --- Editor area ---
    let font_size = state.font_size;
    let error_panel_height = if has_error { 60.0 } else { 0.0 };
    let available_rect = ui.available_rect_before_wrap();

    let syntax = syntax_for_extension(&file_ext);

    // Remove focus/selection border highlight and fill the panel
    {
        let style = ui.style_mut();
        let no_stroke = egui::Stroke::NONE;
        style.visuals.widgets.active.bg_stroke = no_stroke;
        style.visuals.widgets.hovered.bg_stroke = no_stroke;
        style.visuals.widgets.inactive.bg_stroke = no_stroke;
        style.visuals.widgets.noninteractive.bg_stroke = no_stroke;
        style.visuals.selection.stroke = no_stroke;
        style.spacing.item_spacing = egui::vec2(0.0, 0.0);
    }

    let row_height = ui.fonts_mut(|f| f.row_height(&egui::FontId::monospace(font_size)));
    let available_height = (available_rect.height() - error_panel_height).max(row_height);
    let min_rows = (available_height / row_height).floor().max(1.0) as usize;

    if let Some(file) = state.open_files.get_mut(active_idx) {
        // Size the textarea to fit content, with a minimum of the panel height
        let content_lines = file.content.lines().count().max(1);
        let rows = content_lines.max(min_rows);
        let scrollbar_width = ui.style().spacing.scroll.bar_outer_margin
            + ui.style().spacing.scroll.bar_width
            + ui.style().spacing.scroll.bar_inner_margin;

        // Panel scroll handles all scrolling; CodeEditor's internal scroll is disabled
        egui::ScrollArea::vertical()
            .id_salt("code_editor_panel_scroll")
            .max_height(available_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Use width inside the scroll area, minus scrollbar
                let editor_width = (ui.available_width() - scrollbar_width).max(100.0);

                let output = CodeEditor::default()
                    .id_source(format!("code_editor_{}", active_idx))
                    .with_fontsize(font_size)
                    .with_theme(ColorTheme::GRUVBOX)
                    .with_syntax(syntax)
                    .with_numlines(true)
                    .with_rows(rows)
                    .vscroll(false)
                    .desired_width(editor_width)
                    .show(ui, &mut file.content);

                if output.response.changed() {
                    file.is_modified = true;
                }

                // Auto-scroll to cursor
                if output.response.has_focus() {
                    if let Some(cursor_range) = output.cursor_range {
                        let idx = cursor_range.primary.index;
                        let cursor_row = file.content[..idx.min(file.content.len())]
                            .chars()
                            .filter(|c| *c == '\n')
                            .count();
                        let cursor_y = cursor_row as f32 * row_height;
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(0.0, cursor_y),
                            egui::vec2(1.0, row_height * 2.0),
                        );
                        ui.scroll_to_rect(rect, Some(egui::Align::Center));
                    }
                }
            });
    }

    // --- Error panel ---
    if has_error {
        if let Some(file) = state.open_files.get(active_idx) {
            if let Some(ref error) = file.error {
                let error_rect = egui::Rect::from_min_max(
                    egui::pos2(available_rect.min.x, available_rect.max.y - error_panel_height),
                    available_rect.max,
                );

                ui.painter()
                    .rect_filled(error_rect, 0.0, egui::Color32::from_rgb(60, 30, 30));
                ui.painter().line_segment(
                    [
                        egui::pos2(error_rect.min.x, error_rect.min.y),
                        egui::pos2(error_rect.max.x, error_rect.min.y),
                    ],
                    egui::Stroke::new(2.0, error_color),
                );

                ui.painter().text(
                    egui::pos2(error_rect.min.x + 12.0, error_rect.min.y + 20.0),
                    egui::Align2::LEFT_CENTER,
                    WARNING,
                    FontId::proportional(18.0),
                    error_color,
                );

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
                    error_color,
                );

                let panel_width = available_rect.width();
                let max_chars = ((panel_width - 50.0) / 7.0) as usize;
                let msg = if error.message.len() > max_chars {
                    format!("{}...", &error.message[..max_chars.saturating_sub(3)])
                } else {
                    error.message.clone()
                };

                ui.painter().text(
                    egui::pos2(error_rect.min.x + 36.0, error_rect.min.y + 38.0),
                    egui::Align2::LEFT_CENTER,
                    &msg,
                    FontId::new(12.0, FontFamily::Monospace),
                    secondary,
                );
            }
        }
    }
}

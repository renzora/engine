use bevy_egui::egui::{
    self, text::CCursor, text::CCursorRange, Align2, Color32, CursorIcon, FontFamily, FontId, Id,
    Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use egui_phosphor::regular::{
    ARROW_RIGHT, CODE, FILE_PLUS, FLOPPY_DISK, LIST_NUMBERS, MAGNIFYING_GLASS, WARNING, X,
};
use renzora_theme::Theme;
use std::path::PathBuf;

use crate::actions::{
    byte_to_char, byte_to_line, char_to_byte, find_matching_bracket, line_to_byte,
    toggle_line_comment,
};
use crate::autocomplete::{self, ApiSymbol};
use crate::highlight::{highlight, Language, TokenStyle};
use crate::state::CodeEditorState;

/// Width reserved for the line-number gutter (scales with font size).
fn gutter_width(line_count: usize, font_size: f32) -> f32 {
    let digits = line_count.max(1).to_string().len() as f32;
    // ~0.6 em per monospace digit + 10px padding on each side.
    (digits * font_size * 0.62) + 20.0
}

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

    // Empty state -----------------------------------------------------------
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

    // Tab bar ---------------------------------------------------------------
    render_tab_bar(ui, state, theme);
    ui.separator();

    let Some(active_idx) = state.active_tab else { return; };
    if active_idx >= state.open_files.len() {
        return;
    }

    let file_ext = state.open_files[active_idx]
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    let lang = Language::from_extension(&file_ext);
    let has_error = state.open_files[active_idx].error.is_some();

    // Toolbar + shortcut handling -------------------------------------------
    render_toolbar(ui, state, theme, &scripts_dir, active_idx);
    handle_global_shortcuts(ui, state, lang, active_idx);

    // Find + goto bars ------------------------------------------------------
    if state.find_open {
        render_find_bar(ui, state, theme);
    }
    if state.goto_line_open {
        render_goto_line_bar(ui, state, theme, active_idx);
    }

    // Editor area -----------------------------------------------------------
    let font_size = state.font_size;
    let error_panel_height = if has_error { 60.0 } else { 0.0 };
    let available_rect = ui.available_rect_before_wrap();

    // Remove focus/selection borders so the editor fills cleanly.
    {
        let style = ui.style_mut();
        let no_stroke = Stroke::NONE;
        style.visuals.widgets.active.bg_stroke = no_stroke;
        style.visuals.widgets.hovered.bg_stroke = no_stroke;
        style.visuals.widgets.inactive.bg_stroke = no_stroke;
        style.visuals.widgets.noninteractive.bg_stroke = no_stroke;
        style.visuals.selection.stroke = no_stroke;
        style.spacing.item_spacing = Vec2::ZERO;
    }

    let row_height = ui.fonts_mut(|f| f.row_height(&FontId::monospace(font_size)));
    let available_height = (available_rect.height() - error_panel_height).max(row_height);

    let editor_id = Id::new(("code_editor_textedit", active_idx));

    let style = TokenStyle::from_theme(FontId::monospace(font_size), theme);

    // Snapshot data we need after the scroll area.
    let file_content_line_count;
    let cursor_char_idx_after: Option<usize>;
    let editor_rect_after: Rect;
    let text_start_x;
    let should_scroll_to_cursor;

    // Pending autocomplete trigger (filled inside scroll area).
    let mut trigger_autocomplete = false;
    // Pending comment toggle key (processed after TextEdit to know cursor).
    let mut comment_key_pressed = false;

    // --- Pre-TextEdit keyboard handling: consume keys while popup is open ---
    let autocomplete_is_open = state.autocomplete_open;
    let mut popup_enter_insert = false;
    let mut popup_tab_insert = false;
    let mut popup_close = false;
    let mut popup_up = false;
    let mut popup_down = false;

    ui.input_mut(|i| {
        if autocomplete_is_open {
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                popup_enter_insert = true;
            }
            if i.consume_key(Modifiers::NONE, Key::Tab) {
                popup_tab_insert = true;
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                popup_close = true;
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                popup_up = true;
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                popup_down = true;
            }
        }
    });

    // --- Ctrl+Space (open autocomplete) ---
    ui.input_mut(|i| {
        if i.consume_key(Modifiers::CTRL, Key::Space) {
            trigger_autocomplete = true;
        }
        // Ctrl+/ — comment toggle (key is `/`, sometimes `Slash`)
        if i.consume_key(Modifiers::CTRL, Key::Slash) {
            comment_key_pressed = true;
        }
    });

    // --- Scroll area with gutter + TextEdit ---
    let scroll_id = Id::new(("code_editor_panel_scroll", active_idx));
    let scroll_output = egui::ScrollArea::vertical()
        .id_salt(scroll_id)
        .max_height(available_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let line_count = state.open_files[active_idx]
                .content
                .lines()
                .count()
                .max(1);
            let gw = gutter_width(line_count, font_size);

            // Prepare layouter borrowing the theme style.
            let mut layouter =
                |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                    let mut job = highlight(text.as_str(), lang, &style);
                    job.wrap.max_width = wrap_width;
                    ui.painter().layout_job(job)
                };

            // Lay out: [gutter space | TextEdit]
            let start_x = ui.cursor().min.x;
            ui.horizontal_top(|ui| {
                // Reserve gutter space — painted after TextEdit.
                ui.add_space(gw);

                let file = &mut state.open_files[active_idx];
                let avail = ui.available_width().max(100.0);

                let output = egui::TextEdit::multiline(&mut file.content)
                    .id(editor_id)
                    .font(FontId::monospace(font_size))
                    .code_editor()
                    .lock_focus(true)
                    .frame(false)
                    .layouter(&mut layouter)
                    .desired_width(avail)
                    .desired_rows(1)
                    .show(ui);

                if output.response.changed() {
                    file.is_modified = true;
                }

                // Gather state we need outside the closure.
                let editor_rect = output.response.rect;
                let cur_char_idx = output
                    .cursor_range
                    .map(|c| c.primary.index);

                // Paint gutter in reserved strip ----------------------------
                let gutter_rect = Rect::from_min_max(
                    Pos2::new(start_x, editor_rect.min.y),
                    Pos2::new(start_x + gw, editor_rect.max.y),
                );
                paint_gutter(ui, gutter_rect, font_size, row_height, line_count, cur_char_idx, &file.content, theme);

                // Bracket match highlight -----------------------------------
                if let Some(cidx) = cur_char_idx {
                    let byte_idx = char_to_byte(&file.content, cidx);
                    if let Some((a, b)) = find_matching_bracket(&file.content, byte_idx) {
                        paint_bracket_highlight(ui, &file.content, editor_rect, row_height, font_size, a, theme);
                        paint_bracket_highlight(ui, &file.content, editor_rect, row_height, font_size, b, theme);
                    }
                }

                // Scroll to cursor only when the cursor actually moved ------
                let mut should_scroll = false;
                if let Some(cidx) = cur_char_idx {
                    if file.last_cursor_index != Some(cidx) {
                        // Compute cursor rect
                        let byte_idx = char_to_byte(&file.content, cidx);
                        let line = byte_to_line(&file.content, byte_idx);
                        let y = editor_rect.min.y + line as f32 * row_height;
                        let cursor_rect = Rect::from_min_size(
                            Pos2::new(editor_rect.min.x, y),
                            Vec2::new(2.0, row_height),
                        );
                        let clip = ui.clip_rect();
                        // Only scroll if cursor is actually out of view
                        if !(clip.min.y <= cursor_rect.min.y && cursor_rect.max.y <= clip.max.y) {
                            should_scroll = true;
                            ui.scroll_to_rect(cursor_rect.expand2(Vec2::new(0.0, row_height)), None);
                        }
                        file.last_cursor_index = Some(cidx);
                    }
                }

                (editor_rect, cur_char_idx, line_count, editor_rect.min.x, should_scroll)
            }).inner
        });

    let (editor_rect_after_v, cur_after_v, line_count_v, text_start_x_v, should_scroll_v) =
        scroll_output.inner;
    editor_rect_after = editor_rect_after_v;
    cursor_char_idx_after = cur_after_v;
    file_content_line_count = line_count_v;
    text_start_x = text_start_x_v;
    should_scroll_to_cursor = should_scroll_v;
    let _ = (file_content_line_count, should_scroll_to_cursor, text_start_x);

    // --- Handle key-triggered actions after TextEdit ---

    if popup_close {
        state.autocomplete_open = false;
    }
    if popup_up {
        if state.autocomplete_selected == 0 {
            // wrap
            state.autocomplete_selected = 0;
        } else {
            state.autocomplete_selected -= 1;
        }
    }
    if popup_down {
        state.autocomplete_selected = state.autocomplete_selected.saturating_add(1);
    }

    if trigger_autocomplete {
        if let Some(cidx) = cursor_char_idx_after {
            let file = &state.open_files[active_idx];
            let byte_idx = char_to_byte(&file.content, cidx);
            let (prefix_start_byte, prefix) = autocomplete::extract_prefix(&file.content, byte_idx)
                .unwrap_or((byte_idx, ""));
            state.autocomplete_filter = prefix.to_string();
            state.autocomplete_prefix_start = prefix_start_byte;
            state.autocomplete_selected = 0;
            state.autocomplete_open = true;

            // Anchor below the cursor position on screen.
            let line = byte_to_line(&file.content, byte_idx);
            let y = editor_rect_after.min.y + (line as f32 + 1.0) * row_height;
            state.autocomplete_anchor = Some(Pos2::new(
                editor_rect_after.min.x + 40.0,
                y,
            ));
        }
    }

    if comment_key_pressed {
        apply_comment_toggle(state, active_idx, lang, ui.ctx());
    }

    // Autocomplete popup (filtered list) ------------------------------------
    if state.autocomplete_open {
        // Keep filter synced with the text at prefix_start..cursor.
        if let Some(cidx) = cursor_char_idx_after {
            let file = &state.open_files[active_idx];
            let byte_idx = char_to_byte(&file.content, cidx);
            if byte_idx < state.autocomplete_prefix_start || byte_idx > file.content.len() {
                state.autocomplete_open = false;
            } else {
                let start = state.autocomplete_prefix_start.min(file.content.len());
                let slice = &file.content[start..byte_idx];
                // If user typed a non-identifier char, close.
                if slice.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '_')) {
                    state.autocomplete_open = false;
                } else {
                    state.autocomplete_filter = slice.to_string();
                }
            }
        }
    }

    let wants_insert =
        popup_enter_insert || popup_tab_insert || state.autocomplete_click_commit;
    state.autocomplete_click_commit = false;
    if state.autocomplete_open {
        let matches = autocomplete::matching_symbols(lang, &state.autocomplete_filter);
        if matches.is_empty() {
            state.autocomplete_open = false;
        } else {
            // Clamp selection.
            if state.autocomplete_selected >= matches.len() {
                state.autocomplete_selected = matches.len() - 1;
            }

            if wants_insert {
                let chosen = matches[state.autocomplete_selected];
                apply_autocomplete_insert(state, active_idx, chosen, ui.ctx(), editor_id);
                state.autocomplete_open = false;
            } else if let Some(anchor) = state.autocomplete_anchor {
                render_autocomplete_popup(ui, state, &matches, anchor, theme);
            }
        }
    }

    // Consumed pending goto-line? Apply now that we know the file + ctx.
    if let Some(line) = state.pending_goto_line.take() {
        apply_goto_line(state, active_idx, line, ui.ctx(), editor_id, row_height);
    }

    // Error panel -----------------------------------------------------------
    if has_error {
        render_error_panel(ui, state, active_idx, available_rect, error_panel_height, secondary, error_color);
    }

    let _ = surface_panel;
}

// -------------------------------------------------------------------------
// Tab bar
// -------------------------------------------------------------------------

fn render_tab_bar(ui: &mut egui::Ui, state: &mut CodeEditorState, theme: &Theme) {
    let muted = theme.text.muted.to_color32();
    let surface_panel = theme.surfaces.panel.to_color32();

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
                            egui::Label::new(RichText::new(&label).size(11.0))
                                .sense(Sense::click()),
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
}

// -------------------------------------------------------------------------
// Toolbar
// -------------------------------------------------------------------------

fn render_toolbar(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    theme: &Theme,
    scripts_dir: &Option<PathBuf>,
    active_idx: usize,
) {
    let muted = theme.text.muted.to_color32();
    let disabled = theme.text.disabled.to_color32();
    let surface_panel = theme.surfaces.panel.to_color32();

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
                if save_btn.clicked() {
                    state.save_active();
                }

                if let Some(dir) = scripts_dir.clone() {
                    let new_btn = ui
                        .button(RichText::new(FILE_PLUS).size(12.0))
                        .on_hover_text("New Script");
                    if new_btn.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if new_btn.clicked() {
                        state.create_new_script(dir);
                    }
                }

                let find_btn = ui
                    .button(RichText::new(MAGNIFYING_GLASS).size(12.0))
                    .on_hover_text("Find / Replace (Ctrl+F)");
                if find_btn.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if find_btn.clicked() {
                    state.find_open = !state.find_open;
                    state.find_focus_requested = state.find_open;
                }

                let goto_btn = ui
                    .button(RichText::new(LIST_NUMBERS).size(12.0))
                    .on_hover_text("Go to Line (Ctrl+G)");
                if goto_btn.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if goto_btn.clicked() {
                    state.goto_line_open = !state.goto_line_open;
                    state.goto_line_focus_requested = state.goto_line_open;
                    state.goto_line_buffer.clear();
                }

                ui.separator();

                // Zoom controls
                let zoom_pct = (state.font_size / 16.0 * 100.0).round() as i32;
                if ui.small_button(RichText::new("\u{2212}").size(12.0)).clicked() {
                    state.zoom_out();
                }
                let zoom_label = ui.add(
                    egui::Label::new(
                        RichText::new(format!("{}%", zoom_pct)).size(11.0).color(muted),
                    )
                    .sense(Sense::click()),
                );
                if zoom_label.on_hover_text("Reset zoom").clicked() {
                    state.zoom_reset();
                }
                if ui.small_button(RichText::new("+").size(12.0)).clicked() {
                    state.zoom_in();
                }

                // Right-aligned: file path
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let path_str = state.open_files[active_idx].path.display().to_string();
                    ui.label(RichText::new(path_str).size(11.0).color(disabled));
                });
            });
        });
}

// -------------------------------------------------------------------------
// Shortcuts
// -------------------------------------------------------------------------

fn handle_global_shortcuts(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    _lang: Language,
    _active_idx: usize,
) {
    ui.input_mut(|i| {
        if i.consume_key(Modifiers::CTRL, Key::S) {
            state.save_active();
        }
        if i.consume_key(Modifiers::CTRL, Key::F) || i.consume_key(Modifiers::CTRL, Key::H) {
            state.find_open = true;
            state.find_focus_requested = true;
        }
        if i.consume_key(Modifiers::CTRL, Key::G) {
            state.goto_line_open = true;
            state.goto_line_focus_requested = true;
            state.goto_line_buffer.clear();
        }
        if !state.autocomplete_open && i.consume_key(Modifiers::NONE, Key::Escape) {
            state.find_open = false;
            state.goto_line_open = false;
        }
    });

    // Ctrl+scroll zoom
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
}

// -------------------------------------------------------------------------
// Find bar
// -------------------------------------------------------------------------

fn render_find_bar(ui: &mut egui::Ui, state: &mut CodeEditorState, theme: &Theme) {
    let surface_faint = theme.surfaces.faint.to_color32();
    let muted = theme.text.muted.to_color32();

    egui::Frame::new()
        .fill(surface_faint)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Find").size(11.0).color(muted));
                let find_resp = ui.add(
                    egui::TextEdit::singleline(&mut state.find_text)
                        .desired_width(180.0)
                        .hint_text("search..."),
                );
                if state.find_focus_requested {
                    find_resp.request_focus();
                    state.find_focus_requested = false;
                }
                let next_clicked = ui.small_button("Next").clicked()
                    || (find_resp.lost_focus()
                        && ui.ctx().input(|i| i.key_pressed(Key::Enter)));

                ui.checkbox(&mut state.find_case_sensitive, RichText::new("Aa").size(11.0))
                    .on_hover_text("Case sensitive");

                ui.separator();

                ui.label(RichText::new("Replace").size(11.0).color(muted));
                ui.add(
                    egui::TextEdit::singleline(&mut state.replace_text)
                        .desired_width(180.0)
                        .hint_text("replace with..."),
                );

                let replace_clicked = ui.small_button("Replace").clicked();
                let replace_all_clicked = ui.small_button("Replace All").clicked();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(RichText::new(X).size(11.0)).frame(false))
                        .clicked()
                    {
                        state.find_open = false;
                    }
                });

                if next_clicked {
                    find_and_select_next(state, ui.ctx());
                }
                if replace_clicked {
                    replace_current(state);
                }
                if replace_all_clicked {
                    let n = state.replace_all_active();
                    log::info!("Replaced {} occurrences", n);
                }
            });
        });
}

/// Search forward from the current cursor position and place the selection on
/// the match so the user can see it. Wraps around on EOF.
fn find_and_select_next(state: &mut CodeEditorState, ctx: &egui::Context) {
    let Some(idx) = state.active_tab else { return };
    let Some(file) = state.open_files.get(idx) else { return };
    if state.find_text.is_empty() {
        return;
    }

    // Cursor start = current caret (char index). Translate to bytes.
    let editor_id = Id::new(("code_editor_textedit", idx));
    let start_char = egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range().map(|r| r.primary.index))
        .unwrap_or(0);
    let start_byte = char_to_byte(&file.content, start_char);

    let found = CodeEditorState::find_next_in(
        &file.content,
        &state.find_text,
        start_byte,
        state.find_case_sensitive,
    );
    let Some(pos) = found else { return };

    let from_char = byte_to_char(&file.content, pos);
    let to_char = byte_to_char(&file.content, pos + state.find_text.len());
    if let Some(mut s) = egui::TextEdit::load_state(ctx, editor_id) {
        s.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(from_char),
            CCursor::new(to_char),
        )));
        s.store(ctx, editor_id);
    }
    ctx.memory_mut(|m| m.request_focus(editor_id));
}

fn replace_current(state: &mut CodeEditorState) {
    let Some(idx) = state.active_tab else { return };
    let Some(file) = state.open_files.get_mut(idx) else { return };
    if state.find_text.is_empty() {
        return;
    }
    let pos = CodeEditorState::find_next_in(
        &file.content,
        &state.find_text,
        0,
        state.find_case_sensitive,
    );
    if let Some(start) = pos {
        let end = start + state.find_text.len();
        if end <= file.content.len() {
            file.content.replace_range(start..end, &state.replace_text);
            file.is_modified = true;
        }
    }
}

// -------------------------------------------------------------------------
// Goto line bar
// -------------------------------------------------------------------------

fn render_goto_line_bar(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    theme: &Theme,
    _active_idx: usize,
) {
    let surface_faint = theme.surfaces.faint.to_color32();
    let muted = theme.text.muted.to_color32();

    egui::Frame::new()
        .fill(surface_faint)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} Go to line", ARROW_RIGHT)).size(11.0).color(muted));
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut state.goto_line_buffer)
                        .desired_width(80.0)
                        .hint_text("1"),
                );
                if state.goto_line_focus_requested {
                    resp.request_focus();
                    state.goto_line_focus_requested = false;
                }
                let go = ui.small_button("Go").clicked()
                    || (resp.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter)));
                if go {
                    if let Ok(n) = state.goto_line_buffer.trim().parse::<usize>() {
                        state.pending_goto_line = Some(n.max(1));
                        state.goto_line_open = false;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(RichText::new(X).size(11.0)).frame(false))
                        .clicked()
                    {
                        state.goto_line_open = false;
                    }
                });
            });
        });
}

fn apply_goto_line(
    state: &mut CodeEditorState,
    active_idx: usize,
    line_1based: usize,
    ctx: &egui::Context,
    editor_id: Id,
    _row_height: f32,
) {
    let Some(file) = state.open_files.get(active_idx) else { return };
    let line = line_1based.saturating_sub(1);
    let byte_idx = line_to_byte(&file.content, line);
    let char_idx = byte_to_char(&file.content, byte_idx);

    if let Some(mut s) = egui::TextEdit::load_state(ctx, editor_id) {
        s.cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(char_idx))));
        s.store(ctx, editor_id);
    }
    ctx.memory_mut(|m| m.request_focus(editor_id));
    // The scroll-to-cursor logic in the next frame will bring it into view.
}

// -------------------------------------------------------------------------
// Comment toggle application
// -------------------------------------------------------------------------

fn apply_comment_toggle(
    state: &mut CodeEditorState,
    active_idx: usize,
    lang: Language,
    ctx: &egui::Context,
) {
    let editor_id = Id::new(("code_editor_textedit", active_idx));
    let Some(file) = state.open_files.get_mut(active_idx) else { return };

    // Selection in char range — map to bytes.
    let (sel_start_byte, sel_end_byte) = egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| {
            let a = char_to_byte(&file.content, r.primary.index);
            let b = char_to_byte(&file.content, r.secondary.index);
            (a.min(b), a.max(b))
        })
        .unwrap_or((0, 0));

    if let Some((new_start_byte, new_end_byte)) =
        toggle_line_comment(&mut file.content, sel_start_byte, sel_end_byte, lang)
    {
        file.is_modified = true;
        // Restore selection at the new byte positions.
        let a_char = byte_to_char(&file.content, new_start_byte);
        let b_char = byte_to_char(&file.content, new_end_byte);
        if let Some(mut s) = egui::TextEdit::load_state(ctx, editor_id) {
            s.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(a_char),
                CCursor::new(b_char),
            )));
            s.store(ctx, editor_id);
        }
    }
}

// -------------------------------------------------------------------------
// Autocomplete popup
// -------------------------------------------------------------------------

fn render_autocomplete_popup(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    matches: &[&'static ApiSymbol],
    anchor: Pos2,
    theme: &Theme,
) {
    let popup_id = Id::new("code_editor_autocomplete_popup");
    let bg = theme.surfaces.panel.to_color32();
    let sel_bg = theme.semantic.selection.to_color32();
    let border = theme.widgets.border.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let text_secondary = theme.text.secondary.to_color32();

    let max_items = matches.len().min(10);
    let item_height = 22.0;
    let popup_width = 380.0;
    let popup_height = item_height * max_items as f32 + 4.0;

    egui::Area::new(popup_id)
        .fixed_pos(anchor)
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ui.ctx(), |ui| {
            let rect = Rect::from_min_size(
                anchor,
                Vec2::new(popup_width, popup_height),
            );
            ui.painter().rect_filled(rect, 4.0, bg);
            ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, border), egui::StrokeKind::Inside);

            // Ensure selected is within the visible window (scroll view).
            let first_visible = if state.autocomplete_selected >= max_items {
                state.autocomplete_selected + 1 - max_items
            } else {
                0
            };

            for i in 0..max_items {
                let global = first_visible + i;
                if global >= matches.len() {
                    break;
                }
                let sym = matches[global];
                let row_rect = Rect::from_min_size(
                    Pos2::new(anchor.x + 2.0, anchor.y + 2.0 + i as f32 * item_height),
                    Vec2::new(popup_width - 4.0, item_height),
                );
                let is_selected = global == state.autocomplete_selected;
                if is_selected {
                    ui.painter().rect_filled(row_rect, 2.0, sel_bg);
                }

                let resp = ui.allocate_rect(row_rect, Sense::click());
                if resp.clicked() {
                    state.autocomplete_selected = global;
                    state.autocomplete_click_commit = true;
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }

                // Icon / name
                ui.painter().text(
                    Pos2::new(row_rect.min.x + 10.0, row_rect.center().y),
                    Align2::LEFT_CENTER,
                    sym.name,
                    FontId::new(12.0, FontFamily::Monospace),
                    if is_selected { Color32::WHITE } else { text_primary },
                );
                // Category badge
                ui.painter().text(
                    Pos2::new(row_rect.max.x - 10.0, row_rect.center().y),
                    Align2::RIGHT_CENTER,
                    sym.category,
                    FontId::proportional(10.0),
                    text_muted,
                );
            }

            // Footer: signature of the highlighted item
            if let Some(sym) = matches.get(state.autocomplete_selected) {
                let footer_rect = Rect::from_min_size(
                    Pos2::new(anchor.x, anchor.y + popup_height + 2.0),
                    Vec2::new(popup_width, 36.0),
                );
                ui.painter().rect_filled(footer_rect, 4.0, bg);
                ui.painter().rect_stroke(footer_rect, 4.0, Stroke::new(1.0, border), egui::StrokeKind::Inside);
                ui.painter().text(
                    Pos2::new(footer_rect.min.x + 8.0, footer_rect.min.y + 12.0),
                    Align2::LEFT_TOP,
                    sym.signature,
                    FontId::new(11.0, FontFamily::Monospace),
                    text_primary,
                );
                ui.painter().text(
                    Pos2::new(footer_rect.min.x + 8.0, footer_rect.min.y + 24.0),
                    Align2::LEFT_TOP,
                    sym.doc,
                    FontId::proportional(10.0),
                    text_secondary,
                );
            }
        });
}

fn apply_autocomplete_insert(
    state: &mut CodeEditorState,
    active_idx: usize,
    sym: &'static ApiSymbol,
    ctx: &egui::Context,
    editor_id: Id,
) {
    let start_byte = state.autocomplete_prefix_start;
    let file = match state.open_files.get_mut(active_idx) {
        Some(f) => f,
        None => return,
    };

    // Replace [start_byte, current cursor byte] with sym.name
    let cur_char = egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range().map(|r| r.primary.index))
        .unwrap_or_else(|| byte_to_char(&file.content, start_byte));
    let cur_byte = char_to_byte(&file.content, cur_char);
    let end_byte = cur_byte.max(start_byte);
    let s = start_byte.min(file.content.len());
    let e = end_byte.min(file.content.len());
    file.content.replace_range(s..e, sym.name);
    file.is_modified = true;

    // Place cursor after inserted text.
    let new_char = byte_to_char(&file.content, s + sym.name.len());
    if let Some(mut st) = egui::TextEdit::load_state(ctx, editor_id) {
        st.cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(new_char))));
        st.store(ctx, editor_id);
    }
    ctx.memory_mut(|m| m.request_focus(editor_id));
}

// -------------------------------------------------------------------------
// Gutter + bracket highlight painters
// -------------------------------------------------------------------------

fn paint_gutter(
    ui: &egui::Ui,
    rect: Rect,
    font_size: f32,
    row_height: f32,
    line_count: usize,
    cursor_char_idx: Option<usize>,
    content: &str,
    theme: &Theme,
) {
    let painter = ui.painter();
    let bg = theme.surfaces.faint.to_color32();
    let line_color = theme.widgets.border.to_color32();
    let muted = theme.text.muted.to_color32();
    let emph = theme.text.primary.to_color32();

    painter.rect_filled(rect, 0.0, bg);
    // Divider on the right edge
    painter.line_segment(
        [Pos2::new(rect.max.x, rect.min.y), Pos2::new(rect.max.x, rect.max.y)],
        Stroke::new(1.0, line_color),
    );

    let current_line = cursor_char_idx.map(|ci| {
        let bi = char_to_byte(content, ci);
        byte_to_line(content, bi)
    });

    let font = FontId::new(font_size * 0.95, FontFamily::Monospace);
    for line in 0..line_count {
        let y = rect.min.y + line as f32 * row_height + row_height * 0.5;
        let is_current = current_line == Some(line);
        let color = if is_current { emph } else { muted };
        painter.text(
            Pos2::new(rect.max.x - 8.0, y),
            Align2::RIGHT_CENTER,
            format!("{}", line + 1),
            font.clone(),
            color,
        );
    }
}

fn paint_bracket_highlight(
    ui: &egui::Ui,
    content: &str,
    editor_rect: Rect,
    row_height: f32,
    font_size: f32,
    byte_idx: usize,
    theme: &Theme,
) {
    let line = byte_to_line(content, byte_idx);
    let line_start = line_to_byte(content, line);
    let col_bytes = byte_idx.saturating_sub(line_start);
    let col_chars = content[line_start..line_start + col_bytes].chars().count();

    // Approximate monospace advance: row_height usually ≈ 1.3 * font_size.
    // Advance per char is roughly font_size * 0.6 for monospace.
    let advance = font_size * 0.6;
    let x = editor_rect.min.x + col_chars as f32 * advance;
    let y = editor_rect.min.y + line as f32 * row_height;
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(advance, row_height));

    let accent = theme.semantic.accent.to_color32();
    let fill =
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 40);
    ui.painter().rect_filled(r, 2.0, fill);
    ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0, accent), egui::StrokeKind::Inside);
}

// -------------------------------------------------------------------------
// Error panel (unchanged behaviour)
// -------------------------------------------------------------------------

fn render_error_panel(
    ui: &mut egui::Ui,
    state: &CodeEditorState,
    active_idx: usize,
    available_rect: Rect,
    error_panel_height: f32,
    secondary: Color32,
    error_color: Color32,
) {
    let Some(file) = state.open_files.get(active_idx) else { return };
    let Some(error) = file.error.as_ref() else { return };

    let error_rect = Rect::from_min_max(
        Pos2::new(available_rect.min.x, available_rect.max.y - error_panel_height),
        available_rect.max,
    );

    ui.painter()
        .rect_filled(error_rect, 0.0, Color32::from_rgb(60, 30, 30));
    ui.painter().line_segment(
        [
            Pos2::new(error_rect.min.x, error_rect.min.y),
            Pos2::new(error_rect.max.x, error_rect.min.y),
        ],
        Stroke::new(2.0, error_color),
    );

    ui.painter().text(
        Pos2::new(error_rect.min.x + 12.0, error_rect.min.y + 20.0),
        Align2::LEFT_CENTER,
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
        Pos2::new(error_rect.min.x + 36.0, error_rect.min.y + 16.0),
        Align2::LEFT_CENTER,
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
        Pos2::new(error_rect.min.x + 36.0, error_rect.min.y + 38.0),
        Align2::LEFT_CENTER,
        &msg,
        FontId::new(12.0, FontFamily::Monospace),
        secondary,
    );
}

use bevy::prelude::*;
use bevy_egui::egui::{
    self, text::CCursor, text::CCursorRange, Align2, Color32, CursorIcon, FontFamily, FontId, Id,
    Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use renzora::core::keybindings::{EditorAction, KeyBindings};
use egui_phosphor::regular::{
    ARROW_RIGHT, CODE, FILE_PLUS, FLOPPY_DISK, LIST_NUMBERS, MAGNIFYING_GLASS,
    PARAGRAPH, SQUARES_FOUR, WARNING, X,
};
use renzora_theme::Theme;
use std::path::PathBuf;

use crate::actions::{
    byte_to_char, byte_to_line, char_to_byte, delete_lines, duplicate_lines,
    find_all_occurrences, find_matching_bracket, indent_selection,
    leading_whitespace_of_line, line_byte_range, line_to_byte, move_lines_down,
    move_lines_up, select_next_occurrence, smart_home_target, toggle_block_comment,
    toggle_line_comment, word_range_at_byte, TAB_SIZE,
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

/// Snapshot of which user-bound code-editor shortcuts fired this frame. The
/// individual `consume_key` calls happen up-front in `collect()` so egui's
/// `TextEdit` doesn't also see the same events.
#[derive(Default, Debug, Clone, Copy)]
pub struct EditorShortcuts {
    pub save: bool,
    pub save_all: bool,
    pub close_tab: bool,
    pub next_tab: bool,
    pub prev_tab: bool,
    pub find: bool,
    pub replace: bool,
    pub goto_line: bool,
    pub line_comment: bool,
    pub block_comment: bool,
    pub autocomplete: bool,
    pub select_next_match: bool,
    pub duplicate_line: bool,
    pub delete_line: bool,
    pub move_line_up: bool,
    pub move_line_down: bool,
}

impl EditorShortcuts {
    /// Read the current user keybindings and consume any matching events.
    pub fn collect(ui: &mut egui::Ui, world: &World) -> Self {
        let mut out = Self::default();
        let Some(kb) = world.get_resource::<KeyBindings>() else {
            return out;
        };
        let mut take = |action: EditorAction| -> bool {
            let Some(binding) = kb.bindings.get(&action) else { return false };
            let Some(key) = keycode_to_egui_key(binding.key) else { return false };
            let modifiers = Modifiers {
                ctrl: binding.ctrl,
                shift: binding.shift,
                alt: binding.alt,
                ..Default::default()
            };
            ui.input_mut(|i| i.consume_key(modifiers, key))
        };
        // Order matters when two bindings differ only by modifier (e.g.
        // Ctrl+Shift+Tab vs Ctrl+Tab) — check the more-modified variant first
        // so it wins.
        out.save_all = take(EditorAction::CodeSaveAll);
        out.save = take(EditorAction::CodeSaveFile);
        out.close_tab = take(EditorAction::CodeCloseTab);
        out.prev_tab = take(EditorAction::CodePrevTab);
        out.next_tab = take(EditorAction::CodeNextTab);
        out.find = take(EditorAction::CodeFind);
        out.replace = take(EditorAction::CodeReplace);
        out.goto_line = take(EditorAction::CodeGotoLine);
        out.block_comment = take(EditorAction::CodeToggleBlockComment);
        out.line_comment = take(EditorAction::CodeToggleLineComment);
        out.autocomplete = take(EditorAction::CodeTriggerAutocomplete);
        out.duplicate_line = take(EditorAction::CodeDuplicateLine);
        out.select_next_match = take(EditorAction::CodeSelectNextMatch);
        out.delete_line = take(EditorAction::CodeDeleteLine);
        out.move_line_up = take(EditorAction::CodeMoveLineUp);
        out.move_line_down = take(EditorAction::CodeMoveLineDown);
        out
    }
}

/// Map a Bevy `KeyCode` to an egui `Key` so user-configured bindings can drive
/// `egui::InputState::consume_key`.
fn keycode_to_egui_key(kc: bevy::input::keyboard::KeyCode) -> Option<Key> {
    use bevy::input::keyboard::KeyCode as KC;
    use Key as K;
    Some(match kc {
        KC::KeyA => K::A,  KC::KeyB => K::B,  KC::KeyC => K::C,  KC::KeyD => K::D,
        KC::KeyE => K::E,  KC::KeyF => K::F,  KC::KeyG => K::G,  KC::KeyH => K::H,
        KC::KeyI => K::I,  KC::KeyJ => K::J,  KC::KeyK => K::K,  KC::KeyL => K::L,
        KC::KeyM => K::M,  KC::KeyN => K::N,  KC::KeyO => K::O,  KC::KeyP => K::P,
        KC::KeyQ => K::Q,  KC::KeyR => K::R,  KC::KeyS => K::S,  KC::KeyT => K::T,
        KC::KeyU => K::U,  KC::KeyV => K::V,  KC::KeyW => K::W,  KC::KeyX => K::X,
        KC::KeyY => K::Y,  KC::KeyZ => K::Z,
        KC::Digit0 => K::Num0, KC::Digit1 => K::Num1, KC::Digit2 => K::Num2,
        KC::Digit3 => K::Num3, KC::Digit4 => K::Num4, KC::Digit5 => K::Num5,
        KC::Digit6 => K::Num6, KC::Digit7 => K::Num7, KC::Digit8 => K::Num8,
        KC::Digit9 => K::Num9,
        KC::Slash => K::Slash, KC::Comma => K::Comma, KC::Period => K::Period,
        KC::Semicolon => K::Semicolon, KC::Quote => K::Quote, KC::Backslash => K::Backslash,
        KC::BracketLeft => K::OpenBracket, KC::BracketRight => K::CloseBracket,
        KC::Equal => K::Equals, KC::Minus => K::Minus, KC::Backquote => K::Backtick,
        KC::Space => K::Space, KC::Tab => K::Tab, KC::Enter => K::Enter,
        KC::Escape => K::Escape, KC::Backspace => K::Backspace, KC::Delete => K::Delete,
        KC::Home => K::Home, KC::End => K::End, KC::PageUp => K::PageUp, KC::PageDown => K::PageDown,
        KC::Insert => K::Insert,
        KC::ArrowUp => K::ArrowUp, KC::ArrowDown => K::ArrowDown,
        KC::ArrowLeft => K::ArrowLeft, KC::ArrowRight => K::ArrowRight,
        KC::F1 => K::F1, KC::F2 => K::F2, KC::F3 => K::F3, KC::F4 => K::F4,
        KC::F5 => K::F5, KC::F6 => K::F6, KC::F7 => K::F7, KC::F8 => K::F8,
        KC::F9 => K::F9, KC::F10 => K::F10, KC::F11 => K::F11, KC::F12 => K::F12,
        _ => return None,
    })
}

pub fn render_code_editor_content(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    theme: &Theme,
    scripts_dir: Option<PathBuf>,
    shortcuts: EditorShortcuts,
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
    handle_global_shortcuts(ui, state, lang, active_idx, shortcuts);

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
    let mono_advance = ui.fonts_mut(|f| f.glyph_width(&FontId::monospace(font_size), 'M'));
    let status_bar_height = 22.0;
    let available_height = (available_rect.height() - error_panel_height - status_bar_height)
        .max(row_height);

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
    // Pending comment toggle keys (processed after TextEdit to know cursor).
    let mut comment_key_pressed = false;
    let mut block_comment_key_pressed = false;

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

    // Keybinding-driven shortcuts (already consumed in EditorShortcuts::collect).
    if shortcuts.autocomplete {
        trigger_autocomplete = true;
    }
    if shortcuts.line_comment {
        comment_key_pressed = true;
    }
    if shortcuts.block_comment {
        block_comment_key_pressed = true;
    }

    // --- Editor focus + Tab/Enter/Home (kept hardcoded — these conflict with
    // text input, so they need to consume the egui event rather than route
    // through the keybinding system). ---
    let editor_focused = ui.ctx().memory(|m| m.has_focus(editor_id));
    let mut text_shortcut = if editor_focused && !state.autocomplete_open {
        pick_editor_shortcut(ui)
    } else {
        None
    };
    // User-configurable shortcuts (consumed via EditorShortcuts::collect).
    if shortcuts.duplicate_line {
        text_shortcut = Some(EditorShortcut::DuplicateLine);
    }
    if shortcuts.delete_line {
        text_shortcut = Some(EditorShortcut::DeleteLine);
    }
    if shortcuts.move_line_up {
        text_shortcut = Some(EditorShortcut::MoveLineUp);
    }
    if shortcuts.move_line_down {
        text_shortcut = Some(EditorShortcut::MoveLineDown);
    }
    if shortcuts.select_next_match {
        text_shortcut = Some(EditorShortcut::SelectNextOccurrence);
    }
    if let Some(sc) = text_shortcut {
        apply_editor_shortcut(state, active_idx, ui.ctx(), editor_id, sc);
    }

    // --- Auto-close brackets / quotes (intercept the typed character) ---
    if state.auto_close_pairs && editor_focused {
        let cursor_char = egui::TextEdit::load_state(ui.ctx(), editor_id)
            .and_then(|s| s.cursor.char_range())
            .map(|r| r.primary.index);

        if let Some(cursor_char_v) = cursor_char {
            // Snapshot what's at the cursor before mutating anything.
            let (cursor_byte, next_ch) = {
                let content = &state.open_files[active_idx].content;
                let cb = char_to_byte(content, cursor_char_v);
                let nc = content[cb..].chars().next();
                (cb, nc)
            };

            let mut action: Option<AutocloseAction> = None;
            ui.input_mut(|i| {
                let mut hit = None;
                for (idx, event) in i.events.iter().enumerate() {
                    if let egui::Event::Text(s) = event {
                        if s.chars().count() != 1 {
                            continue;
                        }
                        let ch = s.chars().next().unwrap();
                        if let Some(a) = decide_autoclose(ch, next_ch) {
                            hit = Some((idx, a));
                            break;
                        }
                    }
                }
                if let Some((idx, a)) = hit {
                    i.events.remove(idx);
                    action = Some(a);
                }
            });

            if let Some(a) = action {
                apply_autoclose(
                    state,
                    active_idx,
                    cursor_char_v,
                    cursor_byte,
                    a,
                    ui.ctx(),
                    editor_id,
                );
            }
        }
    }

    // --- Split editor area into [scroll area | optional minimap] ---
    let minimap_width = if state.show_minimap { 120.0 } else { 0.0 };
    let editor_area = Rect::from_min_size(
        available_rect.min,
        Vec2::new(available_rect.width(), available_height),
    );
    let scroll_area_rect = Rect::from_min_max(
        editor_area.min,
        Pos2::new(editor_area.max.x - minimap_width, editor_area.max.y),
    );
    let minimap_rect = Rect::from_min_max(
        Pos2::new(editor_area.max.x - minimap_width, editor_area.min.y),
        editor_area.max,
    );

    // Scope scroll area to its sub-rect so the minimap has room on the right.
    let mut scroll_ui = ui.new_child(egui::UiBuilder::new().max_rect(scroll_area_rect));

    // --- Scroll area with gutter + TextEdit ---
    let scroll_id = Id::new(("code_editor_panel_scroll", active_idx));
    let pending_scroll = state.pending_scroll_offset.take();
    let mut scroll_builder = egui::ScrollArea::vertical()
        .id_salt(scroll_id)
        .max_height(available_height)
        .auto_shrink([false, false]);
    if let Some(y) = pending_scroll {
        scroll_builder = scroll_builder.vertical_scroll_offset(y);
    }
    let scroll_output = scroll_builder.show(&mut scroll_ui, |ui| {
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

                // Capture scalar flags before the mut-borrow of open_files so
                // we can read them while `file` is live.
                let show_whitespace = state.show_whitespace;
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

                // Current line highlight (subtle accent tint) ---------------
                if let Some(cidx) = cur_char_idx {
                    let byte_idx = char_to_byte(&file.content, cidx);
                    paint_current_line_highlight(
                        ui,
                        &file.content,
                        editor_rect,
                        row_height,
                        byte_idx,
                        theme,
                    );
                }

                // Bracket match highlight -----------------------------------
                if let Some(cidx) = cur_char_idx {
                    let byte_idx = char_to_byte(&file.content, cidx);
                    if let Some((a, b)) = find_matching_bracket(&file.content, byte_idx) {
                        paint_bracket_highlight(ui, &file.content, editor_rect, row_height, mono_advance, a, theme);
                        paint_bracket_highlight(ui, &file.content, editor_rect, row_height, mono_advance, b, theme);
                    }
                }

                // Occurrence highlight --------------------------------------
                if let Some(cidx) = cur_char_idx {
                    let byte = char_to_byte(&file.content, cidx);
                    if let Some((ws, we)) = word_range_at_byte(&file.content, byte) {
                        let word = &file.content[ws..we];
                        if word.len() >= 2 {
                            let matches = find_all_occurrences(&file.content, word);
                            if matches.len() > 1 {
                                for (mstart, mend) in matches {
                                    if mstart == ws {
                                        continue;
                                    }
                                    paint_range_overlay(
                                        ui,
                                        &file.content,
                                        editor_rect,
                                        row_height,
                                        mono_advance,
                                        mstart,
                                        mend,
                                        theme,
                                    );
                                }
                            }
                        }
                    }
                }

                // Inline error squiggle -------------------------------------
                if let Some(err) = file.error.as_ref() {
                    if let Some(line_1) = err.line {
                        let line = line_1.saturating_sub(1);
                        paint_error_squiggle(ui, editor_rect, row_height, line);
                    }
                }

                // Whitespace overlay ---------------------------------------
                if show_whitespace {
                    paint_whitespace_overlay(
                        ui,
                        &file.content,
                        editor_rect,
                        row_height,
                        font_size,
                        mono_advance,
                        theme,
                    );
                }

                // Indent guides --------------------------------------------
                paint_indent_guides(
                    ui,
                    &file.content,
                    editor_rect,
                    row_height,
                    mono_advance,
                    theme,
                );

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
    let _ = (should_scroll_to_cursor, text_start_x);

    // --- Minimap (right sidebar, independent of scroll area) ---
    if state.show_minimap && minimap_width > 0.0 {
        let scroll_y = scroll_output.state.offset.y;
        let first_visible = (scroll_y / row_height).floor() as usize;
        let visible_lines = (minimap_rect.height() / row_height).ceil() as usize;
        let last_visible = (first_visible + visible_lines).min(file_content_line_count);
        let clicked_line = render_minimap(
            ui,
            minimap_rect,
            &state.open_files[active_idx].content,
            file_content_line_count,
            first_visible,
            last_visible,
            theme,
        );
        if let Some(line) = clicked_line {
            state.pending_scroll_offset = Some(line as f32 * row_height);
            ui.ctx().request_repaint();
        }
    }

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
    if block_comment_key_pressed {
        apply_block_comment_toggle(state, active_idx, lang, ui.ctx());
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
        let error_rect = Rect::from_min_max(
            Pos2::new(
                available_rect.min.x,
                available_rect.max.y - error_panel_height - status_bar_height,
            ),
            Pos2::new(available_rect.max.x, available_rect.max.y - status_bar_height),
        );
        render_error_panel(ui, state, active_idx, error_rect, secondary, error_color);
    }

    // Status bar -----------------------------------------------------------
    let status_rect = Rect::from_min_max(
        Pos2::new(available_rect.min.x, available_rect.max.y - status_bar_height),
        available_rect.max,
    );
    render_status_bar(
        ui,
        state,
        active_idx,
        cursor_char_idx_after,
        lang,
        status_rect,
        theme,
    );

    // Close-confirm modal -------------------------------------------------
    if let Some(idx) = state.close_confirm_tab {
        if idx >= state.open_files.len() {
            state.close_confirm_tab = None;
        } else {
            render_close_confirm(ui, state, idx, theme);
        }
    }

    let _ = surface_panel;
}

// -------------------------------------------------------------------------
// Tab bar
// -------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum TabAction {
    Switch(usize),
    Close(usize),
    CloseOthers(usize),
    CloseAll,
    Save(usize),
    CopyPath(usize),
    RevealInExplorer(usize),
}

fn render_tab_bar(ui: &mut egui::Ui, state: &mut CodeEditorState, theme: &Theme) {
    let muted = theme.text.muted.to_color32();
    let surface_panel = theme.surfaces.panel.to_color32();

    let mut action: Option<TabAction> = None;

    ui.horizontal(|ui| {
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
                            action = Some(TabAction::Switch(idx));
                        }
                        // Middle-click closes too (VSCode habit).
                        if tab_resp.middle_clicked() {
                            action = Some(TabAction::Close(idx));
                        }
                        // Right-click context menu.
                        tab_resp.context_menu(|ui| {
                            ui.set_min_width(180.0);
                            if ui.button("Save").clicked() {
                                action = Some(TabAction::Save(idx));
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Close").clicked() {
                                action = Some(TabAction::Close(idx));
                                ui.close();
                            }
                            if ui.button("Close Others").clicked() {
                                action = Some(TabAction::CloseOthers(idx));
                                ui.close();
                            }
                            if ui.button("Close All").clicked() {
                                action = Some(TabAction::CloseAll);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Copy Path").clicked() {
                                action = Some(TabAction::CopyPath(idx));
                                ui.close();
                            }
                            if ui.button("Reveal in File Explorer").clicked() {
                                action = Some(TabAction::RevealInExplorer(idx));
                                ui.close();
                            }
                        });

                        let close_resp = ui.add(
                            egui::Button::new(RichText::new(X).size(10.0).color(muted))
                                .frame(false),
                        );
                        if close_resp.clicked() {
                            action = Some(TabAction::Close(idx));
                        }
                        if close_resp.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                    });
                });
        }
    });

    if let Some(a) = action {
        apply_tab_action(state, ui.ctx(), a);
    }
}

fn apply_tab_action(state: &mut CodeEditorState, ctx: &egui::Context, action: TabAction) {
    match action {
        TabAction::Switch(idx) => {
            state.active_tab = Some(idx);
        }
        TabAction::Close(idx) => {
            let modified = state
                .open_files
                .get(idx)
                .map(|f| f.is_modified)
                .unwrap_or(false);
            if modified {
                state.close_confirm_tab = Some(idx);
            } else {
                state.close_tab(idx);
            }
        }
        TabAction::CloseOthers(idx) => {
            state.close_others(idx);
        }
        TabAction::CloseAll => {
            state.close_all();
        }
        TabAction::Save(idx) => {
            state.save_file(idx);
        }
        TabAction::CopyPath(idx) => {
            if let Some(file) = state.open_files.get(idx) {
                let path_str = file.path.display().to_string();
                ctx.copy_text(path_str);
            }
        }
        TabAction::RevealInExplorer(idx) => {
            if let Some(file) = state.open_files.get(idx) {
                reveal_in_file_explorer(&file.path);
            }
        }
    }
}

fn reveal_in_file_explorer(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        // /select, highlights the file in Explorer.
        let _ = std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .args(["-R", &path.display().to_string()])
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // No standard "reveal and select" on Linux — open the parent folder.
        if let Some(parent) = path.parent() {
            let _ = std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn();
        }
    }
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

                let save_all_btn = ui
                    .button(RichText::new(format!("{} All", FLOPPY_DISK)).size(12.0))
                    .on_hover_text("Save every modified tab");
                if save_all_btn.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if save_all_btn.clicked() {
                    state.save_all();
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

                let ws_btn = ui
                    .selectable_label(
                        state.show_whitespace,
                        RichText::new(PARAGRAPH).size(12.0),
                    )
                    .on_hover_text("Show whitespace");
                if ws_btn.clicked() {
                    state.show_whitespace = !state.show_whitespace;
                }

                let mm_btn = ui
                    .selectable_label(
                        state.show_minimap,
                        RichText::new(SQUARES_FOUR).size(12.0),
                    )
                    .on_hover_text("Toggle minimap");
                if mm_btn.clicked() {
                    state.show_minimap = !state.show_minimap;
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
    shortcuts: EditorShortcuts,
) {
    if shortcuts.save {
        state.save_active();
    }
    if shortcuts.save_all {
        state.save_all();
    }
    if shortcuts.find || shortcuts.replace {
        state.find_open = true;
        state.find_focus_requested = true;
    }
    if shortcuts.goto_line {
        state.goto_line_open = true;
        state.goto_line_focus_requested = true;
        state.goto_line_buffer.clear();
    }
    if shortcuts.close_tab {
        if let Some(idx) = state.active_tab {
            let modified = state
                .open_files
                .get(idx)
                .map(|f| f.is_modified)
                .unwrap_or(false);
            if modified {
                state.close_confirm_tab = Some(idx);
            } else {
                state.close_tab(idx);
            }
        }
    }
    if shortcuts.prev_tab {
        state.prev_tab();
    } else if shortcuts.next_tab {
        state.next_tab();
    }
    // Esc still goes through the raw input — it isn't a configurable binding
    // because it's a universal "dismiss overlay" key.
    ui.input_mut(|i| {
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

fn apply_block_comment_toggle(
    state: &mut CodeEditorState,
    active_idx: usize,
    lang: Language,
    ctx: &egui::Context,
) {
    // Languages without /* */ fall back to line comments — nicer than a
    // silent no-op when the user presses Ctrl+Shift+/ in Lua/Python.
    if lang.block_comment().is_none() {
        apply_comment_toggle(state, active_idx, lang, ctx);
        return;
    }

    let editor_id = Id::new(("code_editor_textedit", active_idx));
    let Some(file) = state.open_files.get_mut(active_idx) else { return };

    let (sel_start_byte, sel_end_byte) = egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| {
            let a = char_to_byte(&file.content, r.primary.index);
            let b = char_to_byte(&file.content, r.secondary.index);
            (a.min(b), a.max(b))
        })
        .unwrap_or((0, 0));

    if let Some((na, nb)) =
        toggle_block_comment(&mut file.content, sel_start_byte, sel_end_byte, lang)
    {
        file.is_modified = true;
        let a_char = byte_to_char(&file.content, na);
        let b_char = byte_to_char(&file.content, nb);
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

/// Draw the whole file as a scaled-down strip in `rect`, plus a viewport
/// indicator and click-to-scroll. Returns the clicked line (0-based) if any.
///
/// The minimap content stops at the last line of the file rather than
/// stretching to fill the reserved sidebar — short scripts no longer leave
/// a long empty grey strip below.
fn render_minimap(
    ui: &mut egui::Ui,
    rect: Rect,
    content: &str,
    line_count: usize,
    first_visible: usize,
    last_visible: usize,
    theme: &Theme,
) -> Option<usize> {
    if line_count == 0 {
        return None;
    }

    let bg = theme.surfaces.faint.to_color32();
    let fg = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();
    let border = theme.widgets.border.to_color32();

    let mini_row = (rect.height() / line_count.max(1) as f32).clamp(1.0, 3.0);

    // Content height = exactly what the file occupies, capped at the rect.
    let content_height = (line_count as f32 * mini_row).min(rect.height());
    let content_rect = Rect::from_min_max(
        rect.min,
        Pos2::new(rect.max.x, rect.min.y + content_height),
    );

    ui.painter().rect_filled(content_rect, 0.0, bg);
    ui.painter().line_segment(
        [content_rect.min, Pos2::new(content_rect.min.x, content_rect.max.y)],
        Stroke::new(1.0, border),
    );

    let mini_padding_x = 6.0;
    let mini_advance = 1.2; // px per char in the minimap
    let max_cols = ((rect.width() - mini_padding_x * 2.0) / mini_advance) as usize;

    // Paint each line's non-whitespace run as a flat strip.
    for (line_idx, line) in content.lines().enumerate() {
        let y = rect.min.y + line_idx as f32 * mini_row;
        if y >= content_rect.max.y {
            break;
        }
        let trimmed_start = line
            .bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        let visual_start = trimmed_start.min(line.len());
        let content_len = line[visual_start..].chars().count().min(max_cols);
        if content_len == 0 {
            continue;
        }
        let x0 = rect.min.x + mini_padding_x + trimmed_start as f32 * mini_advance;
        let strip_width = (content_len as f32 * mini_advance).min(rect.width() - mini_padding_x);
        let strip = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(strip_width, mini_row));
        ui.painter().rect_filled(strip, 0.0, fg.linear_multiply(0.45));
    }

    // Viewport indicator: translucent band over the visible line range.
    if last_visible > first_visible {
        let vp_top = rect.min.y + first_visible as f32 * mini_row;
        let vp_bot = rect.min.y + last_visible as f32 * mini_row;
        let vp_rect = Rect::from_min_max(
            Pos2::new(rect.min.x, vp_top.min(content_rect.max.y)),
            Pos2::new(rect.max.x, vp_bot.min(content_rect.max.y)),
        );
        let fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 30);
        ui.painter().rect_filled(vp_rect, 0.0, fill);
        ui.painter().rect_stroke(
            vp_rect,
            0.0,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 120)),
            egui::StrokeKind::Inside,
        );
    }

    // Only the content strip is interactive; clicking below an end-of-file
    // line does nothing.
    let resp = ui.allocate_rect(content_rect, Sense::click_and_drag());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if (resp.clicked() || resp.dragged()) && mini_row > 0.0 {
        let pos = resp
            .interact_pointer_pos()
            .or_else(|| ui.ctx().pointer_latest_pos())?;
        let y_in = (pos.y - rect.min.y).max(0.0);
        let line = (y_in / mini_row) as usize;
        let visible_lines = last_visible.saturating_sub(first_visible).max(1);
        let target = line.saturating_sub(visible_lines / 2);
        return Some(target.min(line_count.saturating_sub(1)));
    }

    None
}

/// Paint thin vertical guides at each indent level so block structure is
/// readable at a glance — same idea as VSCode/Sublime indent guides.
fn paint_indent_guides(
    ui: &egui::Ui,
    content: &str,
    editor_rect: Rect,
    row_height: f32,
    advance: f32,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();
    let color = Color32::from_rgba_unmultiplied(muted.r(), muted.g(), muted.b(), 50);
    let stroke = Stroke::new(1.0, color);

    // Skip lines outside the visible clip rect — minor optimisation that
    // matters on large files.
    let clip = ui.clip_rect();
    let first_line =
        (((clip.min.y - editor_rect.min.y) / row_height).floor().max(0.0)) as usize;
    let last_line =
        (((clip.max.y - editor_rect.min.y) / row_height).ceil().max(0.0)) as usize;

    for (line_idx, line) in content.lines().enumerate() {
        if line_idx < first_line {
            continue;
        }
        if line_idx > last_line {
            break;
        }

        // Indent in spaces (tab counts as TAB_SIZE).
        let mut col = 0usize;
        for ch in line.chars() {
            match ch {
                ' ' => col += 1,
                '\t' => col += TAB_SIZE,
                _ => break,
            }
        }
        let level_count = col / TAB_SIZE;
        if level_count == 0 {
            continue;
        }

        let y_top = editor_rect.min.y + line_idx as f32 * row_height;
        let y_bot = y_top + row_height;
        for level in 1..=level_count {
            let x = editor_rect.min.x + (level * TAB_SIZE) as f32 * advance;
            ui.painter()
                .line_segment([Pos2::new(x, y_top), Pos2::new(x, y_bot)], stroke);
        }
    }
}

/// Overlay dots for spaces and arrows for tabs across the visible content.
fn paint_whitespace_overlay(
    ui: &egui::Ui,
    content: &str,
    editor_rect: Rect,
    row_height: f32,
    font_size: f32,
    advance: f32,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();
    let color = Color32::from_rgba_unmultiplied(muted.r(), muted.g(), muted.b(), 140);
    let font = FontId::new(font_size * 0.9, FontFamily::Monospace);

    // Only paint lines that are within the clip rect; everything outside will
    // be culled anyway, but skipping avoids tons of painter calls on big files.
    let clip = ui.clip_rect();
    let first_line =
        (((clip.min.y - editor_rect.min.y) / row_height).floor().max(0.0)) as usize;
    let last_line =
        (((clip.max.y - editor_rect.min.y) / row_height).ceil().max(0.0)) as usize;

    for (line_idx, line) in content.lines().enumerate() {
        if line_idx < first_line {
            continue;
        }
        if line_idx > last_line {
            break;
        }
        let y = editor_rect.min.y + line_idx as f32 * row_height + row_height * 0.5;
        for (col, ch) in line.chars().enumerate() {
            let x = editor_rect.min.x + col as f32 * advance + advance * 0.5;
            match ch {
                ' ' => {
                    ui.painter().text(
                        Pos2::new(x, y),
                        Align2::CENTER_CENTER,
                        "·",
                        font.clone(),
                        color,
                    );
                }
                '\t' => {
                    ui.painter().text(
                        Pos2::new(x, y),
                        Align2::CENTER_CENTER,
                        "→",
                        font.clone(),
                        color,
                    );
                }
                _ => {}
            }
        }
    }
}

fn paint_range_overlay(
    ui: &egui::Ui,
    content: &str,
    editor_rect: Rect,
    row_height: f32,
    advance: f32,
    byte_start: usize,
    byte_end: usize,
    theme: &Theme,
) {
    let line = byte_to_line(content, byte_start);
    let (ls, _) = line_byte_range(content, line);
    let col_bytes = byte_start.saturating_sub(ls);
    let col_chars = content[ls..ls + col_bytes].chars().count();
    let width_chars = content[byte_start..byte_end.min(content.len())].chars().count();

    let x = editor_rect.min.x + col_chars as f32 * advance;
    let y = editor_rect.min.y + line as f32 * row_height;
    let r = Rect::from_min_size(
        Pos2::new(x, y),
        Vec2::new(width_chars as f32 * advance, row_height),
    );

    let border = theme.widgets.border.to_color32();
    ui.painter()
        .rect_stroke(r, 2.0, Stroke::new(1.0, border), egui::StrokeKind::Inside);
}

fn paint_error_squiggle(ui: &egui::Ui, editor_rect: Rect, row_height: f32, line: usize) {
    let y_top = editor_rect.min.y + line as f32 * row_height;
    let y = y_top + row_height - 2.0;
    let x_start = editor_rect.min.x + 2.0;
    let x_end = editor_rect.max.x - 2.0;
    let amp = 1.6;
    let wavelength = 4.0;
    let color = Color32::from_rgb(220, 80, 80);
    let stroke = Stroke::new(1.0, color);
    let mut x = x_start;
    while x < x_end {
        let next = (x + wavelength).min(x_end);
        let mid = x + wavelength * 0.5;
        ui.painter()
            .line_segment([Pos2::new(x, y), Pos2::new(mid, y + amp)], stroke);
        ui.painter()
            .line_segment([Pos2::new(mid, y + amp), Pos2::new(next, y)], stroke);
        x = next;
    }
}

fn paint_bracket_highlight(
    ui: &egui::Ui,
    content: &str,
    editor_rect: Rect,
    row_height: f32,
    advance: f32,
    byte_idx: usize,
    theme: &Theme,
) {
    let line = byte_to_line(content, byte_idx);
    let line_start = line_to_byte(content, line);
    let col_bytes = byte_idx.saturating_sub(line_start);
    let col_chars = content[line_start..line_start + col_bytes].chars().count();

    let x = editor_rect.min.x + col_chars as f32 * advance;
    let y = editor_rect.min.y + line as f32 * row_height;
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(advance, row_height));

    let accent = theme.semantic.accent.to_color32();
    let fill =
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 70);
    ui.painter().rect_filled(r, 2.0, fill);
}

/// Subtle background tint on the line containing the cursor.
fn paint_current_line_highlight(
    ui: &egui::Ui,
    content: &str,
    editor_rect: Rect,
    row_height: f32,
    cursor_byte: usize,
    theme: &Theme,
) {
    let line = byte_to_line(content, cursor_byte);
    let y = editor_rect.min.y + line as f32 * row_height;
    let rect = Rect::from_min_size(
        Pos2::new(editor_rect.min.x, y),
        Vec2::new(editor_rect.width(), row_height),
    );
    let accent = theme.semantic.accent.to_color32();
    let bg = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 12);
    ui.painter().rect_filled(rect, 0.0, bg);
}

// -------------------------------------------------------------------------
// Editor shortcuts (VSCode-style editing)
// -------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum EditorShortcut {
    Tab,
    ShiftTab,
    Enter,
    Home,
    DuplicateLine,
    DeleteLine,
    MoveLineUp,
    MoveLineDown,
    SelectNextOccurrence,
}

/// Picks Tab / Shift+Tab / Enter / Home — the four shortcuts that conflict
/// with text input and therefore have to consume egui events directly rather
/// than going through the keybinding system. The other VSCode-style shortcuts
/// (DuplicateLine, DeleteLine, MoveLineUp/Down, SelectNextOccurrence) are now
/// driven by `EditorShortcuts` via the central keybindings.
fn pick_editor_shortcut(ui: &mut egui::Ui) -> Option<EditorShortcut> {
    let mut out = None;
    ui.input_mut(|i| {
        if i.consume_key(Modifiers::SHIFT, Key::Tab) {
            out = Some(EditorShortcut::ShiftTab);
            return;
        }
        if i.consume_key(Modifiers::NONE, Key::Tab) {
            out = Some(EditorShortcut::Tab);
            return;
        }
        if i.consume_key(Modifiers::NONE, Key::Enter) {
            out = Some(EditorShortcut::Enter);
            return;
        }
        if i.consume_key(Modifiers::NONE, Key::Home) {
            out = Some(EditorShortcut::Home);
        }
    });
    out
}

fn apply_editor_shortcut(
    state: &mut CodeEditorState,
    active_idx: usize,
    ctx: &egui::Context,
    editor_id: Id,
    sc: EditorShortcut,
) {
    // Read selection from stored state — in char coordinates.
    let (sel_a_char, sel_b_char) = egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| (r.primary.index, r.secondary.index))
        .unwrap_or((0, 0));

    let Some(file) = state.open_files.get_mut(active_idx) else { return };

    // Translate to bytes.
    let sel_a = char_to_byte(&file.content, sel_a_char);
    let sel_b = char_to_byte(&file.content, sel_b_char);
    let (lo, hi) = if sel_a <= sel_b { (sel_a, sel_b) } else { (sel_b, sel_a) };
    let has_selection = lo != hi;

    let mut new_selection_bytes: Option<(usize, usize)> = None;
    let mut mutated = false;

    match sc {
        EditorShortcut::Tab => {
            if has_selection {
                new_selection_bytes = Some(indent_selection(&mut file.content, sel_a, sel_b, false));
                mutated = true;
            } else {
                // Insert 4 spaces at cursor
                let indent = " ".repeat(TAB_SIZE);
                file.content.insert_str(sel_a.min(file.content.len()), &indent);
                let new = sel_a + TAB_SIZE;
                new_selection_bytes = Some((new, new));
                mutated = true;
            }
        }
        EditorShortcut::ShiftTab => {
            // Always dedent by line, whether or not there's a selection.
            let (a, b) = if has_selection { (sel_a, sel_b) } else { (sel_a, sel_a) };
            new_selection_bytes = Some(indent_selection(&mut file.content, a, b, true));
            mutated = true;
        }
        EditorShortcut::Enter => {
            // Auto-indent: newline + leading whitespace of the current line.
            let indent = leading_whitespace_of_line(&file.content, sel_a);
            let insertion = format!("\n{}", indent);
            let clamped_lo = lo.min(file.content.len());
            let clamped_hi = hi.min(file.content.len());
            file.content.replace_range(clamped_lo..clamped_hi, &insertion);
            let new = clamped_lo + insertion.len();
            new_selection_bytes = Some((new, new));
            mutated = true;
        }
        EditorShortcut::Home => {
            // Move cursor (no selection) to smart home target. If Shift is held
            // we'd want to extend selection, but egui's consume_key already
            // stripped the modifier — for now just move without selecting.
            let target = smart_home_target(&file.content, sel_a);
            new_selection_bytes = Some((target, target));
            // No content change.
        }
        EditorShortcut::DuplicateLine => {
            let (a, b) = if has_selection { (sel_a, sel_b) } else { (sel_a, sel_a) };
            new_selection_bytes = Some(duplicate_lines(&mut file.content, a, b));
            mutated = true;
        }
        EditorShortcut::DeleteLine => {
            let (a, b) = if has_selection { (sel_a, sel_b) } else { (sel_a, sel_a) };
            let new_cursor = delete_lines(&mut file.content, a, b);
            new_selection_bytes = Some((new_cursor, new_cursor));
            mutated = true;
        }
        EditorShortcut::MoveLineUp => {
            let (a, b) = if has_selection { (sel_a, sel_b) } else { (sel_a, sel_a) };
            if let Some(new) = move_lines_up(&mut file.content, a, b) {
                new_selection_bytes = Some(new);
                mutated = true;
            }
        }
        EditorShortcut::MoveLineDown => {
            let (a, b) = if has_selection { (sel_a, sel_b) } else { (sel_a, sel_a) };
            if let Some(new) = move_lines_down(&mut file.content, a, b) {
                new_selection_bytes = Some(new);
                mutated = true;
            }
        }
        EditorShortcut::SelectNextOccurrence => {
            if let Some((a, b)) = select_next_occurrence(&file.content, sel_a, sel_b, false) {
                new_selection_bytes = Some((a, b));
            }
        }
    }

    if mutated {
        file.is_modified = true;
    }

    if let Some((na, nb)) = new_selection_bytes {
        let a_char = byte_to_char(&file.content, na);
        let b_char = byte_to_char(&file.content, nb);
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
// Error panel (unchanged behaviour)
// -------------------------------------------------------------------------

fn render_error_panel(
    ui: &mut egui::Ui,
    state: &CodeEditorState,
    active_idx: usize,
    error_rect: Rect,
    secondary: Color32,
    error_color: Color32,
) {
    let Some(file) = state.open_files.get(active_idx) else { return };
    let Some(error) = file.error.as_ref() else { return };

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

    let panel_width = error_rect.width();
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

// -------------------------------------------------------------------------
// Auto-close brackets / quotes
// -------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum AutocloseAction {
    /// Insert (open, close) at the cursor; cursor lands between them.
    InsertPair(char, char),
    /// Cursor sits next to a closing char that the user just typed; skip
    /// over the existing one instead of doubling it.
    Skip,
}

fn decide_autoclose(typed: char, next_ch: Option<char>) -> Option<AutocloseAction> {
    match typed {
        '(' => Some(AutocloseAction::InsertPair('(', ')')),
        '[' => Some(AutocloseAction::InsertPair('[', ']')),
        '{' => Some(AutocloseAction::InsertPair('{', '}')),
        '"' => {
            if next_ch == Some('"') {
                Some(AutocloseAction::Skip)
            } else {
                Some(AutocloseAction::InsertPair('"', '"'))
            }
        }
        '\'' => {
            if next_ch == Some('\'') {
                Some(AutocloseAction::Skip)
            } else {
                Some(AutocloseAction::InsertPair('\'', '\''))
            }
        }
        ')' | ']' | '}' if next_ch == Some(typed) => Some(AutocloseAction::Skip),
        _ => None,
    }
}

fn apply_autoclose(
    state: &mut CodeEditorState,
    active_idx: usize,
    cursor_char: usize,
    cursor_byte: usize,
    action: AutocloseAction,
    ctx: &egui::Context,
    editor_id: Id,
) {
    match action {
        AutocloseAction::InsertPair(open, close) => {
            let Some(file) = state.open_files.get_mut(active_idx) else { return };
            let mut buf = String::with_capacity(2);
            buf.push(open);
            buf.push(close);
            let insert_at = cursor_byte.min(file.content.len());
            file.content.insert_str(insert_at, &buf);
            file.is_modified = true;
            // Cursor goes between open and close.
            let new_char = cursor_char + 1;
            if let Some(mut s) = egui::TextEdit::load_state(ctx, editor_id) {
                s.cursor
                    .set_char_range(Some(CCursorRange::one(CCursor::new(new_char))));
                s.store(ctx, editor_id);
            }
        }
        AutocloseAction::Skip => {
            let new_char = cursor_char + 1;
            if let Some(mut s) = egui::TextEdit::load_state(ctx, editor_id) {
                s.cursor
                    .set_char_range(Some(CCursorRange::one(CCursor::new(new_char))));
                s.store(ctx, editor_id);
            }
        }
    }
}

// -------------------------------------------------------------------------
// Status bar
// -------------------------------------------------------------------------

fn render_status_bar(
    ui: &egui::Ui,
    state: &CodeEditorState,
    active_idx: usize,
    cursor_char: Option<usize>,
    lang: Language,
    rect: Rect,
    theme: &Theme,
) {
    let bg = theme.surfaces.faint.to_color32();
    let muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();
    let modified_color = Color32::from_rgb(220, 180, 80);

    ui.painter().rect_filled(rect, 0.0, bg);
    ui.painter().line_segment(
        [rect.min, Pos2::new(rect.max.x, rect.min.y)],
        Stroke::new(1.0, theme.widgets.border.to_color32()),
    );

    let Some(file) = state.open_files.get(active_idx) else { return };

    let font = FontId::proportional(11.0);
    let center_y = rect.center().y;

    // Left: file path + modified dot
    let mut left_x = rect.min.x + 8.0;
    if file.is_modified {
        ui.painter().text(
            Pos2::new(left_x, center_y),
            Align2::LEFT_CENTER,
            "●",
            font.clone(),
            modified_color,
        );
        left_x += 12.0;
    }
    let path_str = file.path.display().to_string();
    ui.painter().text(
        Pos2::new(left_x, center_y),
        Align2::LEFT_CENTER,
        &path_str,
        font.clone(),
        muted,
    );

    // Right: line/col then language
    let mut right_x = rect.max.x - 8.0;

    let lang_label = match lang {
        Language::Lua => "Lua",
        Language::Rhai => "Rhai",
        Language::Rust => "Rust",
        Language::Wgsl => "WGSL",
        Language::Python => "Python",
        Language::Shell => "Shell",
        Language::Sql => "SQL",
        Language::Json => "JSON",
        Language::Toml => "TOML",
        Language::PlainText => "Text",
    };
    ui.painter().text(
        Pos2::new(right_x, center_y),
        Align2::RIGHT_CENTER,
        lang_label,
        font.clone(),
        accent,
    );
    right_x -= 8.0 + lang_label.len() as f32 * 7.0;

    if let Some(cidx) = cursor_char {
        let byte_idx = char_to_byte(&file.content, cidx);
        let line = byte_to_line(&file.content, byte_idx);
        let line_start = line_to_byte(&file.content, line);
        let col = file.content[line_start..byte_idx].chars().count();
        let pos_text = format!("Ln {}, Col {}", line + 1, col + 1);
        ui.painter().text(
            Pos2::new(right_x, center_y),
            Align2::RIGHT_CENTER,
            &pos_text,
            font.clone(),
            muted,
        );
    }
}

// -------------------------------------------------------------------------
// Close-confirm modal
// -------------------------------------------------------------------------

fn render_close_confirm(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    idx: usize,
    _theme: &Theme,
) {
    let name = state
        .open_files
        .get(idx)
        .map(|f| f.name.clone())
        .unwrap_or_default();

    let mut open = true;
    let mut action: Option<CloseConfirmAction> = None;

    egui::Window::new("Save changes?")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui| {
            ui.label(format!("\"{}\" has unsaved changes.", name));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    action = Some(CloseConfirmAction::Save);
                }
                if ui.button("Discard").clicked() {
                    action = Some(CloseConfirmAction::Discard);
                }
                if ui.button("Cancel").clicked() {
                    action = Some(CloseConfirmAction::Cancel);
                }
            });
        });

    if !open {
        action = Some(CloseConfirmAction::Cancel);
    }

    if let Some(a) = action {
        match a {
            CloseConfirmAction::Save => {
                state.save_file(idx);
                state.close_tab(idx);
            }
            CloseConfirmAction::Discard => {
                state.close_tab(idx);
            }
            CloseConfirmAction::Cancel => {}
        }
        state.close_confirm_tab = None;
    }
}

enum CloseConfirmAction {
    Save,
    Discard,
    Cancel,
}

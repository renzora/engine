use bevy::prelude::*;
use bevy_egui::egui::{
    self, text::CCursor, text::CCursorRange, Align2, Color32, CursorIcon, FontFamily, FontId, Id,
    Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use renzora::core::keybindings::{EditorAction, KeyBindings};
use egui_phosphor::regular::{
    ARROW_RIGHT, BRACKETS_CURLY, CODE, COLUMNS_PLUS_RIGHT, FILE_PLUS, FLOPPY_DISK,
    FLOPPY_DISK_BACK, GIT_DIFF, LIST_NUMBERS, MAGNIFYING_GLASS, PARAGRAPH,
    SQUARES_FOUR, WARNING, X,
};
use renzora_theme::Theme;
use std::path::PathBuf;

use crate::actions::{
    byte_to_char, byte_to_line, char_to_byte, compute_foldable_lines, delete_lines,
    duplicate_lines, find_all_occurrences, find_matching_bracket,
    indent_selection, leading_whitespace_of_line, line_byte_range, line_to_byte,
    move_lines_down, move_lines_up, select_next_occurrence, smart_home_target,
    toggle_block_comment, toggle_line_comment, word_range_at_byte, TAB_SIZE,
};
use crate::autocomplete::{self, CompletionItem};
use crate::highlight::{highlight, Language, TokenStyle};
use crate::state::{find_all_matches, CodeEditorState};

/// Width reserved for the line-number gutter (scales with font size).
/// Includes a left strip for breakpoint dots and a right strip for fold
/// chevrons.
fn gutter_width(line_count: usize, font_size: f32) -> f32 {
    let digits = line_count.max(1).to_string().len() as f32;
    let bp_strip = 14.0;
    let chev_strip = 12.0;
    let digits_w = digits * font_size * 0.62;
    bp_strip + digits_w + chev_strip + 8.0
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
    pub goto_definition: bool,
    pub format: bool,
    pub show_diff: bool,
    pub toggle_fold: bool,
    pub add_cursor_above: bool,
    pub add_cursor_below: bool,
    pub clear_extra_cursors: bool,
    pub split_right: bool,
}

impl EditorShortcuts {
    /// Read the current user keybindings and consume any matching events.
    pub fn collect(ui: &mut egui::Ui, world: &World) -> Self {
        let mut out = Self::default();
        let Some(kb) = world.get_resource::<KeyBindings>() else {
            return out;
        };
        #[allow(unused_mut)]
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
        out.goto_definition = take(EditorAction::CodeGoToDefinition);
        out.format = take(EditorAction::CodeFormat);
        out.show_diff = take(EditorAction::CodeShowDiff);
        out.toggle_fold = take(EditorAction::CodeToggleFold);
        out.add_cursor_above = take(EditorAction::CodeAddCursorAbove);
        out.add_cursor_below = take(EditorAction::CodeAddCursorBelow);
        out.clear_extra_cursors = take(EditorAction::CodeClearExtraCursors);
        out.split_right = take(EditorAction::CodeSplitRight);
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

/// Outer entry point. When `split_active_tab` is set, lays out two panes
/// horizontally: the existing full-featured editor on the left and a
/// stripped-down second editor on the right.
pub fn render_code_editor_panel(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    theme: &Theme,
    scripts_dir: Option<PathBuf>,
    shortcuts: EditorShortcuts,
) {
    // Validate the split tab so deletions don't dangle.
    if let Some(idx) = state.split_active_tab {
        if idx >= state.open_files.len() || Some(idx) == state.active_tab {
            state.split_active_tab = None;
        }
    }

    let split = state.split_active_tab;
    if split.is_none() {
        render_code_editor_content(ui, state, theme, scripts_dir, shortcuts);
        return;
    }

    let avail = ui.available_rect_before_wrap();
    let half_w = avail.width() * 0.5;
    let left_size = Vec2::new(half_w - 2.0, avail.height());
    let right_size = Vec2::new(avail.width() - half_w - 2.0, avail.height());

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(left_size, egui::Layout::top_down(egui::Align::Min), |ui| {
            render_code_editor_content(ui, state, theme, scripts_dir.clone(), shortcuts);
        });
        ui.separator();
        ui.allocate_ui_with_layout(right_size, egui::Layout::top_down(egui::Align::Min), |ui| {
            render_split_pane(ui, state, theme);
        });
    });
}

/// Stripped-down second editor for the right side of a split. Same buffer
/// and same edit operations as the main pane, but only the tab strip and
/// textedit — no toolbar, find bar, modals or status bar (those live in the
/// main pane to avoid duplicate keybinding events).
fn render_split_pane(ui: &mut egui::Ui, state: &mut CodeEditorState, theme: &Theme) {
    let muted = theme.text.muted.to_color32();
    let disabled = theme.text.disabled.to_color32();
    let surface_panel = theme.surfaces.panel.to_color32();
    let surface_faint = theme.surfaces.faint.to_color32();

    let Some(active_idx) = state.split_active_tab else { return };
    if active_idx >= state.open_files.len() {
        return;
    }

    // Mini tab strip with click-to-switch + close-split button.
    let mut close_split = false;
    let mut new_idx: Option<usize> = None;
    ui.horizontal(|ui| {
        for (i, file) in state.open_files.iter().enumerate() {
            let is_active = i == active_idx;
            let bg = if is_active { surface_panel } else { surface_faint };
            egui::Frame::new()
                .fill(bg)
                .inner_margin(egui::Margin::symmetric(6, 3))
                .show(ui, |ui| {
                    let label = if file.is_modified {
                        format!("{} *", file.name)
                    } else {
                        file.name.clone()
                    };
                    let resp = ui.add(
                        egui::Label::new(RichText::new(label).size(11.0))
                            .sense(Sense::click()),
                    );
                    if resp.clicked() {
                        new_idx = Some(i);
                    }
                });
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new(RichText::new(X).size(10.0).color(muted)).frame(false))
                .clicked()
            {
                close_split = true;
            }
        });
    });
    if let Some(i) = new_idx {
        state.split_active_tab = Some(i);
    }
    if close_split {
        state.split_active_tab = None;
        return;
    }
    ui.separator();

    let file_ext = state.open_files[active_idx]
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    let lang = Language::from_extension(&file_ext);

    let font_size = state.font_size;
    let editor_id = Id::new(("code_editor_textedit_split", active_idx));
    let style = TokenStyle::from_theme(FontId::monospace(font_size), theme);
    let row_height = ui.fonts_mut(|f| f.row_height(&FontId::monospace(font_size)));

    let avail = ui.available_rect_before_wrap();
    let scroll_id = Id::new(("code_editor_split_scroll", active_idx));
    egui::ScrollArea::vertical()
        .id_salt(scroll_id)
        .max_height(avail.height() - 4.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let line_count = state.open_files[active_idx]
                .content
                .lines()
                .count()
                .max(1);
            let gw = gutter_width(line_count, font_size);

            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                let mut job = highlight(text.as_str(), lang, &style);
                job.wrap.max_width = wrap_width;
                ui.painter().layout_job(job)
            };

            let start_x = ui.cursor().min.x;
            let text_pad_left = 10.0;
            ui.horizontal_top(|ui| {
                ui.add_space(gw + text_pad_left);
                let file = &mut state.open_files[active_idx];
                let avail_w = ui.available_width().max(100.0);
                let output = egui::TextEdit::multiline(&mut file.content)
                    .id(editor_id)
                    .font(FontId::monospace(font_size))
                    .code_editor()
                    .lock_focus(true)
                    .frame(false)
                    .layouter(&mut layouter)
                    .desired_width(avail_w)
                    .desired_rows(1)
                    .show(ui);
                if output.response.changed() {
                    file.is_modified = true;
                }

                let editor_rect = output.response.rect;
                let cur_char_idx = output.cursor_range.map(|c| c.primary.index);

                let gutter_rect = Rect::from_min_max(
                    Pos2::new(start_x, editor_rect.min.y),
                    Pos2::new(start_x + gw, editor_rect.max.y),
                );
                let foldable = compute_foldable_lines(&file.content, lang);
                let folded_set: std::collections::HashSet<usize> =
                    file.folds.keys().copied().collect();
                paint_gutter(
                    ui,
                    gutter_rect,
                    font_size,
                    row_height,
                    line_count,
                    cur_char_idx,
                    &file.content,
                    &file.breakpoints,
                    &folded_set,
                    &foldable,
                    theme,
                );
            });
        });

    let _ = (disabled, surface_panel);
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
    handle_extra_shortcuts(ui, state, lang, active_idx, shortcuts);

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

    // Remove focus/selection borders so the editor fills cleanly. egui
    // paints its selection rect *over* glyphs in `code_editor()` mode, so
    // we disable it entirely and render the selection ourselves as a
    // per-section `TextFormat::background` in the layouter (painted BEHIND
    // glyphs — the only reliable way to keep text readable).
    let now_time = ui.ctx().input(|i| i.time);
    let recently_typed = state
        .last_cursor_visible_reset
        .map(|t| now_time - t < 0.5)
        .unwrap_or(false);
    {
        let style = ui.style_mut();
        let no_stroke = Stroke::NONE;
        style.visuals.widgets.active.bg_stroke = no_stroke;
        style.visuals.widgets.hovered.bg_stroke = no_stroke;
        style.visuals.widgets.inactive.bg_stroke = no_stroke;
        style.visuals.widgets.noninteractive.bg_stroke = no_stroke;
        style.visuals.selection.stroke = no_stroke;
        // Suppress egui's selection rect. We paint it via TextFormat::background.
        style.visuals.selection.bg_fill = Color32::TRANSPARENT;
        if recently_typed {
            style.visuals.text_cursor.blink = false;
        }
        style.spacing.item_spacing = Vec2::ZERO;
    }
    let selection_bg = Color32::from_rgb(30, 60, 100);

    // While inside the keep-visible window, keep requesting repaints so the
    // cursor resumes blinking right after the window expires.
    if recently_typed {
        ui.ctx().request_repaint();
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
    // Filled inside the TextEdit closure when Alt+click adds a cursor.
    let mut pending_alt_extra: Option<usize> = None;
    // Filled inside the TextEdit closure when the user clicks a fold chevron.
    let mut pending_fold_toggle: Option<usize> = None;
    // Filled inside the TextEdit closure whenever content changed — drives
    // the "keep cursor visible" window so blink effectively resets on typing.
    let mut pending_blink_reset = false;

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

    // --- Multi-cursor edits (intercept text + backspace events) ---
    // When extra cursors are set, we own keyboard handling at the primary
    // cursor too — autoclose is bypassed so behavior stays consistent across
    // all cursors.
    if !state.extra_cursors.is_empty() && editor_focused {
        apply_multi_cursor_input(state, active_idx, ui, editor_id);
    } else if editor_focused {
        // Smart backspace: collapse a run of indentation to the previous tab
        // stop. Only intercepts when the caret is sitting in leading
        // whitespace with no selection — plain backspace behavior otherwise.
        try_smart_backspace(state, active_idx, ui, editor_id);
    }

    // --- Auto-close brackets / quotes (intercept the typed character) ---
    if state.auto_close_pairs && state.extra_cursors.is_empty() && editor_focused {
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

            // Read the pre-TextEdit cursor range from stored state so the
            // layouter can bake the selection in as a per-section background.
            // We keep the *raw* range (not filtered to "has selection") so we
            // can compare it against the post-show range later and request a
            // repaint if anything moved — fixes the "selection sticks until I
            // bump the font size" galley-cache staleness bug.
            let pre_cursor_range: Option<(usize, usize)> =
                egui::TextEdit::load_state(ui.ctx(), editor_id)
                    .and_then(|s| s.cursor.char_range())
                    .map(|r| (r.primary.index, r.secondary.index));
            let sel_range_chars: Option<(usize, usize)> =
                pre_cursor_range.and_then(|(a, b)| {
                    if a == b {
                        None
                    } else {
                        Some((a.min(b), a.max(b)))
                    }
                });

            // Prepare layouter borrowing the theme style.
            let mut layouter =
                |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                    let mut job = highlight(text.as_str(), lang, &style);
                    job.wrap.max_width = wrap_width;
                    if let Some((sa, sb)) = sel_range_chars {
                        let text_str = text.as_str();
                        let byte_start = char_to_byte(text_str, sa);
                        let byte_end = char_to_byte(text_str, sb);
                        apply_selection_background(&mut job, byte_start, byte_end, selection_bg);
                    }
                    ui.painter().layout_job(job)
                };

            // Lay out: [gutter space | padding | TextEdit]
            let start_x = ui.cursor().min.x;
            let text_pad_left = 10.0;
            ui.horizontal_top(|ui| {
                // Reserve gutter space — painted after TextEdit.
                ui.add_space(gw + text_pad_left);

                // Capture scalar flags before the mut-borrow of open_files so
                // we can read them while `file` is live.
                let show_whitespace = state.show_whitespace;
                let state_find_open = state.find_open;
                let state_find_text = state.find_text.clone();
                let state_find_case = state.find_case_sensitive;
                let state_find_word = state.find_whole_word;
                let state_extra_cursors = state.extra_cursors.clone();
                let file = &mut state.open_files[active_idx];
                let avail = ui.available_width().max(100.0);

                // Snapshot the primary cursor before TextEdit so an Alt+click
                // can spawn an extra cursor without losing the original one.
                let pre_primary_char = egui::TextEdit::load_state(ui.ctx(), editor_id)
                    .and_then(|s| s.cursor.char_range())
                    .map(|r| r.primary.index);
                let alt_held_before = ui.ctx().input(|i| i.modifiers.alt);

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
                    pending_blink_reset = true;
                }

                // Gather state we need outside the closure.
                let editor_rect = output.response.rect;
                let cur_char_idx = output
                    .cursor_range
                    .map(|c| c.primary.index);

                // Selection sync: the layouter baked the selection background
                // using the cursor range as of the PREVIOUS frame (what
                // `load_state` saw before `TextEdit::show` ran). If this
                // frame's cursor_range moved, the current galley has a stale
                // selection highlight baked in — request one more paint so
                // the layouter reruns with the fresh range. Without this,
                // egui's reactive repainting leaves the last stale galley
                // on screen until something else (font size change, click
                // elsewhere) forces a relayout.
                let cur_range_now = output
                    .cursor_range
                    .map(|c| (c.primary.index, c.secondary.index));
                if cur_range_now != pre_cursor_range {
                    ui.ctx().request_repaint();
                }

                // Alt+click → add an extra cursor at the clicked position
                // and restore the original primary cursor. Cheap "column
                // selection by clicks" workflow without a custom hit-test.
                if alt_held_before && output.response.clicked() {
                    if let (Some(pre), Some(now)) = (pre_primary_char, cur_char_idx) {
                        if pre != now {
                            let now_byte = char_to_byte(&file.content, now);
                            if !state_extra_cursors.iter().any(|&c| c == now_byte) {
                                // Capture for the post-textedit merge below.
                                pending_alt_extra = Some(now_byte);
                            }
                            // Restore primary cursor.
                            if let Some(mut s) = egui::TextEdit::load_state(ui.ctx(), editor_id) {
                                s.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(pre))));
                                s.store(ui.ctx(), editor_id);
                            }
                        }
                    }
                }

                // Paint gutter in reserved strip ----------------------------
                let gutter_rect = Rect::from_min_max(
                    Pos2::new(start_x, editor_rect.min.y),
                    Pos2::new(start_x + gw, editor_rect.max.y),
                );
                let foldable = compute_foldable_lines(&file.content, lang);
                let folded_set: std::collections::HashSet<usize> =
                    file.folds.keys().copied().collect();
                let gutter_click = paint_gutter(
                    ui,
                    gutter_rect,
                    font_size,
                    row_height,
                    line_count,
                    cur_char_idx,
                    &file.content,
                    &file.breakpoints,
                    &folded_set,
                    &foldable,
                    theme,
                );
                if let Some(line) = gutter_click.toggle_breakpoint {
                    if !file.breakpoints.insert(line) {
                        file.breakpoints.remove(&line);
                    }
                }
                if let Some(line) = gutter_click.toggle_fold {
                    pending_fold_toggle = Some(line);
                }

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

                // Find-all match highlights ---------------------------------
                if state_find_open && !state_find_text.is_empty() {
                    let matches = find_all_matches(
                        &file.content,
                        &state_find_text,
                        state_find_case,
                        state_find_word,
                    );
                    for (s, e) in matches {
                        paint_find_match(
                            ui,
                            &file.content,
                            editor_rect,
                            row_height,
                            mono_advance,
                            s,
                            e,
                            theme,
                        );
                    }
                }

                // Extra cursors (multi-cursor) ------------------------------
                for &extra_byte in &state_extra_cursors {
                    paint_extra_cursor(
                        ui,
                        &file.content,
                        editor_rect,
                        row_height,
                        mono_advance,
                        extra_byte,
                        theme,
                    );
                }

                // Breakpoint indicators on the editor side: thin tint on the
                // line so the user sees them even with the gutter scrolled
                // out of view.
                for &bp_line in &file.breakpoints {
                    if bp_line < line_count {
                        paint_breakpoint_line_tint(ui, editor_rect, row_height, bp_line);
                    }
                }

                // Fold-start lines: stamp a "··· N lines folded" badge at
                // the end of the row so the user knows content is hidden.
                for (fold_start, folded_text) in &file.folds {
                    let hidden = folded_text.lines().count();
                    paint_fold_badge(ui, editor_rect, row_height, *fold_start, hidden, theme);
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

    if let Some(byte) = pending_alt_extra {
        if !state.extra_cursors.contains(&byte) {
            state.extra_cursors.push(byte);
        }
    }

    if let Some(line) = pending_fold_toggle {
        apply_fold_toggle(state, active_idx, line, lang);
    }

    if pending_blink_reset {
        state.last_cursor_visible_reset = Some(now_time);
    }

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
        let matches = autocomplete::matching_completions(lang, &state.autocomplete_filter);
        if matches.is_empty() {
            state.autocomplete_open = false;
        } else {
            // Clamp selection.
            if state.autocomplete_selected >= matches.len() {
                state.autocomplete_selected = matches.len() - 1;
            }

            if wants_insert {
                let chosen = matches[state.autocomplete_selected];
                apply_completion_insert(state, active_idx, chosen, ui.ctx(), editor_id);
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

    // Diff modal ----------------------------------------------------------
    if state.diff_open {
        render_diff_modal(ui, state, active_idx, theme);
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
    use renzora_editor::MonoFont;

    let muted = theme.text.muted.to_color32();
    let disabled = theme.text.disabled.to_color32();
    let surface_panel = theme.surfaces.panel.to_color32();
    let border = theme.widgets.border.to_color32();

    egui::Frame::new()
        .fill(surface_panel)
        .inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                // --- File group -----------------------------------------
                if icon_btn(ui, FLOPPY_DISK, muted, theme, "Save  (Ctrl+S)") {
                    state.save_active();
                }
                if icon_btn(ui, FLOPPY_DISK_BACK, muted, theme, "Save All  (Ctrl+Shift+S)") {
                    state.save_all();
                }
                if let Some(dir) = scripts_dir.clone() {
                    if icon_btn(ui, FILE_PLUS, muted, theme, "New Script") {
                        state.create_new_script(dir);
                    }
                }

                toolbar_sep(ui, border);

                // --- Navigation group -----------------------------------
                if icon_btn(ui, MAGNIFYING_GLASS, muted, theme, "Find / Replace  (Ctrl+F)") {
                    state.find_open = !state.find_open;
                    state.find_focus_requested = state.find_open;
                }
                if icon_btn(ui, LIST_NUMBERS, muted, theme, "Go to Line  (Ctrl+G)") {
                    state.goto_line_open = !state.goto_line_open;
                    state.goto_line_focus_requested = state.goto_line_open;
                    state.goto_line_buffer.clear();
                }

                toolbar_sep(ui, border);

                // --- Edit helpers ---------------------------------------
                if icon_btn(ui, BRACKETS_CURLY, muted, theme, "Format Document  (Ctrl+Shift+F)") {
                    let lang = state
                        .open_files
                        .get(active_idx)
                        .and_then(|f| f.path.extension())
                        .and_then(|e| e.to_str())
                        .map(crate::highlight::Language::from_extension)
                        .unwrap_or(crate::highlight::Language::PlainText);
                    apply_format(state, active_idx, lang);
                }
                if icon_btn(ui, GIT_DIFF, muted, theme, "Diff vs Saved  (Ctrl+Alt+D)") {
                    state.diff_open = true;
                }

                toolbar_sep(ui, border);

                // --- Toggles --------------------------------------------
                if icon_toggle(
                    ui,
                    COLUMNS_PLUS_RIGHT,
                    state.split_active_tab.is_some(),
                    muted,
                    theme,
                    "Split Right  (Ctrl+\\)",
                ) {
                    if state.split_active_tab.is_some() {
                        state.split_active_tab = None;
                    } else {
                        state.split_active_tab = Some(active_idx);
                    }
                }
                if icon_toggle(ui, PARAGRAPH, state.show_whitespace, muted, theme, "Show whitespace") {
                    state.show_whitespace = !state.show_whitespace;
                }
                if icon_toggle(ui, SQUARES_FOUR, state.show_minimap, muted, theme, "Toggle minimap") {
                    state.show_minimap = !state.show_minimap;
                }

                // --- Right-aligned: font + size + path ------------------
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let path_str = state.open_files[active_idx].path.display().to_string();
                    ui.label(RichText::new(path_str).size(11.0).color(disabled));

                    toolbar_sep(ui, border);

                    // Font size stepper
                    if icon_btn(ui, "+", muted, theme, "Zoom in") {
                        state.zoom_in();
                    }
                    let size_text = format!("{}px", state.font_size as i32);
                    let size_resp = ui.add(
                        egui::Label::new(
                            RichText::new(size_text)
                                .size(11.0)
                                .color(muted)
                                .monospace(),
                        )
                        .sense(Sense::click()),
                    );
                    if size_resp.clone().on_hover_text("Reset zoom").clicked() {
                        state.zoom_reset();
                    }
                    if icon_btn(ui, "−", muted, theme, "Zoom out") {
                        state.zoom_out();
                    }

                    toolbar_sep(ui, border);

                    // Font family dropdown (mirrors the one in Settings)
                    egui::ComboBox::from_id_salt("code_editor_mono_font")
                        .width(140.0)
                        .selected_text(
                            RichText::new(state.mono_font.label())
                                .size(11.0)
                                .color(muted),
                        )
                        .show_ui(ui, |ui| {
                            for font in MonoFont::BUILTIN {
                                ui.selectable_value(
                                    &mut state.mono_font,
                                    font.clone(),
                                    font.label(),
                                );
                            }
                        });
                });
            });
        });
}

/// Custom-rendered icon button: 26x22 hit box, hover gets a subtle rounded
/// tint, no egui Button chrome. Tooltip shows on hover.
fn icon_btn(ui: &mut egui::Ui, glyph: &str, color: Color32, theme: &Theme, tooltip: &str) -> bool {
    let size = Vec2::new(26.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() {
        let h = theme.widgets.hovered_bg.to_color32();
        let fill = Color32::from_rgba_unmultiplied(h.r(), h.g(), h.b(), 120);
        ui.painter().rect_filled(rect, 3.0, fill);
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(13.0),
        color,
    );
    resp.on_hover_text(tooltip).clicked()
}

/// Like `icon_btn` but paints a persistent accent tint when `active`.
fn icon_toggle(
    ui: &mut egui::Ui,
    glyph: &str,
    active: bool,
    color: Color32,
    theme: &Theme,
    tooltip: &str,
) -> bool {
    let size = Vec2::new(26.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    if active {
        let accent = theme.semantic.accent.to_color32();
        let fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 70);
        ui.painter().rect_filled(rect, 3.0, fill);
    } else if resp.hovered() {
        let h = theme.widgets.hovered_bg.to_color32();
        let fill = Color32::from_rgba_unmultiplied(h.r(), h.g(), h.b(), 120);
        ui.painter().rect_filled(rect, 3.0, fill);
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let glyph_color = if active {
        theme.text.primary.to_color32()
    } else {
        color
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(13.0),
        glyph_color,
    );
    resp.on_hover_text(tooltip).clicked()
}

/// Thin vertical separator for grouping toolbar actions. Flanked by small
/// horizontal padding so groups read as distinct.
fn toolbar_sep(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 22.0), Sense::hover());
    let x = rect.center().x;
    let inset = 4.0;
    ui.painter().line_segment(
        [
            Pos2::new(x, rect.min.y + inset),
            Pos2::new(x, rect.max.y - inset),
        ],
        Stroke::new(1.0, color),
    );
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

/// Dispatch the keybindings that landed after the editor textedit ran.
/// These are handled here (instead of inside the textedit closure) so they
/// can mutate `state` freely without fighting the borrow checker.
fn handle_extra_shortcuts(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    lang: Language,
    active_idx: usize,
    shortcuts: EditorShortcuts,
) {
    let editor_id = Id::new(("code_editor_textedit", active_idx));

    if shortcuts.goto_definition {
        apply_goto_definition(state, active_idx, lang, ui.ctx(), editor_id);
    }
    if shortcuts.format {
        apply_format(state, active_idx, lang);
    }
    if shortcuts.show_diff {
        state.diff_open = true;
    }
    if shortcuts.toggle_fold {
        toggle_fold_at_cursor(state, active_idx, lang, ui.ctx(), editor_id);
    }
    if shortcuts.add_cursor_above {
        add_cursor_relative(state, active_idx, ui.ctx(), editor_id, -1);
    }
    if shortcuts.add_cursor_below {
        add_cursor_relative(state, active_idx, ui.ctx(), editor_id, 1);
    }
    if shortcuts.clear_extra_cursors {
        state.extra_cursors.clear();
    }
    if shortcuts.split_right {
        if state.split_active_tab.is_some() {
            state.split_active_tab = None;
        } else {
            state.split_active_tab = Some(active_idx);
        }
    }
}

/// F12: jump the cursor to the line where the identifier under the caret
/// is defined (function/class/struct in the same file via the outline parser).
fn apply_goto_definition(
    state: &mut CodeEditorState,
    active_idx: usize,
    lang: Language,
    ctx: &egui::Context,
    editor_id: Id,
) {
    let Some(file) = state.open_files.get(active_idx) else { return };

    // Identify the word under the cursor.
    let cursor_char = egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| r.primary.index)
        .unwrap_or(0);
    let cursor_byte = char_to_byte(&file.content, cursor_char);
    let Some((ws, we)) = word_range_at_byte(&file.content, cursor_byte) else { return };
    let word = &file.content[ws..we];

    // Search the active file's outline first.
    let symbols = crate::outline::extract_symbols(&file.content, lang);
    if let Some(sym) = symbols.iter().find(|s| s.name == word) {
        state.pending_goto_line = Some(sym.line + 1);
        return;
    }

    // Try other open tabs.
    for (i, f) in state.open_files.iter().enumerate() {
        if i == active_idx {
            continue;
        }
        let other_lang = f
            .path
            .extension()
            .and_then(|e| e.to_str())
            .map(Language::from_extension)
            .unwrap_or(Language::PlainText);
        let syms = crate::outline::extract_symbols(&f.content, other_lang);
        if let Some(sym) = syms.iter().find(|s| s.name == word) {
            state.active_tab = Some(i);
            state.pending_goto_line = Some(sym.line + 1);
            return;
        }
    }
    log::info!("Definition for '{}' not found in open files", word);
}

/// Re-indent the active file. Handles brace-based languages and Lua/Python by
/// matching block-opening / block-closing keywords. Anything else is left as-is.
fn apply_format(state: &mut CodeEditorState, active_idx: usize, lang: Language) {
    let Some(file) = state.open_files.get_mut(active_idx) else { return };
    let formatted = crate::format::format_source(&file.content, lang);
    if formatted != file.content {
        file.content = formatted;
        file.is_modified = true;
        log::info!("Formatted {}", file.name);
    }
}

/// Ctrl+Shift+[ : toggle folding the block that contains the caret.
fn toggle_fold_at_cursor(
    state: &mut CodeEditorState,
    active_idx: usize,
    lang: Language,
    ctx: &egui::Context,
    editor_id: Id,
) {
    let cursor_char = egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| r.primary.index)
        .unwrap_or(0);
    let cursor_line = {
        let Some(file) = state.open_files.get(active_idx) else { return };
        let cursor_byte = char_to_byte(&file.content, cursor_char);
        byte_to_line(&file.content, cursor_byte)
    };

    let target = {
        let Some(file) = state.open_files.get(active_idx) else { return };
        // If the caret is already on a fold-start line, toggle that fold
        // directly (including unfolding it).
        if file.folds.contains_key(&cursor_line) {
            Some(cursor_line)
        } else {
            // Otherwise walk up to the nearest foldable line.
            let foldable = compute_foldable_lines(&file.content, lang);
            let mut found = None;
            for line in (0..=cursor_line).rev() {
                if foldable.contains(&line) {
                    found = Some(line);
                    break;
                }
            }
            found
        }
    };
    if let Some(line) = target {
        apply_fold_toggle(state, active_idx, line, lang);
    }
}

/// Fold (by extracting the content into `file.folds`) or unfold (by
/// re-inserting from the map) the block starting at `start_line`.
fn apply_fold_toggle(
    state: &mut CodeEditorState,
    active_idx: usize,
    start_line: usize,
    lang: Language,
) {
    let Some(file) = state.open_files.get_mut(active_idx) else { return };

    if file.folds.contains_key(&start_line) {
        // Unfold: pop the stored text back into the buffer right after the
        // start line's newline.
        if let Some(text) = file.folds.remove(&start_line) {
            let insert_at = end_of_line_with_newline(&file.content, start_line);
            let clamp = insert_at.min(file.content.len());
            file.content.insert_str(clamp, &text);
            file.is_modified = true;
        }
        return;
    }

    // Fold: extract lines (start_line + 1)..=fold_end.
    let end_line = crate::actions::fold_end_line(&file.content, start_line, lang);
    if end_line <= start_line {
        return;
    }
    let extract_start = end_of_line_with_newline(&file.content, start_line);
    let extract_end = end_of_line_with_newline(&file.content, end_line);
    if extract_end <= extract_start || extract_end > file.content.len() {
        return;
    }
    let extracted: String = file.content[extract_start..extract_end].to_string();
    file.content.replace_range(extract_start..extract_end, "");
    file.folds.insert(start_line, extracted);
    file.is_modified = true;
}

/// Byte offset just past the end of `line` (including its trailing newline
/// when present). Returns `content.len()` for lines past the last one.
fn end_of_line_with_newline(content: &str, line: usize) -> usize {
    let bytes = content.as_bytes();
    let mut seen = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            if seen == line {
                return i + 1;
            }
            seen += 1;
        }
    }
    content.len()
}

/// Ctrl+Alt+Up/Down: clone the cursor to the same column on the line
/// above/below.
fn add_cursor_relative(
    state: &mut CodeEditorState,
    active_idx: usize,
    ctx: &egui::Context,
    editor_id: Id,
    delta: i32,
) {
    let cursor_char = match egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range())
    {
        Some(r) => r.primary.index,
        None => return,
    };
    let Some(file) = state.open_files.get(active_idx) else { return };
    let cursor_byte = char_to_byte(&file.content, cursor_char);
    let line = byte_to_line(&file.content, cursor_byte);
    let line_start = line_to_byte(&file.content, line);
    let col = file.content[line_start..cursor_byte].chars().count();

    let target_line_signed = line as i32 + delta;
    if target_line_signed < 0 {
        return;
    }
    let target_line = target_line_signed as usize;
    let line_count = file.content.lines().count();
    if target_line >= line_count {
        return;
    }

    let (ts, te) = line_byte_range(&file.content, target_line);
    let target_text = &file.content[ts..te];
    let target_chars = target_text.chars().count();
    let final_col = col.min(target_chars);
    let target_byte = ts + target_text.chars().take(final_col).map(|c| c.len_utf8()).sum::<usize>();

    if !state.extra_cursors.contains(&target_byte) {
        state.extra_cursors.push(target_byte);
    }
}

/// Backspace-in-indent dedent: if the caret sits at the end of a run of
/// leading whitespace (no selection, no extra cursors), a single backspace
/// deletes back to the previous TAB_SIZE-aligned column instead of just one
/// space. Matches VSCode's behavior with "editor.useTabStops".
fn try_smart_backspace(
    state: &mut CodeEditorState,
    active_idx: usize,
    ui: &mut egui::Ui,
    editor_id: Id,
) {
    // Only intercept if there's no selection. Reading cursor state from
    // TextEdit's memory up front so we can consume the event if we decide to.
    let cursor_range = egui::TextEdit::load_state(ui.ctx(), editor_id)
        .and_then(|s| s.cursor.char_range());
    let Some(range) = cursor_range else { return };
    if range.primary.index != range.secondary.index {
        return;
    }
    let cursor_char = range.primary.index;

    let (should_dedent, delete_len_bytes) = {
        let Some(file) = state.open_files.get(active_idx) else { return };
        let cursor_byte = char_to_byte(&file.content, cursor_char);
        if cursor_byte == 0 {
            return;
        }
        let line = byte_to_line(&file.content, cursor_byte);
        let line_start = line_to_byte(&file.content, line);
        if cursor_byte <= line_start {
            return;
        }
        let leading = &file.content[line_start..cursor_byte];
        // Only activate if every char from line-start to cursor is a space
        // or tab (i.e. we're still in the indentation zone).
        if !leading.chars().all(|c| c == ' ' || c == '\t') {
            return;
        }
        let col = leading.chars().count();
        let prev_stop = if col % TAB_SIZE == 0 {
            col - TAB_SIZE
        } else {
            col - (col % TAB_SIZE)
        };
        let delete_chars = col - prev_stop;
        if delete_chars <= 1 {
            return;
        }
        // Convert char count to byte length for a whitespace-only slice —
        // spaces/tabs are 1 byte each, so chars == bytes here.
        (true, delete_chars)
    };
    if !should_dedent {
        return;
    }

    // Consume the backspace event so TextEdit doesn't also process it.
    let mut pressed = false;
    ui.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::Backspace) {
            pressed = true;
        }
    });
    if !pressed {
        return;
    }

    let now = ui.ctx().input(|i| i.time);
    let Some(file) = state.open_files.get_mut(active_idx) else { return };
    let cursor_byte = char_to_byte(&file.content, cursor_char);
    let from = cursor_byte.saturating_sub(delete_len_bytes);
    file.content.replace_range(from..cursor_byte, "");
    file.is_modified = true;

    let new_char = byte_to_char(&file.content, from);
    if let Some(mut s) = egui::TextEdit::load_state(ui.ctx(), editor_id) {
        s.cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(new_char))));
        s.store(ui.ctx(), editor_id);
    }
    state.last_cursor_visible_reset = Some(now);
}

/// Multi-cursor input handler. Consumes Text and Backspace events from egui
/// when extra cursors are set, then applies the same operation at the primary
/// cursor and every extra cursor. Operations are applied bottom-up so earlier
/// edits don't shift later byte offsets.
fn apply_multi_cursor_input(
    state: &mut CodeEditorState,
    active_idx: usize,
    ui: &mut egui::Ui,
    editor_id: Id,
) {
    let primary_char = match egui::TextEdit::load_state(ui.ctx(), editor_id)
        .and_then(|s| s.cursor.char_range())
    {
        Some(r) => r.primary.index,
        None => return,
    };
    let primary_byte = {
        let file = &state.open_files[active_idx];
        char_to_byte(&file.content, primary_char)
    };

    let mut insertion: Option<String> = None;
    let mut backspace = false;
    ui.input_mut(|i| {
        let mut idx_to_remove = None;
        for (idx, ev) in i.events.iter().enumerate() {
            if let egui::Event::Text(s) = ev {
                if s.chars().count() == 1 {
                    insertion = Some(s.clone());
                    idx_to_remove = Some(idx);
                    break;
                }
            }
        }
        if let Some(idx) = idx_to_remove {
            i.events.remove(idx);
        }
        if i.consume_key(Modifiers::NONE, Key::Backspace) {
            backspace = true;
        }
    });

    if let Some(text) = insertion {
        multi_insert(state, active_idx, primary_byte, &text, ui.ctx(), editor_id);
        state.last_cursor_visible_reset = Some(ui.ctx().input(|i| i.time));
    }
    if backspace {
        multi_backspace(state, active_idx, primary_byte, ui.ctx(), editor_id);
        state.last_cursor_visible_reset = Some(ui.ctx().input(|i| i.time));
    }
}

fn multi_insert(
    state: &mut CodeEditorState,
    active_idx: usize,
    primary_byte: usize,
    text: &str,
    ctx: &egui::Context,
    editor_id: Id,
) {
    let inserted_len = text.len();
    let Some(file) = state.open_files.get_mut(active_idx) else { return };

    // Sorted list of all cursor positions including the primary.
    let mut positions: Vec<usize> = state.extra_cursors.clone();
    positions.push(primary_byte);
    positions.sort_unstable();
    positions.dedup();

    // Insert from largest to smallest so earlier insertions don't shift later
    // positions inside this loop.
    let mut sorted_desc = positions.clone();
    sorted_desc.sort_unstable_by(|a, b| b.cmp(a));
    for pos in sorted_desc {
        let p = pos.min(file.content.len());
        file.content.insert_str(p, text);
    }
    file.is_modified = true;

    // Compute final cursor positions: cursor at sorted-asc index k shifts by
    // inserted_len * (k + 1) (k earlier inserts plus its own).
    let new_positions: Vec<usize> = positions
        .iter()
        .enumerate()
        .map(|(k, p)| p + inserted_len * (k + 1))
        .collect();

    let primary_index = positions.iter().position(|&p| p == primary_byte).unwrap_or(0);
    let new_primary_byte = new_positions[primary_index];
    let new_extras: Vec<usize> = new_positions
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != primary_index)
        .map(|(_, &p)| p)
        .collect();
    state.extra_cursors = new_extras;

    let new_primary_char = byte_to_char(&file.content, new_primary_byte);
    if let Some(mut s) = egui::TextEdit::load_state(ctx, editor_id) {
        s.cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(new_primary_char))));
        s.store(ctx, editor_id);
    }
}

fn multi_backspace(
    state: &mut CodeEditorState,
    active_idx: usize,
    primary_byte: usize,
    ctx: &egui::Context,
    editor_id: Id,
) {
    let Some(file) = state.open_files.get_mut(active_idx) else { return };

    let mut positions: Vec<usize> = state.extra_cursors.clone();
    positions.push(primary_byte);
    positions.sort_unstable();
    positions.dedup();

    // Determine each cursor's "delete one char before" range. We work in
    // characters (not bytes) so the deletion mirrors a real backspace on
    // multi-byte chars.
    let mut ranges: Vec<(usize, usize)> = positions
        .iter()
        .filter_map(|&p| {
            if p == 0 {
                return None;
            }
            // Walk back one char from byte p.
            let prev_char_len = file.content[..p]
                .chars()
                .next_back()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            if prev_char_len == 0 {
                None
            } else {
                Some((p - prev_char_len, p))
            }
        })
        .collect();

    // Apply largest-first.
    ranges.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    for (s_byte, e_byte) in &ranges {
        let s = (*s_byte).min(file.content.len());
        let e = (*e_byte).min(file.content.len());
        if s < e {
            file.content.replace_range(s..e, "");
        }
    }
    file.is_modified = true;

    // Compute new cursor positions.
    // For each original position, count how many ranges started before-or-at
    // its position, and sum up the bytes deleted.
    let new_positions: Vec<usize> = positions
        .iter()
        .map(|&p| {
            // Find this position in the (deletion ranges) — its own range
            // ended at p, so the byte at the deletion start is the new cursor.
            let mut shift = 0usize;
            for (s_byte, e_byte) in &ranges {
                if *e_byte <= p {
                    shift += e_byte - s_byte;
                }
            }
            p.saturating_sub(shift)
        })
        .collect();

    let primary_index = positions
        .iter()
        .position(|&p| p == primary_byte)
        .unwrap_or(0);
    let new_primary_byte = new_positions[primary_index];
    let new_extras: Vec<usize> = new_positions
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != primary_index)
        .map(|(_, &p)| p)
        .collect();
    state.extra_cursors = new_extras;

    let new_primary_char = byte_to_char(&file.content, new_primary_byte);
    if let Some(mut s) = egui::TextEdit::load_state(ctx, editor_id) {
        s.cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(new_primary_char))));
        s.store(ctx, editor_id);
    }
}

// -------------------------------------------------------------------------
// Find bar
// -------------------------------------------------------------------------

fn render_find_bar(ui: &mut egui::Ui, state: &mut CodeEditorState, theme: &Theme) {
    let surface_faint = theme.surfaces.faint.to_color32();
    let muted = theme.text.muted.to_color32();

    // Live match count for the active file (cheap on small/medium files).
    let match_count = state
        .active_tab
        .and_then(|i| state.open_files.get(i))
        .filter(|_| !state.find_text.is_empty())
        .map(|f| {
            find_all_matches(
                &f.content,
                &state.find_text,
                state.find_case_sensitive,
                state.find_whole_word,
            )
            .len()
        })
        .unwrap_or(0);

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
                ui.checkbox(&mut state.find_whole_word, RichText::new("ab").size(11.0))
                    .on_hover_text("Whole word");

                if !state.find_text.is_empty() {
                    let label = if match_count == 0 {
                        "No results".to_string()
                    } else {
                        format!("{} match{}", match_count, if match_count == 1 { "" } else { "es" })
                    };
                    ui.label(RichText::new(label).size(11.0).color(muted));
                }

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
    matches: &[CompletionItem],
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

                // Snippet items get a "{}" prefix glyph so they read as
                // distinct from API symbols.
                let glyph = match sym {
                    CompletionItem::Snippet(_) => "{}",
                    CompletionItem::Symbol(_) => "ƒ ",
                };
                let glyph_color = match sym {
                    CompletionItem::Snippet(_) => theme.semantic.accent.to_color32(),
                    CompletionItem::Symbol(_) => text_muted,
                };
                ui.painter().text(
                    Pos2::new(row_rect.min.x + 6.0, row_rect.center().y),
                    Align2::LEFT_CENTER,
                    glyph,
                    FontId::new(11.0, FontFamily::Monospace),
                    glyph_color,
                );

                ui.painter().text(
                    Pos2::new(row_rect.min.x + 26.0, row_rect.center().y),
                    Align2::LEFT_CENTER,
                    sym.label(),
                    FontId::new(12.0, FontFamily::Monospace),
                    if is_selected { Color32::WHITE } else { text_primary },
                );
                // Category badge
                ui.painter().text(
                    Pos2::new(row_rect.max.x - 10.0, row_rect.center().y),
                    Align2::RIGHT_CENTER,
                    sym.category(),
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
                    sym.detail(),
                    FontId::new(11.0, FontFamily::Monospace),
                    text_primary,
                );
                ui.painter().text(
                    Pos2::new(footer_rect.min.x + 8.0, footer_rect.min.y + 24.0),
                    Align2::LEFT_TOP,
                    sym.doc(),
                    FontId::proportional(10.0),
                    text_secondary,
                );
            }
        });
}

fn apply_completion_insert(
    state: &mut CodeEditorState,
    active_idx: usize,
    item: CompletionItem,
    ctx: &egui::Context,
    editor_id: Id,
) {
    let start_byte = state.autocomplete_prefix_start;
    let file = match state.open_files.get_mut(active_idx) {
        Some(f) => f,
        None => return,
    };

    let cur_char = egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range().map(|r| r.primary.index))
        .unwrap_or_else(|| byte_to_char(&file.content, start_byte));
    let cur_byte = char_to_byte(&file.content, cur_char);
    let end_byte = cur_byte.max(start_byte);
    let s = start_byte.min(file.content.len());
    let e = end_byte.min(file.content.len());

    // Build the inserted text. Snippets keep their indentation aligned with
    // the line where the prefix starts so the body lands at the right depth.
    let (text, caret_offset_chars) = match item {
        CompletionItem::Symbol(sym) => (sym.name.to_string(), sym.name.chars().count()),
        CompletionItem::Snippet(snip) => {
            let line_indent = leading_whitespace_of_line(&file.content, s);
            let body = snip.body.replace('\n', &format!("\n{}", line_indent));
            // Locate `$0` placeholder; if missing, place caret at end.
            let caret_byte_in_body = body.find("$0").unwrap_or(body.len());
            let stripped = body.replacen("$0", "", 1);
            let caret_chars = stripped[..caret_byte_in_body.min(stripped.len())]
                .chars()
                .count();
            (stripped, caret_chars)
        }
    };

    file.content.replace_range(s..e, &text);
    file.is_modified = true;

    let new_char = byte_to_char(&file.content, s) + caret_offset_chars;
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
    ui: &mut egui::Ui,
    rect: Rect,
    font_size: f32,
    row_height: f32,
    line_count: usize,
    cursor_char_idx: Option<usize>,
    content: &str,
    breakpoints: &std::collections::HashSet<usize>,
    folded_lines: &std::collections::HashSet<usize>,
    foldable_lines: &std::collections::HashSet<usize>,
    theme: &Theme,
) -> GutterClick {
    let bg = theme.surfaces.faint.to_color32();
    let line_color = theme.widgets.border.to_color32();
    let muted = theme.text.muted.to_color32();
    let emph = theme.text.primary.to_color32();

    {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, bg);
        painter.line_segment(
            [Pos2::new(rect.max.x, rect.min.y), Pos2::new(rect.max.x, rect.max.y)],
            Stroke::new(1.0, line_color),
        );
    }

    let current_line = cursor_char_idx.map(|ci| {
        let bi = char_to_byte(content, ci);
        byte_to_line(content, bi)
    });

    // Reserve a strip on the left for breakpoint dots and the area right next
    // to the number column for fold chevrons.
    let bp_strip_w = 14.0;
    let chevron_strip_w = 12.0;
    let bp_strip = Rect::from_min_max(
        rect.min,
        Pos2::new(rect.min.x + bp_strip_w, rect.max.y),
    );
    let chevron_strip = Rect::from_min_max(
        Pos2::new(rect.max.x - chevron_strip_w, rect.min.y),
        rect.max,
    );

    // Sense clicks on the breakpoint and chevron strips so we can toggle.
    let bp_resp = ui.allocate_rect(bp_strip, Sense::click());
    let chev_resp = ui.allocate_rect(chevron_strip, Sense::click());

    let breakpoint_color = Color32::from_rgb(220, 80, 80);
    let chev_color = Color32::from_rgba_unmultiplied(muted.r(), muted.g(), muted.b(), 200);

    let painter = ui.painter();
    let font = FontId::new(font_size * 0.95, FontFamily::Monospace);
    for line in 0..line_count {
        let y_top = rect.min.y + line as f32 * row_height;
        let y_mid = y_top + row_height * 0.5;
        let is_current = current_line == Some(line);
        let color = if is_current { emph } else { muted };

        // Breakpoint dot (left of the line number)
        if breakpoints.contains(&line) {
            painter.circle_filled(
                Pos2::new(bp_strip.min.x + 7.0, y_mid),
                4.0,
                breakpoint_color,
            );
        }

        // Line number
        painter.text(
            Pos2::new(rect.max.x - chevron_strip_w - 2.0, y_mid),
            Align2::RIGHT_CENTER,
            format!("{}", line + 1),
            font.clone(),
            color,
        );

        // Fold chevron: visible if the line is currently foldable OR has
        // been folded (so the user can unfold even after the indent/brace
        // structure is gone from the visible buffer).
        let is_folded = folded_lines.contains(&line);
        if foldable_lines.contains(&line) || is_folded {
            let glyph = if is_folded { "▶" } else { "▼" };
            painter.text(
                Pos2::new(chevron_strip.min.x + 4.0, y_mid),
                Align2::LEFT_CENTER,
                glyph,
                FontId::new(font_size * 0.75, FontFamily::Monospace),
                chev_color,
            );
        }
    }

    let mut click = GutterClick::default();
    if bp_resp.clicked() {
        if let Some(pos) = bp_resp.interact_pointer_pos() {
            let line = ((pos.y - rect.min.y) / row_height).floor() as usize;
            if line < line_count {
                click.toggle_breakpoint = Some(line);
            }
        }
    }
    if chev_resp.clicked() {
        if let Some(pos) = chev_resp.interact_pointer_pos() {
            let line = ((pos.y - rect.min.y) / row_height).floor() as usize;
            if line < line_count
                && (foldable_lines.contains(&line) || folded_lines.contains(&line))
            {
                click.toggle_fold = Some(line);
            }
        }
    }
    if bp_resp.hovered() || chev_resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    click
}

#[derive(Default, Debug, Clone, Copy)]
struct GutterClick {
    toggle_breakpoint: Option<usize>,
    toggle_fold: Option<usize>,
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

/// Yellow translucent background covering each find-all match.
fn paint_find_match(
    ui: &egui::Ui,
    content: &str,
    editor_rect: Rect,
    row_height: f32,
    advance: f32,
    byte_start: usize,
    byte_end: usize,
    _theme: &Theme,
) {
    let line = byte_to_line(content, byte_start);
    let (ls, _) = line_byte_range(content, line);
    let col_chars = content[ls..byte_start.min(content.len())].chars().count();
    let width_chars = content[byte_start..byte_end.min(content.len())].chars().count();
    let x = editor_rect.min.x + col_chars as f32 * advance;
    let y = editor_rect.min.y + line as f32 * row_height;
    let r = Rect::from_min_size(
        Pos2::new(x, y),
        Vec2::new(width_chars as f32 * advance, row_height),
    );
    let fill = Color32::from_rgba_unmultiplied(240, 200, 80, 70);
    ui.painter().rect_filled(r, 1.0, fill);
}

/// Thin caret indicating a secondary cursor position.
fn paint_extra_cursor(
    ui: &egui::Ui,
    content: &str,
    editor_rect: Rect,
    row_height: f32,
    advance: f32,
    byte_idx: usize,
    theme: &Theme,
) {
    let line = byte_to_line(content, byte_idx);
    let (ls, _) = line_byte_range(content, line);
    let col_chars = content[ls..byte_idx.min(content.len())].chars().count();
    let x = editor_rect.min.x + col_chars as f32 * advance;
    let y = editor_rect.min.y + line as f32 * row_height;
    let accent = theme.semantic.accent.to_color32();
    let stroke = Stroke::new(2.0, accent);
    ui.painter()
        .line_segment([Pos2::new(x, y), Pos2::new(x, y + row_height)], stroke);
}

/// Stamp a "··· N lines folded" pill at the right of a fold-start row.
/// Lines are actually removed from the buffer, so we only need to flag that
/// there's hidden content behind this line.
fn paint_fold_badge(
    ui: &egui::Ui,
    editor_rect: Rect,
    row_height: f32,
    fold_start: usize,
    hidden_lines: usize,
    theme: &Theme,
) {
    let badge_y = editor_rect.min.y + fold_start as f32 * row_height + row_height * 0.5;
    let badge_text = format!("···  {} lines", hidden_lines);
    let muted = theme.text.muted.to_color32();
    let border = theme.widgets.border.to_color32();
    // Pill background + stroke.
    let text_x = editor_rect.max.x - 12.0;
    let width = badge_text.len() as f32 * 5.5 + 10.0;
    let pill = Rect::from_min_size(
        Pos2::new(text_x - width, badge_y - row_height * 0.4),
        Vec2::new(width, row_height * 0.8),
    );
    let bg = theme.surfaces.faint.to_color32();
    ui.painter().rect_filled(pill, 6.0, bg);
    ui.painter().rect_stroke(
        pill,
        6.0,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        Pos2::new(text_x - 5.0, badge_y),
        Align2::RIGHT_CENTER,
        badge_text,
        FontId::proportional(10.0),
        muted,
    );
}

/// Split any `LayoutJob` section that straddles `[byte_start, byte_end)`
/// and mark the in-range portion with `bg` as its `TextFormat::background`.
/// egui paints per-section backgrounds behind glyphs, which keeps text
/// readable over the selection highlight.
fn apply_selection_background(
    job: &mut egui::text::LayoutJob,
    byte_start: usize,
    byte_end: usize,
    bg: Color32,
) {
    if byte_end <= byte_start {
        return;
    }
    let old = std::mem::take(&mut job.sections);
    for section in old {
        let s = section.byte_range.start;
        let e = section.byte_range.end;
        if e <= byte_start || s >= byte_end {
            job.sections.push(section);
            continue;
        }
        // `leading_space` only belongs to the very first sub-section.
        let mut leading = section.leading_space;
        if s < byte_start {
            job.sections.push(egui::text::LayoutSection {
                leading_space: leading,
                byte_range: s..byte_start,
                format: section.format.clone(),
            });
            leading = 0.0;
        }
        let mid_start = s.max(byte_start);
        let mid_end = e.min(byte_end);
        let mut fmt = section.format.clone();
        fmt.background = bg;
        job.sections.push(egui::text::LayoutSection {
            leading_space: leading,
            byte_range: mid_start..mid_end,
            format: fmt,
        });
        if e > byte_end {
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: byte_end..e,
                format: section.format.clone(),
            });
        }
    }
}

/// Subtle red tint on a line that has a breakpoint set.
fn paint_breakpoint_line_tint(ui: &egui::Ui, editor_rect: Rect, row_height: f32, line: usize) {
    let y = editor_rect.min.y + line as f32 * row_height;
    let rect = Rect::from_min_size(
        Pos2::new(editor_rect.min.x, y),
        Vec2::new(editor_rect.width(), row_height),
    );
    let fill = Color32::from_rgba_unmultiplied(220, 80, 80, 18);
    ui.painter().rect_filled(rect, 0.0, fill);
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
    let bg = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 42);
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
    // Any shortcut that edits or moves the caret counts as user activity —
    // bump the blink-reset timestamp so the cursor flashes on.
    state.last_cursor_visible_reset = Some(ctx.input(|i| i.time));
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

// -------------------------------------------------------------------------
// Diff modal
// -------------------------------------------------------------------------

fn render_diff_modal(
    ui: &mut egui::Ui,
    state: &mut CodeEditorState,
    active_idx: usize,
    theme: &Theme,
) {
    let Some(active) = state.open_files.get(active_idx) else {
        state.diff_open = false;
        return;
    };
    let active_name = active.name.clone();
    let active_content = active.content.clone();

    // Right-side options: every other open tab + "saved on disk".
    let other_tabs: Vec<(usize, String)> = state
        .open_files
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != active_idx)
        .map(|(i, f)| (i, f.name.clone()))
        .collect();

    let (_right_label, right_content) = match state.diff_other_tab {
        Some(idx) if idx < state.open_files.len() && idx != active_idx => {
            let f = &state.open_files[idx];
            (format!("{} (tab)", f.name), f.content.clone())
        }
        _ => match std::fs::read_to_string(&active.path) {
            Ok(c) => (format!("{} (on disk)", active_name), c),
            Err(_) => (format!("{} (no disk version)", active_name), String::new()),
        },
    };

    let rows = crate::diff::diff_lines(&active_content, &right_content);
    let identical = crate::diff::is_identical(&rows);

    let mut open = true;
    let added = theme.semantic.success.to_color32();
    let removed = theme.semantic.error.to_color32();
    let muted = theme.text.muted.to_color32();
    let primary = theme.text.primary.to_color32();
    let panel_bg = theme.surfaces.panel.to_color32();

    egui::Window::new("Diff")
        .collapsible(false)
        .resizable(true)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .default_size([900.0, 600.0])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Left: {}", active_name)).color(muted));
                ui.separator();
                ui.label(RichText::new("Right:").color(muted));
                let current_label = match state.diff_other_tab {
                    Some(idx) => state
                        .open_files
                        .get(idx)
                        .map(|f| f.name.clone())
                        .unwrap_or_else(|| "saved on disk".to_string()),
                    None => "saved on disk".to_string(),
                };
                egui::ComboBox::from_id_salt("diff_right_picker")
                    .selected_text(current_label)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.diff_other_tab, None, "saved on disk");
                        for (i, name) in &other_tabs {
                            ui.selectable_value(
                                &mut state.diff_other_tab,
                                Some(*i),
                                name.as_str(),
                            );
                        }
                    });
            });

            ui.add_space(4.0);
            if identical {
                ui.label(
                    RichText::new("Files are identical")
                        .color(muted)
                        .italics(),
                );
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let row_h = 18.0;
                    let half_w = (ui.available_width() - 8.0) * 0.5;
                    let font = FontId::monospace(11.0);

                    for row in &rows {
                        let (rect, _resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), row_h),
                            Sense::hover(),
                        );

                        let bg_left = match row.op {
                            crate::diff::DiffOp::Removed => {
                                Color32::from_rgba_unmultiplied(removed.r(), removed.g(), removed.b(), 40)
                            }
                            _ => panel_bg,
                        };
                        let bg_right = match row.op {
                            crate::diff::DiffOp::Added => {
                                Color32::from_rgba_unmultiplied(added.r(), added.g(), added.b(), 40)
                            }
                            _ => panel_bg,
                        };

                        let left_rect = Rect::from_min_size(
                            rect.min,
                            Vec2::new(half_w, row_h),
                        );
                        let right_rect = Rect::from_min_size(
                            Pos2::new(rect.min.x + half_w + 8.0, rect.min.y),
                            Vec2::new(half_w, row_h),
                        );
                        ui.painter().rect_filled(left_rect, 0.0, bg_left);
                        ui.painter().rect_filled(right_rect, 0.0, bg_right);

                        // Line numbers
                        if let Some(n) = row.left_lineno {
                            ui.painter().text(
                                Pos2::new(left_rect.min.x + 30.0, rect.center().y),
                                Align2::RIGHT_CENTER,
                                format!("{}", n),
                                font.clone(),
                                muted,
                            );
                        }
                        if let Some(n) = row.right_lineno {
                            ui.painter().text(
                                Pos2::new(right_rect.min.x + 30.0, rect.center().y),
                                Align2::RIGHT_CENTER,
                                format!("{}", n),
                                font.clone(),
                                muted,
                            );
                        }

                        // Marker
                        let mark_left = match row.op {
                            crate::diff::DiffOp::Removed => "-",
                            crate::diff::DiffOp::Same => " ",
                            _ => " ",
                        };
                        let mark_right = match row.op {
                            crate::diff::DiffOp::Added => "+",
                            crate::diff::DiffOp::Same => " ",
                            _ => " ",
                        };
                        ui.painter().text(
                            Pos2::new(left_rect.min.x + 40.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            mark_left,
                            font.clone(),
                            removed,
                        );
                        ui.painter().text(
                            Pos2::new(right_rect.min.x + 40.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            mark_right,
                            font.clone(),
                            added,
                        );

                        if let Some(t) = &row.left {
                            ui.painter().text(
                                Pos2::new(left_rect.min.x + 52.0, rect.center().y),
                                Align2::LEFT_CENTER,
                                truncate_for_diff(t, half_w - 60.0),
                                font.clone(),
                                primary,
                            );
                        }
                        if let Some(t) = &row.right {
                            ui.painter().text(
                                Pos2::new(right_rect.min.x + 52.0, rect.center().y),
                                Align2::LEFT_CENTER,
                                truncate_for_diff(t, half_w - 60.0),
                                font.clone(),
                                primary,
                            );
                        }
                    }
                });
        });

    if !open {
        state.diff_open = false;
    }
}

fn truncate_for_diff(s: &str, max_width_px: f32) -> String {
    let max_chars = (max_width_px / 7.0) as usize;
    if s.chars().count() > max_chars {
        let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    } else {
        s.to_string()
    }
}

//! Console panel rendering — toolbar, log entries, and input bar.

use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, Key, RichText, ScrollArea};
use renzora_theme::Theme;

use egui_phosphor::regular::{
    ARROW_ELBOW_DOWN_LEFT, CARET_RIGHT, CHECK_CIRCLE, CLIPBOARD, FUNNEL, INFO,
    MAGNIFYING_GLASS, TRASH, WARNING, X_CIRCLE,
};

use crate::state::{ConsoleState, LogEntry, LogLevel};

/// A grouped log entry that combines consecutive identical messages.
struct GroupedLogEntry<'a> {
    entry: &'a LogEntry,
    count: usize,
}

/// Render the console content.
pub fn render_console_content(ui: &mut egui::Ui, console: &mut ConsoleState, theme: &Theme) {
    let muted_color = theme.text.muted.to_color32();
    let disabled_color = theme.text.disabled.to_color32();

    let info_active = theme.semantic.accent.to_color32();
    let success_active = theme.semantic.success.to_color32();
    let warning_active = theme.semantic.warning.to_color32();
    let error_active = theme.semantic.error.to_color32();

    let available_width = ui.available_width();
    let is_narrow = available_width < 500.0;

    // --- Toolbar ---
    ui.add_space(4.0);
    render_toolbar(
        ui,
        console,
        theme,
        muted_color,
        disabled_color,
        info_active,
        success_active,
        warning_active,
        error_active,
        is_narrow,
    );
    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    // --- Log entries ---
    let filtered_entries: Vec<_> = console.filtered_entries().collect();
    let grouped_entries = group_consecutive_entries(&filtered_entries);

    let text_color = theme.text.primary.to_color32();
    let category_color = theme.text.hyperlink.to_color32();

    let available = ui.available_height() - 42.0;

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(available.max(20.0))
        .stick_to_bottom(console.auto_scroll)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            for grouped in &grouped_entries {
                render_log_entry(
                    ui,
                    grouped.entry,
                    grouped.count,
                    text_color,
                    category_color,
                    theme,
                    is_narrow,
                );
            }

            if grouped_entries.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("No log entries")
                            .size(13.0)
                            .color(muted_color),
                    );
                });
            }
        });

    // --- Input bar ---
    ui.separator();
    render_input_bar(ui, console, theme);
}

/// Render the toolbar.
fn render_toolbar(
    ui: &mut egui::Ui,
    console: &mut ConsoleState,
    _theme: &Theme,
    muted_color: Color32,
    disabled_color: Color32,
    info_active: Color32,
    success_active: Color32,
    warning_active: Color32,
    error_active: Color32,
    is_narrow: bool,
) {
    ui.horizontal(|ui| {
        // Clear button
        let clear_label = if is_narrow {
            TRASH.to_string()
        } else {
            format!("{} Clear", TRASH)
        };
        let clear_btn = ui.button(RichText::new(clear_label).size(12.0));
        if clear_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if clear_btn.on_hover_text("Clear").clicked() {
            console.clear();
        }

        // Copy button
        if !is_narrow {
            let copy_btn = ui.button(RichText::new(format!("{} Copy", CLIPBOARD)).size(12.0));
            if copy_btn.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if copy_btn
                .on_hover_text("Copy filtered logs to clipboard")
                .clicked()
            {
                copy_filtered_logs(ui, console);
            }
        } else {
            let copy_btn = ui.button(RichText::new(CLIPBOARD).size(12.0));
            if copy_btn.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if copy_btn
                .on_hover_text("Copy filtered logs to clipboard")
                .clicked()
            {
                copy_filtered_logs(ui, console);
            }
        }

        ui.separator();

        // Filter toggles
        let filters: &[(bool, &str, Color32)] = &[
            (console.show_info, INFO, info_active),
            (console.show_success, CHECK_CIRCLE, success_active),
            (console.show_warnings, WARNING, warning_active),
            (console.show_errors, X_CIRCLE, error_active),
        ];

        let toggles: Vec<bool> = filters
            .iter()
            .map(|(active, icon, active_color)| {
                let color = if *active { *active_color } else { disabled_color };
                let btn = ui.add(
                    egui::Button::new(RichText::new(*icon).color(color).size(14.0))
                        .fill(Color32::TRANSPARENT),
                );
                if btn.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                btn.clicked()
            })
            .collect();

        if toggles[0] {
            console.show_info = !console.show_info;
        }
        if toggles[1] {
            console.show_success = !console.show_success;
        }
        if toggles[2] {
            console.show_warnings = !console.show_warnings;
        }
        if toggles[3] {
            console.show_errors = !console.show_errors;
        }

        ui.separator();

        // Search box
        let search_width = if is_narrow { 80.0 } else { 150.0 };
        ui.add_space(4.0);
        ui.label(RichText::new(MAGNIFYING_GLASS).size(12.0).color(muted_color));
        ui.add(
            egui::TextEdit::singleline(&mut console.search_filter)
                .hint_text("Search...")
                .desired_width(search_width),
        );

        // Category filter
        if !is_narrow {
            ui.add_space(8.0);
            ui.label(RichText::new(FUNNEL).size(12.0).color(muted_color));
            ui.add(
                egui::TextEdit::singleline(&mut console.category_filter)
                    .hint_text("Category...")
                    .desired_width(100.0),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !is_narrow {
                ui.checkbox(&mut console.auto_scroll, "Auto-scroll");
            } else {
                ui.checkbox(&mut console.auto_scroll, "");
            }

            let total = console.entries.len();
            let filtered: Vec<_> = console.filtered_entries().collect();
            ui.label(
                RichText::new(format!("{}/{}", filtered.len(), total))
                    .size(11.0)
                    .color(muted_color),
            );
        });
    });
}

fn copy_filtered_logs(ui: &mut egui::Ui, console: &ConsoleState) {
    let filtered: Vec<_> = console.filtered_entries().collect();
    let text = filtered
        .iter()
        .map(|e| {
            let level = match e.level {
                LogLevel::Info => "INFO",
                LogLevel::Success => "SUCCESS",
                LogLevel::Warning => "WARNING",
                LogLevel::Error => "ERROR",
            };
            if e.category.is_empty() {
                format!("[{}] {}", level, e.message)
            } else {
                format!("[{}] [{}] {}", level, e.category, e.message)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    ui.ctx().copy_text(text);
}

/// Render the input bar at the bottom of the console.
fn render_input_bar(ui: &mut egui::Ui, console: &mut ConsoleState, theme: &Theme) {
    let accent_color = theme.semantic.accent.to_color32();
    let muted_color = theme.text.muted.to_color32();
    let text_color = theme.text.primary.to_color32();

    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.add_space(6.0);

        // Prompt chevron
        ui.label(RichText::new(CARET_RIGHT).size(14.0).color(accent_color));

        // Input field
        let input_id = ui.id().with("console_input");
        let response = ui.add(
            egui::TextEdit::singleline(&mut console.input_buffer)
                .hint_text("Type /help for commands...")
                .desired_width(ui.available_width() - 24.0)
                .font(egui::TextStyle::Monospace)
                .text_color(text_color)
                .frame(false)
                .id(input_id),
        );

        // Focus on first render or when requested
        if console.focus_input {
            response.request_focus();
            console.focus_input = false;
        }

        // Submit hint icon
        ui.label(RichText::new(ARROW_ELBOW_DOWN_LEFT).size(12.0).color(muted_color));

        // Handle keyboard
        let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let has_focus = response.has_focus();

        if has_focus || submitted {
            let (up, down) = ui.input(|i| {
                (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown))
            });

            if submitted && !console.input_buffer.trim().is_empty() {
                let command = console.input_buffer.trim().to_string();

                // Log the command as input
                console.log(LogLevel::Info, "Input", format!("> {}", command));

                // Try console command (starts with /)
                if command.starts_with('/') {
                    execute_command(&command, console);
                } else {
                    // Just log non-command input as Info
                    console.log(LogLevel::Info, "Input", &command);
                }

                // Push to history
                console.command_history.push(command);
                console.history_index = None;
                console.saved_input.clear();
                console.input_buffer.clear();
                console.auto_scroll = true;

                // Re-focus input
                console.focus_input = true;
            }

            // History navigation: Up
            if up && !console.command_history.is_empty() {
                match console.history_index {
                    None => {
                        console.saved_input = console.input_buffer.clone();
                        let idx = console.command_history.len() - 1;
                        console.history_index = Some(idx);
                        console.input_buffer = console.command_history[idx].clone();
                    }
                    Some(idx) if idx > 0 => {
                        let new_idx = idx - 1;
                        console.history_index = Some(new_idx);
                        console.input_buffer = console.command_history[new_idx].clone();
                    }
                    _ => {}
                }
            }

            // History navigation: Down
            if down {
                if let Some(idx) = console.history_index {
                    if idx + 1 < console.command_history.len() {
                        let new_idx = idx + 1;
                        console.history_index = Some(new_idx);
                        console.input_buffer = console.command_history[new_idx].clone();
                    } else {
                        console.history_index = None;
                        console.input_buffer = console.saved_input.clone();
                        console.saved_input.clear();
                    }
                }
            }
        }
    });
}

// ── Console Commands ───────────────────────────────────────────────

struct CommandDef {
    name: &'static str,
    usage: &'static str,
    description: &'static str,
}

const COMMANDS: &[CommandDef] = &[
    CommandDef {
        name: "clear",
        usage: "/clear",
        description: "Clear console output",
    },
    CommandDef {
        name: "help",
        usage: "/help [command]",
        description: "List all commands, or show help for a specific command",
    },
];

/// Execute a console command.
fn execute_command(input: &str, console: &mut ConsoleState) {
    let trimmed = input.strip_prefix('/').unwrap_or(input);
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    let cmd = parts[0];
    let args = &parts[1..];

    let result: Result<String, String> = match cmd {
        "clear" => {
            console.clear();
            Ok("Console cleared.".to_string())
        }

        "help" => {
            if let Some(name) = args.first() {
                if let Some(def) = COMMANDS.iter().find(|c| c.name == *name) {
                    Ok(format!("{}\n  {}", def.usage, def.description))
                } else {
                    Err(format!("Unknown command: /{}", name))
                }
            } else {
                let mut help = String::from("Available commands:\n");
                for def in COMMANDS {
                    help.push_str(&format!("  {:30} {}\n", def.usage, def.description));
                }
                Ok(help)
            }
        }

        _ => Err(format!(
            "Unknown command: /{}. Type /help for a list of commands.",
            cmd
        )),
    };

    match result {
        Ok(msg) => console.log(LogLevel::Success, "Command", msg),
        Err(msg) => console.log(LogLevel::Error, "Command", msg),
    }
}

/// Group consecutive identical log entries.
fn group_consecutive_entries<'a>(entries: &[&'a LogEntry]) -> Vec<GroupedLogEntry<'a>> {
    let mut grouped = Vec::new();

    for entry in entries {
        let should_group = grouped.last().map_or(false, |last: &GroupedLogEntry| {
            last.entry.level == entry.level
                && last.entry.category == entry.category
                && last.entry.message == entry.message
        });

        if should_group {
            if let Some(last) = grouped.last_mut() {
                last.count += 1;
            }
        } else {
            grouped.push(GroupedLogEntry { entry, count: 1 });
        }
    }

    grouped
}

fn render_log_entry(
    ui: &mut egui::Ui,
    entry: &LogEntry,
    count: usize,
    text_color: Color32,
    category_color: Color32,
    theme: &Theme,
    is_narrow: bool,
) {
    let color = match entry.level {
        LogLevel::Info => theme.semantic.accent.to_color32(),
        LogLevel::Success => theme.semantic.success.to_color32(),
        LogLevel::Warning => theme.semantic.warning.to_color32(),
        LogLevel::Error => theme.semantic.error.to_color32(),
    };

    ui.horizontal(|ui| {
        // Count badge
        if count > 1 {
            let badge_text = if count > 999 {
                "999+".to_string()
            } else {
                count.to_string()
            };

            let badge_color = color.gamma_multiply(0.3);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 16.0), egui::Sense::hover());

            ui.painter()
                .rect_filled(rect, CornerRadius::same(8), badge_color);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &badge_text,
                egui::FontId::proportional(10.0),
                color,
            );

            ui.add_space(2.0);
        }

        // Level icon
        let icon = match entry.level {
            LogLevel::Info => INFO,
            LogLevel::Success => CHECK_CIRCLE,
            LogLevel::Warning => WARNING,
            LogLevel::Error => X_CIRCLE,
        };
        ui.label(RichText::new(icon).color(color).size(12.0));

        // Category badge
        if !entry.category.is_empty() && !is_narrow {
            ui.label(
                RichText::new(format!("[{}]", entry.category))
                    .size(11.0)
                    .color(category_color),
            );
        }

        // Message
        let is_repl = entry.category == "Input" || entry.category == "Output";
        if is_repl {
            ui.label(
                RichText::new(&entry.message)
                    .size(12.0)
                    .color(text_color)
                    .monospace(),
            );
        } else {
            ui.label(RichText::new(&entry.message).size(12.0).color(text_color));
        }
    });
}

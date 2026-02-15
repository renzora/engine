//! Console panel for displaying logs

use bevy_egui::egui::{self, Color32, CursorIcon, RichText, ScrollArea, Rounding};

use crate::core::{ConsoleState, LogEntry, LogLevel};
use renzora_theme::Theme;

use egui_phosphor::regular::{
    TRASH, FUNNEL, INFO, CHECK_CIRCLE, WARNING, X_CIRCLE, MAGNIFYING_GLASS, CLIPBOARD,
};

/// A grouped log entry that combines consecutive identical messages
struct GroupedLogEntry<'a> {
    entry: &'a LogEntry,
    count: usize,
}

/// Render the console content
pub fn render_console_content(ui: &mut egui::Ui, console: &mut ConsoleState, theme: &Theme) {
    // Get colors from theme
    let muted_color = theme.text.muted.to_color32();
    let disabled_color = theme.text.disabled.to_color32();

    // Semantic colors from theme
    let info_active = theme.semantic.accent.to_color32();
    let success_active = theme.semantic.success.to_color32();
    let warning_active = theme.semantic.warning.to_color32();
    let error_active = theme.semantic.error.to_color32();

    // Toolbar
    ui.horizontal(|ui| {
        // Clear button
        let clear_btn = ui.button(RichText::new(format!("{} Clear", TRASH)).size(12.0));
        if clear_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if clear_btn.clicked() {
            console.clear();
        }

        // Copy to clipboard button
        let copy_btn = ui.button(RichText::new(format!("{} Copy", CLIPBOARD)).size(12.0));
        if copy_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if copy_btn.on_hover_text("Copy filtered logs to clipboard").clicked() {
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

        ui.separator();

        // Filter toggles
        let info_color = if console.show_info {
            info_active
        } else {
            disabled_color
        };
        let info_btn = ui.add(egui::Button::new(
            RichText::new(INFO).color(info_color).size(14.0)
        ).fill(Color32::TRANSPARENT));
        if info_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if info_btn.clicked() {
            console.show_info = !console.show_info;
        }

        let success_color = if console.show_success {
            success_active
        } else {
            disabled_color
        };
        let success_btn = ui.add(egui::Button::new(
            RichText::new(CHECK_CIRCLE).color(success_color).size(14.0)
        ).fill(Color32::TRANSPARENT));
        if success_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if success_btn.clicked() {
            console.show_success = !console.show_success;
        }

        let warning_color = if console.show_warnings {
            warning_active
        } else {
            disabled_color
        };
        let warning_btn = ui.add(egui::Button::new(
            RichText::new(WARNING).color(warning_color).size(14.0)
        ).fill(Color32::TRANSPARENT));
        if warning_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if warning_btn.clicked() {
            console.show_warnings = !console.show_warnings;
        }

        let error_color = if console.show_errors {
            error_active
        } else {
            disabled_color
        };
        let error_btn = ui.add(egui::Button::new(
            RichText::new(X_CIRCLE).color(error_color).size(14.0)
        ).fill(Color32::TRANSPARENT));
        if error_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if error_btn.clicked() {
            console.show_errors = !console.show_errors;
        }

        ui.separator();

        // Search box
        ui.add_space(4.0);
        ui.label(RichText::new(MAGNIFYING_GLASS).size(12.0).color(muted_color));
        ui.add(
            egui::TextEdit::singleline(&mut console.search_filter)
                .hint_text("Search...")
                .desired_width(150.0)
        );

        // Category filter
        ui.add_space(8.0);
        ui.label(RichText::new(FUNNEL).size(12.0).color(muted_color));
        ui.add(
            egui::TextEdit::singleline(&mut console.category_filter)
                .hint_text("Category...")
                .desired_width(100.0)
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Auto-scroll toggle
            ui.checkbox(&mut console.auto_scroll, "Auto-scroll");

            // Entry count
            let total = console.entries.len();
            let filtered: Vec<_> = console.filtered_entries().collect();
            ui.label(
                RichText::new(format!("{}/{} entries", filtered.len(), total))
                    .size(11.0)
                    .color(muted_color)
            );
        });
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(2.0);

    // Log entries - group consecutive identical messages
    let filtered_entries: Vec<_> = console.filtered_entries().collect();
    let grouped_entries = group_consecutive_entries(&filtered_entries);

    let scroll_area = ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(console.auto_scroll);

    let text_color = theme.text.primary.to_color32();
    let category_color = theme.text.hyperlink.to_color32();
    scroll_area.show(ui, |ui| {
        ui.set_min_width(ui.available_width());

        for grouped in &grouped_entries {
            render_log_entry(ui, grouped.entry, grouped.count, text_color, category_color, theme);
        }

        // Empty state
        if grouped_entries.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No log entries")
                        .size(13.0)
                        .color(muted_color)
                );
            });
        }
    });
}

/// Group consecutive identical log entries
fn group_consecutive_entries<'a>(entries: &[&'a LogEntry]) -> Vec<GroupedLogEntry<'a>> {
    let mut grouped = Vec::new();

    for entry in entries {
        // Check if this entry matches the previous one
        let should_group = grouped.last().map_or(false, |last: &GroupedLogEntry| {
            last.entry.level == entry.level
                && last.entry.category == entry.category
                && last.entry.message == entry.message
        });

        if should_group {
            // Increment count of the last grouped entry
            if let Some(last) = grouped.last_mut() {
                last.count += 1;
            }
        } else {
            // Start a new group
            grouped.push(GroupedLogEntry {
                entry,
                count: 1,
            });
        }
    }

    grouped
}

fn render_log_entry(ui: &mut egui::Ui, entry: &LogEntry, count: usize, text_color: Color32, category_color: Color32, theme: &Theme) {
    // Get log level color from theme
    let color = match entry.level {
        LogLevel::Info => theme.semantic.accent.to_color32(),
        LogLevel::Success => theme.semantic.success.to_color32(),
        LogLevel::Warning => theme.semantic.warning.to_color32(),
        LogLevel::Error => theme.semantic.error.to_color32(),
    };

    ui.horizontal(|ui| {
        // Count badge (shown when count > 1)
        if count > 1 {
            let badge_text = if count > 999 {
                "999+".to_string()
            } else {
                count.to_string()
            };

            // Draw badge background
            let badge_color = color.gamma_multiply(0.3);
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(24.0, 16.0),
                egui::Sense::hover()
            );

            ui.painter().rect_filled(
                rect,
                Rounding::same(8),
                badge_color
            );

            // Draw badge text centered
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &badge_text,
                egui::FontId::proportional(10.0),
                color
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
        if !entry.category.is_empty() {
            ui.label(
                RichText::new(format!("[{}]", entry.category))
                    .size(11.0)
                    .color(category_color)
            );
        }

        // Message
        ui.label(
            RichText::new(&entry.message)
                .size(12.0)
                .color(text_color)
        );
    });
}

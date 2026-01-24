//! Console panel for displaying logs

use bevy_egui::egui::{self, Color32, RichText, ScrollArea, Vec2};

use crate::core::{ConsoleState, LogLevel};

use egui_phosphor::regular::{
    TRASH, FUNNEL, INFO, CHECK_CIRCLE, WARNING, X_CIRCLE, MAGNIFYING_GLASS,
};

/// Render the console content
pub fn render_console_content(ui: &mut egui::Ui, console: &mut ConsoleState) {
    // Toolbar
    ui.horizontal(|ui| {
        // Clear button
        if ui.button(RichText::new(format!("{} Clear", TRASH)).size(12.0)).clicked() {
            console.clear();
        }

        ui.separator();

        // Filter toggles
        let info_color = if console.show_info {
            Color32::from_rgb(140, 180, 220)
        } else {
            Color32::from_rgb(80, 80, 90)
        };
        if ui.add(egui::Button::new(
            RichText::new(INFO).color(info_color).size(14.0)
        ).fill(Color32::TRANSPARENT)).clicked() {
            console.show_info = !console.show_info;
        }

        let success_color = if console.show_success {
            Color32::from_rgb(100, 200, 120)
        } else {
            Color32::from_rgb(80, 80, 90)
        };
        if ui.add(egui::Button::new(
            RichText::new(CHECK_CIRCLE).color(success_color).size(14.0)
        ).fill(Color32::TRANSPARENT)).clicked() {
            console.show_success = !console.show_success;
        }

        let warning_color = if console.show_warnings {
            Color32::from_rgb(230, 180, 80)
        } else {
            Color32::from_rgb(80, 80, 90)
        };
        if ui.add(egui::Button::new(
            RichText::new(WARNING).color(warning_color).size(14.0)
        ).fill(Color32::TRANSPARENT)).clicked() {
            console.show_warnings = !console.show_warnings;
        }

        let error_color = if console.show_errors {
            Color32::from_rgb(220, 80, 80)
        } else {
            Color32::from_rgb(80, 80, 90)
        };
        if ui.add(egui::Button::new(
            RichText::new(X_CIRCLE).color(error_color).size(14.0)
        ).fill(Color32::TRANSPARENT)).clicked() {
            console.show_errors = !console.show_errors;
        }

        ui.separator();

        // Search box
        ui.add_space(4.0);
        ui.label(RichText::new(MAGNIFYING_GLASS).size(12.0).color(Color32::from_rgb(120, 120, 130)));
        ui.add(
            egui::TextEdit::singleline(&mut console.search_filter)
                .hint_text("Search...")
                .desired_width(150.0)
        );

        // Category filter
        ui.add_space(8.0);
        ui.label(RichText::new(FUNNEL).size(12.0).color(Color32::from_rgb(120, 120, 130)));
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
                    .color(Color32::from_rgb(120, 120, 130))
            );
        });
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(2.0);

    // Log entries
    let filtered_entries: Vec<_> = console.filtered_entries().cloned().collect();

    let scroll_area = ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(console.auto_scroll);

    scroll_area.show(ui, |ui| {
        ui.set_min_width(ui.available_width());

        for entry in &filtered_entries {
            render_log_entry(ui, entry);
        }

        // Empty state
        if filtered_entries.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No log entries")
                        .size(13.0)
                        .color(Color32::from_rgb(100, 100, 110))
                );
            });
        }
    });
}

fn render_log_entry(ui: &mut egui::Ui, entry: &crate::core::LogEntry) {
    let [r, g, b] = entry.level.color();
    let color = Color32::from_rgb(r, g, b);

    ui.horizontal(|ui| {
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
                    .color(Color32::from_rgb(100, 140, 180))
            );
        }

        // Message
        ui.label(
            RichText::new(&entry.message)
                .size(12.0)
                .color(Color32::from_rgb(200, 200, 210))
        );
    });
}

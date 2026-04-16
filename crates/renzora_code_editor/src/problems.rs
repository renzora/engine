//! Problems panel — aggregates `ScriptError`s across every open editor tab.
//! Click a row to switch to that file and jump to the error line.

use bevy::prelude::*;
use bevy_egui::egui::{self, CursorIcon, FontFamily, FontId, RichText, Sense, Vec2};
use egui_phosphor::regular::{CHECK_CIRCLE, WARNING};
use renzora_editor_framework::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::state::CodeEditorState;

pub struct ProblemsPanel;

impl EditorPanel for ProblemsPanel {
    fn id(&self) -> &str {
        "problems"
    }
    fn title(&self) -> &str {
        "Problems"
    }
    fn icon(&self) -> Option<&str> {
        Some(WARNING)
    }
    fn closable(&self) -> bool {
        true
    }
    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let muted = theme.text.muted.to_color32();
        let primary = theme.text.primary.to_color32();
        let secondary = theme.text.secondary.to_color32();
        let error_color = theme.semantic.error.to_color32();
        let success_color = theme.semantic.success.to_color32();

        let Some(state) = world.get_resource::<CodeEditorState>() else {
            return;
        };

        // Aggregate errors from every open file (file_idx, file_name, error_clone).
        let problems: Vec<(usize, String, crate::state::ScriptError)> = state
            .open_files
            .iter()
            .enumerate()
            .filter_map(|(idx, f)| f.error.as_ref().map(|e| (idx, f.name.clone(), e.clone())))
            .collect();

        if problems.is_empty() {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(CHECK_CIRCLE).size(20.0).color(success_color));
                ui.add_space(4.0);
                ui.label(RichText::new("No problems detected").size(11.0).color(muted));
            });
            return;
        }

        let cmds = world.get_resource::<EditorCommands>();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 0.0;
                for (file_idx, file_name, err) in problems {
                    let row_height = 32.0;
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        Sense::click(),
                    );

                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            theme.widgets.hovered_bg.to_color32(),
                        );
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    // Warning icon
                    ui.painter().text(
                        egui::Pos2::new(rect.min.x + 10.0, rect.center().y),
                        egui::Align2::CENTER_CENTER,
                        WARNING,
                        FontId::proportional(14.0),
                        error_color,
                    );

                    // Message (first line, truncated)
                    let msg_first_line = err.message.lines().next().unwrap_or("").to_string();
                    let max_chars = ((rect.width() - 220.0) / 7.0).max(20.0) as usize;
                    let msg_display = if msg_first_line.len() > max_chars {
                        format!("{}…", &msg_first_line[..max_chars.saturating_sub(1)])
                    } else {
                        msg_first_line
                    };
                    ui.painter().text(
                        egui::Pos2::new(rect.min.x + 30.0, rect.min.y + 6.0),
                        egui::Align2::LEFT_TOP,
                        &msg_display,
                        FontId::new(11.5, FontFamily::Monospace),
                        primary,
                    );

                    // File · line:col on second row
                    let location = match (err.line, err.column) {
                        (Some(line), Some(col)) => format!("{}:{}:{}", file_name, line, col),
                        (Some(line), None) => format!("{}:{}", file_name, line),
                        _ => file_name.clone(),
                    };
                    ui.painter().text(
                        egui::Pos2::new(rect.min.x + 30.0, rect.min.y + 22.0),
                        egui::Align2::LEFT_TOP,
                        &location,
                        FontId::proportional(10.0),
                        secondary,
                    );

                    if resp.clicked() {
                        let line_1based = err.line.unwrap_or(1).max(1);
                        if let Some(c) = cmds {
                            c.push(move |world: &mut World| {
                                if let Some(mut s) =
                                    world.get_resource_mut::<CodeEditorState>()
                                {
                                    s.active_tab = Some(file_idx);
                                    s.pending_goto_line = Some(line_1based);
                                }
                            });
                        }
                    }
                }
            });

        let _ = muted;
    }
}

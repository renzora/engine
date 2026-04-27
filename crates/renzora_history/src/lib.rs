//! History panel — view and jump through the undo/redo stack.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense};
use egui_phosphor::regular;
use renzora_editor::{
    empty_state, AppEditorExt, EditorCommands, EditorPanel, PanelLocation,
};
use renzora_theme::{Theme, ThemeManager};
use renzora_undo::UndoStacks;

enum Action {
    Undo(usize),
    Redo(usize),
}

pub struct HistoryPanel;

impl Default for HistoryPanel {
    fn default() -> Self {
        Self
    }
}

impl EditorPanel for HistoryPanel {
    fn id(&self) -> &str {
        "history"
    }

    fn title(&self) -> &str {
        "History"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::CLOCK_COUNTER_CLOCKWISE)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };

        let Some(stacks) = world.get_resource::<UndoStacks>() else {
            return;
        };
        let (undo, redo) = stacks.labels(&stacks.active);
        let cmds = world.get_resource::<EditorCommands>();

        if undo.is_empty() && redo.is_empty() {
            empty_state(
                ui,
                regular::CLOCK_COUNTER_CLOCKWISE,
                "No History",
                "Actions you perform will appear here.",
                &theme,
            );
            return;
        }

        let mut requested: Option<Action> = None;
        let n_undo = undo.len();

        egui::ScrollArea::vertical()
            .id_salt("history_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(4.0);

                section_header(ui, &theme, "Undo Stack");
                if n_undo <= 1 {
                    empty_section_hint(ui, &theme, "No earlier states.");
                } else {
                    // Render undo[0..n_undo-1] — exclude the most recent which IS the current state.
                    for (i, label) in undo.iter().take(n_undo - 1).enumerate() {
                        if history_row(
                            ui,
                            &theme,
                            regular::ARROW_BEND_UP_LEFT,
                            label,
                            RowKind::Past,
                        ) {
                            // Undo enough so this entry becomes the new current.
                            requested = Some(Action::Undo(n_undo - 1 - i));
                        }
                    }
                }

                section_header(ui, &theme, "Current State");
                let current_label = undo
                    .last()
                    .map(|s| s.as_str())
                    .unwrap_or("Initial state");
                history_row(
                    ui,
                    &theme,
                    regular::CARET_RIGHT,
                    current_label,
                    RowKind::Current,
                );

                section_header(ui, &theme, "Redo Stack");
                if redo.is_empty() {
                    empty_section_hint(ui, &theme, "Nothing to redo.");
                } else {
                    // Most-immediate redo first (back of deque).
                    for (i, label) in redo.iter().rev().enumerate() {
                        if history_row(
                            ui,
                            &theme,
                            regular::ARROW_BEND_UP_RIGHT,
                            label,
                            RowKind::Future,
                        ) {
                            requested = Some(Action::Redo(i + 1));
                        }
                    }
                }

                ui.add_space(8.0);
            });

        if let (Some(cmds), Some(action)) = (cmds, requested) {
            match action {
                Action::Undo(n) => {
                    cmds.push(move |world: &mut World| {
                        for _ in 0..n {
                            renzora_undo::undo_once(world);
                        }
                    });
                }
                Action::Redo(n) => {
                    cmds.push(move |world: &mut World| {
                        for _ in 0..n {
                            renzora_undo::redo_once(world);
                        }
                    });
                }
            }
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}

#[derive(Copy, Clone, PartialEq)]
enum RowKind {
    Past,
    Current,
    Future,
}

fn section_header(ui: &mut egui::Ui, theme: &Theme, label: &str) {
    let text_muted = theme.text.muted.to_color32();
    let header_bg = theme.panels.category_frame_bg.to_color32();
    egui::Frame::new()
        .fill(header_bg)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(label)
                        .size(11.0)
                        .strong()
                        .color(text_muted),
                );
            });
        });
}

fn empty_section_hint(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    let muted = theme.text.muted.to_color32();
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).italics().color(muted));
        });
}

/// Render a single history row. Returns true if it was clicked.
fn history_row(ui: &mut egui::Ui, theme: &Theme, icon: &str, label: &str, kind: RowKind) -> bool {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let highlight = theme.semantic.selection_stroke.to_color32();

    let (icon_color, label_color, is_current) = match kind {
        RowKind::Current => (highlight, text_primary, true),
        RowKind::Past => (text_muted, text_primary, false),
        RowKind::Future => (text_muted, text_muted, false),
    };

    let sense = if is_current { Sense::hover() } else { Sense::click() };
    let row_height = 22.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), sense);

    if is_current {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            Color32::from_rgba_premultiplied(
                highlight.r(),
                highlight.g(),
                highlight.b(),
                40,
            ),
        );
    } else if response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            Color32::from_rgba_premultiplied(255, 255, 255, 12),
        );
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let painter = ui.painter_at(rect);
    let mut x = rect.left() + 12.0;
    let y_center = rect.center().y;

    painter.text(
        egui::pos2(x, y_center),
        egui::Align2::LEFT_CENTER,
        icon,
        egui::FontId::proportional(12.0),
        icon_color,
    );
    x += 18.0;

    painter.text(
        egui::pos2(x, y_center),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        label_color,
    );

    response.clicked()
}

#[derive(Default)]
pub struct HistoryPanelPlugin;

impl Plugin for HistoryPanelPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] HistoryPanelPlugin");
        app.register_panel(HistoryPanel::default());
    }
}


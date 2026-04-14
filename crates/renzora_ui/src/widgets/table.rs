//! Sortable table with column headers.
//!
//! Caller supplies rows + a compare function. Clicking a column header cycles
//! the sort: ascending → descending → none. Persisted in egui memory by
//! widget id.

use bevy_egui::egui::{self, Color32, Sense, Stroke, Vec2};
use renzora_theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortOrder {
    None,
    Ascending,
    Descending,
}

impl SortOrder {
    fn cycle(self) -> Self {
        match self {
            SortOrder::None => SortOrder::Ascending,
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::None,
        }
    }
}

/// Persisted sort state: (column index, order).
#[derive(Clone, Copy, Debug, Default)]
pub struct TableSort {
    pub column: Option<usize>,
    pub order: SortOrder,
}

impl Default for SortOrder {
    fn default() -> Self { SortOrder::None }
}

/// Render a simple table. `headers` labels each column, `row_count` is the
/// total number of rows, and `cell` is called for each visible cell.
/// Returns the current `TableSort` so the caller can sort their data source.
pub fn table(
    ui: &mut egui::Ui,
    id: egui::Id,
    headers: &[&str],
    row_count: usize,
    row_height: f32,
    theme: &Theme,
    mut cell: impl FnMut(&mut egui::Ui, usize, usize),
) -> TableSort {
    let mut sort: TableSort = ui
        .memory(|m| m.data.get_temp::<TableSort>(id))
        .unwrap_or_default();

    let col_count = headers.len().max(1);
    let col_w = ui.available_width() / col_count as f32;

    // Header row
    ui.horizontal(|ui| {
        for (i, h) in headers.iter().enumerate() {
            let (cell_rect, resp) = ui.allocate_exact_size(Vec2::new(col_w, 20.0), Sense::click());
            ui.painter().rect_filled(cell_rect, 0.0, theme.surfaces.faint.to_color32());
            let label = if sort.column == Some(i) {
                match sort.order {
                    SortOrder::Ascending => format!("{h} ▲"),
                    SortOrder::Descending => format!("{h} ▼"),
                    SortOrder::None => h.to_string(),
                }
            } else {
                h.to_string()
            };
            ui.painter().text(
                cell_rect.left_center() + Vec2::new(4.0, 0.0),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(11.0),
                theme.text.primary.to_color32(),
            );
            if resp.clicked() {
                if sort.column == Some(i) {
                    sort.order = sort.order.cycle();
                    if sort.order == SortOrder::None {
                        sort.column = None;
                    }
                } else {
                    sort.column = Some(i);
                    sort.order = SortOrder::Ascending;
                }
            }
        }
    });
    ui.painter().line_segment(
        [
            ui.min_rect().left_bottom(),
            ui.min_rect().right_bottom(),
        ],
        Stroke::new(1.0, theme.widgets.border.to_color32()),
    );

    // Rows
    egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show_rows(
        ui,
        row_height,
        row_count,
        |ui, range| {
            for row in range {
                let bg = if row % 2 == 0 {
                    theme.panels.inspector_row_even.to_color32()
                } else {
                    theme.panels.inspector_row_odd.to_color32()
                };
                let (row_rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), Sense::hover());
                ui.painter().rect_filled(row_rect, 0.0, bg);
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(row_rect).layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        for col in 0..col_count {
                            let cell_rect = egui::Rect::from_min_size(
                                row_rect.min + Vec2::new(col as f32 * col_w, 0.0),
                                Vec2::new(col_w, row_height),
                            );
                            ui.scope_builder(
                                egui::UiBuilder::new().max_rect(cell_rect).layout(
                                    egui::Layout::left_to_right(egui::Align::Center),
                                ),
                                |ui| cell(ui, row, col),
                            );
                        }
                    },
                );
            }
        },
    );

    ui.memory_mut(|m| m.data.insert_temp(id, sort));
    sort
}

// Silence the unused warning for Color32 in debug builds of some configs.
const _: Color32 = Color32::TRANSPARENT;

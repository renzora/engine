//! Virtual list — renders only the visible slice of a large item set.
//!
//! Ideal for hierarchies or asset folders with thousands of rows; egui's
//! default linear layout would allocate and measure every row otherwise.

use bevy_egui::egui;

/// Render a virtualized list with a uniform row height. `add_row` is called
/// only for rows in the visible viewport plus a small overscan buffer.
pub fn virtual_list(
    ui: &mut egui::Ui,
    item_count: usize,
    row_height: f32,
    mut add_row: impl FnMut(&mut egui::Ui, usize),
) {
    let total_h = item_count as f32 * row_height;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
        ui.set_height(total_h);
        let first_visible = (viewport.min.y / row_height).floor().max(0.0) as usize;
        let last_visible =
            ((viewport.max.y / row_height).ceil() as usize + 1).min(item_count);
        let first = first_visible.saturating_sub(4);
        let last = (last_visible + 4).min(item_count);

        let offset_y = first as f32 * row_height;
        ui.allocate_space(egui::vec2(1.0, offset_y));
        for i in first..last {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), row_height),
                egui::Sense::hover(),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            add_row(&mut child, i);
        }
        let remaining = (item_count - last) as f32 * row_height;
        if remaining > 0.0 {
            ui.allocate_space(egui::vec2(1.0, remaining));
        }
    });
}

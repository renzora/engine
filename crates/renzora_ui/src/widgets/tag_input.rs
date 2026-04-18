//! Tag input — chip list + free-text field. Type a tag and press Enter.

use bevy_egui::egui::{self, Color32, Sense, Stroke};
use renzora_theme::Theme;

/// Render a tag input. Mutates the tag list directly. Returns `true` if a
/// tag was added or removed this frame.
pub fn tag_input(
    ui: &mut egui::Ui,
    id: egui::Id,
    tags: &mut Vec<String>,
    hint: &str,
    theme: &Theme,
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let mut remove_idx: Option<usize> = None;
        for (i, tag) in tags.iter().enumerate() {
            let font = egui::FontId::proportional(11.0);
            let text_w = ui.fonts_mut(|f| {
                f.layout_no_wrap(tag.clone(), font.clone(), Color32::WHITE).rect.width()
            });
            let (rect, resp) = ui.allocate_exact_size(
                egui::Vec2::new(text_w + 28.0, 20.0),
                Sense::click(),
            );
            ui.painter().rect_filled(rect, 10.0, theme.widgets.active_bg.to_color32().gamma_multiply(0.6));
            ui.painter().text(
                egui::pos2(rect.min.x + 8.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                tag,
                font,
                Color32::WHITE,
            );
            ui.painter().text(
                egui::pos2(rect.max.x - 8.0, rect.center().y),
                egui::Align2::CENTER_CENTER,
                "×",
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );
            if resp.clicked() {
                remove_idx = Some(i);
            }
        }
        if let Some(i) = remove_idx {
            tags.remove(i);
            changed = true;
        }

        // Free-text input
        let buf_id = id.with("buf");
        let mut buf = ui
            .ctx()
            .memory_mut(|m| m.data.get_temp::<String>(buf_id).unwrap_or_default());
        let resp = ui.add(
            egui::TextEdit::singleline(&mut buf)
                .hint_text(hint)
                .desired_width(120.0),
        );
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if resp.lost_focus() && enter && !buf.trim().is_empty() {
            tags.push(buf.trim().to_string());
            buf.clear();
            changed = true;
        }
        ui.ctx().memory_mut(|m| m.data.insert_temp(buf_id, buf));

        let _ = Stroke::NONE;
    });
    changed
}

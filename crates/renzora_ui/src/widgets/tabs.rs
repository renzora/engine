//! In-panel tab bar (distinct from `dock_tree` window tabs).

use bevy_egui::egui::{self, Sense, Stroke, Vec2};
use renzora_theme::Theme;

pub struct TabDef<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub icon: Option<&'a str>,
}

/// Render a horizontal tab bar. Returns the clicked tab id when the user
/// changes selection; otherwise returns `None`. `selected` is the currently
/// active id.
pub fn tab_bar(
    ui: &mut egui::Ui,
    tabs: &[TabDef<'_>],
    selected: &str,
    theme: &Theme,
) -> Option<String> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for t in tabs {
            let is_active = t.id == selected;
            let label = if let Some(i) = t.icon {
                format!("{i}  {}", t.label)
            } else {
                t.label.to_string()
            };
            let (rect, resp) = ui.allocate_exact_size(
                Vec2::new(ui.fonts_mut(|f| f.layout_no_wrap(label.clone(), egui::FontId::proportional(12.0), theme.text.primary.to_color32()).rect.width()) + 20.0, 26.0),
                Sense::click(),
            );
            let bg = if is_active {
                theme.surfaces.panel.to_color32()
            } else if resp.hovered() {
                theme.surfaces.faint.to_color32()
            } else {
                theme.panels.inspector_row_even.to_color32()
            };
            ui.painter().rect_filled(rect, 0.0, bg);
            let text_color = if is_active {
                theme.widgets.active_bg.to_color32()
            } else {
                theme.text.primary.to_color32()
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(12.0),
                text_color,
            );
            if is_active {
                ui.painter().line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(2.0, theme.widgets.active_bg.to_color32()),
                );
            }
            if resp.clicked() {
                picked = Some(t.id.to_string());
            }
        }
    });
    picked
}

//! Top title/menu bar with centered layout tabs.

use bevy_egui::egui::{self, Color32, CursorIcon, Pos2, Rect, Sense, Vec2};
use renzora_theme::Theme;

use crate::layouts::LayoutManager;
use crate::panel::PanelRegistry;

/// Actions returned from the title bar that the caller should handle.
pub enum TitleBarAction {
    None,
    SwitchLayout(usize),
    NewProject,
    OpenProject,
    NewScene,
    OpenScene,
    Save,
    SaveAs,
    Export,
}

const TITLE_BAR_HEIGHT: f32 = 28.0;
const TAB_PADDING: f32 = 16.0;
const TAB_FONT_SIZE: f32 = 11.5;
const TAB_CORNER_RADIUS: f32 = 3.0;
const UNDERLINE_HEIGHT: f32 = 2.0;
const UNDERLINE_INSET: f32 = 3.0;

/// Render the title bar at the top of the editor window. Returns an action to handle.
pub fn render_title_bar(
    ctx: &egui::Context,
    theme: &Theme,
    registry: &PanelRegistry,
    layout_manager: &LayoutManager,
) -> TitleBarAction {
    let mut action = TitleBarAction::None;

    egui::TopBottomPanel::top("renzora_title_bar")
        .exact_height(TITLE_BAR_HEIGHT)
        .show(ctx, |ui| {
            let panel_rect = ui.available_rect_before_wrap();

            egui::MenuBar::new().ui(ui, |ui| {
                // --- Left: menus ---
                ui.menu_button("File", |ui| {
                    if ui.button("New Project").clicked() {
                        action = TitleBarAction::NewProject;
                        ui.close();
                    }
                    if ui.button("Open Project...").clicked() {
                        action = TitleBarAction::OpenProject;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("New Scene").clicked() {
                        action = TitleBarAction::NewScene;
                        ui.close();
                    }
                    if ui.button("Open Scene...").clicked() {
                        action = TitleBarAction::OpenScene;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Save").clicked() {
                        action = TitleBarAction::Save;
                        ui.close();
                    }
                    if ui.button("Save As...").clicked() {
                        action = TitleBarAction::SaveAs;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Export Project...").clicked() {
                        action = TitleBarAction::Export;
                        ui.close();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        ui.close();
                    }
                    if ui.button("Redo").clicked() {
                        ui.close();
                    }
                });

                ui.menu_button("Window", |ui| {
                    for panel in registry.iter() {
                        let label = if let Some(icon) = panel.icon() {
                            format!("{} {}", icon, panel.title())
                        } else {
                            panel.title().to_string()
                        };
                        if ui.button(label).clicked() {
                            ui.close();
                        }
                    }
                    if registry.iter().next().is_none() {
                        ui.label(
                            egui::RichText::new("No panels registered")
                                .color(theme.text.muted.to_color32()),
                        );
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About Renzora").clicked() {
                        ui.close();
                    }
                });

                // --- Center: layout tabs ---
                let font = egui::FontId::proportional(TAB_FONT_SIZE);

                // Measure total tabs width for centering
                let tab_widths: Vec<f32> = layout_manager
                    .layouts
                    .iter()
                    .map(|l| {
                        let galley = ui.painter().layout_no_wrap(
                            l.name.clone(),
                            font.clone(),
                            Color32::WHITE,
                        );
                        galley.rect.width() + TAB_PADDING
                    })
                    .collect();
                let tab_spacing = 2.0;
                let total_tabs_width: f32 = tab_widths.iter().sum::<f32>()
                    + tab_spacing * (tab_widths.len().saturating_sub(1)) as f32;

                // Center the tabs in the panel
                let cursor_x = ui.cursor().left();
                let center_x = panel_rect.center().x;
                let desired_start = center_x - total_tabs_width / 2.0;
                let leading = (desired_start - cursor_x).max(12.0);
                ui.add_space(leading);

                let window_bg = theme.surfaces.window.to_color32();
                let accent = theme.semantic.accent.to_color32();
                let tab_y = panel_rect.min.y;
                let tab_h = panel_rect.height();

                for (i, layout) in layout_manager.layouts.iter().enumerate() {
                    let is_active = i == layout_manager.active_index;
                    let tw = tab_widths[i];

                    let tab_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().left(), tab_y),
                        Vec2::new(tw, tab_h),
                    );

                    let tab_id = ui.id().with(("layout_tab", i));
                    let response = ui.interact(tab_rect, tab_id, Sense::click());

                    // Background
                    let bg = if is_active {
                        brighten(window_bg, 18)
                    } else if response.hovered() {
                        brighten(window_bg, 10)
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(
                        tab_rect,
                        egui::CornerRadius::same(TAB_CORNER_RADIUS as u8),
                        bg,
                    );

                    // Text
                    let text_color = if is_active {
                        Color32::WHITE
                    } else if response.hovered() {
                        theme.text.secondary.to_color32()
                    } else {
                        theme.text.muted.to_color32()
                    };
                    ui.painter().text(
                        tab_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &layout.name,
                        font.clone(),
                        text_color,
                    );

                    // Active underline
                    if is_active {
                        let underline_rect = Rect::from_min_size(
                            Pos2::new(
                                tab_rect.min.x + UNDERLINE_INSET,
                                tab_rect.max.y - UNDERLINE_HEIGHT,
                            ),
                            Vec2::new(
                                tab_rect.width() - UNDERLINE_INSET * 2.0,
                                UNDERLINE_HEIGHT,
                            ),
                        );
                        ui.painter().rect_filled(
                            underline_rect,
                            egui::CornerRadius::same(1),
                            accent,
                        );
                    }

                    if response.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    if response.clicked() {
                        action = TitleBarAction::SwitchLayout(i);
                    }

                    // Advance cursor past this tab + spacing
                    ui.add_space(tw + tab_spacing);
                }
            });
        });

    action
}

/// Add a fixed brightness delta to each RGB channel of a color.
fn brighten(c: Color32, delta: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c.r().saturating_add(delta),
        c.g().saturating_add(delta),
        c.b().saturating_add(delta),
        c.a(),
    )
}

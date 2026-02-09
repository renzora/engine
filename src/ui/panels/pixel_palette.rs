//! Pixel Palette Panel
//!
//! Color swatches, primary/secondary color, recent colors, hex input,
//! HSV color picker, and palette grid.

use bevy_egui::egui::{self, Color32, Sense, Stroke, Vec2};
use crate::pixel_editor::{PixelEditorState, DefaultPalette};
use crate::theming::Theme;

/// Render the pixel palette panel
pub fn render_pixel_palette_content(
    ui: &mut egui::Ui,
    state: &mut PixelEditorState,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();

    egui::ScrollArea::vertical().show(ui, |ui| {
        // Primary/Secondary color display
        ui.horizontal(|ui| {
            // Primary color swatch
            let primary = color_to_color32(state.primary_color);
            let (p_rect, p_resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
            ui.painter().rect_filled(p_rect, 4.0, primary);
            ui.painter().rect_stroke(p_rect, 4.0, Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Outside);
            if p_resp.clicked() {
                // Could open full color picker
            }

            // Swap button
            if ui.small_button("\u{21C4}").on_hover_text("Swap colors (X)").clicked() {
                state.swap_colors();
            }

            // Secondary color swatch
            let secondary = color_to_color32(state.secondary_color);
            let (s_rect, _s_resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
            ui.painter().rect_filled(s_rect, 4.0, secondary);
            ui.painter().rect_stroke(s_rect, 4.0, Stroke::new(1.0, Color32::from_gray(100)), egui::StrokeKind::Outside);
        });

        ui.add_space(8.0);

        // RGB sliders
        ui.label(egui::RichText::new("Color").size(11.0).color(muted));
        let mut r = state.primary_color[0] as i32;
        let mut g = state.primary_color[1] as i32;
        let mut b = state.primary_color[2] as i32;
        let mut a = state.primary_color[3] as i32;

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("R").size(10.0).color(Color32::from_rgb(220, 80, 80)));
            if ui.add(egui::Slider::new(&mut r, 0..=255).show_value(true)).changed() {
                state.primary_color[0] = r as u8;
            }
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("G").size(10.0).color(Color32::from_rgb(80, 220, 80)));
            if ui.add(egui::Slider::new(&mut g, 0..=255).show_value(true)).changed() {
                state.primary_color[1] = g as u8;
            }
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("B").size(10.0).color(Color32::from_rgb(80, 80, 220)));
            if ui.add(egui::Slider::new(&mut b, 0..=255).show_value(true)).changed() {
                state.primary_color[2] = b as u8;
            }
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("A").size(10.0).color(muted));
            if ui.add(egui::Slider::new(&mut a, 0..=255).show_value(true)).changed() {
                state.primary_color[3] = a as u8;
            }
        });

        // Hex input
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("#").size(11.0).color(muted));
            let hex = format!("{:02X}{:02X}{:02X}{:02X}",
                state.primary_color[0], state.primary_color[1],
                state.primary_color[2], state.primary_color[3]);
            state.hex_input = hex;
            let resp = ui.add(egui::TextEdit::singleline(&mut state.hex_input)
                .desired_width(80.0)
                .font(egui::FontId::monospace(11.0)));
            if resp.lost_focus() {
                if let Some(color) = parse_hex_color(&state.hex_input) {
                    state.primary_color = color;
                }
            }
        });

        ui.add_space(8.0);
        ui.separator();

        // Palette selector
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Palette:").size(11.0).color(muted));
            egui::ComboBox::from_id_salt("palette_sel")
                .selected_text(&state.palette_name)
                .width(80.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(state.palette_name == "Pico-8", "Pico-8").clicked() {
                        state.palette = DefaultPalette::pico8();
                        state.palette_name = "Pico-8".to_string();
                    }
                    if ui.selectable_label(state.palette_name == "DB32", "DB32").clicked() {
                        state.palette = DefaultPalette::db32();
                        state.palette_name = "DB32".to_string();
                    }
                });
        });

        // Palette grid
        ui.add_space(4.0);
        let swatch_size = 18.0;
        let spacing = 2.0;
        let available_w = ui.available_width();
        let cols = ((available_w + spacing) / (swatch_size + spacing)).floor() as usize;
        let cols = cols.max(1);

        let palette = state.palette.clone();
        for (idx, _color) in palette.iter().enumerate() {
            if idx % cols == 0 && idx > 0 {
                ui.end_row();
            }
            if idx % cols == 0 {
                ui.horizontal(|ui| {
                    for j in 0..cols {
                        let ci = idx + j;
                        if ci >= palette.len() {
                            break;
                        }
                        let c = palette[ci];
                        let c32 = color_to_color32(c);
                        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(swatch_size), Sense::click());
                        ui.painter().rect_filled(rect, 2.0, c32);
                        if state.primary_color == c {
                            ui.painter().rect_stroke(rect, 2.0, Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Outside);
                        }
                        if resp.clicked() {
                            state.primary_color = c;
                        }
                        if resp.secondary_clicked() {
                            state.secondary_color = c;
                        }
                    }
                });
            }
        }

        // Recent colors
        if !state.recent_colors.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Recent").size(10.0).color(muted));
            ui.horizontal_wrapped(|ui| {
                for color in &state.recent_colors.clone() {
                    let c32 = color_to_color32(*color);
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::click());
                    ui.painter().rect_filled(rect, 1.0, c32);
                    if resp.clicked() {
                        state.primary_color = *color;
                    }
                }
            });
        }
    });
}

fn color_to_color32(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

fn parse_hex_color(hex: &str) -> Option<[u8; 4]> {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some([r, g, b, 255])
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some([r, g, b, a])
        }
        _ => None,
    }
}

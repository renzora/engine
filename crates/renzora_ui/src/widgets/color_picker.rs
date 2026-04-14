//! Richer color picker: swatch button + hex text entry.
//!
//! Wraps egui's native color picker popup so callers can also type a hex
//! string (`#ffffff`, `ff00ff`, `rgba(...)`-style prefixes stripped) and pick
//! with or without alpha. egui's `color_edit_button_*` already provides a
//! popup; this adds the hex text field beside it for keyboard-driven entry.

use bevy_egui::egui;

/// Pick mode — RGB only, or RGBA including alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Rgb,
    Rgba,
}

/// Configuration for `color_picker_with_hex`.
#[derive(Clone, Copy, Debug)]
pub struct ColorPickerConfig {
    pub mode: ColorMode,
    /// Width reserved for the hex input. Swatch button takes the rest.
    pub hex_width: f32,
}

impl Default for ColorPickerConfig {
    fn default() -> Self {
        Self { mode: ColorMode::Rgb, hex_width: 72.0 }
    }
}

/// Draw a swatch color-picker button followed by a hex input field.
/// `rgba` is mutated in-place; alpha is ignored when `mode == Rgb`.
pub fn color_picker_with_hex(
    ui: &mut egui::Ui,
    rgba: &mut [f32; 4],
    cfg: ColorPickerConfig,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        let swatch_resp = match cfg.mode {
            ColorMode::Rgb => {
                let mut rgb = [rgba[0], rgba[1], rgba[2]];
                let r = ui.color_edit_button_rgb(&mut rgb);
                rgba[0] = rgb[0]; rgba[1] = rgb[1]; rgba[2] = rgb[2];
                r
            }
            ColorMode::Rgba => ui.color_edit_button_rgba_unmultiplied(rgba),
        };

        // Hex state cached in egui memory so typing doesn't lose focus per-frame.
        let id = ui.id().with("hex_input");
        let current_hex = rgba_to_hex(*rgba, cfg.mode);
        let mut hex = ui.ctx().memory_mut(|m| {
            m.data
                .get_temp_mut_or_insert_with(id, || current_hex.clone())
                .clone()
        });
        // Sync cached hex when the picker changes the color externally.
        if swatch_resp.changed() {
            hex = current_hex.clone();
            ui.ctx().memory_mut(|m| m.data.insert_temp(id, hex.clone()));
        }

        let text_resp = ui.add_sized(
            [cfg.hex_width, 16.0],
            egui::TextEdit::singleline(&mut hex)
                .hint_text("#rrggbb")
                .font(egui::TextStyle::Monospace)
                .desired_width(cfg.hex_width),
        );

        if text_resp.changed() {
            ui.ctx().memory_mut(|m| m.data.insert_temp(id, hex.clone()));
            if let Some(parsed) = parse_hex(&hex, cfg.mode) {
                *rgba = parsed;
            }
        }
        // On focus loss, snap cached hex back to canonical form.
        if text_resp.lost_focus() {
            let canonical = rgba_to_hex(*rgba, cfg.mode);
            ui.ctx().memory_mut(|m| m.data.insert_temp(id, canonical));
        }

        swatch_resp.union(text_resp)
    })
    .inner
}

fn rgba_to_hex(c: [f32; 4], mode: ColorMode) -> String {
    let to_byte = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u8;
    let r = to_byte(c[0]);
    let g = to_byte(c[1]);
    let b = to_byte(c[2]);
    match mode {
        ColorMode::Rgb => format!("#{:02x}{:02x}{:02x}", r, g, b),
        ColorMode::Rgba => {
            let a = to_byte(c[3]);
            format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)
        }
    }
}

fn parse_hex(s: &str, mode: ColorMode) -> Option<[f32; 4]> {
    let s = s.trim().trim_start_matches('#');
    let bytes = match s.len() {
        3 => {
            // #rgb → #rrggbb
            let r = u8::from_str_radix(&s[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&s[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&s[2..3].repeat(2), 16).ok()?;
            [r, g, b, 255]
        }
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            [r, g, b, 255]
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            [r, g, b, a]
        }
        _ => return None,
    };
    let to_f = |b: u8| b as f32 / 255.0;
    Some(match mode {
        ColorMode::Rgb => [to_f(bytes[0]), to_f(bytes[1]), to_f(bytes[2]), 1.0],
        ColorMode::Rgba => [to_f(bytes[0]), to_f(bytes[1]), to_f(bytes[2]), to_f(bytes[3])],
    })
}

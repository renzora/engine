//! Horizontal slider with min/max labels.
//!
//! A labeled alternative to egui's bare `Slider` / `DragValue` for bounded
//! float properties. Shows the min value on the left, max on the right, with
//! the slider track between — useful for 0..1 ranges and similar.

use bevy_egui::egui;
use renzora_theme::Theme;

/// Configuration for `labeled_slider`.
#[derive(Clone, Copy, Debug)]
pub struct SliderConfig {
    pub min: f32,
    pub max: f32,
    /// If true, show min/max labels at each end of the track.
    pub show_endpoints: bool,
    /// Number of decimal places to display.
    pub decimals: usize,
}

impl Default for SliderConfig {
    fn default() -> Self {
        Self { min: 0.0, max: 1.0, show_endpoints: true, decimals: 2 }
    }
}

impl SliderConfig {
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max, ..Self::default() }
    }
    pub fn decimals(mut self, d: usize) -> Self {
        self.decimals = d;
        self
    }
    pub fn show_endpoints(mut self, b: bool) -> Self {
        self.show_endpoints = b;
        self
    }
}

/// Horizontal slider with optional min/max endpoint labels.
pub fn labeled_slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    cfg: SliderConfig,
    theme: &Theme,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let muted = theme.text.muted.to_color32();

        if cfg.show_endpoints {
            ui.label(
                egui::RichText::new(format!("{:.*}", cfg.decimals, cfg.min))
                    .size(10.0)
                    .color(muted),
            );
        }

        let slider_w = (ui.available_width()
            - if cfg.show_endpoints { 44.0 } else { 0.0 })
            .max(40.0);
        let r = ui.add_sized(
            [slider_w, 16.0],
            egui::Slider::new(value, cfg.min..=cfg.max)
                .show_value(true)
                .trailing_fill(true),
        );

        if cfg.show_endpoints {
            ui.label(
                egui::RichText::new(format!("{:.*}", cfg.decimals, cfg.max))
                    .size(10.0)
                    .color(muted),
            );
        }

        r
    })
    .inner
}

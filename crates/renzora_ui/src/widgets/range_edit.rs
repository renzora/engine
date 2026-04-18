//! Paired min/max range input.
//!
//! Two DragValues side-by-side with the constraint that `min ≤ max`. Useful
//! for animation frame ranges, spawn-count ranges, and any `Range<f32>`-like
//! property.

use bevy_egui::egui;

/// Configuration for `range_edit`.
#[derive(Clone, Copy, Debug)]
pub struct RangeConfig {
    pub speed: f32,
    /// Absolute lower bound for `min`.
    pub hard_min: Option<f32>,
    /// Absolute upper bound for `max`.
    pub hard_max: Option<f32>,
}

impl Default for RangeConfig {
    fn default() -> Self {
        Self { speed: 0.1, hard_min: None, hard_max: None }
    }
}

impl RangeConfig {
    pub fn new(speed: f32) -> Self {
        Self { speed, ..Self::default() }
    }
    pub fn with_bounds(mut self, hard_min: f32, hard_max: f32) -> Self {
        self.hard_min = Some(hard_min);
        self.hard_max = Some(hard_max);
        self
    }
}

/// Edit a `(min, max)` pair. Enforces `min ≤ max` after each drag; if the
/// user pushes min above max (or vice-versa) the other side is clamped.
pub fn range_edit(
    ui: &mut egui::Ui,
    min: &mut f32,
    max: &mut f32,
    cfg: RangeConfig,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let w = ((ui.available_width() - 24.0) / 2.0).max(40.0);

        let muted = ui.style().visuals.weak_text_color();
        ui.label(egui::RichText::new("min").size(10.0).color(muted));
        let mut dmin = egui::DragValue::new(min).speed(cfg.speed);
        if let Some(h) = cfg.hard_min { dmin = dmin.range(h..=*max); } else { dmin = dmin.range(f32::NEG_INFINITY..=*max); }
        let r1 = ui.add_sized([w, 16.0], dmin);

        ui.label(egui::RichText::new("max").size(10.0).color(muted));
        let mut dmax = egui::DragValue::new(max).speed(cfg.speed);
        if let Some(h) = cfg.hard_max { dmax = dmax.range(*min..=h); } else { dmax = dmax.range(*min..=f32::INFINITY); }
        let r2 = ui.add_sized([w, 16.0], dmax);

        // Final clamp in case both were edited in the same frame.
        if *min > *max { *min = *max; }

        r1.union(r2)
    })
    .inner
}

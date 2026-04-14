//! Number input with ± stepper buttons.

use bevy_egui::egui;

/// Configuration for `number_stepper`.
#[derive(Clone, Copy, Debug)]
pub struct NumberStepperConfig {
    pub step: f32,
    pub min: f32,
    pub max: f32,
    pub decimals: usize,
}

impl Default for NumberStepperConfig {
    fn default() -> Self {
        Self { step: 1.0, min: f32::NEG_INFINITY, max: f32::INFINITY, decimals: 2 }
    }
}

/// A labeled number input with − / + buttons flanking a DragValue.
pub fn number_stepper(
    ui: &mut egui::Ui,
    value: &mut f32,
    cfg: NumberStepperConfig,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if ui.small_button("−").clicked() {
            *value = (*value - cfg.step).max(cfg.min);
        }
        let drag = ui.add(
            egui::DragValue::new(value)
                .speed(cfg.step * 0.1)
                .range(cfg.min..=cfg.max)
                .fixed_decimals(cfg.decimals),
        );
        if ui.small_button("+").clicked() {
            *value = (*value + cfg.step).min(cfg.max);
        }
        drag
    })
    .inner
}

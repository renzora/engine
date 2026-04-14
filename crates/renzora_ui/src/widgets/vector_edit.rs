//! Vector editors — Vec2/3/4 and Quat (euler).
//!
//! All editors are colored-axis inline inputs designed to fit inside a
//! `inline_property` row. Each component is a `DragValue` with shared speed
//! and optional range clamping.

use bevy::math::{Quat, Vec2, Vec3, Vec4};
use bevy_egui::egui;

const AXIS_X: egui::Color32 = egui::Color32::from_rgb(230, 90, 90);
const AXIS_Y: egui::Color32 = egui::Color32::from_rgb(130, 200, 90);
const AXIS_Z: egui::Color32 = egui::Color32::from_rgb(90, 150, 230);
const AXIS_W: egui::Color32 = egui::Color32::from_rgb(210, 170, 220);

/// Shared configuration for all vector editors.
#[derive(Clone, Copy, Debug)]
pub struct VecEditConfig {
    pub speed: f32,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

impl Default for VecEditConfig {
    fn default() -> Self {
        Self { speed: 0.1, min: None, max: None }
    }
}

impl VecEditConfig {
    pub fn new(speed: f32) -> Self {
        Self { speed, min: None, max: None }
    }
    pub fn with_range(mut self, min: f32, max: f32) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }
}

/// Render one labeled axis cell. Shared helper for all vector editors.
fn axis_cell(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
    value: &mut f32,
    cell_width: f32,
    cfg: &VecEditConfig,
) -> egui::Response {
    ui.label(egui::RichText::new(label).size(10.0).color(color));
    let mut drag = egui::DragValue::new(value).speed(cfg.speed);
    if let (Some(min), Some(max)) = (cfg.min, cfg.max) {
        drag = drag.range(min..=max);
    }
    ui.add_sized([cell_width, 16.0], drag)
}

/// Compute per-cell width for `n` axes, accounting for label padding.
fn cell_width(ui: &egui::Ui, n: usize) -> f32 {
    let reserved = 16.0 * n as f32; // label widths
    ((ui.available_width() - reserved) / n as f32).max(30.0)
}

/// Vec2 editor — X/Y drag cells.
pub fn vec2_edit(ui: &mut egui::Ui, value: &mut Vec2, cfg: VecEditConfig) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let w = cell_width(ui, 2);
        let r1 = axis_cell(ui, "X", AXIS_X, &mut value.x, w, &cfg);
        let r2 = axis_cell(ui, "Y", AXIS_Y, &mut value.y, w, &cfg);
        r1.union(r2)
    })
    .inner
}

/// Vec3 editor — X/Y/Z drag cells.
pub fn vec3_edit(ui: &mut egui::Ui, value: &mut Vec3, cfg: VecEditConfig) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let w = cell_width(ui, 3);
        let r1 = axis_cell(ui, "X", AXIS_X, &mut value.x, w, &cfg);
        let r2 = axis_cell(ui, "Y", AXIS_Y, &mut value.y, w, &cfg);
        let r3 = axis_cell(ui, "Z", AXIS_Z, &mut value.z, w, &cfg);
        r1.union(r2).union(r3)
    })
    .inner
}

/// Vec4 editor — X/Y/Z/W drag cells.
pub fn vec4_edit(ui: &mut egui::Ui, value: &mut Vec4, cfg: VecEditConfig) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let w = cell_width(ui, 4);
        let mut x = value.x;
        let mut y = value.y;
        let mut z = value.z;
        let mut w_val = value.w;
        let r1 = axis_cell(ui, "X", AXIS_X, &mut x, w, &cfg);
        let r2 = axis_cell(ui, "Y", AXIS_Y, &mut y, w, &cfg);
        let r3 = axis_cell(ui, "Z", AXIS_Z, &mut z, w, &cfg);
        let r4 = axis_cell(ui, "W", AXIS_W, &mut w_val, w, &cfg);
        *value = Vec4::new(x, y, z, w_val);
        r1.union(r2).union(r3).union(r4)
    })
    .inner
}

/// Quat editor that presents euler angles (degrees) to the user.
///
/// Reads via `to_euler(YXZ)` and writes back via `from_euler(YXZ)`, so the
/// underlying quaternion round-trips without gimbal-induced flipping as long
/// as the user stays in a single editing session. Values are displayed in
/// degrees for readability.
pub fn quat_edit_euler(ui: &mut egui::Ui, value: &mut Quat, cfg: VecEditConfig) -> egui::Response {
    let (y, x, z) = value.to_euler(bevy::math::EulerRot::YXZ);
    let mut euler_deg = Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees());
    let before = euler_deg;
    let response = vec3_edit(ui, &mut euler_deg, cfg);
    if euler_deg != before {
        *value = Quat::from_euler(
            bevy::math::EulerRot::YXZ,
            euler_deg.y.to_radians(),
            euler_deg.x.to_radians(),
            euler_deg.z.to_radians(),
        );
    }
    response
}

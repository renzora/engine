//! Gameplay component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;

use egui_phosphor::regular::HEART;

// ============================================================================
// Data Types
// ============================================================================

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct HealthData {
    pub max_health: f32,
    pub current_health: f32,
    pub regeneration_rate: f32,
    pub invincible: bool,
    pub destroy_on_death: bool,
}

impl Default for HealthData {
    fn default() -> Self {
        Self {
            max_health: 100.0,
            current_health: 100.0,
            regeneration_rate: 0.0,
            invincible: false,
            destroy_on_death: true,
        }
    }
}

// ============================================================================
// Custom Inspectors
// ============================================================================

fn inspect_health(
    ui: &mut egui::Ui, world: &mut World, entity: Entity,
    _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;
    if let Some(mut data) = world.get_mut::<HealthData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Max Health:");
            if ui.add(egui::DragValue::new(&mut data.max_health).speed(1.0).range(1.0..=10000.0)).changed() { changed = true; }
        });

        let max_health = data.max_health;
        ui.horizontal(|ui| {
            ui.label("Current:");
            if ui.add(egui::DragValue::new(&mut data.current_health).speed(1.0).range(0.0..=max_health)).changed() { changed = true; }
        });

        let health_pct = data.current_health / data.max_health;
        let bar_color = if health_pct > 0.5 {
            egui::Color32::from_rgb(100, 200, 100)
        } else if health_pct > 0.25 {
            egui::Color32::from_rgb(200, 200, 100)
        } else {
            egui::Color32::from_rgb(200, 100, 100)
        };
        ui.add(egui::ProgressBar::new(health_pct).fill(bar_color));

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Regen/sec:");
            if ui.add(egui::DragValue::new(&mut data.regeneration_rate).speed(0.1).range(0.0..=100.0)).changed() { changed = true; }
        });

        if ui.checkbox(&mut data.invincible, "Invincible").changed() { changed = true; }
        if ui.checkbox(&mut data.destroy_on_death, "Destroy on Death").changed() { changed = true; }
    }
    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(HealthData {
        type_id: "health",
        display_name: "Health",
        category: ComponentCategory::Gameplay,
        icon: HEART,
        priority: 0,
        custom_inspector: inspect_health,
    }));
}

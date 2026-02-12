//! Underwater / Rain on Lens effect component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::DROP;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::post_process::UnderwaterSettings;
use crate::register_component;
use crate::shared::UnderwaterData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(UnderwaterData {
        type_id: "underwater",
        display_name: "Underwater",
        category: ComponentCategory::PostProcess,
        icon: DROP,
        custom_inspector: inspect_underwater,
    }));
}

fn inspect_underwater(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut uw) = world.get_mut::<UnderwaterData>(entity) else {
        return false;
    };

    sanitize_f32(&mut uw.distortion, 0.0, 0.1, 0.01);
    sanitize_f32(&mut uw.tint_strength, 0.0, 1.0, 0.3);
    sanitize_f32(&mut uw.wave_speed, 0.0, 5.0, 1.0);
    sanitize_f32(&mut uw.wave_scale, 1.0, 20.0, 5.0);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Distortion", |ui| {
        ui.add(egui::DragValue::new(&mut uw.distortion).speed(0.001).range(0.0..=0.1)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Tint Color", |ui| {
        let mut color = egui::Color32::from_rgb(
            (uw.tint_color.0 * 255.0) as u8,
            (uw.tint_color.1 * 255.0) as u8,
            (uw.tint_color.2 * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            uw.tint_color = (
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });
    row += 1;

    changed |= inline_property(ui, row, "Tint Strength", |ui| {
        ui.add(egui::DragValue::new(&mut uw.tint_strength).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Wave Speed", |ui| {
        ui.add(egui::DragValue::new(&mut uw.wave_speed).speed(0.1).range(0.0..=5.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Wave Scale", |ui| {
        ui.add(egui::DragValue::new(&mut uw.wave_scale).speed(0.5).range(1.0..=20.0)).changed()
    });

    changed
}

pub(crate) fn sync_underwater(
    mut commands: Commands,
    query: Query<
        (&UnderwaterData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<UnderwaterData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    time: Res<Time>,
) {
    if query.is_empty() {
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((uw, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("underwater"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(UnderwaterSettings {
                    distortion: uw.distortion,
                    tint_r: uw.tint_color.0,
                    tint_g: uw.tint_color.1,
                    tint_b: uw.tint_color.2,
                    tint_strength: uw.tint_strength,
                    wave_speed: uw.wave_speed,
                    wave_scale: uw.wave_scale,
                    time: time.elapsed_secs(),
                });
            } else {
                commands.entity(cam).remove::<UnderwaterSettings>();
            }
        }
    }
}

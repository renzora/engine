//! Distortion / Heat Haze effect component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::WAVES;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::post_process::DistortionSettings;
use crate::register_component;
use crate::shared::DistortionData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(DistortionData {
        type_id: "distortion",
        display_name: "Distortion",
        category: ComponentCategory::PostProcess,
        icon: WAVES,
        custom_inspector: inspect_distortion,
    }));
}

fn inspect_distortion(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut d) = world.get_mut::<DistortionData>(entity) else {
        return false;
    };

    sanitize_f32(&mut d.intensity, 0.0, 0.1, 0.01);
    sanitize_f32(&mut d.speed, 0.0, 10.0, 1.0);
    sanitize_f32(&mut d.scale, 1.0, 50.0, 10.0);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut d.intensity).speed(0.001).range(0.0..=0.1)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Speed", |ui| {
        ui.add(egui::DragValue::new(&mut d.speed).speed(0.1).range(0.0..=10.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Scale", |ui| {
        ui.add(egui::DragValue::new(&mut d.scale).speed(0.5).range(1.0..=50.0)).changed()
    });

    changed
}

pub(crate) fn sync_distortion(
    mut commands: Commands,
    query: Query<
        (&DistortionData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<DistortionData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    time: Res<Time>,
) {
    if query.is_empty() {
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((d, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("distortion"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(DistortionSettings {
                    intensity: d.intensity,
                    speed: d.speed,
                    scale: d.scale,
                    time: time.elapsed_secs(),
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<DistortionSettings>();
            }
        }
    }
}

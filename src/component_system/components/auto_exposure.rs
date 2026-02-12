//! Auto Exposure component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::post_process::auto_exposure::AutoExposure;
use bevy_egui::egui;
use egui_phosphor::regular::SUN_DIM;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::AutoExposureData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(AutoExposureData {
        type_id: "auto_exposure",
        display_name: "Auto Exposure",
        category: ComponentCategory::PostProcess,
        icon: SUN_DIM,
        custom_inspector: inspect_auto_exposure,
    }));
}

fn inspect_auto_exposure(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut ae) = world.get_mut::<AutoExposureData>(entity) else {
        return false;
    };

    sanitize_f32(&mut ae.speed_brighten, 0.1, 20.0, 3.0);
    sanitize_f32(&mut ae.speed_darken, 0.1, 20.0, 1.0);
    sanitize_f32(&mut ae.range_min, -16.0, 0.0, -8.0);
    sanitize_f32(&mut ae.range_max, 0.0, 16.0, 8.0);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Speed Brighten", |ui| {
        ui.add(egui::DragValue::new(&mut ae.speed_brighten).speed(0.1).range(0.1..=20.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Speed Darken", |ui| {
        ui.add(egui::DragValue::new(&mut ae.speed_darken).speed(0.1).range(0.1..=20.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Range Min", |ui| {
        ui.add(egui::DragValue::new(&mut ae.range_min).speed(0.1).range(-16.0..=0.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Range Max", |ui| {
        ui.add(egui::DragValue::new(&mut ae.range_max).speed(0.1).range(0.0..=16.0)).changed()
    });

    changed
}

pub(crate) fn sync_auto_exposure(
    mut commands: Commands,
    query: Query<
        (&AutoExposureData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<AutoExposureData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if query.is_empty() {
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((ae, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("auto_exposure"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(AutoExposure {
                    range: ae.range_min..=ae.range_max,
                    speed_brighten: ae.speed_brighten,
                    speed_darken: ae.speed_darken,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<AutoExposure>();
            }
        }
    }
}

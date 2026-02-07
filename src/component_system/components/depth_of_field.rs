//! Depth of field component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy_egui::egui;
use egui_phosphor::regular::APERTURE;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::DepthOfFieldData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(DepthOfFieldData {
        type_id: "depth_of_field",
        display_name: "Depth of Field",
        category: ComponentCategory::PostProcess,
        icon: APERTURE,
        custom_inspector: inspect_depth_of_field,
    }));
}

fn inspect_depth_of_field(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut dof) = world.get_mut::<DepthOfFieldData>(entity) else {
        return false;
    };

    sanitize_f32(&mut dof.focal_distance, 0.1, 100.0, 10.0);
    sanitize_f32(&mut dof.aperture, 0.001, 0.5, 0.05);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Enabled", |ui| {
        ui.checkbox(&mut dof.enabled, "").changed()
    });
    row += 1;

    if dof.enabled {
        changed |= inline_property(ui, row, "Focal Distance", |ui| {
            ui.add(egui::DragValue::new(&mut dof.focal_distance).speed(0.1).range(0.1..=100.0)).changed()
        });
        row += 1;

        changed |= inline_property(ui, row, "Aperture", |ui| {
            ui.add(egui::DragValue::new(&mut dof.aperture).speed(0.01).range(0.001..=0.5)).changed()
        });
    }

    changed
}

pub(crate) fn sync_depth_of_field(
    mut commands: Commands,
    dof_query: Query<
        (&DepthOfFieldData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<DepthOfFieldData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if dof_query.is_empty() {
        return;
    }

    let active = dof_query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((dof, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("depth_of_field"));
        for cam in cameras.iter() {
            if !disabled && dof.enabled {
                commands.entity(cam).insert(DepthOfField {
                    focal_distance: dof.focal_distance,
                    aperture_f_stops: dof.aperture,
                    mode: DepthOfFieldMode::Bokeh,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<DepthOfField>();
            }
        }
    }
}

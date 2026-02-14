//! Volumetric Fog component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::light::{VolumetricFog, VolumetricLight};
use bevy_egui::egui;
use egui_phosphor::regular::CLOUD;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::component_system::VolumetricFogData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VolumetricFogData {
        type_id: "volumetric_fog",
        display_name: "Volumetric Fog",
        category: ComponentCategory::PostProcess,
        icon: CLOUD,
        custom_inspector: inspect_volumetric_fog,
    }));
}

fn inspect_volumetric_fog(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut vf) = world.get_mut::<VolumetricFogData>(entity) else {
        return false;
    };

    sanitize_f32(&mut vf.ambient_intensity, 0.0, 1.0, 0.1);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Ambient Color", |ui| {
        let mut color = egui::Color32::from_rgb(
            (vf.ambient_color.0 * 255.0) as u8,
            (vf.ambient_color.1 * 255.0) as u8,
            (vf.ambient_color.2 * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            vf.ambient_color = (
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });
    row += 1;

    changed |= inline_property(ui, row, "Ambient Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut vf.ambient_intensity).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Step Count", |ui| {
        ui.add(egui::DragValue::new(&mut vf.step_count).speed(1).range(8..=256)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Volumetric Light", |ui| {
        ui.checkbox(&mut vf.volumetric_light, "").changed()
    });

    changed
}

pub(crate) fn sync_volumetric_fog(
    mut commands: Commands,
    query: Query<
        (&VolumetricFogData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<VolumetricFogData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    dir_lights: Query<Entity, With<DirectionalLight>>,
    has_data: Query<(), With<VolumetricFogData>>,
    mut removed: RemovedComponents<VolumetricFogData>,
) {
    let had_removals = removed.read().count() > 0;
    if query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<VolumetricFog>();
        }
        for light in dir_lights.iter() {
            commands.entity(light).remove::<VolumetricLight>();
        }
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((vf, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("volumetric_fog"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(VolumetricFog {
                    ambient_color: Color::srgb(vf.ambient_color.0, vf.ambient_color.1, vf.ambient_color.2),
                    ambient_intensity: vf.ambient_intensity,
                    step_count: vf.step_count,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<VolumetricFog>();
            }
        }

        // Add/remove VolumetricLight on directional lights
        for light in dir_lights.iter() {
            if !disabled && vf.volumetric_light {
                commands.entity(light).insert(VolumetricLight);
            } else {
                commands.entity(light).remove::<VolumetricLight>();
            }
        }
    }
}

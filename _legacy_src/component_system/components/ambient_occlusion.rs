//! Ambient occlusion component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy_egui::egui;
use egui_phosphor::regular::EYE;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::component_system::AmbientOcclusionData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(AmbientOcclusionData {
        type_id: "ambient_occlusion",
        display_name: "Ambient Occlusion",
        category: ComponentCategory::PostProcess,
        icon: EYE,
        custom_inspector: inspect_ambient_occlusion,
    }));
}

fn inspect_ambient_occlusion(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut ao) = world.get_mut::<AmbientOcclusionData>(entity) else {
        return false;
    };

    sanitize_f32(&mut ao.intensity, 0.0, 3.0, 1.0);
    sanitize_f32(&mut ao.radius, 0.01, 2.0, 0.5);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut ao.intensity).speed(0.1).range(0.0..=3.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Radius", |ui| {
        ui.add(egui::DragValue::new(&mut ao.radius).speed(0.01).range(0.01..=2.0)).changed()
    });

    changed
}

pub(crate) fn sync_ambient_occlusion(
    mut commands: Commands,
    ao_query: Query<
        (&AmbientOcclusionData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<AmbientOcclusionData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<AmbientOcclusionData>>,
    mut removed: RemovedComponents<AmbientOcclusionData>,
) {
    let had_removals = removed.read().count() > 0;
    if ao_query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<ScreenSpaceAmbientOcclusion>();
        }
        return;
    }

    let active = ao_query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((ao, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("ambient_occlusion"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(ScreenSpaceAmbientOcclusion::default());
            } else {
                commands.entity(cam).remove::<ScreenSpaceAmbientOcclusion>();
            }
        }
    }
}

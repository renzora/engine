//! Fog component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy_egui::egui;
use egui_phosphor::regular::CLOUD;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::FogData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

/// Register FogData with the component registry.
pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(FogData {
        type_id: "fog",
        display_name: "Fog",
        category: ComponentCategory::PostProcess,
        icon: CLOUD,
        custom_inspector: inspect_fog,
    }));
}

fn inspect_fog(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut fog) = world.get_mut::<FogData>(entity) else {
        return false;
    };

    sanitize_f32(&mut fog.start, 0.0, 10000.0, 10.0);
    sanitize_f32(&mut fog.end, 0.0, 10000.0, 100.0);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Color", |ui| {
        let mut color = egui::Color32::from_rgb(
            (fog.color.0 * 255.0) as u8,
            (fog.color.1 * 255.0) as u8,
            (fog.color.2 * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            fog.color = (
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });
    row += 1;

    changed |= inline_property(ui, row, "Start", |ui| {
        ui.add(egui::DragValue::new(&mut fog.start).speed(0.1)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "End", |ui| {
        ui.add(egui::DragValue::new(&mut fog.end).speed(0.1)).changed()
    });

    changed
}

/// Sync system: applies FogData to viewport cameras as DistanceFog.
pub(crate) fn sync_fog(
    mut commands: Commands,
    fog_query: Query<
        (&FogData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<FogData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if fog_query.is_empty() {
        return;
    }

    // Find first visible entity with FogData
    let active_fog = fog_query.iter()
        .find(|(_, editor, _)| editor.visible);

    if let Some((fog, _editor, dc)) = active_fog {
        let disabled = dc.map_or(false, |d| d.is_disabled("fog"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(DistanceFog {
                    color: Color::srgba(fog.color.0, fog.color.1, fog.color.2, 1.0),
                    falloff: FogFalloff::Linear {
                        start: fog.start,
                        end: fog.end,
                    },
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<DistanceFog>();
            }
        }
    }
}

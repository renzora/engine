//! Pixelation effect component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::GRID_FOUR;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::post_process::PixelationSettings;
use crate::register_component;
use crate::shared::PixelationData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(PixelationData {
        type_id: "pixelation",
        display_name: "Pixelation",
        category: ComponentCategory::PostProcess,
        icon: GRID_FOUR,
        custom_inspector: inspect_pixelation,
    }));
}

fn inspect_pixelation(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut p) = world.get_mut::<PixelationData>(entity) else {
        return false;
    };

    sanitize_f32(&mut p.pixel_size, 1.0, 64.0, 4.0);

    let mut changed = false;

    changed |= inline_property(ui, 0, "Pixel Size", |ui| {
        ui.add(egui::DragValue::new(&mut p.pixel_size).speed(0.5).range(1.0..=64.0)).changed()
    });

    changed
}

pub(crate) fn sync_pixelation(
    mut commands: Commands,
    query: Query<
        (&PixelationData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<PixelationData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if query.is_empty() {
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((p, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("pixelation"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(PixelationSettings {
                    pixel_size: p.pixel_size,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<PixelationSettings>();
            }
        }
    }
}

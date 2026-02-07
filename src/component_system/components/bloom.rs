//! Bloom component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::post_process::bloom::Bloom;
use bevy_egui::egui;
use egui_phosphor::regular::FLOWER_LOTUS;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::BloomData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(BloomData {
        type_id: "bloom",
        display_name: "Bloom",
        category: ComponentCategory::PostProcess,
        icon: FLOWER_LOTUS,
        custom_inspector: inspect_bloom,
    }));
}

fn inspect_bloom(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut bloom) = world.get_mut::<BloomData>(entity) else {
        return false;
    };

    sanitize_f32(&mut bloom.intensity, 0.0, 1.0, 0.15);
    sanitize_f32(&mut bloom.threshold, 0.0, 5.0, 1.0);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut bloom.intensity).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Threshold", |ui| {
        ui.add(egui::DragValue::new(&mut bloom.threshold).speed(0.1).range(0.0..=5.0)).changed()
    });

    changed
}

pub(crate) fn sync_bloom(
    mut commands: Commands,
    bloom_query: Query<
        (&BloomData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<BloomData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if bloom_query.is_empty() {
        return;
    }

    let active = bloom_query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((bloom, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("bloom"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(Bloom {
                    intensity: bloom.intensity,
                    low_frequency_boost: bloom.threshold * 0.5,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<Bloom>();
            }
        }
    }
}

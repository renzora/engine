//! CRT display effect component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::MONITOR;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::post_process::CrtSettings;
use crate::register_component;
use crate::shared::CrtData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(CrtData {
        type_id: "crt",
        display_name: "CRT Effect",
        category: ComponentCategory::PostProcess,
        icon: MONITOR,
        custom_inspector: inspect_crt,
    }));
}

fn inspect_crt(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut crt) = world.get_mut::<CrtData>(entity) else {
        return false;
    };

    sanitize_f32(&mut crt.scanline_intensity, 0.0, 1.0, 0.3);
    sanitize_f32(&mut crt.curvature, 0.0, 0.1, 0.02);
    sanitize_f32(&mut crt.chromatic_amount, 0.0, 0.05, 0.005);
    sanitize_f32(&mut crt.vignette_amount, 0.0, 1.0, 0.3);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Scanlines", |ui| {
        ui.add(egui::DragValue::new(&mut crt.scanline_intensity).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Curvature", |ui| {
        ui.add(egui::DragValue::new(&mut crt.curvature).speed(0.001).range(0.0..=0.1)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Chromatic", |ui| {
        ui.add(egui::DragValue::new(&mut crt.chromatic_amount).speed(0.001).range(0.0..=0.05)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Vignette", |ui| {
        ui.add(egui::DragValue::new(&mut crt.vignette_amount).speed(0.01).range(0.0..=1.0)).changed()
    });

    changed
}

pub(crate) fn sync_crt(
    mut commands: Commands,
    query: Query<
        (&CrtData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<CrtData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<CrtData>>,
    mut removed: RemovedComponents<CrtData>,
) {
    let had_removals = removed.read().count() > 0;
    if query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<CrtSettings>();
        }
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((crt, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("crt"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(CrtSettings {
                    scanline_intensity: crt.scanline_intensity,
                    curvature: crt.curvature,
                    chromatic_amount: crt.chromatic_amount,
                    vignette_amount: crt.vignette_amount,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<CrtSettings>();
            }
        }
    }
}

//! Vignette effect component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::CIRCLE_HALF;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::post_process::VignetteSettings;
use crate::register_component;
use crate::shared::VignetteData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VignetteData {
        type_id: "vignette",
        display_name: "Vignette",
        category: ComponentCategory::PostProcess,
        icon: CIRCLE_HALF,
        custom_inspector: inspect_vignette,
    }));
}

fn inspect_vignette(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut v) = world.get_mut::<VignetteData>(entity) else {
        return false;
    };

    sanitize_f32(&mut v.intensity, 0.0, 1.0, 0.5);
    sanitize_f32(&mut v.radius, 0.0, 2.0, 0.8);
    sanitize_f32(&mut v.smoothness, 0.0, 1.0, 0.3);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut v.intensity).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Radius", |ui| {
        ui.add(egui::DragValue::new(&mut v.radius).speed(0.01).range(0.0..=2.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Smoothness", |ui| {
        ui.add(egui::DragValue::new(&mut v.smoothness).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Color", |ui| {
        let mut color = egui::Color32::from_rgb(
            (v.color.0 * 255.0) as u8,
            (v.color.1 * 255.0) as u8,
            (v.color.2 * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            v.color = (
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });

    changed
}

pub(crate) fn sync_vignette(
    mut commands: Commands,
    query: Query<
        (&VignetteData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<VignetteData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<VignetteData>>,
    mut removed: RemovedComponents<VignetteData>,
) {
    let had_removals = removed.read().count() > 0;
    if query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<VignetteSettings>();
        }
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((v, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("vignette"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(VignetteSettings {
                    intensity: v.intensity,
                    radius: v.radius,
                    smoothness: v.smoothness,
                    color_r: v.color.0,
                    color_g: v.color.1,
                    color_b: v.color.2,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<VignetteSettings>();
            }
        }
    }
}

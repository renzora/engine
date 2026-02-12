//! Gaussian Blur effect component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::DROP_HALF;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::post_process::GaussianBlurSettings;
use crate::register_component;
use crate::shared::GaussianBlurData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(GaussianBlurData {
        type_id: "gaussian_blur",
        display_name: "Gaussian Blur",
        category: ComponentCategory::PostProcess,
        icon: DROP_HALF,
        custom_inspector: inspect_gaussian_blur,
    }));
}

fn inspect_gaussian_blur(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut gb) = world.get_mut::<GaussianBlurData>(entity) else {
        return false;
    };

    sanitize_f32(&mut gb.sigma, 0.1, 20.0, 2.0);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Sigma", |ui| {
        ui.add(egui::DragValue::new(&mut gb.sigma).speed(0.1).range(0.1..=20.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Kernel Size", |ui| {
        ui.add(egui::DragValue::new(&mut gb.kernel_size).speed(2).range(3..=31)).changed()
    });

    changed
}

pub(crate) fn sync_gaussian_blur(
    mut commands: Commands,
    query: Query<
        (&GaussianBlurData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<GaussianBlurData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if query.is_empty() {
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((gb, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("gaussian_blur"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(GaussianBlurSettings {
                    sigma: gb.sigma,
                    kernel_size: gb.kernel_size,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<GaussianBlurSettings>();
            }
        }
    }
}

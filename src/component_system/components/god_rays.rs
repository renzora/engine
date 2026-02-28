//! God Rays (light shafts) effect component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::SUN;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::post_process::GodRaysSettings;
use crate::register_component;
use crate::component_system::GodRaysData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(GodRaysData {
        type_id: "god_rays",
        display_name: "God Rays",
        category: ComponentCategory::PostProcess,
        icon: SUN,
        custom_inspector: inspect_god_rays,
    }));
}

fn inspect_god_rays(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut gr) = world.get_mut::<GodRaysData>(entity) else {
        return false;
    };

    sanitize_f32(&mut gr.intensity, 0.0, 2.0, 0.5);
    sanitize_f32(&mut gr.decay, 0.9, 1.0, 0.97);
    sanitize_f32(&mut gr.density, 0.1, 3.0, 1.0);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, &crate::locale::t("comp.god_rays.intensity"), |ui| {
        ui.add(egui::DragValue::new(&mut gr.intensity).speed(0.01).range(0.0..=2.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, &crate::locale::t("comp.god_rays.decay"), |ui| {
        ui.add(egui::DragValue::new(&mut gr.decay).speed(0.001).range(0.9..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, &crate::locale::t("comp.god_rays.density"), |ui| {
        ui.add(egui::DragValue::new(&mut gr.density).speed(0.01).range(0.1..=3.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Samples", |ui| {
        ui.add(egui::DragValue::new(&mut gr.num_samples).speed(1).range(16..=128)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Light X", |ui| {
        ui.add(egui::DragValue::new(&mut gr.light_screen_pos.0).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Light Y", |ui| {
        ui.add(egui::DragValue::new(&mut gr.light_screen_pos.1).speed(0.01).range(0.0..=1.0)).changed()
    });

    changed
}

pub(crate) fn sync_god_rays(
    mut commands: Commands,
    query: Query<
        (&GodRaysData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<GodRaysData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<GodRaysData>>,
    mut removed: RemovedComponents<GodRaysData>,
) {
    let had_removals = removed.read().count() > 0;
    if query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<GodRaysSettings>();
        }
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((gr, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("god_rays"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(GodRaysSettings {
                    intensity: gr.intensity,
                    decay: gr.decay,
                    density: gr.density,
                    num_samples: gr.num_samples,
                    light_pos_x: gr.light_screen_pos.0,
                    light_pos_y: gr.light_screen_pos.1,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<GodRaysSettings>();
            }
        }
    }
}

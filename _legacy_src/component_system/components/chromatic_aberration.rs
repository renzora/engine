//! Chromatic Aberration component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::post_process::effect_stack::ChromaticAberration;
use bevy_egui::egui;
use egui_phosphor::regular::RAINBOW;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::component_system::ChromaticAberrationData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(ChromaticAberrationData {
        type_id: "chromatic_aberration",
        display_name: "Chromatic Aberration",
        category: ComponentCategory::PostProcess,
        icon: RAINBOW,
        custom_inspector: inspect_chromatic_aberration,
    }));
}

fn inspect_chromatic_aberration(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut ca) = world.get_mut::<ChromaticAberrationData>(entity) else {
        return false;
    };

    sanitize_f32(&mut ca.intensity, 0.0, 0.5, 0.02);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, &crate::locale::t("comp.chromatic_aberration.intensity"), |ui| {
        ui.add(egui::DragValue::new(&mut ca.intensity).speed(0.001).range(0.0..=0.5)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Max Samples", |ui| {
        ui.add(egui::DragValue::new(&mut ca.max_samples).speed(1).range(1..=64)).changed()
    });

    changed
}

pub(crate) fn sync_chromatic_aberration(
    mut commands: Commands,
    query: Query<
        (&ChromaticAberrationData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<ChromaticAberrationData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<ChromaticAberrationData>>,
    mut removed: RemovedComponents<ChromaticAberrationData>,
) {
    let had_removals = removed.read().count() > 0;
    if query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<ChromaticAberration>();
        }
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((ca, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("chromatic_aberration"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(ChromaticAberration {
                    intensity: ca.intensity,
                    max_samples: ca.max_samples,
                    color_lut: None,
                });
            } else {
                commands.entity(cam).remove::<ChromaticAberration>();
            }
        }
    }
}

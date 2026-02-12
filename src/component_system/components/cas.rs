//! CAS (Contrast Adaptive Sharpening) component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening;
use bevy_egui::egui;
use egui_phosphor::regular::DIAMOND;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::CasData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(CasData {
        type_id: "cas",
        display_name: "Sharpening (CAS)",
        category: ComponentCategory::PostProcess,
        icon: DIAMOND,
        custom_inspector: inspect_cas,
    }));
}

fn inspect_cas(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut cas) = world.get_mut::<CasData>(entity) else {
        return false;
    };

    sanitize_f32(&mut cas.sharpening_strength, 0.0, 1.0, 0.6);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Strength", |ui| {
        ui.add(egui::DragValue::new(&mut cas.sharpening_strength).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Denoise", |ui| {
        ui.checkbox(&mut cas.denoise, "").changed()
    });

    changed
}

pub(crate) fn sync_cas(
    mut commands: Commands,
    query: Query<
        (&CasData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<CasData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if query.is_empty() {
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((cas, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("cas"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(ContrastAdaptiveSharpening {
                    enabled: true,
                    sharpening_strength: cas.sharpening_strength,
                    denoise: cas.denoise,
                });
            } else {
                commands.entity(cam).remove::<ContrastAdaptiveSharpening>();
            }
        }
    }
}

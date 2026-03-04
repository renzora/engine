//! Palette Quantization (color reduction) effect component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::SWATCHES;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::post_process::PaletteQuantizationSettings;
use crate::register_component;
use crate::component_system::PaletteQuantizationData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(PaletteQuantizationData {
        type_id: "palette_quantization",
        display_name: "Palette Quantization",
        category: ComponentCategory::PostProcess,
        icon: SWATCHES,
        custom_inspector: inspect_palette_quantization,
    }));
}

fn inspect_palette_quantization(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut pq) = world.get_mut::<PaletteQuantizationData>(entity) else {
        return false;
    };

    sanitize_f32(&mut pq.dithering, 0.0, 2.0, 0.5);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Colors", |ui| {
        ui.add(egui::DragValue::new(&mut pq.num_colors).speed(1).range(2..=256)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Dithering", |ui| {
        ui.add(egui::DragValue::new(&mut pq.dithering).speed(0.01).range(0.0..=2.0)).changed()
    });

    changed
}

pub(crate) fn sync_palette_quantization(
    mut commands: Commands,
    query: Query<
        (&PaletteQuantizationData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<PaletteQuantizationData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<PaletteQuantizationData>>,
    mut removed: RemovedComponents<PaletteQuantizationData>,
) {
    let had_removals = removed.read().count() > 0;
    if query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<PaletteQuantizationSettings>();
        }
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((pq, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("palette_quantization"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(PaletteQuantizationSettings {
                    num_colors: pq.num_colors,
                    dithering: pq.dithering,
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<PaletteQuantizationSettings>();
            }
        }
    }
}

//! Tonemapping component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::Tonemapping as BevyTonemapping;
use bevy_egui::egui;
use egui_phosphor::regular::PALETTE;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::{TonemappingData, TonemappingMode};
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(TonemappingData {
        type_id: "tonemapping",
        display_name: "Tonemapping",
        category: ComponentCategory::PostProcess,
        icon: PALETTE,
        custom_inspector: inspect_tonemapping,
    }));
}

fn inspect_tonemapping(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut tm) = world.get_mut::<TonemappingData>(entity) else {
        return false;
    };

    sanitize_f32(&mut tm.ev100, 0.0, 16.0, 9.7);

    let mut changed = false;
    let mut row = 0;

    inline_property(ui, row, "Mode", |ui| {
        let tonemap_options = ["None", "Reinhard", "ACES", "AgX", "Filmic"];
        let mut tonemap_index = match tm.mode {
            TonemappingMode::None => 0,
            TonemappingMode::Reinhard | TonemappingMode::ReinhardLuminance => 1,
            TonemappingMode::AcesFitted => 2,
            TonemappingMode::AgX => 3,
            TonemappingMode::BlenderFilmic | TonemappingMode::SomewhatBoringDisplayTransform | TonemappingMode::TonyMcMapface => 4,
        };
        egui::ComboBox::from_id_salt("tm_tonemap_combo")
            .selected_text(tonemap_options[tonemap_index])
            .show_ui(ui, |ui| {
                for (i, option) in tonemap_options.iter().enumerate() {
                    if ui.selectable_value(&mut tonemap_index, i, *option).changed() {
                        tm.mode = match tonemap_index {
                            0 => TonemappingMode::None,
                            1 => TonemappingMode::Reinhard,
                            2 => TonemappingMode::AcesFitted,
                            3 => TonemappingMode::AgX,
                            4 => TonemappingMode::BlenderFilmic,
                            _ => TonemappingMode::Reinhard,
                        };
                        changed = true;
                    }
                }
            });
    });
    row += 1;

    changed |= inline_property(ui, row, "EV100", |ui| {
        ui.add(egui::DragValue::new(&mut tm.ev100).speed(0.1).range(0.0..=16.0)).changed()
    });

    changed
}

pub(crate) fn sync_tonemapping(
    mut commands: Commands,
    tm_query: Query<
        (&TonemappingData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<TonemappingData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if tm_query.is_empty() {
        return;
    }

    let active = tm_query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((tm, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("tonemapping"));
        let effective = if disabled { &TonemappingData::default() } else { tm };

        let bevy_tonemap = match effective.mode {
            TonemappingMode::None => BevyTonemapping::None,
            TonemappingMode::Reinhard => BevyTonemapping::Reinhard,
            TonemappingMode::ReinhardLuminance => BevyTonemapping::ReinhardLuminance,
            TonemappingMode::AcesFitted => BevyTonemapping::AcesFitted,
            TonemappingMode::AgX => BevyTonemapping::AgX,
            TonemappingMode::SomewhatBoringDisplayTransform => BevyTonemapping::SomewhatBoringDisplayTransform,
            TonemappingMode::TonyMcMapface => BevyTonemapping::TonyMcMapface,
            TonemappingMode::BlenderFilmic => BevyTonemapping::BlenderFilmic,
        };

        for cam in cameras.iter() {
            commands.entity(cam).insert(bevy_tonemap.clone());
            commands.entity(cam).insert(Exposure { ev100: effective.ev100 });
        }
    }
}

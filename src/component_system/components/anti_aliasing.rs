//! Anti-aliasing component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::anti_alias::fxaa::Fxaa;
use bevy_egui::egui;
use egui_phosphor::regular::SHIELD_CHECK;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::component_system::AntiAliasingData;
use crate::ui::inline_property;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(AntiAliasingData {
        type_id: "anti_aliasing",
        display_name: "Anti-Aliasing",
        category: ComponentCategory::PostProcess,
        icon: SHIELD_CHECK,
        custom_inspector: inspect_anti_aliasing,
    }));
}

fn inspect_anti_aliasing(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut aa) = world.get_mut::<AntiAliasingData>(entity) else {
        return false;
    };

    let mut changed = false;
    let mut row = 0;

    inline_property(ui, row, &crate::locale::t("comp.anti_aliasing.mode"), |ui| {
        let msaa_options = [
            crate::locale::t("comp.anti_aliasing.none"),
            crate::locale::t("comp.anti_aliasing.msaa_2x"),
            crate::locale::t("comp.anti_aliasing.msaa_4x"),
            crate::locale::t("comp.anti_aliasing.msaa_8x"),
        ];
        let mut msaa_index = match aa.msaa_samples {
            1 => 0, 2 => 1, 4 => 2, 8 => 3, _ => 2,
        };
        egui::ComboBox::from_id_salt("aa_msaa_combo")
            .selected_text(&msaa_options[msaa_index])
            .show_ui(ui, |ui| {
                for (i, option) in msaa_options.iter().enumerate() {
                    if ui.selectable_value(&mut msaa_index, i, option).changed() {
                        aa.msaa_samples = match msaa_index {
                            0 => 1, 1 => 2, 2 => 4, 3 => 8, _ => 4,
                        };
                        changed = true;
                    }
                }
            });
    });
    row += 1;

    changed |= inline_property(ui, row, &crate::locale::t("comp.anti_aliasing.fxaa"), |ui| {
        ui.checkbox(&mut aa.fxaa_enabled, "").changed()
    });

    changed
}

pub(crate) fn sync_anti_aliasing(
    mut commands: Commands,
    aa_query: Query<
        (&AntiAliasingData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<AntiAliasingData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<AntiAliasingData>>,
    mut removed: RemovedComponents<AntiAliasingData>,
) {
    let had_removals = removed.read().count() > 0;
    if aa_query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<Fxaa>();
        }
        return;
    }

    let active = aa_query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((aa, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("anti_aliasing"));
        for cam in cameras.iter() {
            if !disabled && aa.fxaa_enabled {
                commands.entity(cam).insert(Fxaa::default());
            } else {
                commands.entity(cam).remove::<Fxaa>();
            }
        }
    }
}

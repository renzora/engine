//! SMAA (Subpixel Morphological Anti-Aliasing) component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::anti_alias::smaa::{Smaa as BevySmaa, SmaaPreset};
use bevy_egui::egui;
use egui_phosphor::regular::SHIELD_CHECK;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::{SmaaData, SmaaPresetMode};
use crate::ui::inline_property;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(SmaaData {
        type_id: "smaa",
        display_name: "SMAA",
        category: ComponentCategory::PostProcess,
        icon: SHIELD_CHECK,
        custom_inspector: inspect_smaa,
    }));
}

fn inspect_smaa(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut smaa) = world.get_mut::<SmaaData>(entity) else {
        return false;
    };

    let mut changed = false;

    inline_property(ui, 0, "Preset", |ui| {
        let options = ["Low", "Medium", "High", "Ultra"];
        let mut idx = match smaa.preset {
            SmaaPresetMode::Low => 0,
            SmaaPresetMode::Medium => 1,
            SmaaPresetMode::High => 2,
            SmaaPresetMode::Ultra => 3,
        };
        egui::ComboBox::from_id_salt("smaa_preset_combo")
            .selected_text(options[idx])
            .show_ui(ui, |ui| {
                for (i, option) in options.iter().enumerate() {
                    if ui.selectable_value(&mut idx, i, *option).changed() {
                        smaa.preset = match idx {
                            0 => SmaaPresetMode::Low,
                            1 => SmaaPresetMode::Medium,
                            2 => SmaaPresetMode::High,
                            _ => SmaaPresetMode::Ultra,
                        };
                        changed = true;
                    }
                }
            });
    });

    changed
}

pub(crate) fn sync_smaa(
    mut commands: Commands,
    query: Query<
        (&SmaaData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<SmaaData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<SmaaData>>,
    mut removed: RemovedComponents<SmaaData>,
) {
    let had_removals = removed.read().count() > 0;
    if query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<BevySmaa>();
        }
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((smaa, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("smaa"));
        for cam in cameras.iter() {
            if !disabled {
                let preset = match smaa.preset {
                    SmaaPresetMode::Low => SmaaPreset::Low,
                    SmaaPresetMode::Medium => SmaaPreset::Medium,
                    SmaaPresetMode::High => SmaaPreset::High,
                    SmaaPresetMode::Ultra => SmaaPreset::Ultra,
                };
                commands.entity(cam).insert(BevySmaa { preset });
            } else {
                commands.entity(cam).remove::<BevySmaa>();
            }
        }
    }
}

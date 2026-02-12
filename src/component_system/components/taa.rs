//! TAA (Temporal Anti-Aliasing) component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy_egui::egui;
use egui_phosphor::regular::SHIELD_CHECK;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::TaaData;
use crate::ui::inline_property;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(TaaData {
        type_id: "taa",
        display_name: "TAA",
        category: ComponentCategory::PostProcess,
        icon: SHIELD_CHECK,
        custom_inspector: inspect_taa,
    }));
}

fn inspect_taa(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut taa) = world.get_mut::<TaaData>(entity) else {
        return false;
    };

    let mut changed = false;

    changed |= inline_property(ui, 0, "Reset", |ui| {
        ui.checkbox(&mut taa.reset, "").changed()
    });

    changed
}

pub(crate) fn sync_taa(
    mut commands: Commands,
    query: Query<
        (&TaaData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<TaaData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<TaaData>>,
    mut removed: RemovedComponents<TaaData>,
) {
    let had_removals = removed.read().count() > 0;
    if query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<TemporalAntiAliasing>();
        }
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((taa, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("taa"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(TemporalAntiAliasing {
                    reset: taa.reset,
                });
            } else {
                commands.entity(cam).remove::<TemporalAntiAliasing>();
            }
        }
    }
}

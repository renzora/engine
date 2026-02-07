//! Reflections component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::pbr::ScreenSpaceReflections;
use bevy_egui::egui;
use egui_phosphor::regular::SPARKLE;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::ReflectionsData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(ReflectionsData {
        type_id: "reflections",
        display_name: "Reflections",
        category: ComponentCategory::PostProcess,
        icon: SPARKLE,
        custom_inspector: inspect_reflections,
    }));
}

fn inspect_reflections(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut ssr) = world.get_mut::<ReflectionsData>(entity) else {
        return false;
    };

    sanitize_f32(&mut ssr.intensity, 0.0, 1.0, 0.5);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut ssr.intensity).speed(0.1).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Max Steps", |ui| {
        let mut steps = ssr.max_steps as i32;
        let resp = ui.add(egui::DragValue::new(&mut steps).range(16..=256)).changed();
        if resp { ssr.max_steps = steps as u32; }
        resp
    });

    changed
}

pub(crate) fn sync_reflections(
    mut commands: Commands,
    ssr_query: Query<
        (&ReflectionsData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<ReflectionsData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if ssr_query.is_empty() {
        return;
    }

    let active = ssr_query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((ssr, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("reflections"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(ScreenSpaceReflections::default());
            } else {
                commands.entity(cam).remove::<ScreenSpaceReflections>();
            }
        }
    }
}

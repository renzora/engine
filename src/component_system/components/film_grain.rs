//! Film Grain effect component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::FILM_STRIP;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::post_process::FilmGrainSettings;
use crate::register_component;
use crate::shared::FilmGrainData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(FilmGrainData {
        type_id: "film_grain",
        display_name: "Film Grain",
        category: ComponentCategory::PostProcess,
        icon: FILM_STRIP,
        custom_inspector: inspect_film_grain,
    }));
}

fn inspect_film_grain(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut fg) = world.get_mut::<FilmGrainData>(entity) else {
        return false;
    };

    sanitize_f32(&mut fg.intensity, 0.0, 1.0, 0.1);
    sanitize_f32(&mut fg.grain_size, 0.1, 10.0, 1.0);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut fg.intensity).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Grain Size", |ui| {
        ui.add(egui::DragValue::new(&mut fg.grain_size).speed(0.1).range(0.1..=10.0)).changed()
    });

    changed
}

pub(crate) fn sync_film_grain(
    mut commands: Commands,
    query: Query<
        (&FilmGrainData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<FilmGrainData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    has_data: Query<(), With<FilmGrainData>>,
    mut removed: RemovedComponents<FilmGrainData>,
    time: Res<Time>,
) {
    let had_removals = removed.read().count() > 0;
    if query.is_empty() && !had_removals {
        return;
    }
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<FilmGrainSettings>();
        }
        return;
    }

    let active = query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((fg, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("film_grain"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(FilmGrainSettings {
                    intensity: fg.intensity,
                    grain_size: fg.grain_size,
                    time: time.elapsed_secs(),
                    ..default()
                });
            } else {
                commands.entity(cam).remove::<FilmGrainSettings>();
            }
        }
    }
}

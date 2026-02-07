//! Motion blur component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy::post_process::motion_blur::MotionBlur;
use bevy_egui::egui;
use egui_phosphor::regular::FAST_FORWARD;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::MotionBlurData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(MotionBlurData {
        type_id: "motion_blur",
        display_name: "Motion Blur",
        category: ComponentCategory::PostProcess,
        icon: FAST_FORWARD,
        custom_inspector: inspect_motion_blur,
    }));
}

fn inspect_motion_blur(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut mb) = world.get_mut::<MotionBlurData>(entity) else {
        return false;
    };

    sanitize_f32(&mut mb.intensity, 0.0, 1.0, 0.5);

    let mut changed = false;
    let row = 0;

    changed |= inline_property(ui, row, "Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut mb.intensity).speed(0.01).range(0.0..=1.0)).changed()
    });

    changed
}

pub(crate) fn sync_motion_blur(
    mut commands: Commands,
    mb_query: Query<
        (&MotionBlurData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<MotionBlurData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
) {
    if mb_query.is_empty() {
        return;
    }

    let active = mb_query.iter().find(|(_, editor, _)| editor.visible);

    if let Some((mb, _editor, dc)) = active {
        let disabled = dc.map_or(false, |d| d.is_disabled("motion_blur"));
        for cam in cameras.iter() {
            if !disabled {
                commands.entity(cam).insert(MotionBlur {
                    shutter_angle: mb.intensity * 360.0,
                    samples: 4,
                });
            } else {
                commands.entity(cam).remove::<MotionBlur>();
            }
        }
    }
}

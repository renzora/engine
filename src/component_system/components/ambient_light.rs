//! Ambient light component — registration, inspector, and sync system.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::SUN_DIM;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{EditorEntity, ViewportCamera};
use crate::register_component;
use crate::component_system::AmbientLightData;
use crate::ui::inline_property;
use crate::ui::inspectors::sanitize_f32;

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(AmbientLightData {
        type_id: "ambient_light",
        display_name: "Ambient Light",
        category: ComponentCategory::Lighting,
        icon: SUN_DIM,
        custom_inspector: inspect_ambient_light,
    }));
}

fn inspect_ambient_light(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<AmbientLightData>(entity) else {
        return false;
    };

    sanitize_f32(&mut data.brightness, 0.0, 1000.0, 300.0);

    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Color", |ui| {
        let mut color = egui::Color32::from_rgb(
            (data.color.0 * 255.0) as u8,
            (data.color.1 * 255.0) as u8,
            (data.color.2 * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            data.color = (
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });
    row += 1;

    changed |= inline_property(ui, row, "Brightness", |ui| {
        ui.add(egui::DragValue::new(&mut data.brightness).speed(10.0).range(0.0..=1000.0)).changed()
    });

    changed
}

pub(crate) fn sync_ambient_light(
    ambient_query: Query<
        (&AmbientLightData, &EditorEntity),
        Or<(Changed<AmbientLightData>, Changed<EditorEntity>)>,
    >,
    all_ambient: Query<(&AmbientLightData, &EditorEntity)>,
    mut ambient_light: ResMut<GlobalAmbientLight>,
) {
    if ambient_query.is_empty() {
        return;
    }

    if let Some((data, _)) = all_ambient.iter().find(|(_, editor)| editor.visible) {
        ambient_light.color = Color::srgb(data.color.0, data.color.1, data.color.2);
        ambient_light.brightness = data.brightness;
    }
}

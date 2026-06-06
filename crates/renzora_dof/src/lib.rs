use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor::{AppEditorExt, InspectorEntry},
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DepthOfFieldSettings {
    /// 0 = Gaussian, 1 = Bokeh
    pub mode: u32,
    pub focal_distance: f32,
    pub aperture_f_stops: f32,
    pub max_circle_of_confusion_diameter: f32,
    pub enabled: bool,
}

impl Default for DepthOfFieldSettings {
    fn default() -> Self {
        Self {
            mode: 0,
            focal_distance: 10.0,
            aperture_f_stops: 1.0,
            max_circle_of_confusion_diameter: 64.0,
            enabled: true,
        }
    }
}

fn sync_dof(
    mut commands: Commands,
    sources: Query<(Entity, Ref<DepthOfFieldSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    found = true;
                    break;
                }
                if settings.enabled {
                    let mode = match settings.mode {
                        1 => DepthOfFieldMode::Bokeh,
                        _ => DepthOfFieldMode::Gaussian,
                    };
                    commands.entity(*target).insert(DepthOfField {
                        mode,
                        focal_distance: settings.focal_distance,
                        sensor_height: 0.01866,
                        aperture_f_stops: settings.aperture_f_stops,
                        max_circle_of_confusion_diameter: settings.max_circle_of_confusion_diameter,
                        max_depth: f32::INFINITY,
                    });
                } else {
                    commands.entity(*target).remove::<DepthOfField>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<DepthOfField>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "depth_of_field",
        display_name: "Depth of Field",
        icon: regular::CAMERA,
        category: "rendering",
        has_fn: |world, entity| world.get::<DepthOfFieldSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(DepthOfFieldSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(DepthOfFieldSettings, DepthOfField)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<DepthOfFieldSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) {
                s.enabled = val;
            }
        }),
        // Declarative fields render natively (bevy_ui).
        fields: vec![
            renzora_editor::enum_u32_field!("Mode", DepthOfFieldSettings, mode, ["Gaussian", "Bokeh"]),
            renzora_editor::float_field!("Focal Distance", DepthOfFieldSettings, focal_distance, 0.1, 0.1, 1000.0),
            renzora_editor::float_field!("Aperture", DepthOfFieldSettings, aperture_f_stops, 0.1, 0.1, 64.0),
            renzora_editor::float_field!("Max CoC", DepthOfFieldSettings, max_circle_of_confusion_diameter, 1.0, 1.0, 256.0),
        ],
        custom_ui_fn: None,
    }
}

fn cleanup_dof(
    mut commands: Commands,
    mut removed: RemovedComponents<DepthOfFieldSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<DepthOfField>();
            }
        }
    }
}

#[derive(Default)]
pub struct DepthOfFieldPlugin;

impl Plugin for DepthOfFieldPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] DepthOfFieldPlugin");
        app.register_type::<DepthOfFieldSettings>();
        app.add_systems(Update, (sync_dof, cleanup_dof));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(DepthOfFieldPlugin);

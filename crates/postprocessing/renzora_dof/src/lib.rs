use bevy::prelude::*;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
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
    query: Query<(Entity, &DepthOfFieldSettings), Changed<DepthOfFieldSettings>>,
    render_target: Res<renzora_core::RenderTarget>,
) {
    for (entity, settings) in &query {
        let target = render_target.0.unwrap_or(entity);
        if settings.enabled {
            let mode = match settings.mode {
                1 => DepthOfFieldMode::Bokeh,
                _ => DepthOfFieldMode::Gaussian,
            };
            commands.entity(target).insert(DepthOfField {
                mode,
                focal_distance: settings.focal_distance,
                sensor_height: 0.01866,
                aperture_f_stops: settings.aperture_f_stops,
                max_circle_of_confusion_diameter: settings.max_circle_of_confusion_diameter,
                max_depth: f32::INFINITY,
            });
        } else {
            commands.entity(target).remove::<DepthOfField>();
        }
    }
}

#[cfg(feature = "editor")]
const DOF_MODE_LABELS: [&str; 2] = ["Gaussian", "Bokeh"];

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "depth_of_field",
        display_name: "Depth of Field",
        icon: regular::CAMERA,
        category: "rendering",
        has_fn: |world, entity| world.get::<DepthOfFieldSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(DepthOfFieldSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(DepthOfFieldSettings, DepthOfField)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<DepthOfFieldSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(dof_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn dof_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<DepthOfFieldSettings>(entity) else {
        return;
    };

    let mut row = 0;

    // Mode
    let current = settings.mode as usize;
    inline_property(ui, row, "Mode", theme, |ui| {
        let mut new_idx = current;
        egui::ComboBox::from_id_salt("dof_mode")
            .selected_text(*DOF_MODE_LABELS.get(current).unwrap_or(&"Unknown"))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (i, label) in DOF_MODE_LABELS.iter().enumerate() {
                    if ui.selectable_value(&mut new_idx, i, *label).changed() {
                        let mode = new_idx as u32;
                        cmds.push(move |world: &mut World| {
                            if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) {
                                s.mode = mode;
                            }
                        });
                    }
                }
            });
    });
    row += 1;

    // Focal Distance
    let mut focal = settings.focal_distance;
    inline_property(ui, row, "Focal Distance", theme, |ui| {
        let orig = focal;
        ui.add(egui::DragValue::new(&mut focal).speed(0.1).range(0.1..=1000.0));
        if focal != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) { s.focal_distance = focal; }
            });
        }
    });
    row += 1;

    // Aperture
    let mut aperture = settings.aperture_f_stops;
    inline_property(ui, row, "Aperture", theme, |ui| {
        let orig = aperture;
        ui.add(egui::DragValue::new(&mut aperture).speed(0.1).range(0.1..=64.0));
        if aperture != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) { s.aperture_f_stops = aperture; }
            });
        }
    });
    row += 1;

    // Max CoC
    let mut coc = settings.max_circle_of_confusion_diameter;
    inline_property(ui, row, "Max CoC", theme, |ui| {
        let orig = coc;
        ui.add(egui::DragValue::new(&mut coc).speed(1.0).range(1.0..=256.0));
        if coc != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) { s.max_circle_of_confusion_diameter = coc; }
            });
        }
    });
}

fn cleanup_dof(
    mut commands: Commands,
    mut removed: RemovedComponents<DepthOfFieldSettings>,
    render_target: Res<renzora_core::RenderTarget>,
) {
    for entity in removed.read() {
        if let Some(target) = render_target.0 {
            if let Ok(mut ec) = commands.get_entity(target) {
                ec.remove::<DepthOfField>();
            }
        } else if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<DepthOfField>();
        }
    }
}

pub struct DepthOfFieldPlugin;

impl Plugin for DepthOfFieldPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DepthOfFieldSettings>();
        app.add_systems(Update, (sync_dof, cleanup_dof));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

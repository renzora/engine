use bevy::prelude::*;
use bevy::pbr::{DistanceFog, FogFalloff};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

/// Fog falloff mode:
/// 0 = Linear, 1 = Exponential, 2 = ExponentialSquared, 3 = Atmospheric
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DistanceFogSettings {
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub directional_light_color_r: f32,
    pub directional_light_color_g: f32,
    pub directional_light_color_b: f32,
    pub directional_light_exponent: f32,
    /// 0=Linear, 1=Exponential, 2=ExponentialSquared, 3=Atmospheric
    pub mode: u32,
    pub start: f32,
    pub end: f32,
    pub density: f32,
    pub extinction_r: f32,
    pub extinction_g: f32,
    pub extinction_b: f32,
    pub inscattering_r: f32,
    pub inscattering_g: f32,
    pub inscattering_b: f32,
    pub enabled: bool,
}

impl Default for DistanceFogSettings {
    fn default() -> Self {
        Self {
            color_r: 0.5,
            color_g: 0.5,
            color_b: 0.5,
            directional_light_color_r: 0.0,
            directional_light_color_g: 0.0,
            directional_light_color_b: 0.0,
            directional_light_exponent: 8.0,
            mode: 0,
            start: 10.0,
            end: 100.0,
            density: 0.02,
            extinction_r: 0.02,
            extinction_g: 0.02,
            extinction_b: 0.02,
            inscattering_r: 0.01,
            inscattering_g: 0.01,
            inscattering_b: 0.01,
            enabled: true,
        }
    }
}

fn sync_distance_fog(
    mut commands: Commands,
    query: Query<(Entity, &DistanceFogSettings), Changed<DistanceFogSettings>>,
) {
    for (entity, settings) in &query {
        if !settings.enabled {
            commands.entity(entity).remove::<DistanceFog>();
            continue;
        }
        let falloff = match settings.mode {
            1 => FogFalloff::Exponential { density: settings.density },
            2 => FogFalloff::ExponentialSquared { density: settings.density },
            3 => FogFalloff::Atmospheric {
                extinction: Vec3::new(settings.extinction_r, settings.extinction_g, settings.extinction_b),
                inscattering: Vec3::new(settings.inscattering_r, settings.inscattering_g, settings.inscattering_b),
            },
            _ => FogFalloff::Linear {
                start: settings.start,
                end: settings.end,
            },
        };
        commands.entity(entity).insert(DistanceFog {
            color: Color::srgb(settings.color_r, settings.color_g, settings.color_b),
            directional_light_color: Color::srgb(
                settings.directional_light_color_r,
                settings.directional_light_color_g,
                settings.directional_light_color_b,
            ),
            directional_light_exponent: settings.directional_light_exponent,
            falloff,
        });
    }
}

#[cfg(feature = "editor")]
const FOG_MODE_LABELS: [&str; 4] = ["Linear", "Exponential", "Exponential Squared", "Atmospheric"];

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "distance_fog",
        display_name: "Distance Fog",
        icon: regular::CLOUD_FOG,
        category: "rendering",
        has_fn: |world, entity| world.get::<DistanceFogSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(DistanceFogSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(DistanceFogSettings, DistanceFog)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<DistanceFogSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(fog_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn fog_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<DistanceFogSettings>(entity) else { return };
    let mut row = 0;

    // Color
    let mut color = [settings.color_r, settings.color_g, settings.color_b];
    inline_property(ui, row, "Color", theme, |ui| {
        let orig = color;
        if ui.color_edit_button_rgb(&mut color).changed() && color != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) {
                    s.color_r = color[0]; s.color_g = color[1]; s.color_b = color[2];
                }
            });
        }
    });
    row += 1;

    // Dir Light Color
    let mut dl_color = [settings.directional_light_color_r, settings.directional_light_color_g, settings.directional_light_color_b];
    inline_property(ui, row, "Dir Light Color", theme, |ui| {
        let orig = dl_color;
        if ui.color_edit_button_rgb(&mut dl_color).changed() && dl_color != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) {
                    s.directional_light_color_r = dl_color[0]; s.directional_light_color_g = dl_color[1]; s.directional_light_color_b = dl_color[2];
                }
            });
        }
    });
    row += 1;

    // Dir Light Exponent
    let mut exponent = settings.directional_light_exponent;
    inline_property(ui, row, "Light Exponent", theme, |ui| {
        let orig = exponent;
        ui.add(egui::DragValue::new(&mut exponent).speed(0.1).range(1.0..=64.0));
        if exponent != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.directional_light_exponent = exponent; }
            });
        }
    });
    row += 1;

    // Falloff mode
    let current_mode = settings.mode as usize;
    inline_property(ui, row, "Falloff", theme, |ui| {
        let mut new_mode = current_mode;
        egui::ComboBox::from_id_salt("fog_mode")
            .selected_text(*FOG_MODE_LABELS.get(current_mode).unwrap_or(&"Unknown"))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (i, label) in FOG_MODE_LABELS.iter().enumerate() {
                    if ui.selectable_value(&mut new_mode, i, *label).changed() {
                        let mode = new_mode as u32;
                        cmds.push(move |world: &mut World| {
                            if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.mode = mode; }
                        });
                    }
                }
            });
    });
    row += 1;

    // Mode-dependent fields
    match current_mode {
        0 => {
            let mut start = settings.start;
            inline_property(ui, row, "Start", theme, |ui| {
                let orig = start;
                ui.add(egui::DragValue::new(&mut start).speed(0.5).range(0.0..=10000.0));
                if start != orig {
                    cmds.push(move |world: &mut World| {
                        if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.start = start; }
                    });
                }
            });
            row += 1;

            let mut end = settings.end;
            inline_property(ui, row, "End", theme, |ui| {
                let orig = end;
                ui.add(egui::DragValue::new(&mut end).speed(0.5).range(0.0..=10000.0));
                if end != orig {
                    cmds.push(move |world: &mut World| {
                        if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.end = end; }
                    });
                }
            });
        }
        1 | 2 => {
            let mut density = settings.density;
            inline_property(ui, row, "Density", theme, |ui| {
                let orig = density;
                ui.add(egui::DragValue::new(&mut density).speed(0.001).range(0.0..=1.0));
                if density != orig {
                    cmds.push(move |world: &mut World| {
                        if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.density = density; }
                    });
                }
            });
        }
        3 => {
            let mut ext = [settings.extinction_r, settings.extinction_g, settings.extinction_b];
            inline_property(ui, row, "Extinction", theme, |ui| {
                let orig = ext;
                let w = ((ui.available_width() - 48.0) / 3.0).max(30.0);
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(egui::RichText::new("R").size(10.0).color(egui::Color32::from_rgb(230, 90, 90)));
                ui.add_sized([w, 16.0], egui::DragValue::new(&mut ext[0]).speed(0.001).range(0.0..=1.0));
                ui.label(egui::RichText::new("G").size(10.0).color(egui::Color32::from_rgb(130, 200, 90)));
                ui.add_sized([w, 16.0], egui::DragValue::new(&mut ext[1]).speed(0.001).range(0.0..=1.0));
                ui.label(egui::RichText::new("B").size(10.0).color(egui::Color32::from_rgb(90, 150, 230)));
                ui.add_sized([w, 16.0], egui::DragValue::new(&mut ext[2]).speed(0.001).range(0.0..=1.0));
                if ext != orig {
                    cmds.push(move |world: &mut World| {
                        if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) {
                            s.extinction_r = ext[0]; s.extinction_g = ext[1]; s.extinction_b = ext[2];
                        }
                    });
                }
            });
            row += 1;

            let mut ins = [settings.inscattering_r, settings.inscattering_g, settings.inscattering_b];
            inline_property(ui, row, "Inscattering", theme, |ui| {
                let orig = ins;
                let w = ((ui.available_width() - 48.0) / 3.0).max(30.0);
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(egui::RichText::new("R").size(10.0).color(egui::Color32::from_rgb(230, 90, 90)));
                ui.add_sized([w, 16.0], egui::DragValue::new(&mut ins[0]).speed(0.001).range(0.0..=1.0));
                ui.label(egui::RichText::new("G").size(10.0).color(egui::Color32::from_rgb(130, 200, 90)));
                ui.add_sized([w, 16.0], egui::DragValue::new(&mut ins[1]).speed(0.001).range(0.0..=1.0));
                ui.label(egui::RichText::new("B").size(10.0).color(egui::Color32::from_rgb(90, 150, 230)));
                ui.add_sized([w, 16.0], egui::DragValue::new(&mut ins[2]).speed(0.001).range(0.0..=1.0));
                if ins != orig {
                    cmds.push(move |world: &mut World| {
                        if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) {
                            s.inscattering_r = ins[0]; s.inscattering_g = ins[1]; s.inscattering_b = ins[2];
                        }
                    });
                }
            });
        }
        _ => {}
    }
}

fn cleanup_distance_fog(mut commands: Commands, mut removed: RemovedComponents<DistanceFogSettings>) {
    for entity in removed.read() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<DistanceFog>();
        }
    }
}

pub struct DistanceFogPlugin;

impl Plugin for DistanceFogPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DistanceFogSettings>();
        app.add_systems(Update, (sync_distance_fog, cleanup_distance_fog));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

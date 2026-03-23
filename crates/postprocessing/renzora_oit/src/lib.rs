use bevy::prelude::*;
use bevy::core_pipeline::oit::OrderIndependentTransparencySettings;
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
pub struct OitSettings {
    pub layer_count: i32,
    pub alpha_threshold: f32,
    pub enabled: bool,
}

impl Default for OitSettings {
    fn default() -> Self {
        Self {
            layer_count: 8,
            alpha_threshold: 0.0,
            enabled: true,
        }
    }
}

fn sync_oit(
    mut commands: Commands,
    sources: Query<(Entity, Ref<OitSettings>)>,
    routing: Res<renzora_core::EffectRouting>,
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
                    commands.entity(*target)
                        .insert(Msaa::Off)
                        .insert(OrderIndependentTransparencySettings {
                            layer_count: settings.layer_count,
                            alpha_threshold: settings.alpha_threshold,
                        });
                } else {
                    commands.entity(*target).remove::<OrderIndependentTransparencySettings>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<OrderIndependentTransparencySettings>();
            }
        }
    }
}

fn cleanup_oit(
    mut commands: Commands,
    mut removed: RemovedComponents<OitSettings>,
    routing: Res<renzora_core::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<OrderIndependentTransparencySettings>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn oit_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "oit",
        display_name: "OIT Transparency",
        icon: regular::STACK,
        category: "rendering",
        has_fn: |world, entity| world.get::<OitSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(OitSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(OitSettings, OrderIndependentTransparencySettings)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<OitSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<OitSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(oit_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn oit_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<OitSettings>(entity) else { return };
    let mut row = 0;

    let mut layers = settings.layer_count;
    inline_property(ui, row, "Layers", theme, |ui| {
        let orig = layers;
        ui.add(egui::DragValue::new(&mut layers).speed(1).range(1..=32));
        if layers != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<OitSettings>(entity) { s.layer_count = layers; }
            });
        }
    });
    row += 1;

    let mut threshold = settings.alpha_threshold;
    inline_property(ui, row, "Alpha Threshold", theme, |ui| {
        let orig = threshold;
        ui.add(egui::DragValue::new(&mut threshold).speed(0.01).range(0.0..=1.0));
        if threshold != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<OitSettings>(entity) { s.alpha_threshold = threshold; }
            });
        }
    });
}

pub struct OitPlugin;

impl Plugin for OitPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] OitPlugin");
        app.register_type::<OitSettings>();
        app.add_systems(Update, (sync_oit, cleanup_oit));
        #[cfg(feature = "editor")]
        app.register_inspector(oit_entry());
    }
}

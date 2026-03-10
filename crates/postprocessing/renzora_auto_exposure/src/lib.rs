use bevy::prelude::*;
use bevy::post_process::auto_exposure::AutoExposure;
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
pub struct AutoExposureSettings {
    pub speed_brighten: f32,
    pub speed_darken: f32,
    pub range_min: f32,
    pub range_max: f32,
    pub enabled: bool,
}

impl Default for AutoExposureSettings {
    fn default() -> Self {
        Self {
            speed_brighten: 3.0,
            speed_darken: 1.0,
            range_min: -8.0,
            range_max: 8.0,
            enabled: true,
        }
    }
}

fn sync_auto_exposure(
    mut commands: Commands,
    sources: Query<(Entity, Ref<AutoExposureSettings>)>,
    routing: Res<renzora_core::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() { found = true; break; }
                if settings.enabled {
                    commands.entity(*target).insert(AutoExposure {
                        range: settings.range_min..=settings.range_max,
                        speed_brighten: settings.speed_brighten,
                        speed_darken: settings.speed_darken,
                        ..default()
                    });
                } else {
                    commands.entity(*target).remove::<AutoExposure>();
                }
                found = true; break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) { ec.remove::<AutoExposure>(); }
        }
    }
}

fn cleanup_auto_exposure(
    mut commands: Commands,
    mut removed: RemovedComponents<AutoExposureSettings>,
    routing: Res<renzora_core::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) { ec.remove::<AutoExposure>(); }
        }
    }
}

#[cfg(feature = "editor")]
fn auto_exposure_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "auto_exposure",
        display_name: "Auto Exposure",
        icon: regular::SUN_DIM,
        category: "rendering",
        has_fn: |world, entity| world.get::<AutoExposureSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(AutoExposureSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(AutoExposureSettings, AutoExposure)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<AutoExposureSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<AutoExposureSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(auto_exposure_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn auto_exposure_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<AutoExposureSettings>(entity) else { return };
    let mut row = 0;

    let mut speed_b = settings.speed_brighten;
    inline_property(ui, row, "Speed Brighten", theme, |ui| {
        let orig = speed_b;
        ui.add(egui::DragValue::new(&mut speed_b).speed(0.1).range(0.1..=20.0));
        if speed_b != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<AutoExposureSettings>(entity) { s.speed_brighten = speed_b; }
            });
        }
    });
    row += 1;

    let mut speed_d = settings.speed_darken;
    inline_property(ui, row, "Speed Darken", theme, |ui| {
        let orig = speed_d;
        ui.add(egui::DragValue::new(&mut speed_d).speed(0.1).range(0.1..=20.0));
        if speed_d != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<AutoExposureSettings>(entity) { s.speed_darken = speed_d; }
            });
        }
    });
    row += 1;

    let mut rmin = settings.range_min;
    inline_property(ui, row, "Range Min (EV)", theme, |ui| {
        let orig = rmin;
        ui.add(egui::DragValue::new(&mut rmin).speed(0.1).range(-16.0..=0.0));
        if rmin != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<AutoExposureSettings>(entity) { s.range_min = rmin; }
            });
        }
    });
    row += 1;

    let mut rmax = settings.range_max;
    inline_property(ui, row, "Range Max (EV)", theme, |ui| {
        let orig = rmax;
        ui.add(egui::DragValue::new(&mut rmax).speed(0.1).range(0.0..=16.0));
        if rmax != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<AutoExposureSettings>(entity) { s.range_max = rmax; }
            });
        }
    });
}

pub struct AutoExposurePlugin;

impl Plugin for AutoExposurePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<AutoExposureSettings>();
        app.add_systems(Update, (sync_auto_exposure, cleanup_auto_exposure));
        #[cfg(feature = "editor")]
        app.register_inspector(auto_exposure_entry());
    }
}

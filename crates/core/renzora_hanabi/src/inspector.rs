//! Editor inspector registration for HanabiEffectData.

use bevy::prelude::*;
use bevy_egui::egui;
use renzora_editor::{EditorCommands, InspectorEntry, InspectorRegistry};
use renzora_theme::Theme;

use crate::data::*;

fn hanabi_has(world: &World, entity: Entity) -> bool {
    world.get::<HanabiEffectData>(entity).is_some()
}

fn hanabi_add(world: &mut World, entity: Entity) {
    world.entity_mut(entity).insert(HanabiEffectData::default());
}

fn hanabi_remove(world: &mut World, entity: Entity) {
    world.entity_mut(entity).remove::<HanabiEffectData>();
}

fn hanabi_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    _theme: &Theme,
) {
    let Some(data) = world.get::<HanabiEffectData>(entity) else {
        return;
    };

    // Runtime controls
    let playing = data.playing;
    let rate_mult = data.rate_multiplier;
    let scale_mult = data.scale_multiplier;
    let time_scale = data.time_scale;
    let tint = data.color_tint;

    // Source info
    let source_label = match &data.source {
        EffectSource::Asset { path } => format!("Asset: {}", if path.is_empty() { "(none)" } else { path.as_str() }),
        EffectSource::Inline { definition } => format!("Inline: {}", definition.name),
    };

    ui.label(format!("Source: {}", source_label));
    ui.separator();

    ui.label("Runtime Controls");

    // Playing toggle
    let mut new_playing = playing;
    if ui.checkbox(&mut new_playing, "Playing").changed() {
        cmds.push(move |world: &mut World| {
            if let Some(mut d) = world.get_mut::<HanabiEffectData>(entity) {
                d.playing = new_playing;
            }
        });
    }

    // Rate multiplier
    let mut new_rate = rate_mult;
    ui.horizontal(|ui| {
        ui.label("Rate Multiplier:");
        if ui.add(egui::DragValue::new(&mut new_rate).speed(0.05).range(0.0..=10.0)).changed() {
            cmds.push(move |world: &mut World| {
                if let Some(mut d) = world.get_mut::<HanabiEffectData>(entity) {
                    d.rate_multiplier = new_rate;
                }
            });
        }
    });

    // Scale multiplier
    let mut new_scale = scale_mult;
    ui.horizontal(|ui| {
        ui.label("Scale Multiplier:");
        if ui.add(egui::DragValue::new(&mut new_scale).speed(0.05).range(0.0..=10.0)).changed() {
            cmds.push(move |world: &mut World| {
                if let Some(mut d) = world.get_mut::<HanabiEffectData>(entity) {
                    d.scale_multiplier = new_scale;
                }
            });
        }
    });

    // Time scale
    let mut new_ts = time_scale;
    ui.horizontal(|ui| {
        ui.label("Time Scale:");
        if ui.add(egui::DragValue::new(&mut new_ts).speed(0.05).range(0.0..=5.0)).changed() {
            cmds.push(move |world: &mut World| {
                if let Some(mut d) = world.get_mut::<HanabiEffectData>(entity) {
                    d.time_scale = new_ts;
                }
            });
        }
    });

    // Color tint
    let mut rgba = egui::Rgba::from_rgba_premultiplied(tint[0], tint[1], tint[2], tint[3]);
    ui.horizontal(|ui| {
        ui.label("Color Tint:");
        if egui::color_picker::color_edit_button_rgba(ui, &mut rgba, egui::color_picker::Alpha::OnlyBlend).changed() {
            let new_tint = [rgba.r(), rgba.g(), rgba.b(), rgba.a()];
            cmds.push(move |world: &mut World| {
                if let Some(mut d) = world.get_mut::<HanabiEffectData>(entity) {
                    d.color_tint = new_tint;
                }
            });
        }
    });

    // Show effect details for inline sources
    if let EffectSource::Inline { definition } = &data.source {
        ui.separator();
        ui.collapsing("Effect Details", |ui| {
            ui.label(format!("Capacity: {}", definition.capacity));
            ui.label(format!("Spawn Mode: {:?}", definition.spawn_mode));
            ui.label(format!("Spawn Rate: {:.1}/s", definition.spawn_rate));
            ui.label(format!("Lifetime: {:.2} - {:.2}s", definition.lifetime_min, definition.lifetime_max));
            ui.label(format!("Emit Shape: {:?}", definition.emit_shape));
            ui.label(format!("Velocity Mode: {:?}", definition.velocity_mode));
            ui.label(format!("Alpha Mode: {:?}", definition.alpha_mode));
            ui.label(format!("Simulation Space: {:?}", definition.simulation_space));
        });
    }
}

fn register_inspector_system(world: &mut World) {
    let entry = InspectorEntry {
        type_id: "hanabi_effect",
        display_name: "Hanabi Effect",
        icon: egui_phosphor::regular::SPARKLE,
        category: "effects",
        has_fn: hanabi_has,
        add_fn: Some(hanabi_add),
        remove_fn: Some(hanabi_remove),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(hanabi_custom_ui),
    };

    if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
        registry.register(entry);
    }
}

pub fn register_inspector(app: &mut App) {
    app.add_systems(Startup, register_inspector_system);
}

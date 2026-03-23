//! Inspector entries for audio components (AudioPlayer, AudioListener).

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_audio::{AudioPlayer, AudioListener, MixerState};
use renzora_editor::{EditorCommands, FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry};
use renzora_theme::Theme;

pub fn register_audio_inspectors(registry: &mut InspectorRegistry) {
    registry.register(audio_player_entry());
    registry.register(audio_listener_entry());
}

fn audio_player_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    _theme: &Theme,
) {
    let Some(data) = world.get::<AudioPlayer>(entity) else {
        return;
    };

    // Clip
    let mut clip = data.clip.clone();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Clip").size(11.0));
        if ui.text_edit_singleline(&mut clip).changed() {
            let v = clip.clone();
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.clip = v;
                }
            });
        }
    });

    // Volume
    let mut volume = data.volume;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Volume").size(11.0));
        if ui.add(egui::Slider::new(&mut volume, 0.0..=2.0)).changed() {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.volume = volume;
                }
            });
        }
    });

    // Pitch
    let mut pitch = data.pitch;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Pitch").size(11.0));
        if ui.add(egui::Slider::new(&mut pitch, 0.1..=4.0)).changed() {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.pitch = pitch;
                }
            });
        }
    });

    // Panning
    let mut panning = data.panning;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Panning").size(11.0));
        if ui.add(egui::Slider::new(&mut panning, -1.0..=1.0)).changed() {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.panning = panning;
                }
            });
        }
    });

    // Bus dropdown — sourced from MixerState
    let current_bus = data.bus.clone();
    let mut buses = vec![
        "Master".to_string(),
        "Sfx".to_string(),
        "Music".to_string(),
        "Ambient".to_string(),
    ];
    if let Some(mixer) = world.get_resource::<MixerState>() {
        for (name, _) in &mixer.custom_buses {
            buses.push(name.clone());
        }
    }

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Bus").size(11.0));
        egui::ComboBox::from_id_salt(ui.id().with("audio_bus"))
            .selected_text(&current_bus)
            .show_ui(ui, |ui| {
                for bus_name in &buses {
                    if ui.selectable_label(*bus_name == current_bus, bus_name).clicked() {
                        let v = bus_name.clone();
                        commands.push(move |w| {
                            if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                                d.bus = v;
                            }
                        });
                    }
                }
            });
    });

    // Looping
    let mut looping = data.looping;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Looping").size(11.0));
        if ui.checkbox(&mut looping, "").changed() {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.looping = looping;
                }
            });
        }
    });

    // Autoplay
    let mut autoplay = data.autoplay;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Autoplay").size(11.0));
        if ui.checkbox(&mut autoplay, "").changed() {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.autoplay = autoplay;
                }
            });
        }
    });

    // Fade In
    let mut fade_in = data.fade_in;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Fade In").size(11.0));
        if ui.add(egui::DragValue::new(&mut fade_in).speed(0.05).range(0.0..=10.0).suffix("s")).changed() {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.fade_in = fade_in;
                }
            });
        }
    });

    // Spatial
    let mut spatial = data.spatial;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Spatial").size(11.0));
        if ui.checkbox(&mut spatial, "").changed() {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.spatial = spatial;
                }
            });
        }
    });

    if data.spatial {
        let mut min_dist = data.spatial_min_distance;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("  Min Distance").size(11.0));
            if ui.add(egui::DragValue::new(&mut min_dist).speed(0.1).range(0.01..=1000.0)).changed() {
                commands.push(move |w| {
                    if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                        d.spatial_min_distance = min_dist;
                    }
                });
            }
        });

        let mut max_dist = data.spatial_max_distance;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("  Max Distance").size(11.0));
            if ui.add(egui::DragValue::new(&mut max_dist).speed(0.5).range(0.1..=10000.0)).changed() {
                commands.push(move |w| {
                    if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                        d.spatial_max_distance = max_dist;
                    }
                });
            }
        });
    }

    // Reverb Send
    let mut reverb = data.reverb_send;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Reverb Send").size(11.0));
        if ui.add(egui::Slider::new(&mut reverb, 0.0..=1.0)).changed() {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.reverb_send = reverb;
                }
            });
        }
    });

    // Delay Send
    let mut delay = data.delay_send;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Delay Send").size(11.0));
        if ui.add(egui::Slider::new(&mut delay, 0.0..=1.0)).changed() {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.delay_send = delay;
                }
            });
        }
    });
}

fn audio_player_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "audio_player",
        display_name: "Audio Player",
        icon: regular::SPEAKER_HIGH,
        category: "Audio",
        has_fn: |world, entity| world.get::<AudioPlayer>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(AudioPlayer::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<AudioPlayer>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(audio_player_custom_ui),
    }
}

fn audio_listener_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "audio_listener",
        display_name: "Audio Listener",
        icon: regular::EAR,
        category: "Audio",
        has_fn: |world, entity| world.get::<AudioListener>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(AudioListener { active: true });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<AudioListener>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<AudioListener>(entity)
                .map(|l| l.active)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, enabled| {
            if let Some(mut l) = world.get_mut::<AudioListener>(entity) {
                l.active = enabled;
            }
        }),
        fields: vec![
            FieldDef {
                name: "Active",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world.get::<AudioListener>(entity)
                        .map(|l| FieldValue::Bool(l.active))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<AudioListener>(entity) {
                            l.active = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

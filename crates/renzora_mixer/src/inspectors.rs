//! Inspector entries for audio components (AudioPlayer, AudioListener).

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_audio::{AudioListener, AudioPlayer, MixerState, RolloffType};
use renzora_editor::{
    asset_drop_target, enum_combobox, inline_property, labeled_slider, section_header,
    toggle_switch, AssetDragPayload, EditorCommands, FieldDef, FieldType, FieldValue,
    InspectorEntry, InspectorRegistry, SliderConfig,
};
use renzora_theme::Theme;

pub fn register_audio_inspectors(registry: &mut InspectorRegistry) {
    registry.register(audio_player_entry());
    registry.register(audio_listener_entry());
}

/// Rolloff options, indexed to match the dropdown order below.
const ROLLOFF_LABELS: &[&str] = &["Logarithmic", "Linear"];

fn rolloff_to_index(r: &RolloffType) -> usize {
    match r {
        RolloffType::Logarithmic => 0,
        RolloffType::Linear => 1,
    }
}

fn rolloff_from_index(i: usize) -> RolloffType {
    match i {
        1 => RolloffType::Linear,
        _ => RolloffType::Logarithmic,
    }
}

fn audio_player_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(audio) = world.get::<AudioPlayer>(entity) else {
        return;
    };

    // Work on a local copy; non-clip fields are applied in one deferred write
    // at the end. The clip is handled separately (drag-drop / clear) and is
    // preserved by that final write so the two paths don't clobber each other.
    let mut data = audio.clone();
    let mut changed = false;
    let mut row = 0;

    // ----- Clip (drag an audio file from the asset browser onto this) -----
    section_header(ui, "Clip", theme);
    {
        let payload = world.get_resource::<AssetDragPayload>();
        let exts = ["ogg", "wav", "mp3", "flac"];
        let current = if data.clip.is_empty() {
            None
        } else {
            Some(data.clip.as_str())
        };
        let result = inline_property(ui, row, "File", theme, |ui| {
            asset_drop_target(
                ui,
                ui.id().with("audio_clip"),
                current,
                &exts,
                "Drag audio here",
                theme,
                payload,
            )
        });
        row += 1;

        if let Some(path) = result.dropped_path {
            let rel = world
                .get_resource::<renzora::core::CurrentProject>()
                .map(|p| p.make_asset_relative(&path))
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.clip = rel;
                }
            });
        }
        if result.cleared {
            commands.push(move |w| {
                if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                    d.clip.clear();
                }
            });
        }
    }

    // ----- Clip Pool (random one-shots fired by play_audio()) -----
    section_header(ui, "Clip Pool", theme);
    {
        let exts = ["ogg", "wav", "mp3", "flac"];

        // Existing pool entries: filename + remove button.
        for (i, clip) in data.clips.iter().enumerate() {
            let name = clip.rsplit(['/', '\\']).next().unwrap_or(clip).to_string();
            let remove = inline_property(ui, row, "", theme, |ui| {
                let mut remove = false;
                ui.horizontal(|ui| {
                    if ui.small_button(regular::TRASH).clicked() {
                        remove = true;
                    }
                    ui.label(egui::RichText::new(&name).size(11.0));
                });
                remove
            });
            row += 1;
            if remove {
                commands.push(move |w| {
                    if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                        if i < d.clips.len() {
                            d.clips.remove(i);
                        }
                    }
                });
            }
        }

        // Drop zone — appends every audio file dropped (multi-select works).
        let payload = world.get_resource::<AssetDragPayload>();
        let result = inline_property(ui, row, "Add", theme, |ui| {
            asset_drop_target(
                ui,
                ui.id().with("audio_clip_pool_add"),
                None,
                &exts,
                "Drag audio file(s) here",
                theme,
                payload,
            )
        });
        row += 1;
        if result.dropped_path.is_some() {
            if let Some(p) = payload {
                let project = world.get_resource::<renzora::core::CurrentProject>();
                let mut to_add: Vec<String> = Vec::new();
                for path in &p.paths {
                    let is_audio = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| {
                            let e = e.to_ascii_lowercase();
                            exts.iter().any(|x| *x == e)
                        })
                        .unwrap_or(false);
                    if is_audio {
                        let rel = project
                            .map(|pr| pr.make_asset_relative(path))
                            .unwrap_or_else(|| path.to_string_lossy().to_string());
                        to_add.push(rel);
                    }
                }
                if !to_add.is_empty() {
                    commands.push(move |w| {
                        if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                            d.clips.extend(to_add);
                        }
                    });
                }
            }
        }
    }

    // ----- Playback -----
    section_header(ui, "Playback", theme);
    if inline_property(ui, row, "Autoplay", theme, |ui| {
        toggle_switch(ui, ui.id().with("ap_autoplay"), data.autoplay)
    }) {
        data.autoplay = !data.autoplay;
        changed = true;
    }
    row += 1;

    if inline_property(ui, row, "Looping", theme, |ui| {
        toggle_switch(ui, ui.id().with("ap_looping"), data.looping)
    }) {
        data.looping = !data.looping;
        changed = true;
    }
    row += 1;

    changed |= inline_property(ui, row, "Fade In", theme, |ui| {
        ui.add(
            egui::DragValue::new(&mut data.fade_in)
                .speed(0.05)
                .range(0.0..=10.0)
                .suffix(" s"),
        )
        .changed()
    });
    row += 1;

    // ----- Mix -----
    section_header(ui, "Mix", theme);
    changed |= inline_property(ui, row, "Volume", theme, |ui| {
        labeled_slider(ui, &mut data.volume, SliderConfig::new(0.0, 2.0), theme).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Vol Jitter", theme, |ui| {
        labeled_slider(ui, &mut data.volume_jitter, SliderConfig::new(0.0, 1.0), theme).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Pitch", theme, |ui| {
        labeled_slider(ui, &mut data.pitch, SliderConfig::new(0.1, 4.0), theme).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Pitch Jitter", theme, |ui| {
        labeled_slider(ui, &mut data.pitch_jitter, SliderConfig::new(0.0, 0.5), theme).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Panning", theme, |ui| {
        labeled_slider(ui, &mut data.panning, SliderConfig::new(-1.0, 1.0), theme).changed()
    });
    row += 1;

    // Bus dropdown — built-in buses plus any custom mixer buses.
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
    let bus_refs: Vec<&str> = buses.iter().map(|s| s.as_str()).collect();
    let bus_idx = buses.iter().position(|b| *b == data.bus).unwrap_or(0);
    let picked_bus = inline_property(ui, row, "Bus", theme, |ui| {
        enum_combobox(ui, ui.id().with("ap_bus"), bus_idx, &bus_refs)
    });
    row += 1;
    if let Some(i) = picked_bus {
        data.bus = buses[i].clone();
        changed = true;
    }

    // ----- Spatial (3D) -----
    section_header(ui, "Spatial", theme);
    if inline_property(ui, row, "Enabled", theme, |ui| {
        toggle_switch(ui, ui.id().with("ap_spatial"), data.spatial)
    }) {
        data.spatial = !data.spatial;
        changed = true;
    }
    row += 1;

    if data.spatial {
        changed |= inline_property(ui, row, "Min Distance", theme, |ui| {
            ui.add(
                egui::DragValue::new(&mut data.spatial_min_distance)
                    .speed(0.1)
                    .range(0.01..=1000.0),
            )
            .changed()
        });
        row += 1;

        changed |= inline_property(ui, row, "Max Distance", theme, |ui| {
            ui.add(
                egui::DragValue::new(&mut data.spatial_max_distance)
                    .speed(0.5)
                    .range(0.1..=10000.0),
            )
            .changed()
        });
        row += 1;

        let rolloff_idx = rolloff_to_index(&data.spatial_rolloff);
        let picked_rolloff = inline_property(ui, row, "Rolloff", theme, |ui| {
            enum_combobox(ui, ui.id().with("ap_rolloff"), rolloff_idx, ROLLOFF_LABELS)
        });
        row += 1;
        if let Some(i) = picked_rolloff {
            data.spatial_rolloff = rolloff_from_index(i);
            changed = true;
        }
    }

    // ----- Sends (routing to global reverb / delay buses) -----
    section_header(ui, "Sends", theme);
    changed |= inline_property(ui, row, "Reverb", theme, |ui| {
        labeled_slider(ui, &mut data.reverb_send, SliderConfig::new(0.0, 1.0), theme).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Delay", theme, |ui| {
        labeled_slider(ui, &mut data.delay_send, SliderConfig::new(0.0, 1.0), theme).changed()
    });

    // Apply all non-clip edits in one write, preserving the live clip value
    // (which the drag-drop / clear commands above may have just changed).
    if changed {
        let new_data = data;
        commands.push(move |w| {
            if let Some(mut d) = w.get_mut::<AudioPlayer>(entity) {
                // Preserve the clip + pool, which the drag-drop/clear/remove
                // commands above manage independently of this bulk write.
                let clip = d.clip.clone();
                let clips = std::mem::take(&mut d.clips);
                *d = new_data;
                d.clip = clip;
                d.clips = clips;
            }
        });
    }
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
            world
                .entity_mut(entity)
                .insert(AudioListener { active: true });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<AudioListener>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<AudioListener>(entity)
                .map(|l| l.active)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, enabled| {
            if let Some(mut l) = world.get_mut::<AudioListener>(entity) {
                l.active = enabled;
            }
        }),
        fields: vec![FieldDef {
            name: "Active",
            field_type: FieldType::Bool,
            get_fn: |world, entity| {
                world
                    .get::<AudioListener>(entity)
                    .map(|l| FieldValue::Bool(l.active))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Bool(v) = val {
                    if let Some(mut l) = world.get_mut::<AudioListener>(entity) {
                        l.active = v;
                    }
                }
            },
        }],
        custom_ui_fn: None,
    }
}

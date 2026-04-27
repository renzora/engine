//! Inspector registration for AnimatorComponent.

use bevy::prelude::*;
use renzora_editor::InspectorEntry;

use crate::component::AnimatorComponent;

pub fn animator_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "animator",
        display_name: "Animator",
        icon: egui_phosphor::regular::PLAY,
        category: "animation",
        has_fn: |world, entity| world.get::<AnimatorComponent>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(AnimatorComponent::new());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<AnimatorComponent>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        custom_ui_fn: Some(animator_custom_ui),
        fields: vec![],
    }
}

fn animator_custom_ui(
    ui: &mut bevy_egui::egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &renzora_editor::EditorCommands,
    theme: &renzora_theme::Theme,
) {
    use bevy_egui::egui;

    let text_color = theme.text.primary.to_color32();
    let muted_color = theme.text.secondary.to_color32();
    let accent_color = theme.semantic.accent.to_color32();
    let surface_color = theme.surfaces.faint.to_color32();

    let Some(animator) = world.get::<AnimatorComponent>(entity) else {
        return;
    };

    // Blend duration
    let mut blend = animator.blend_duration;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Blend Duration").size(11.0).color(muted_color));
        let response = ui.add(
            egui::DragValue::new(&mut blend)
                .speed(0.01)
                .range(0.0..=5.0)
                .suffix("s"),
        );
        if response.changed() {
            let v = blend;
            cmds.push(move |world: &mut World| {
                if let Some(mut a) = world.get_mut::<AnimatorComponent>(entity) {
                    a.blend_duration = v;
                }
            });
        }
    });

    ui.add_space(4.0);

    // Default clip
    let default_clip = animator.default_clip.clone();
    let clip_names: Vec<String> = animator.clips.iter().map(|c| c.name.clone()).collect();

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Default Clip").size(11.0).color(muted_color));
        let current_label = default_clip.as_deref().unwrap_or("(none)");
        egui::ComboBox::from_id_salt("animator_default_clip")
            .selected_text(current_label)
            .width(ui.available_width() - 4.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(default_clip.is_none(), "(none)").clicked() {
                    cmds.push(move |world: &mut World| {
                        if let Some(mut a) = world.get_mut::<AnimatorComponent>(entity) {
                            a.default_clip = None;
                        }
                    });
                }
                for name in &clip_names {
                    let selected = default_clip.as_deref() == Some(name.as_str());
                    if ui.selectable_label(selected, name).clicked() {
                        let n = name.clone();
                        let n2 = name.clone();
                        cmds.push(move |world: &mut World| {
                            if let Some(mut a) = world.get_mut::<AnimatorComponent>(entity) {
                                a.default_clip = Some(n);
                            }
                            // Also play the clip immediately
                            if let Some(mut queue) = world.get_resource_mut::<crate::AnimationCommandQueue>() {
                                queue.commands.push(crate::systems::AnimationCommand::Play {
                                    entity,
                                    name: n2,
                                    looping: true,
                                    speed: 1.0,
                                });
                            }
                        });
                    }
                }
            });
    });

    ui.add_space(6.0);

    // Clips list
    ui.label(egui::RichText::new("Animation Clips").size(11.0).color(text_color));
    ui.add_space(2.0);

    let clips = animator.clips.clone();
    let mut remove_idx: Option<usize> = None;

    if clips.is_empty() {
        ui.label(egui::RichText::new("No clips assigned").size(10.0).color(muted_color));
    }

    for (i, slot) in clips.iter().enumerate() {
        let frame = egui::Frame::new()
            .fill(surface_color)
            .corner_radius(3.0)
            .inner_margin(egui::Margin::same(4));

        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                // Clip name — editable. Persist the edit buffer across frames
                // in egui memory so keystrokes aren't lost when the inspector
                // rebuilds each frame. Commit on focus loss or Enter.
                let edit_id = egui::Id::new(("anim_clip_name", entity, i));
                let mut name_buf: String = ui.data(|d| d.get_temp::<String>(edit_id)).unwrap_or_else(|| slot.name.clone());
                // If the underlying slot name changed externally (e.g. undo),
                // reset the buffer unless this field is currently focused.
                let is_focused = ui.memory(|m| m.focused()) == Some(edit_id);
                if !is_focused && name_buf != slot.name {
                    name_buf = slot.name.clone();
                }
                let response = ui.add(
                    egui::TextEdit::singleline(&mut name_buf)
                        .id(edit_id)
                        .desired_width(140.0)
                        .font(egui::FontId::proportional(11.0)),
                );
                ui.data_mut(|d| d.insert_temp(edit_id, name_buf.clone()));

                let committed = response.lost_focus();
                if committed && name_buf != slot.name {
                    let trimmed = name_buf.trim().to_string();
                    let original = slot.name.clone();
                    if !trimmed.is_empty() && !clips.iter().enumerate().any(|(j, s)| j != i && s.name == trimmed) {
                        cmds.push(move |world: &mut World| {
                            if let Some(mut a) = world.get_mut::<AnimatorComponent>(entity) {
                                if let Some(slot) = a.clips.get_mut(i) {
                                    slot.name = trimmed.clone();
                                }
                                if a.default_clip.as_deref() == Some(original.as_str()) {
                                    a.default_clip = Some(trimmed);
                                }
                            }
                        });
                    } else {
                        // Rejected — reset the persisted buffer so it reverts visually.
                        ui.data_mut(|d| d.remove::<String>(edit_id));
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Remove button
                    if ui.small_button(
                        egui::RichText::new(egui_phosphor::regular::TRASH)
                            .size(11.0)
                            .color(muted_color),
                    ).clicked() {
                        remove_idx = Some(i);
                    }

                    // Looping indicator
                    let loop_color = if slot.looping { accent_color } else { muted_color };
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::REPEAT)
                            .size(10.0)
                            .color(loop_color),
                    );

                    // Speed
                    ui.label(
                        egui::RichText::new(format!("{:.1}x", slot.speed))
                            .size(10.0)
                            .color(muted_color),
                    );
                });
            });

            // Path (read-only)
            ui.label(egui::RichText::new(&slot.path).size(9.0).color(muted_color));
        });

        ui.add_space(2.0);
    }

    if let Some(idx) = remove_idx {
        cmds.push(move |world: &mut World| {
            if let Some(mut a) = world.get_mut::<AnimatorComponent>(entity) {
                if idx < a.clips.len() {
                    a.clips.remove(idx);
                }
            }
        });
    }

    ui.add_space(4.0);

    // Drop zone for adding new .anim files — fixed height so it doesn't
    // stretch to fill the inspector.
    const DROP_ZONE_HEIGHT: f32 = 32.0;
    let drop_frame = egui::Frame::new()
        .fill(surface_color)
        .stroke(egui::Stroke::new(1.0, muted_color))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::same(6));

    let drop_response = drop_frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(DROP_ZONE_HEIGHT);
        ui.set_max_height(DROP_ZONE_HEIGHT);
        ui.horizontal_centered(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} Drop .anim files here",
                    egui_phosphor::regular::PLUS_CIRCLE
                ))
                .size(11.0)
                .color(muted_color),
            );
        });
    }).response;

    // Handle drag-drop from asset browser (supports multi-select).
    if let Some(payload) = world.get_resource::<renzora_ui::asset_drag::AssetDragPayload>() {
        if payload.is_detached
            && drop_response.hovered()
            && !ui.ctx().input(|i| i.pointer.any_down())
        {
            // Collect all .anim paths from the drag (multi-select aware).
            let dragged: Vec<std::path::PathBuf> = payload
                .paths
                .iter()
                .cloned()
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("anim"))
                        .unwrap_or(false)
                })
                .collect();

            if !dragged.is_empty() {
                if let Some(project) = world.get_resource::<renzora::CurrentProject>() {
                    let project = project.clone();
                    cmds.push(move |world: &mut World| {
                        if let Some(mut a) = world.get_mut::<AnimatorComponent>(entity) {
                            for path in &dragged {
                                let asset_path = project.make_asset_relative(path);
                                if a.clips.iter().any(|c| c.path == asset_path) {
                                    continue;
                                }
                                let name = path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("clip");
                                let clip_name = name
                                    .strip_suffix(".anim")
                                    .unwrap_or(name)
                                    .to_string();
                                a.clips.push(crate::AnimClipSlot {
                                    name: clip_name,
                                    path: asset_path,
                                    looping: true,
                                    speed: 1.0,
                                    blend_in: None,
                                    blend_out: None,
                                });
                            }
                        }
                    });
                }
            }
        }
    }
}

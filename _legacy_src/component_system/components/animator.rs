//! AnimatorComponent registration + inspector UI.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular::FILM_STRIP;

use crate::animator::{AnimatorComponent, AnimClipSlot};
use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;
use crate::ui::property_row;
use crate::core::InspectorPanelRenderState;

// ============================================================================
// Add / Remove / Deserialize
// ============================================================================

fn add_animator(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(AnimatorComponent::new());
}

fn remove_animator(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<AnimatorComponent>();
}

fn deserialize_animator(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(comp) = serde_json::from_value::<AnimatorComponent>(data.clone()) {
        entity_commands.insert(comp);
    }
}

// ============================================================================
// Script properties
// ============================================================================

fn animator_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("current_clip", PropertyValueType::String),
        ("blend_duration", PropertyValueType::Float),
    ]
}

fn animator_get_props(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<AnimatorComponent>(entity) else { return vec![] };
    vec![
        ("current_clip", PropertyValue::String(data.current_clip.clone().unwrap_or_default())),
        ("blend_duration", PropertyValue::Float(data.blend_duration)),
    ]
}

fn animator_set_prop(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) else { return false };
    match prop {
        "current_clip" => {
            if let PropertyValue::String(v) = val {
                data.current_clip = if v.is_empty() { None } else { Some(v.clone()) };
                data.initialized = false; // re-trigger graph setup if needed
                true
            } else { false }
        }
        "blend_duration" => {
            if let PropertyValue::Float(v) = val { data.blend_duration = *v; true } else { false }
        }
        _ => false,
    }
}

// ============================================================================
// Inspector UI
// ============================================================================

fn inspect_animator(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;
    let mut should_clear_drag = false;

    let dragging_path = world
        .get_resource::<InspectorPanelRenderState>()
        .and_then(|rs| rs.dragging_asset_path.clone());
    let is_dragging_anim = dragging_path.as_ref().map_or(false, |p| {
        p.extension().and_then(|e| e.to_str()) == Some("anim")
    });

    // ── Blend time ───────────────────────────────────────────────────────────
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Blend time");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut v = world.get::<AnimatorComponent>(entity).map(|d| d.blend_duration).unwrap_or(0.2);
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=2.0).suffix("s").fixed_decimals(2)).changed() {
                    if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                        data.blend_duration = v;
                        changed = true;
                    }
                }
            });
        });
    });

    // ── Clips ─────────────────────────────────────────────────────────────────
    ui.separator();
    ui.label("Clips:");

    let clip_count = world.get::<AnimatorComponent>(entity).map(|d| d.clips.len()).unwrap_or(0);

    let mut remove_idx: Option<usize> = None;

    for i in 0..clip_count {
        let (clip_name, clip_path, clip_loop, clip_speed) = world.get::<AnimatorComponent>(entity)
            .map(|d| {
                let slot = &d.clips[i];
                (slot.name.clone(), slot.path.clone(), slot.looping, slot.speed)
            })
            .unwrap_or_default();

        let row_bg = if is_dragging_anim {
            egui::Color32::from_rgb(40, 70, 130)
        } else {
            egui::Color32::from_rgb(32, 34, 38)
        };

        let row_id = egui::Id::new(("anim_clip_row", entity, i));
        let row_resp = egui::Frame::new()
            .fill(row_bg)
            .inner_margin(egui::Margin::symmetric(6, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Name field
                    let mut name_buf = clip_name.clone();
                    if ui.add(egui::TextEdit::singleline(&mut name_buf)
                        .hint_text("name")
                        .desired_width(70.0)).changed()
                    {
                        if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                            data.clips[i].name = name_buf;
                            data.initialized = false;
                            changed = true;
                        }
                    }

                    // Path field (drag target)
                    let mut path_buf = clip_path.clone();
                    let path_resp = ui.add(egui::TextEdit::singleline(&mut path_buf)
                        .hint_text(if is_dragging_anim { "Drop .anim" } else { "path/to/clip.anim" })
                        .desired_width(ui.available_width() - 80.0));
                    if path_resp.changed() {
                        if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                            data.clips[i].path = path_buf;
                            data.clips[i].handle = None;
                            data.initialized = false;
                            changed = true;
                        }
                    }

                    // Loop checkbox
                    let mut loop_val = clip_loop;
                    if ui.checkbox(&mut loop_val, "").on_hover_text("Loop").changed() {
                        if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                            data.clips[i].looping = loop_val;
                            changed = true;
                        }
                    }

                    // Speed
                    let mut spd = clip_speed;
                    if ui.add(egui::DragValue::new(&mut spd).speed(0.01).range(0.0..=4.0).fixed_decimals(2).prefix("×")).changed() {
                        if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                            data.clips[i].speed = spd;
                            changed = true;
                        }
                    }
                });
            });

        // Right-click to remove
        if row_resp.response.secondary_clicked() {
            remove_idx = Some(i);
        }

        // Accept .anim drop on this row
        let pointer_over = ui.ctx().pointer_hover_pos()
            .map_or(false, |p| row_resp.response.rect.contains(p));
        if is_dragging_anim && pointer_over && ui.ctx().input(|inp| inp.pointer.any_released()) {
            if let Some(ref path) = dragging_path {
                if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                    data.clips[i].path = path.to_string_lossy().to_string();
                    data.clips[i].handle = None;
                    data.initialized = false;
                    changed = true;
                    should_clear_drag = true;
                }
            }
        }

        let _ = row_id;
    }

    // Remove requested slot
    if let Some(idx) = remove_idx {
        if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
            data.clips.remove(idx);
            data.initialized = false;
            changed = true;
        }
    }

    // Add Clip button
    if ui.button("+ Add Clip").clicked() {
        if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
            let n = data.clips.len();
            data.clips.push(AnimClipSlot {
                name: format!("clip{}", n),
                path: String::new(),
                looping: true,
                speed: 1.0,
                handle: None,
                node_index: None,
            });
            changed = true;
        }
    }

    // ── Playback controls ─────────────────────────────────────────────────────
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Playing:");

        let clip_names: Vec<String> = world.get::<AnimatorComponent>(entity)
            .map(|d| d.clips.iter().map(|s| s.name.clone()).collect())
            .unwrap_or_default();
        let current = world.get::<AnimatorComponent>(entity)
            .and_then(|d| d.current_clip.clone())
            .unwrap_or_default();

        egui::ComboBox::from_id_salt(("anim_playing", entity))
            .selected_text(if current.is_empty() { "none" } else { &current })
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_empty(), "none").clicked() {
                    if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                        data.current_clip = None;
                        changed = true;
                    }
                }
                for name in &clip_names {
                    let selected = name == &current;
                    if ui.selectable_label(selected, name).clicked() {
                        if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                            data.current_clip = Some(name.clone());
                            changed = true;
                        }
                    }
                }
            });

        if ui.button("▶ Play").clicked() {
            if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                if data.current_clip.is_none() {
                    if let Some(first) = data.clips.first().map(|s| s.name.clone()) {
                        data.current_clip = Some(first);
                        changed = true;
                    }
                }
            }
        }

        if ui.button("■ Stop").clicked() {
            if let Some(mut data) = world.get_mut::<AnimatorComponent>(entity) {
                data.current_clip = None;
                changed = true;
            }
        }
    });

    if should_clear_drag {
        if let Some(mut rs) = world.get_resource_mut::<InspectorPanelRenderState>() {
            rs.drag_accepted = true;
        }
    }

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(AnimatorComponent {
        type_id: "animator",
        display_name: "Animator",
        category: ComponentCategory::Animation,
        icon: FILM_STRIP,
        priority: 0,
        custom_inspector: inspect_animator,
        custom_add: add_animator,
        custom_remove: remove_animator,
        custom_deserialize: deserialize_animator,
        custom_script_properties: animator_get_props,
        custom_script_set: animator_set_prop,
        custom_script_meta: animator_property_meta,
    }));
}

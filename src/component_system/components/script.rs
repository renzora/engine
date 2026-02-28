//! Scripting component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::core::InspectorPanelRenderState;
use crate::project::CurrentProject;
use crate::scripting::{RhaiScriptEngine, ScriptComponent, ScriptValue};
use crate::ui::{inline_property, property_row};

use egui_phosphor::regular::{CODE, FILE, FOLDER_OPEN, MAGNIFYING_GLASS, X};

// ScriptComponent doesn't derive Serialize/Deserialize/Default,
// so we keep the static definition pattern instead of register_component! macro.

pub static SCRIPT: ComponentDefinition = ComponentDefinition {
    type_id: "script",
    display_name: "Script",
    category: ComponentCategory::Scripting,
    icon: CODE,
    priority: 0,
    add_fn: add_script,
    remove_fn: remove_script,
    has_fn: has_script,
    serialize_fn: serialize_script,
    deserialize_fn: deserialize_script,
    inspector_fn: inspect_script,
    conflicts_with: &[],
    requires: &[],
    get_script_properties_fn: None,
    set_script_property_fn: None,
    script_property_meta_fn: None,
};

pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&SCRIPT);
}

fn add_script(commands: &mut Commands, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    commands.entity(entity).insert(ScriptComponent::new());
}

fn remove_script(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<ScriptComponent>();
}

fn has_script(world: &World, entity: Entity) -> bool {
    world.get::<ScriptComponent>(entity).is_some()
}

fn serialize_script(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let script = world.get::<ScriptComponent>(entity)?;

    let entries: Vec<serde_json::Value> = script.scripts.iter().map(|entry| {
        let mut vars = serde_json::Map::new();
        for (name, value) in entry.variables.iter() {
            let v = match value {
                ScriptValue::Float(f) => json!({ "type": "float", "value": f }),
                ScriptValue::Int(i) => json!({ "type": "int", "value": i }),
                ScriptValue::Bool(b) => json!({ "type": "bool", "value": b }),
                ScriptValue::String(s) => json!({ "type": "string", "value": s }),
                ScriptValue::Vec2(v) => json!({ "type": "vec2", "value": [v.x, v.y] }),
                ScriptValue::Vec3(v) => json!({ "type": "vec3", "value": [v.x, v.y, v.z] }),
                ScriptValue::Color(c) => json!({ "type": "color", "value": [c.x, c.y, c.z, c.w] }),
            };
            vars.insert(name.clone(), v);
        }
        json!({
            "script_id": entry.script_id,
            "script_path": entry.script_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            "enabled": entry.enabled,
            "variables": vars
        })
    }).collect();

    Some(json!({ "scripts": entries }))
}

fn deserialize_variable(data: &serde_json::Value) -> Option<ScriptValue> {
    let ty = data.get("type")?.as_str()?;
    let val = data.get("value")?;
    match ty {
        "float" => Some(ScriptValue::Float(val.as_f64()? as f32)),
        "int" => Some(ScriptValue::Int(val.as_i64()? as i32)),
        "bool" => Some(ScriptValue::Bool(val.as_bool()?)),
        "string" => Some(ScriptValue::String(val.as_str()?.to_string())),
        "vec2" => {
            let arr = val.as_array()?;
            Some(ScriptValue::Vec2(Vec2::new(
                arr.first()?.as_f64()? as f32,
                arr.get(1)?.as_f64()? as f32,
            )))
        }
        "vec3" => {
            let arr = val.as_array()?;
            Some(ScriptValue::Vec3(Vec3::new(
                arr.first()?.as_f64()? as f32,
                arr.get(1)?.as_f64()? as f32,
                arr.get(2)?.as_f64()? as f32,
            )))
        }
        "color" => {
            let arr = val.as_array()?;
            Some(ScriptValue::Color(Vec4::new(
                arr.first()?.as_f64()? as f32,
                arr.get(1)?.as_f64()? as f32,
                arr.get(2)?.as_f64()? as f32,
                arr.get(3)?.as_f64()? as f32,
            )))
        }
        _ => None,
    }
}

fn deserialize_script(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    // New multi-script format
    if let Some(scripts_arr) = data.get("scripts").and_then(|v| v.as_array()) {
        let mut comp = ScriptComponent::new();
        for entry_data in scripts_arr {
            let script_id = entry_data.get("script_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let script_path = entry_data.get("script_path").and_then(|v| v.as_str()).map(std::path::PathBuf::from);
            let enabled = entry_data.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

            let id = if script_path.is_some() {
                comp.add_file_script(script_path.clone().unwrap())
            } else {
                comp.add_script(script_id.clone())
            };

            // Find the entry we just added and set its fields
            if let Some(entry) = comp.scripts.iter_mut().find(|e| e.id == id) {
                entry.script_id = script_id;
                entry.script_path = script_path;
                entry.enabled = enabled;

                // Deserialize variables
                if let Some(vars) = entry_data.get("variables").and_then(|v| v.as_object()) {
                    for (name, var_data) in vars {
                        if let Some(value) = deserialize_variable(var_data) {
                            entry.variables.set(name.clone(), value);
                        }
                    }
                }
            }
        }
        entity_commands.insert(comp);
    } else {
        // Legacy single-script format
        let script_id = data.get("script_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let script_path = data.get("script_path").and_then(|v| v.as_str()).map(std::path::PathBuf::from);
        let enabled = data.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        entity_commands.insert(ScriptComponent::from_legacy(
            script_id,
            script_path,
            enabled,
            Default::default(),
        ));
    }
}

fn is_script_file(path: &str) -> bool {
    path.ends_with(".rhai") || path.ends_with(".blueprint")
}

fn scan_scripts_folder(folder: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let mut scripts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "rhai" || ext == "blueprint" {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                let name = if ext == "blueprint" {
                    format!("{} (Blueprint)", stem)
                } else {
                    stem.to_string()
                };
                scripts.push((name, path));
            }
        }
    }
    scripts.sort_by(|a, b| a.0.cmp(&b.0));
    scripts
}

fn inspect_script(
    ui: &mut egui::Ui, world: &mut World, entity: Entity,
    _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>,
) -> bool {
    // Read immutable state before getting mutable script
    let dragging_path = world.get_resource::<InspectorPanelRenderState>()
        .and_then(|rs| rs.dragging_asset_path.clone());

    let project_path = world.get_resource::<CurrentProject>()
        .map(|p| p.path.clone());

    // Collect OS dropped files from egui context
    let os_dropped_files: Vec<std::path::PathBuf> = ui.ctx().input(|i| {
        i.raw.dropped_files.iter()
            .filter_map(|f| f.path.clone())
            .filter(|p| {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                ext == "rhai" || ext == "blueprint"
            })
            .collect()
    });

    // Collect available scripts for the search box (project + shipped)
    let (all_project_scripts, all_shipped_scripts): (Vec<(String, std::path::PathBuf)>, Vec<(String, std::path::PathBuf)>) = {
        let project = world.get_resource::<RhaiScriptEngine>()
            .map(|e| e.get_available_scripts())
            .unwrap_or_default();
        let shipped = std::fs::canonicalize(std::path::PathBuf::from("assets/scripts"))
            .ok()
            .map(|f| scan_scripts_folder(&f))
            .unwrap_or_default();
        (project, shipped)
    };

    // Collect script props for each file-based script entry
    let script_props: Vec<Vec<crate::scripting::ScriptVariableDefinition>> = {
        let script = world.get::<ScriptComponent>(entity);
        let rhai_engine = world.get_resource::<RhaiScriptEngine>();
        if let (Some(script), Some(engine)) = (script, rhai_engine) {
            script.scripts.iter().map(|entry| {
                if let Some(ref path) = entry.script_path {
                    // Resolve path against project
                    let resolved = if let Some(ref proj) = project_path {
                        if path.is_relative() {
                            proj.join(path)
                        } else {
                            path.clone()
                        }
                    } else {
                        path.clone()
                    };
                    engine.get_script_props(&resolved)
                } else {
                    Vec::new()
                }
            }).collect()
        } else {
            Vec::new()
        }
    };

    // Now get mutable access to the script component
    let Some(mut script) = world.get_mut::<ScriptComponent>(entity) else {
        return false;
    };

    let mut changed = false;
    let mut should_clear_drag = false;
    let mut remove_index: Option<usize> = None;

    let theme_colors = crate::ui::get_inspector_theme(ui.ctx());
    let available_width = ui.available_width();

    // === Search Box ===
    let search_id = ui.id().with("script_search");
    let mut search_text = ui.ctx().data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());

    let search_resp = ui.add(
        egui::TextEdit::singleline(&mut search_text)
            .hint_text(format!("{} {}", MAGNIFYING_GLASS, crate::locale::t("comp.script.search")))
            .desired_width(f32::INFINITY),
    );
    if search_resp.changed() {
        ui.ctx().data_mut(|d| d.insert_temp(search_id, search_text.clone()));
    }

    // === Search Results Popup (floating overlay, doesn't affect layout) ===
    if !search_text.is_empty() && (search_resp.has_focus() || search_resp.lost_focus()) {
        let query = search_text.to_lowercase();
        let matching_project: Vec<_> = all_project_scripts.iter()
            .filter(|(n, _)| n.to_lowercase().contains(&query))
            .collect();
        let matching_shipped: Vec<_> = all_shipped_scripts.iter()
            .filter(|(n, _)| n.to_lowercase().contains(&query))
            .collect();

        let popup_id = search_id.with("popup");
        let popup_pos = search_resp.rect.left_bottom() + egui::vec2(0.0, 2.0);
        let popup_width = search_resp.rect.width();

        // Extract colors before moving into closure
        let inactive_bg = theme_colors.widget_inactive_bg;
        let hovered_bg = theme_colors.widget_hovered_bg;
        let border_color = theme_colors.widget_border;
        let muted_color = theme_colors.text_muted;
        let project_path_ref = project_path.clone();

        let area_resp = egui::Area::new(popup_id)
            .order(egui::Order::Foreground)
            .fixed_pos(popup_pos)
            .show(ui.ctx(), |ui| -> Option<std::path::PathBuf> {
                let mut clicked: Option<std::path::PathBuf> = None;
                ui.set_width(popup_width);

                egui::Frame::new()
                    .fill(inactive_bg)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .corner_radius(egui::CornerRadius::same(4))
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        if matching_project.is_empty() && matching_shipped.is_empty() {
                            ui.label(egui::RichText::new("No scripts found").size(11.0).color(muted_color));
                            return;
                        }

                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .id_salt("script_search_results")
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());

                                if !matching_project.is_empty() {
                                    ui.label(egui::RichText::new(crate::locale::t("comp.script.project")).size(10.0).color(muted_color));
                                    for (name, path) in &matching_project {
                                        let resp = ui.add(
                                            egui::Button::new(
                                                egui::RichText::new(format!("{} {}", CODE, name)).size(11.0)
                                            )
                                            .frame(false)
                                            .min_size(egui::Vec2::new(ui.available_width(), 20.0)),
                                        );
                                        if resp.hovered() {
                                            let [r, g, b, _] = hovered_bg.to_array();
                                            ui.painter().rect_filled(resp.rect, 2.0, egui::Color32::from_rgba_unmultiplied(r, g, b, 80));
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        if resp.clicked() {
                                            clicked = Some(if let Some(ref proj) = project_path_ref {
                                                path.strip_prefix(proj).unwrap_or(path).to_path_buf()
                                            } else {
                                                path.to_path_buf()
                                            });
                                        }
                                    }
                                }

                                if !matching_shipped.is_empty() {
                                    if !matching_project.is_empty() { ui.add_space(4.0); }
                                    ui.label(egui::RichText::new(crate::locale::t("comp.script.builtin")).size(10.0).color(muted_color));
                                    for (name, path) in &matching_shipped {
                                        let resp = ui.add(
                                            egui::Button::new(
                                                egui::RichText::new(format!("{} {}", CODE, name)).size(11.0)
                                            )
                                            .frame(false)
                                            .min_size(egui::Vec2::new(ui.available_width(), 20.0)),
                                        );
                                        if resp.hovered() {
                                            let [r, g, b, _] = hovered_bg.to_array();
                                            ui.painter().rect_filled(resp.rect, 2.0, egui::Color32::from_rgba_unmultiplied(r, g, b, 80));
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        if resp.clicked() {
                                            // Copy built-in script into the project's scripts folder
                                            // so the project owns its own copy.
                                            if let Some(ref proj) = project_path_ref {
                                                let scripts_dir = proj.join("scripts");
                                                let _ = std::fs::create_dir_all(&scripts_dir);
                                                let filename = path.file_name().unwrap_or_default();
                                                let dest = scripts_dir.join(filename);
                                                if !dest.exists() {
                                                    let _ = std::fs::copy(path, &dest);
                                                }
                                                clicked = Some(std::path::PathBuf::from("scripts").join(filename));
                                            } else {
                                                clicked = Some(path.to_path_buf());
                                            }
                                        }
                                    }
                                }
                            });
                    });

                clicked
            });

        if let Some(path) = area_resp.inner {
            script.add_file_script(path);
            changed = true;
            ui.ctx().data_mut(|d| d.insert_temp::<String>(search_id, String::new()));
        }

        // Eat scroll events when the pointer is inside the popup to stop them
        // from bubbling up to the inspector panel's scroll area.
        if area_resp.response.rect.contains(ui.ctx().pointer_hover_pos().unwrap_or_default()) {
            ui.ctx().input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
        }
    }

    ui.add_space(4.0);

    // === Drop Zone ===
    let drop_zone_height = 48.0;
    let (rect, _response) = ui.allocate_exact_size(
        egui::Vec2::new(available_width, drop_zone_height),
        egui::Sense::click_and_drag(),
    );

    let pointer_pos = ui.ctx().pointer_hover_pos();
    let pointer_in_zone = pointer_pos.map_or(false, |p| rect.contains(p));

    // Draw drop zone background
    ui.painter().rect_filled(rect, 4.0, theme_colors.widget_inactive_bg);

    // Highlight on drag hover
    let is_dragging_script = dragging_path.as_ref().map_or(false, |p: &std::path::PathBuf| {
        is_script_file(&p.to_string_lossy())
    });

    if is_dragging_script && pointer_in_zone {
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(2.0, theme_colors.semantic_accent),
            egui::StrokeKind::Inside,
        );
    } else {
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, theme_colors.widget_border),
            egui::StrokeKind::Outside,
        );
    }

    // Drop zone text
    let center = rect.center();
    ui.painter().text(
        egui::pos2(center.x, center.y - 8.0),
        egui::Align2::CENTER_CENTER,
        FILE,
        egui::FontId::proportional(16.0),
        theme_colors.text_muted,
    );
    ui.painter().text(
        egui::pos2(center.x, center.y + 8.0),
        egui::Align2::CENTER_CENTER,
        "Drop .rhai or .blueprint here",
        egui::FontId::proportional(10.0),
        theme_colors.text_muted,
    );

    // Handle asset panel drop
    if is_dragging_script && pointer_in_zone && ui.ctx().input(|i| i.pointer.any_released()) {
        if let Some(ref asset_path) = dragging_path {
            // Make relative to project
            let rel_path: std::path::PathBuf = if let Some(ref proj) = project_path {
                asset_path.strip_prefix(proj).unwrap_or(asset_path).to_path_buf()
            } else {
                asset_path.clone()
            };
            script.add_file_script(rel_path);
            changed = true;
            should_clear_drag = true;
        }
    }

    // Handle OS file drops
    for dropped in &os_dropped_files {
        let rel_path = if let Some(ref proj) = project_path {
            dropped.strip_prefix(proj).unwrap_or(dropped).to_path_buf()
        } else {
            dropped.clone()
        };
        script.add_file_script(rel_path);
        changed = true;
    }

    // Browse button
    ui.horizontal(|ui| {
        if ui.add_sized([available_width, 20.0], egui::Button::new(
            egui::RichText::new(format!("{} Browse...", FOLDER_OPEN)).size(11.0),
        )).clicked() {
            if let Some(file_path) = rfd::FileDialog::new()
                .add_filter("Scripts", &["rhai", "blueprint"])
                .set_title(&crate::locale::t("comp.script.select"))
                .pick_file()
            {
                let rel_path = if let Some(ref proj) = project_path {
                    file_path.strip_prefix(proj).unwrap_or(&file_path).to_path_buf()
                } else {
                    file_path
                };
                script.add_file_script(rel_path);
                changed = true;
            }
        }
    });

    ui.add_space(4.0);

    // === Script Entries List ===
    if script.scripts.is_empty() {
        property_row(ui, 0, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::locale::t("comp.script.no_attached")).size(11.0).color(theme_colors.text_muted));
            });
        });
    }

    for idx in 0..script.scripts.len() {
        let entry = &mut script.scripts[idx];
        let row_base = idx * 2;

        // Header row: enable checkbox | filename | remove button
        let file_name = entry.script_path.as_ref()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                if entry.script_id.is_empty() {
                    "(empty)".to_string()
                } else {
                    entry.script_id.clone()
                }
            });

        property_row(ui, row_base, |ui| {
            ui.horizontal(|ui| {
                // Enable checkbox
                if ui.checkbox(&mut entry.enabled, "").changed() {
                    changed = true;
                }

                // Script icon and name
                ui.label(egui::RichText::new(CODE).size(12.0).color(theme_colors.semantic_accent));
                ui.label(egui::RichText::new(&file_name).size(11.0));

                // Spacer
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Remove button
                    if ui.add(egui::Button::new(
                        egui::RichText::new(X).size(12.0).color(theme_colors.semantic_error),
                    ).frame(false)).clicked() {
                        remove_index = Some(idx);
                        changed = true;
                    }
                });
            });
        });

        // Variable editors (from script props)
        if idx < script_props.len() {
            let props = &script_props[idx];
            for (var_idx, prop) in props.iter().enumerate() {
                // Initialize missing variables from prop defaults
                if entry.variables.get(&prop.name).is_none() {
                    entry.variables.set(prop.name.clone(), prop.default_value.clone());
                    changed = true;
                }

                let var_row = row_base + 2 + var_idx;

                if let Some(value) = entry.variables.get_mut(&prop.name) {
                    inline_property(ui, var_row, &prop.display_name, |ui| {
                        match value {
                            ScriptValue::Float(ref mut v) => {
                                let r = ui.add(egui::DragValue::new(v).speed(0.1));
                                if r.changed() { changed = true; }
                                r
                            }
                            ScriptValue::Int(ref mut v) => {
                                let r = ui.add(egui::DragValue::new(v).speed(1.0));
                                if r.changed() { changed = true; }
                                r
                            }
                            ScriptValue::Bool(ref mut v) => {
                                let r = ui.checkbox(v, "");
                                if r.changed() { changed = true; }
                                r
                            }
                            ScriptValue::String(ref mut v) => {
                                let r = ui.text_edit_singleline(v);
                                if r.changed() { changed = true; }
                                r
                            }
                            ScriptValue::Vec2(ref mut v) => {
                                let mut any_changed = false;
                                let r = ui.horizontal(|ui| {
                                    ui.label("X");
                                    if ui.add(egui::DragValue::new(&mut v.x).speed(0.1)).changed() {
                                        any_changed = true;
                                    }
                                    ui.label("Y");
                                    if ui.add(egui::DragValue::new(&mut v.y).speed(0.1)).changed() {
                                        any_changed = true;
                                    }
                                });
                                if any_changed { changed = true; }
                                r.response
                            }
                            ScriptValue::Vec3(ref mut v) => {
                                let mut any_changed = false;
                                let r = ui.horizontal(|ui| {
                                    ui.label("X");
                                    if ui.add(egui::DragValue::new(&mut v.x).speed(0.1)).changed() {
                                        any_changed = true;
                                    }
                                    ui.label("Y");
                                    if ui.add(egui::DragValue::new(&mut v.y).speed(0.1)).changed() {
                                        any_changed = true;
                                    }
                                    ui.label("Z");
                                    if ui.add(egui::DragValue::new(&mut v.z).speed(0.1)).changed() {
                                        any_changed = true;
                                    }
                                });
                                if any_changed { changed = true; }
                                r.response
                            }
                            ScriptValue::Color(ref mut c) => {
                                let mut color = [c.x, c.y, c.z, c.w];
                                let r = ui.color_edit_button_rgba_unmultiplied(&mut color);
                                if r.changed() {
                                    *c = Vec4::new(color[0], color[1], color[2], color[3]);
                                    changed = true;
                                }
                                r
                            }
                        }
                    });
                }
            }
        }

        // Separator between entries
        if idx < script.scripts.len() - 1 {
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);
        }
    }

    // Remove script entry if requested
    if let Some(idx) = remove_index {
        script.remove_script(idx);
    }

    // Clean up: signal drag was accepted
    drop(script);
    if should_clear_drag {
        if let Some(mut rs) = world.get_resource_mut::<InspectorPanelRenderState>() {
            rs.drag_accepted = true;
        }
    }

    changed
}

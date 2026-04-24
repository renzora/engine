//! Inspector entry for the ScriptComponent.
//!
//! Registered automatically when the `editor` feature is enabled.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor_framework::{
    asset_drop_target, inline_property, search_overlay, toggle_switch, AssetDragPayload,
    EditorCommands, InspectorEntry, OverlayAction, OverlayEntry,
};
use renzora_theme::Theme;

use crate::component::{ScriptComponent, ScriptVariableDefinition};
use crate::engine::ScriptEngine;
use crate::ScriptValue;

/// Register the script inspector entry via `AppEditorExt`.
pub fn register_script_inspector(app: &mut App) {
    use renzora_editor_framework::AppEditorExt;
    app.register_inspector(script_component_entry());
}

fn script_component_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "script_component",
        display_name: "Scripts",
        icon: regular::CODE,
        category: "scripting",
        has_fn: |world, entity| world.get::<ScriptComponent>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(ScriptComponent::new());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<ScriptComponent>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(script_component_ui),
    }
}

/// Recursively scan a directory for script files (.lua, .rhai).
fn scan_script_files(root: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let mut results = Vec::new();
    scan_script_files_inner(root, root, &mut results);
    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

fn scan_script_files_inner(
    dir: &std::path::Path,
    root: &std::path::Path,
    out: &mut Vec<(String, std::path::PathBuf)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.starts_with('.')) {
            continue;
        }
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "target" | "node_modules" | ".git") {
                continue;
            }
            scan_script_files_inner(&path, root, out);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "lua" | "rhai") {
                let display = path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                out.push((display, path));
            }
        }
    }
}

fn make_relative(abs: std::path::PathBuf, world: &World) -> std::path::PathBuf {
    if let Some(project) = world.get_resource::<renzora::CurrentProject>() {
        if let Ok(rel) = abs.strip_prefix(&project.path) {
            return rel.to_path_buf();
        }
    }
    abs
}

fn to_display_name(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn script_component_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(sc) = world.get::<ScriptComponent>(entity) else {
        return;
    };
    let scripts = sc.scripts.clone();
    let payload = world.get_resource::<AssetDragPayload>();

    if !scripts.is_empty() {
        let mut remove_index: Option<usize> = None;
        let mut toggle_index: Option<usize> = None;

        for (i, entry) in scripts.iter().enumerate() {
            let _script_name = if let Some(ref path) = entry.script_path {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            } else if !entry.script_id.is_empty() {
                entry.script_id.clone()
            } else {
                format!("Script #{}", entry.id)
            };

            let row_idx = i * 3;

            let current_path = entry.script_path.as_ref().map(|p| p.to_string_lossy().to_string());
            let current_str = current_path.as_deref();

            inline_property(ui, row_idx, "Script", theme, |ui| {
                let drop = asset_drop_target(
                    ui,
                    ui.id().with(("script_path", i)),
                    current_str,
                    &["lua", "rhai"],
                    "Drop script file",
                    theme,
                    payload,
                );
                if let Some(path) = drop.dropped_path {
                    let idx = i;
                    let rel = make_relative(path, world);
                    cmds.push(move |w: &mut World| {
                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                            if let Some(entry) = sc.scripts.get_mut(idx) {
                                entry.script_path = Some(rel);
                            }
                        }
                    });
                }
                if drop.cleared {
                    let idx = i;
                    cmds.push(move |w: &mut World| {
                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                            if let Some(entry) = sc.scripts.get_mut(idx) {
                                entry.script_path = None;
                            }
                        }
                    });
                }
            });

            inline_property(ui, row_idx + 1, "Enabled", theme, |ui| {
                let id = ui.id().with(("script_enabled", i));
                if toggle_switch(ui, id, entry.enabled) {
                    toggle_index = Some(i);
                }
            });

            // Pull the script's declared prop list (if any) so we can:
            //   - group the vars by `tab = "..."` declared in props()
            //   - render them in a stable order instead of HashMap order
            // `get_script_props` is cached by the backend, so calling this each
            // frame is cheap.
            let defs: Vec<ScriptVariableDefinition> = entry.script_path.as_ref()
                .and_then(|p| world.get_resource::<ScriptEngine>().map(|e| e.get_script_props(p)))
                .unwrap_or_default();

            // Build (tab_name, Vec<(var_name, var_value)>) preserving the order
            // in which tabs first appear in the def list.
            let mut tab_groups: Vec<(String, Vec<(String, ScriptValue)>)> = Vec::new();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            for def in &defs {
                let Some(val) = entry.variables.get(&def.name) else { continue };
                let tab = def.tab.clone().unwrap_or_else(|| "General".to_string());
                let pair = (def.name.clone(), val.clone());
                match tab_groups.iter_mut().find(|(t, _)| *t == tab) {
                    Some((_, vars)) => vars.push(pair),
                    None => tab_groups.push((tab, vec![pair])),
                }
                seen.insert(def.name.clone());
            }
            // Any variables that the script no longer declares (renamed /
            // removed in a rebuild) still deserve to be editable — drop them
            // in an "Other" group so the user can clean them up.
            let orphans: Vec<(String, ScriptValue)> = entry.variables.iter_all()
                .filter(|(k, _)| !seen.contains(*k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if !orphans.is_empty() {
                tab_groups.push(("Other".to_string(), orphans));
            }

            // If the script doesn't use tabs at all and there's nothing
            // orphaned, skip the collapsible header — a single "General" row
            // is just noise on simple scripts.
            let show_headers = !(tab_groups.len() == 1 && tab_groups[0].0 == "General");

            if !tab_groups.is_empty() {
                ui.add_space(2.0);
            }

            for (tab_idx, (tab_name, vars)) in tab_groups.into_iter().enumerate() {
                let render = |ui: &mut egui::Ui, vars: &[(String, ScriptValue)]| {
                for (var_name, var_value) in vars.iter() {
                    let label = to_display_name(var_name);
                    let script_idx = i;
                    let vname = var_name.clone();
                    match var_value.clone() {
                        ScriptValue::Float(mut v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = v;
                                ui.add(egui::DragValue::new(&mut v).speed(0.1));
                                if v != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Float(v));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Int(mut v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = v;
                                ui.add(egui::DragValue::new(&mut v).speed(1.0));
                                if v != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Int(v));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Bool(v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let id = ui.id().with(("script_var_bool", script_idx, &vname));
                                if toggle_switch(ui, id, v) {
                                    let new_val = !v;
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Bool(new_val));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::String(mut s) | ScriptValue::Entity(mut s) => {
                            let is_entity = matches!(var_value, ScriptValue::Entity(_));
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = s.clone();
                                ui.add(
                                    egui::TextEdit::singleline(&mut s)
                                        .desired_width(ui.available_width()),
                                );
                                if s != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                let val = if is_entity {
                                                    ScriptValue::Entity(s)
                                                } else {
                                                    ScriptValue::String(s)
                                                };
                                                entry.variables.set(vname, val);
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Vec2(mut v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = v;
                                let w = ((ui.available_width() - 32.0) / 2.0).max(30.0);
                                ui.spacing_mut().item_spacing.x = 2.0;
                                ui.label(egui::RichText::new("X").size(10.0).color(egui::Color32::from_rgb(230, 90, 90)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.x).speed(0.1));
                                ui.label(egui::RichText::new("Y").size(10.0).color(egui::Color32::from_rgb(130, 200, 90)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.y).speed(0.1));
                                if v != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Vec2(v));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Vec3(mut v) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = v;
                                let w = ((ui.available_width() - 48.0) / 3.0).max(30.0);
                                ui.spacing_mut().item_spacing.x = 2.0;
                                ui.label(egui::RichText::new("X").size(10.0).color(egui::Color32::from_rgb(230, 90, 90)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.x).speed(0.1));
                                ui.label(egui::RichText::new("Y").size(10.0).color(egui::Color32::from_rgb(130, 200, 90)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.y).speed(0.1));
                                ui.label(egui::RichText::new("Z").size(10.0).color(egui::Color32::from_rgb(90, 150, 230)));
                                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v.z).speed(0.1));
                                if v != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Vec3(v));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        ScriptValue::Color(mut c) => {
                            let vname = vname.clone();
                            inline_property(ui, row_idx + 2, &label, theme, |ui| {
                                let orig = c;
                                let mut rgb = [c.x, c.y, c.z];
                                if ui.color_edit_button_rgb(&mut rgb).changed() {
                                    c.x = rgb[0];
                                    c.y = rgb[1];
                                    c.z = rgb[2];
                                }
                                if c != orig {
                                    let vname = vname.clone();
                                    cmds.push(move |w: &mut World| {
                                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                                            if let Some(entry) = sc.scripts.get_mut(script_idx) {
                                                entry.variables.set(vname, ScriptValue::Color(c));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                    }
                }
                };

                if show_headers {
                    let header_id = ui.id().with(("script_tab", i, tab_name.as_str()));
                    egui::CollapsingHeader::new(&tab_name)
                        .id_salt(header_id)
                        .default_open(tab_idx == 0)
                        .show(ui, |ui| render(ui, &vars));
                } else {
                    render(ui, &vars);
                }
            }

            inline_property(ui, row_idx + 2, "", theme, |ui| {
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(format!("{} Remove", regular::TRASH))
                            .size(11.0)
                            .color(theme.semantic.error.to_color32()),
                    ))
                    .clicked()
                {
                    remove_index = Some(i);
                }
            });

            if i < scripts.len() - 1 {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
            }
        }

        if let Some(idx) = toggle_index {
            let new_enabled = !scripts[idx].enabled;
            cmds.push(move |w: &mut World| {
                if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                    if let Some(entry) = sc.scripts.get_mut(idx) {
                        entry.enabled = new_enabled;
                    }
                }
            });
        }
        if let Some(idx) = remove_index {
            cmds.push(move |w: &mut World| {
                if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                    sc.remove_script(idx);
                }
            });
        }
    }

    // Add Script button — opens search overlay
    let overlay_id = egui::Id::new("script_search_overlay_visible");
    let show_overlay = ui.ctx().data_mut(|d| {
        *d.get_persisted_mut_or_insert_with(overlay_id, || false)
    });

    if show_overlay {
        let available = world
            .get_resource::<renzora::CurrentProject>()
            .map(|p| scan_script_files(&p.path))
            .unwrap_or_default();

        let entry_data: Vec<(String, String, String, std::path::PathBuf)> = available
            .iter()
            .map(|(name, path)| {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                let icon = match ext {
                    "lua" => regular::CODE,
                    "rhai" => regular::CODE,
                    _ => regular::FILE,
                };
                let category = ext.to_string();
                (name.clone(), icon.to_string(), category, path.clone())
            })
            .collect();

        let entries: Vec<OverlayEntry> = entry_data
            .iter()
            .map(|(name, icon, cat, _path)| OverlayEntry {
                id: name.as_str(),
                label: name.as_str(),
                icon: icon.as_str(),
                category: if cat.is_empty() { "scripts" } else { cat.as_str() },
            })
            .collect();

        let search_id = egui::Id::new("script_search_text");
        let mut search_text: String = ui.ctx().data_mut(|d| {
            d.get_persisted_mut_or_insert_with(search_id, String::new).clone()
        });

        let ctx = ui.ctx().clone();
        match search_overlay(&ctx, "add_script_overlay", "Add Script", &entries, &mut search_text, theme) {
            OverlayAction::Selected(name) => {
                if let Some((_, _, _, path)) = entry_data.iter().find(|(n, _, _, _)| *n == name) {
                    let rel = make_relative(path.clone(), world);
                    cmds.push(move |w: &mut World| {
                        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                            sc.add_file_script(rel);
                        }
                    });
                }
                ui.ctx().data_mut(|d| d.insert_persisted(overlay_id, false));
                ui.ctx().data_mut(|d| d.insert_persisted(search_id, String::new()));
            }
            OverlayAction::Closed => {
                ui.ctx().data_mut(|d| d.insert_persisted(overlay_id, false));
                ui.ctx().data_mut(|d| d.insert_persisted(search_id, String::new()));
            }
            OverlayAction::None => {
                ui.ctx().data_mut(|d| d.insert_persisted(search_id, search_text));
            }
        }
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} Add Script", regular::PLUS))
                    .size(11.0),
            ))
            .clicked()
        {
            ui.ctx().data_mut(|d| d.insert_persisted(overlay_id, true));
        }
    });

    ui.add_space(4.0);
    let drop = asset_drop_target(
        ui,
        ui.id().with("script_drop_add"),
        None,
        &["lua", "rhai"],
        "Drop to add script",
        theme,
        payload,
    );
    if let Some(path) = drop.dropped_path {
        let rel = make_relative(path, world);
        cmds.push(move |w: &mut World| {
            if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                sc.add_file_script(rel);
            }
        });
    }
    ui.add_space(4.0);
}

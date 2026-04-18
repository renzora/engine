//! Scripts panel — lists script entries on the currently-selected entity and
//! lets you click them open into the code editor, toggle them, or detach.
//! Attaching new scripts is done by dragging from the Asset Browser onto the
//! entity (or via the entity's Add Component menu) — this panel only manages
//! what's already attached.

use bevy::prelude::*;
use bevy_egui::egui::{self, CursorIcon, FontId, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular::{CODE, EYE, EYE_SLASH, FILE_PLUS, TRASH};
use renzora_editor_framework::{EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora_scripting::ScriptComponent;
use renzora_theme::ThemeManager;

use crate::state::CodeEditorState;

const NEW_SCRIPT_TEMPLATE: &str = r#"-- New Script

function on_ready(ctx, vars)
    -- Called once when the script is first attached
end

function on_update(ctx, vars)
    -- Called every frame
end
"#;

/// Create a new `.lua` file under `<project>/scripts/` with a unique name,
/// attach it to `entity` via its `ScriptComponent` (creating one if absent),
/// and open it in the code editor.
fn create_and_attach_new_script(
    world: &mut World,
    entity: Entity,
    project_root: std::path::PathBuf,
) {
    let scripts_dir = project_root.join("scripts");
    if let Err(e) = std::fs::create_dir_all(&scripts_dir) {
        log::error!("Failed to create scripts dir: {}", e);
        return;
    }

    // Pick a unique name.
    let mut idx = 1usize;
    let (abs_path, rel_path) = loop {
        let name = if idx == 1 {
            "new_script.lua".to_string()
        } else {
            format!("new_script_{}.lua", idx)
        };
        let abs = scripts_dir.join(&name);
        let rel = std::path::PathBuf::from("scripts").join(&name);
        if !abs.exists() {
            break (abs, rel);
        }
        idx += 1;
        if idx > 1000 {
            log::error!("Couldn't find a free new_script name");
            return;
        }
    };

    if let Err(e) = std::fs::write(&abs_path, NEW_SCRIPT_TEMPLATE) {
        log::error!("Failed to write new script {}: {}", abs_path.display(), e);
        return;
    }

    // Attach (create ScriptComponent if needed).
    if let Some(mut sc) = world.get_mut::<ScriptComponent>(entity) {
        sc.add_file_script(rel_path);
    } else {
        let mut sc = ScriptComponent::new();
        sc.add_file_script(rel_path);
        world.entity_mut(entity).insert(sc);
    }

    // Open in editor.
    if let Some(mut state) = world.get_resource_mut::<CodeEditorState>() {
        state.open_file(abs_path);
    }
}

pub struct ScriptsOnEntityPanel;

impl EditorPanel for ScriptsOnEntityPanel {
    fn id(&self) -> &str {
        "scripts_on_entity"
    }
    fn title(&self) -> &str {
        "Scripts"
    }
    fn icon(&self) -> Option<&str> {
        Some(CODE)
    }
    fn closable(&self) -> bool {
        true
    }
    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let muted = theme.text.muted.to_color32();
        let primary = theme.text.primary.to_color32();
        let accent = theme.semantic.accent.to_color32();
        let disabled = theme.text.disabled.to_color32();

        let entity = world
            .get_resource::<EditorSelection>()
            .and_then(|s| s.get());
        let project_path = world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.path.clone());
        let cmds = world.get_resource::<EditorCommands>();

        // --- Toolbar: New Script (always visible when an entity is selected) ---
        if let (Some(e), Some(root), Some(c)) = (entity, project_path.clone(), cmds) {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let btn = ui
                    .button(RichText::new(format!("{} New Script", FILE_PLUS)).size(11.0))
                    .on_hover_text("Create a new script in scripts/ and attach it");
                if btn.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if btn.clicked() {
                    let root_clone = root.clone();
                    c.push(move |world: &mut World| {
                        create_and_attach_new_script(world, e, root_clone);
                    });
                }
            });
            ui.separator();
        }

        let Some(entity) = entity else {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Select an entity to see its scripts")
                        .size(11.0)
                        .color(muted),
                );
            });
            return;
        };

        let Some(sc) = world.get::<ScriptComponent>(entity) else {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("This entity has no scripts.")
                        .size(11.0)
                        .color(muted),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Click 'New Script' above or drag a .lua/.rhai file onto the entity.")
                        .size(10.5)
                        .color(disabled),
                );
            });
            return;
        };

        // Snapshot the entries we need so we don't hold a world borrow.
        let entries: Vec<(u32, String, Option<std::path::PathBuf>, bool)> = sc
            .scripts
            .iter()
            .map(|e| {
                (
                    e.id,
                    e.script_id.clone(),
                    e.script_path.clone(),
                    e.enabled,
                )
            })
            .collect();

        if entries.is_empty() {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No scripts attached")
                        .size(11.0)
                        .color(muted),
                );
            });
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 2.0;
                for (id, script_id, script_path, enabled) in entries {
                    let row_height = 26.0;
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        Sense::click(),
                    );

                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            2.0,
                            theme.widgets.hovered_bg.to_color32(),
                        );
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    // Enabled border indicator (left bar).
                    let bar_color = if enabled { accent } else { disabled };
                    ui.painter().line_segment(
                        [
                            egui::Pos2::new(rect.min.x + 1.0, rect.min.y + 4.0),
                            egui::Pos2::new(rect.min.x + 1.0, rect.max.y - 4.0),
                        ],
                        Stroke::new(2.0, bar_color),
                    );

                    let display = script_path
                        .as_ref()
                        .and_then(|p| {
                            p.file_name().and_then(|n| n.to_str()).map(|s| s.to_string())
                        })
                        .unwrap_or_else(|| {
                            if script_id.is_empty() {
                                format!("(script {})", id)
                            } else {
                                script_id.clone()
                            }
                        });

                    // Resolve to check existence — same convention as the
                    // runtime: relative paths are project-root joined.
                    let exists = match (script_path.as_ref(), project_path.as_ref()) {
                        (Some(p), _) if p.is_absolute() => p.exists(),
                        (Some(p), Some(root)) => root.join(p).exists(),
                        // No script_path → built-in script_id; treat as present.
                        _ => true,
                    };

                    let label_color = if !exists {
                        theme.semantic.error.to_color32()
                    } else if enabled {
                        primary
                    } else {
                        muted
                    };
                    ui.painter().text(
                        egui::Pos2::new(rect.min.x + 12.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &display,
                        FontId::proportional(11.5),
                        label_color,
                    );

                    if !exists {
                        // Strikethrough + "missing" badge after the name.
                        let label_w = display.len() as f32 * 6.5;
                        let line_y = rect.center().y;
                        ui.painter().line_segment(
                            [
                                egui::Pos2::new(rect.min.x + 12.0, line_y),
                                egui::Pos2::new(rect.min.x + 12.0 + label_w, line_y),
                            ],
                            Stroke::new(1.0, theme.semantic.error.to_color32()),
                        );
                        ui.painter().text(
                            egui::Pos2::new(rect.min.x + 18.0 + label_w, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "missing",
                            FontId::proportional(9.5),
                            theme.semantic.error.to_color32(),
                        );
                    }

                    // Toggle button (rightmost - 30)
                    let toggle_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(rect.max.x - 50.0, rect.min.y),
                        Vec2::new(20.0, row_height),
                    );
                    let toggle_id = ui.id().with(("script_toggle", id));
                    let toggle_resp = ui.interact(toggle_rect, toggle_id, Sense::click());
                    let toggle_icon = if enabled { EYE } else { EYE_SLASH };
                    let toggle_color = if enabled { accent } else { muted };
                    ui.painter().text(
                        toggle_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        toggle_icon,
                        FontId::proportional(13.0),
                        toggle_color,
                    );
                    if toggle_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    // Detach button
                    let detach_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(rect.max.x - 25.0, rect.min.y),
                        Vec2::new(20.0, row_height),
                    );
                    let detach_id = ui.id().with(("script_detach", id));
                    let detach_resp = ui.interact(detach_rect, detach_id, Sense::click());
                    ui.painter().text(
                        detach_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        TRASH,
                        FontId::proportional(13.0),
                        theme.semantic.error.to_color32(),
                    );
                    if detach_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    // Click on row body opens the file in the editor.
                    // script_path is project-relative; the runtime resolver
                    // simply joins it with the project root (scripts can live
                    // in any subdirectory, not just `scripts/`).
                    if resp.clicked() && !toggle_resp.clicked() && !detach_resp.clicked() {
                        if let (Some(path), Some(root), Some(c)) =
                            (script_path.clone(), project_path.clone(), cmds)
                        {
                            let resolved = if path.is_absolute() {
                                path
                            } else {
                                root.join(&path)
                            };
                            if !resolved.exists() {
                                log::warn!(
                                    "Script file not found: {} (entity {:?})",
                                    resolved.display(),
                                    entity,
                                );
                                continue;
                            }
                            c.push(move |world: &mut World| {
                                if let Some(mut s) =
                                    world.get_resource_mut::<CodeEditorState>()
                                {
                                    s.open_file(resolved);
                                }
                            });
                        }
                    }

                    if toggle_resp.clicked() {
                        if let Some(c) = cmds {
                            c.push(move |world: &mut World| {
                                if let Some(mut sc) =
                                    world.get_mut::<ScriptComponent>(entity)
                                {
                                    if let Some(entry) =
                                        sc.scripts.iter_mut().find(|e| e.id == id)
                                    {
                                        entry.enabled = !entry.enabled;
                                    }
                                }
                            });
                        }
                    }

                    if detach_resp.clicked() {
                        if let Some(c) = cmds {
                            c.push(move |world: &mut World| {
                                if let Some(mut sc) =
                                    world.get_mut::<ScriptComponent>(entity)
                                {
                                    sc.scripts.retain(|e| e.id != id);
                                }
                            });
                        }
                    }
                }
            });
    }
}

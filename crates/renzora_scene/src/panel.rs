//! Scenes panel — lists scene files under `<project>/scenes/` with create + rename.

use std::path::PathBuf;
use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora::core::CurrentProject;
use renzora_camera::OrbitCameraState;
use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_engine::scene_io;
use renzora_theme::ThemeManager;

#[derive(Default)]
struct PanelState {
    /// Entity currently being renamed, plus the in-progress text.
    renaming: Option<(PathBuf, String)>,
    rename_focus_set: bool,
}

#[derive(Default)]
pub struct ScenesPanel {
    state: RwLock<PanelState>,
}

impl EditorPanel for ScenesPanel {
    fn id(&self) -> &str {
        "scenes"
    }

    fn title(&self) -> &str {
        "Scenes"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::FILM_STRIP)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let Some(theme) = world.get_resource::<ThemeManager>().map(|t| t.active_theme.clone())
        else {
            return;
        };
        let Some(commands) = world.get_resource::<EditorCommands>() else {
            return;
        };
        let Some(project) = world.get_resource::<CurrentProject>() else {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("No project open.")
                        .color(theme.text.muted.to_color32()),
                );
            });
            return;
        };

        let scenes_dir = project.resolve_path("scenes");
        // Highlight the scene the active document tab points at, not the
        // project's boot scene — those can differ.
        let current_scene_abs = world
            .get_resource::<renzora_ui::DocumentTabState>()
            .and_then(|tabs| {
                tabs.tabs
                    .get(tabs.active_tab)
                    .and_then(|t| t.scene_path.as_ref().map(|p| project.resolve_path(p)))
            });
        let boot_scene_abs = if project.config.main_scene.is_empty() {
            None
        } else {
            Some(project.resolve_path(&project.config.main_scene))
        };

        let mut state = self.state.write().unwrap();

        let text_primary = theme.text.primary.to_color32();
        let text_muted = theme.text.muted.to_color32();
        let accent = theme.semantic.accent.to_color32();
        let pad = 6.0;

        ui.add_space(pad);

        // ── Header: "+ New Scene" button ─────────────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(pad);
            let btn = egui::Button::new(
                egui::RichText::new(format!("{} New Scene", regular::PLUS))
                    .size(12.0)
                    .color(text_primary),
            )
            .fill(theme.surfaces.panel.to_color32())
            .min_size(egui::vec2(ui.available_width() - pad, 26.0));
            if ui.add(btn).clicked() {
                let dir = scenes_dir.clone();
                commands.push(move |_world: &mut World| {
                    if let Err(e) = std::fs::create_dir_all(&dir) {
                        renzora::core::console_log::console_error(
                            "Scene",
                            format!("Failed to create scenes/: {e}"),
                        );
                        return;
                    }
                    let new_path = unique_scene_path(&dir, "untitled");
                    if let Err(e) = std::fs::write(&new_path, EMPTY_SCENE_RON) {
                        renzora::core::console_log::console_error(
                            "Scene",
                            format!("Failed to create scene: {e}"),
                        );
                        return;
                    }
                    renzora::core::console_log::console_success(
                        "Scene",
                        format!("Created {}", new_path.display()),
                    );
                });
            }
        });

        ui.add_space(pad);

        // ── Scene list ───────────────────────────────────────────────────
        let entries = list_scenes(&scenes_dir);

        if entries.is_empty() {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("No scenes yet. Click \"New Scene\".")
                        .size(11.0)
                        .color(text_muted),
                );
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            let row_height = 24.0;
            let row_spacing = 2.0;
            ui.spacing_mut().item_spacing.y = row_spacing;

            for path in entries {
                let is_current = current_scene_abs
                    .as_ref()
                    .map(|c| paths_equal(c, &path))
                    .unwrap_or(false);
                let is_boot = boot_scene_abs
                    .as_ref()
                    .map(|c| paths_equal(c, &path))
                    .unwrap_or(false);
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?")
                    .to_string();

                let is_renaming = state
                    .renaming
                    .as_ref()
                    .map(|(p, _)| p == &path)
                    .unwrap_or(false);

                ui.horizontal(|ui| {
                    ui.add_space(pad);

                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width() - pad, row_height),
                        egui::Sense::click(),
                    );

                    let bg = if is_current {
                        accent.linear_multiply(0.25)
                    } else if response.hovered() {
                        theme.widgets.hovered_bg.to_color32()
                    } else {
                        theme.surfaces.faint.to_color32()
                    };
                    ui.painter().rect_filled(rect, 4.0, bg);

                    if response.hovered() && !is_renaming {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Icon
                    ui.painter().text(
                        egui::pos2(rect.min.x + 8.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        regular::FILM_SLATE,
                        egui::FontId::proportional(13.0),
                        if is_current { accent } else { text_muted },
                    );

                    // Label or rename field
                    if is_renaming {
                        let text_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.min.x + 28.0, rect.min.y + 3.0),
                            egui::vec2(rect.width() - 36.0, row_height - 6.0),
                        );
                        let buffer = &mut state
                            .renaming
                            .as_mut()
                            .expect("is_renaming implies Some")
                            .1;
                        let te = ui.put(
                            text_rect,
                            egui::TextEdit::singleline(buffer)
                                .desired_width(text_rect.width())
                                .font(egui::FontId::proportional(11.0)),
                        );
                        if !state.rename_focus_set {
                            te.request_focus();
                            state.rename_focus_set = true;
                        }
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                        if enter {
                            let (old_path, new_name) = state.renaming.take().unwrap();
                            state.rename_focus_set = false;
                            let trimmed = new_name.trim().to_string();
                            if !trimmed.is_empty() && trimmed != name {
                                let new_path = old_path
                                    .with_file_name(format!("{trimmed}.ron"));
                                commands.push(move |_world: &mut World| {
                                    if let Err(e) = std::fs::rename(&old_path, &new_path) {
                                        renzora::core::console_log::console_error(
                                            "Scene",
                                            format!("Rename failed: {e}"),
                                        );
                                    } else {
                                        renzora::core::console_log::console_info(
                                            "Scene",
                                            format!("Renamed to {}", new_path.display()),
                                        );
                                    }
                                });
                            }
                        } else if cancel || te.lost_focus() {
                            state.renaming = None;
                            state.rename_focus_set = false;
                        }
                    } else {
                        ui.painter().text(
                            egui::pos2(rect.min.x + 28.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &name,
                            egui::FontId::proportional(11.5),
                            if is_current { text_primary } else { text_primary },
                        );

                        if is_boot {
                            ui.painter().text(
                                egui::pos2(rect.max.x - 10.0, rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                regular::STAR,
                                egui::FontId::proportional(12.0),
                                accent,
                            );
                        }

                        // Double-click opens scene
                        if response.double_clicked() && !is_current {
                            let target = path.clone();
                            commands.push(move |world: &mut World| {
                                open_scene(world, &target);
                            });
                        }

                        // Right-click → rename
                        response.context_menu(|ui| {
                            if ui
                                .button(format!("{} Rename", regular::PENCIL_SIMPLE))
                                .clicked()
                            {
                                state.renaming = Some((path.clone(), name.clone()));
                                state.rename_focus_set = false;
                                ui.close();
                            }
                            if ui
                                .button(format!("{} Open", regular::ARROW_SQUARE_OUT))
                                .clicked()
                            {
                                let target = path.clone();
                                commands.push(move |world: &mut World| {
                                    open_scene(world, &target);
                                });
                                ui.close();
                            }
                            if !is_boot {
                                if ui
                                    .button(format!("{} Set as Boot Scene", regular::STAR))
                                    .clicked()
                                {
                                    let target = path.clone();
                                    commands.push(move |world: &mut World| {
                                        if let Some(mut project) =
                                            world.get_resource_mut::<CurrentProject>()
                                        {
                                            if let Some(rel) = project.make_relative(&target) {
                                                project.config.main_scene = rel;
                                                if let Err(e) = project.save_config() {
                                                    renzora::core::console_log::console_error(
                                                        "Scene",
                                                        format!("Failed to save project config: {e}"),
                                                    );
                                                }
                                            }
                                        }
                                    });
                                    ui.close();
                                }
                            }
                            if ui
                                .button(format!("{} Delete", regular::TRASH))
                                .clicked()
                            {
                                let target = path.clone();
                                commands.push(move |_world: &mut World| {
                                    if let Err(e) = std::fs::remove_file(&target) {
                                        renzora::core::console_log::console_error(
                                            "Scene",
                                            format!("Delete failed: {e}"),
                                        );
                                    }
                                });
                                ui.close();
                            }
                        });
                    }
                });
            }
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}

const EMPTY_SCENE_RON: &str = "(\n    resources: {},\n    entities: {},\n)\n";

fn list_scenes(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in read.flatten() {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) == Some("ron") {
            out.push(p);
        }
    }
    out.sort_by(|a, b| {
        a.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .cmp(
                &b.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase(),
            )
    });
    out
}

fn unique_scene_path(dir: &std::path::Path, base: &str) -> PathBuf {
    let mut i = 0u32;
    loop {
        let name = if i == 0 {
            format!("{base}.ron")
        } else {
            format!("{base}_{i}.ron")
        };
        let p = dir.join(&name);
        if !p.exists() {
            return p;
        }
        i += 1;
    }
}

fn paths_equal(a: &std::path::Path, b: &std::path::Path) -> bool {
    std::fs::canonicalize(a)
        .ok()
        .zip(std::fs::canonicalize(b).ok())
        .map(|(a, b)| a == b)
        .unwrap_or_else(|| a == b)
}

fn open_scene(world: &mut World, path: &std::path::Path) {
    let relative = world
        .get_resource::<CurrentProject>()
        .and_then(|p| p.make_relative(path));

    // If a tab already has this scene open, just activate it.
    if let Some(ref rel) = relative {
        let existing_idx = world
            .get_resource::<renzora_ui::DocumentTabState>()
            .and_then(|ts| {
                ts.tabs
                    .iter()
                    .position(|t| t.scene_path.as_deref() == Some(rel.as_str()))
            });
        if let Some(idx) = existing_idx {
            if let Some(mut project) = world.get_resource_mut::<CurrentProject>() {
                if project.config.editor_last_scene.as_deref() != Some(rel.as_str()) {
                    project.config.editor_last_scene = Some(rel.clone());
                    let _ = project.save_config();
                }
            }
            let ids = world
                .get_resource_mut::<renzora_ui::DocumentTabState>()
                .and_then(|mut ts| ts.activate_tab(idx));
            if let Some((old_id, new_id)) = ids {
                world.insert_resource(renzora::core::TabSwitchRequest {
                    old_tab_id: old_id,
                    new_tab_id: new_id,
                });
            }
            return;
        }
    }

    // Otherwise: save the current tab's scene into its buffer, create a new
    // tab for this scene, and load it from disk. We perform the work inline
    // rather than firing a `TabSwitchRequest` because the target tab has no
    // buffer yet — the standard switch handler would reset to an empty scene.
    let old_tab_id = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|ts| ts.active_tab_id());

    if let Some(old_id) = old_tab_id {
        let scene_ron = scene_io::serialize_scene_to_string(world)
            .unwrap_or_else(|_| "(entities: {}, resources: {})".to_string());
        let (focus, distance, yaw, pitch) =
            if let Some(orbit) = world.get_resource::<OrbitCameraState>() {
                (orbit.focus.to_array(), orbit.distance, orbit.yaw, orbit.pitch)
            } else {
                let def = OrbitCameraState::default();
                (def.focus.to_array(), def.distance, def.yaw, def.pitch)
            };
        let snapshot = renzora::core::TabSceneSnapshot {
            scene_ron,
            camera_focus: focus,
            camera_distance: distance,
            camera_yaw: yaw,
            camera_pitch: pitch,
        };
        if let Some(mut buffers) = world.get_resource_mut::<renzora::core::SceneTabBuffers>() {
            buffers.buffers.insert(old_id, snapshot);
        }
    }

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Scene".into());
    if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        let idx = tabs.add_tab(name, relative.clone());
        tabs.active_tab = idx;
    }

    crate::despawn_scene_entities(world);
    if let Some(mut orbit) = world.get_resource_mut::<OrbitCameraState>() {
        *orbit = OrbitCameraState::default();
    }
    scene_io::load_scene(world, path);
    crate::extract_orbit_from_scene_camera(world);

    if let (Some(rel), Some(mut project)) = (
        relative,
        world.get_resource_mut::<CurrentProject>(),
    ) {
        if project.config.editor_last_scene.as_deref() != Some(rel.as_str()) {
            project.config.editor_last_scene = Some(rel);
            let _ = project.save_config();
        }
    }
}

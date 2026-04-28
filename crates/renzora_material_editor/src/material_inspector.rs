//! Inspector entry for the Material component.
//!
//! Registered automatically by `MaterialEditorPlugin`.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{
    AssetDragPayload, DocTabKind, EditorCommands, InspectorEntry, MaterialThumbnailRegistry,
};
use renzora_shader::material::material_ref::MaterialRef;
use renzora_theme::Theme;


/// Image extensions accepted for auto-material creation.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "ktx2", "tga", "bmp", "dds", "exr", "hdr", "webp"];

/// How deep we recurse when scanning the project for `.material` files. Six
/// levels covers `models/<asset>/materials/` plus a couple of hand-organized
/// subfolders on top of `assets/materials/`. Models with deeper nesting are
/// rare and the user can drop directly to bind those.
const MATERIAL_SCAN_MAX_DEPTH: usize = 6;

pub fn material_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "material_ref",
        display_name: "Material",
        icon: regular::PAINT_BRUSH,
        category: "rendering",
        has_fn: |world, entity| {
            world.get::<MaterialRef>(entity).is_some()
                || world.get::<bevy::pbr::MeshMaterial3d<bevy::pbr::StandardMaterial>>(entity).is_some()
                || world.get::<Mesh3d>(entity).is_some()
        },
        add_fn: None,
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<MaterialRef>();
            world.entity_mut(entity).remove::<renzora_shader::material::resolver::MaterialResolved>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(material_custom_ui),
    }
}

fn material_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let current_path = world
        .get::<MaterialRef>(entity)
        .map(|m| m.0.clone())
        .unwrap_or_default();

    let payload = world.get_resource::<AssetDragPayload>();

    let mut all_exts: Vec<&str> = vec!["material", "material_instance"];
    all_exts.extend_from_slice(IMAGE_EXTENSIONS);

    let current_label = if current_path.is_empty() {
        "None".to_string()
    } else {
        std::path::Path::new(&current_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(current_path.as_str())
            .to_string()
    };

    // ── Unreal-style compact slot row ────────────────────────────────────
    //
    //   ┌──────┐  ┌────────────────────── ▼ ┐ ╭─╮
    //   │      │  │  MaterialName            │ │↺│
    //   │ thumb│  └──────────────────────────┘ ╰─╯
    //   │      │   ⊕  ✎
    //   └──────┘
    //
    // Whole row accepts material/image drops. Dropdown opens the search
    // picker; ⊕ same (it's the explicit "browse" affordance shown in
    // Unreal); ✎ opens the material as a document tab; ↺ clears.

    const ROW_HEIGHT: f32 = 48.0;
    const THUMB_SIZE: f32 = 48.0;
    const GAP: f32 = 6.0;
    const REVERT_W: f32 = 22.0;
    const ICON_BTN_W: f32 = 22.0;
    const ICON_BTN_H: f32 = 18.0;

    ui.add_space(2.0);
    let avail = ui.available_width();
    let (row_rect, _row_resp) = ui.allocate_exact_size(
        egui::vec2(avail, ROW_HEIGHT),
        egui::Sense::hover(),
    );

    // Drop detection across the whole row.
    let compatible_drag = payload
        .filter(|p| p.is_detached && p.matches_extensions(&all_exts))
        .is_some();
    let pointer = ui.ctx().pointer_hover_pos();
    let pointer_in_row = pointer.map_or(false, |p| row_rect.contains(p));
    let row_hovering = compatible_drag && pointer_in_row;
    let mut dropped_path: Option<std::path::PathBuf> = None;
    if row_hovering && !ui.ctx().input(|i| i.pointer.any_down()) {
        if let Some(p) = payload {
            dropped_path = Some(p.path.clone());
        }
    }
    if row_hovering {
        let c = payload.unwrap().color;
        ui.painter().rect_filled(
            row_rect,
            egui::CornerRadius::same(3),
            egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 18),
        );
        ui.painter().rect_stroke(
            row_rect,
            egui::CornerRadius::same(3),
            egui::Stroke::new(1.5, c),
            egui::StrokeKind::Inside,
        );
    }

    // Thumbnail.
    let thumb_rect = egui::Rect::from_min_size(
        row_rect.left_top(),
        egui::vec2(THUMB_SIZE, THUMB_SIZE),
    );
    ui.painter().rect_filled(
        thumb_rect,
        egui::CornerRadius::same(3),
        egui::Color32::from_rgb(14, 14, 18),
    );
    ui.painter().rect_stroke(
        thumb_rect,
        egui::CornerRadius::same(3),
        egui::Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );
    let abs_path_opt = world
        .get_resource::<renzora::core::CurrentProject>()
        .filter(|_| !current_path.is_empty())
        .map(|p| p.resolve_path(&current_path));
    if let Some(abs_path) = abs_path_opt {
        let registry = world.get_resource::<MaterialThumbnailRegistry>();
        if let Some(tid) = registry.and_then(|r| r.get(&abs_path)) {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(tid, thumb_rect, uv, egui::Color32::WHITE);
        } else {
            ui.painter().text(
                thumb_rect.center(),
                egui::Align2::CENTER_CENTER,
                "…",
                egui::FontId::proportional(16.0),
                theme.text.muted.to_color32(),
            );
            cmds.push(move |world: &mut World| {
                if let Some(mut reg) = world.get_resource_mut::<MaterialThumbnailRegistry>() {
                    reg.request(abs_path);
                }
            });
        }
    } else {
        ui.painter().text(
            thumb_rect.center(),
            egui::Align2::CENTER_CENTER,
            "None",
            egui::FontId::proportional(11.0),
            theme.text.muted.to_color32(),
        );
    }

    // Right column geometry.
    let right_x = thumb_rect.right() + GAP;
    let revert_x = row_rect.right() - REVERT_W;
    let right_w = (revert_x - GAP - right_x).max(80.0);
    let dropdown_rect = egui::Rect::from_min_size(
        egui::pos2(right_x, row_rect.top() + 2.0),
        egui::vec2(right_w, 22.0),
    );
    let actions_y = dropdown_rect.bottom() + 4.0;

    // Dropdown — material name + chevron.
    let browse_id = ui.make_persistent_id(("material_browse", entity));
    let mut chosen_material: Option<String> = None;
    let mut open_in_editor_tab = false;
    let mut clear_clicked = false;
    let thumb_requests: std::cell::RefCell<Vec<std::path::PathBuf>> =
        std::cell::RefCell::new(Vec::new());

    let dropdown_label = format!("{}  {}", current_label, regular::CARET_DOWN);
    let dropdown_btn = ui.put(
        dropdown_rect,
        egui::Button::new(egui::RichText::new(&dropdown_label).size(11.0))
            .min_size(dropdown_rect.size())
            .truncate(),
    );
    let dropdown_btn = dropdown_btn.on_hover_text(if current_path.is_empty() {
        "Browse all .material files in the project".to_string()
    } else {
        format!("Current: {}\nClick to browse all .material files", current_path)
    });
    if dropdown_btn.clicked() {
        ui.memory_mut(|m| m.toggle_popup(browse_id));
    }

    // Action icons under the dropdown.
    let browse_icon_rect = egui::Rect::from_min_size(
        egui::pos2(right_x, actions_y),
        egui::vec2(ICON_BTN_W, ICON_BTN_H),
    );
    let browse_icon_resp = ui
        .put(
            browse_icon_rect,
            egui::Button::new(egui::RichText::new(regular::FOLDER_OPEN).size(11.0))
                .min_size(browse_icon_rect.size()),
        )
        .on_hover_text("Browse all .material files in the project");
    if browse_icon_resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(browse_id));
    }

    let edit_icon_rect = egui::Rect::from_min_size(
        egui::pos2(browse_icon_rect.right() + 4.0, actions_y),
        egui::vec2(ICON_BTN_W, ICON_BTN_H),
    );
    let edit_icon_resp = ui
        .add_enabled_ui(!current_path.is_empty(), |ui| {
            ui.put(
                edit_icon_rect,
                egui::Button::new(egui::RichText::new(regular::PENCIL_SIMPLE).size(11.0))
                    .min_size(edit_icon_rect.size()),
            )
            .on_hover_text(if current_path.is_empty() {
                "Assign a material first"
            } else {
                "Open the current material in a new document tab"
            })
        })
        .inner;
    if edit_icon_resp.clicked() {
        open_in_editor_tab = true;
    }

    // Revert / clear button on the far right.
    let revert_rect = egui::Rect::from_min_size(
        egui::pos2(revert_x, row_rect.top() + 2.0),
        egui::vec2(REVERT_W, 22.0),
    );
    let revert_resp = ui
        .add_enabled_ui(!current_path.is_empty(), |ui| {
            ui.put(
                revert_rect,
                egui::Button::new(egui::RichText::new(regular::ARROW_COUNTER_CLOCKWISE).size(11.0))
                    .min_size(revert_rect.size()),
            )
            .on_hover_text(if current_path.is_empty() {
                "No material to clear"
            } else {
                "Clear material binding"
            })
        })
        .inner;
    if revert_resp.clicked() {
        clear_clicked = true;
    }

    // Picker popup anchored under the dropdown button.
    let popup_width = dropdown_btn.rect.width();
    let registry = world.get_resource::<MaterialThumbnailRegistry>();
    egui::popup_below_widget(
        ui,
        browse_id,
        &dropdown_btn,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_width(popup_width);

            let search_id = browse_id.with("search");
            let mut query: String = ui
                .memory_mut(|m| m.data.get_temp::<String>(search_id))
                .unwrap_or_default();

            let resp = ui.add(
                egui::TextEdit::singleline(&mut query)
                    .hint_text(format!("{} Search materials…", regular::MAGNIFYING_GLASS))
                    .desired_width(popup_width - 8.0),
            );
            if resp.changed() {
                ui.memory_mut(|m| m.data.insert_temp(search_id, query.clone()));
            }
            if !ui.memory(|m| m.has_focus(resp.id)) {
                resp.request_focus();
            }

            ui.add_space(2.0);
            ui.separator();

            let project_root = world
                .get_resource::<renzora::core::CurrentProject>()
                .map(|p| p.path.clone());
            let materials = match project_root {
                Some(root) => find_material_files(&root),
                None => Vec::new(),
            };

            if materials.is_empty() {
                ui.label(
                    egui::RichText::new("No .material files found in project")
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                );
                return;
            }

            let lower_q = query.trim().to_ascii_lowercase();
            let filtered: Vec<&(String, String)> = materials
                .iter()
                .filter(|(rel, _abs)| {
                    lower_q.is_empty() || rel.to_ascii_lowercase().contains(&lower_q)
                })
                .collect();

            if filtered.is_empty() {
                ui.label(
                    egui::RichText::new("No matches")
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                );
                return;
            }

            const VISIBLE_LIMIT: usize = 200;
            const PICKER_THUMB: f32 = 18.0;
            const PICKER_ROW_H: f32 = 26.0;
            let hidden = filtered.len().saturating_sub(VISIBLE_LIMIT);

            egui::ScrollArea::vertical()
                .max_height(280.0)
                .show(ui, |ui| {
                    for (rel_path, abs_path) in filtered.iter().take(VISIBLE_LIMIT) {
                        let is_current = rel_path.as_str() == current_path.as_str();
                        let path = std::path::Path::new(rel_path.as_str());
                        let stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(rel_path.as_str())
                            .to_string();
                        let parent_str = path
                            .parent()
                            .and_then(|p| p.to_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string());

                        let row_w = ui.available_width();
                        let (row_rect, row_resp) = ui.allocate_exact_size(
                            egui::vec2(row_w, PICKER_ROW_H),
                            egui::Sense::click(),
                        );

                        if row_resp.hovered() {
                            ui.painter().rect_filled(
                                row_rect,
                                egui::CornerRadius::same(3),
                                egui::Color32::from_white_alpha(14),
                            );
                        }
                        if is_current {
                            ui.painter().rect_stroke(
                                row_rect.shrink(0.5),
                                egui::CornerRadius::same(3),
                                egui::Stroke::new(
                                    1.0,
                                    theme.semantic.accent.to_color32(),
                                ),
                                egui::StrokeKind::Inside,
                            );
                        }

                        let thumb_rect = egui::Rect::from_min_size(
                            row_rect.left_top()
                                + egui::vec2(4.0, (PICKER_ROW_H - PICKER_THUMB) * 0.5),
                            egui::vec2(PICKER_THUMB, PICKER_THUMB),
                        );
                        ui.painter().rect_filled(
                            thumb_rect,
                            egui::CornerRadius::same(2),
                            egui::Color32::from_rgb(14, 14, 18),
                        );
                        let abs_pb = std::path::PathBuf::from(abs_path);
                        let tex_id = registry.and_then(|r| r.get(&abs_pb));
                        if let Some(tid) = tex_id {
                            let uv = egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            );
                            ui.painter().image(
                                tid,
                                thumb_rect,
                                uv,
                                egui::Color32::WHITE,
                            );
                        } else {
                            thumb_requests.borrow_mut().push(abs_pb);
                        }

                        let text_x = thumb_rect.right() + 6.0;
                        let text_clip = egui::Rect::from_min_max(
                            egui::pos2(text_x, row_rect.top()),
                            row_rect.right_bottom(),
                        );
                        let painter = ui.painter().with_clip_rect(text_clip);
                        let name_color = if is_current {
                            theme.semantic.accent.to_color32()
                        } else {
                            theme.text.primary.to_color32()
                        };
                        let name_y = if parent_str.is_some() {
                            row_rect.top() + 4.0
                        } else {
                            row_rect.center().y - 6.0
                        };
                        painter.text(
                            egui::pos2(text_x, name_y),
                            egui::Align2::LEFT_TOP,
                            &stem,
                            egui::FontId::proportional(11.0),
                            name_color,
                        );
                        if let Some(parent) = parent_str {
                            painter.text(
                                egui::pos2(text_x, row_rect.top() + 14.0),
                                egui::Align2::LEFT_TOP,
                                parent,
                                egui::FontId::proportional(9.0),
                                theme.text.muted.to_color32(),
                            );
                        }

                        let click = row_resp.on_hover_text(rel_path.as_str());

                        if click.clicked() {
                            chosen_material = Some(rel_path.clone());
                            ui.memory_mut(|m| m.close_popup(browse_id));
                        }
                    }
                    if hidden > 0 {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("…{} more (refine search)", hidden))
                                .size(10.0)
                                .color(theme.text.muted.to_color32()),
                        );
                    }
                });
        },
    );

    // Drain queued thumbnail requests outside the popup closure (where the
    // World is borrowed immutably). The registry dedups internally.
    {
        let pending: Vec<std::path::PathBuf> = thumb_requests.into_inner();
        if !pending.is_empty() {
            cmds.push(move |world: &mut World| {
                if let Some(mut reg) = world.get_resource_mut::<MaterialThumbnailRegistry>() {
                    for p in pending {
                        reg.request(p);
                    }
                }
            });
        }
    }

    if let Some(rel_path) = chosen_material {
        cmds.push(move |world: &mut World| {
            world.entity_mut(entity).remove::<renzora_shader::material::resolver::MaterialResolved>();
            if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
                mr.0 = rel_path;
            } else {
                world.entity_mut(entity).insert(MaterialRef(rel_path));
            }
        });
    }

    if open_in_editor_tab && !current_path.is_empty() {
        let mat_path = current_path.clone();
        cmds.push(move |world: &mut World| {
            let abs = world
                .get_resource::<renzora::core::CurrentProject>()
                .map(|p| p.resolve_path(&mat_path))
                .unwrap_or_else(|| std::path::PathBuf::from(&mat_path));
            renzora_editor::open_asset_tab(world, &abs, DocTabKind::Material);
        });
    }

    if clear_clicked {
        cmds.push(move |world: &mut World| {
            world.entity_mut(entity).remove::<MaterialRef>();
            world.entity_mut(entity).remove::<renzora_shader::material::resolver::MaterialResolved>();
            world.entity_mut(entity).remove::<MeshMaterial3d<renzora_shader::material::runtime::GraphMaterial>>();
            let default_mat = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
            world.entity_mut(entity).insert(MeshMaterial3d(default_mat));
        });
    }

    if let Some(dropped) = dropped_path {
        let ext = dropped
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if ext == "material" || ext == "material_instance" {
            cmds.push(move |world: &mut World| {
                let mat_path = if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
                    project.make_asset_relative(&dropped)
                } else {
                    dropped.to_string_lossy().to_string()
                };
                world.entity_mut(entity).remove::<renzora_shader::material::resolver::MaterialResolved>();
                if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
                    mr.0 = mat_path;
                } else {
                    world.entity_mut(entity).insert(MaterialRef(mat_path));
                }
            });
        } else if IMAGE_EXTENSIONS.iter().any(|e| ext == *e) {
            cmds.push(move |world: &mut World| {
                let (tex_path, mat_save_dir) = {
                    let project = world.get_resource::<renzora::core::CurrentProject>();
                    let tex = if let Some(p) = project {
                        p.make_asset_relative(&dropped)
                    } else {
                        dropped.to_string_lossy().to_string()
                    };
                    let dir = project
                        .map(|p| p.path.join("materials"))
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    (tex, dir)
                };

                let mat_name = dropped
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("material")
                    .to_string();

                let mut graph = renzora_shader::material::graph::MaterialGraph::new(
                    &mat_name,
                    renzora_shader::material::graph::MaterialDomain::Surface,
                );
                let tex_id = graph.add_node("texture/sample", [-200.0, 0.0]);
                if let Some(node) = graph.get_node_mut(tex_id) {
                    node.input_values.insert(
                        "texture".to_string(),
                        renzora_shader::material::graph::PinValue::TexturePath(tex_path),
                    );
                }
                let output_id = graph.output_node().unwrap().id;
                graph.connect(tex_id, "color", output_id, "base_color");

                let _ = std::fs::create_dir_all(&mat_save_dir);
                let mat_file = mat_save_dir.join(format!("{}.material", mat_name));
                if let Ok(json) = serde_json::to_string_pretty(&graph) {
                    let _ = std::fs::write(&mat_file, &json);
                }

                let mat_asset_path = {
                    let project = world.get_resource::<renzora::core::CurrentProject>();
                    if let Some(p) = project {
                        p.make_asset_relative(&mat_file)
                    } else {
                        mat_file.to_string_lossy().to_string()
                    }
                };

                world.entity_mut(entity).remove::<renzora_shader::material::resolver::MaterialResolved>();
                if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
                    mr.0 = mat_asset_path;
                } else {
                    world.entity_mut(entity).insert(MaterialRef(mat_asset_path));
                }
            });
        }
    }
}

/// Walk the project root for `.material` files and return their
/// `(asset_relative_path, absolute_path)` pairs sorted alphabetically.
///
/// Bounded to [`MATERIAL_SCAN_MAX_DEPTH`] levels to keep the scan cheap. The
/// browse popup rebuilds this list every frame it's open — for a project
/// with hundreds of materials this is well under a millisecond on a SSD,
/// and avoiding a cache means added/renamed files show up immediately.
fn find_material_files(project_root: &std::path::Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(project_root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            // Skip dotfiles / hidden dirs (.git, .vscode) and target dirs —
            // they're noisy and never contain user-authored materials.
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') || name == "target" {
                    continue;
                }
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                if depth + 1 < MATERIAL_SCAN_MAX_DEPTH {
                    stack.push((path, depth + 1));
                }
            } else if ft.is_file()
                && matches!(
                    path.extension().and_then(|e| e.to_str()),
                    Some("material") | Some("material_instance")
                )
            {
                if let Ok(rel) = path.strip_prefix(project_root) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    out.push((rel_str, path.to_string_lossy().to_string()));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

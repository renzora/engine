//! Inspector entry for the Material component.
//!
//! Registered automatically by `MaterialEditorPlugin`.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor_framework::{
    asset_drop_target, AssetDragPayload, EditorCommands, InspectorEntry, MaterialThumbnailRegistry,
};
use renzora_shader::material::material_ref::MaterialRef;
use renzora_theme::Theme;

use crate::{MaterialEditorState, MaterialEditMode};

/// Image extensions accepted for auto-material creation.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "ktx2", "tga", "bmp", "dds", "exr", "hdr", "webp"];

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

    let mut all_exts: Vec<&str> = vec!["material"];
    all_exts.extend_from_slice(IMAGE_EXTENSIONS);
    let ext_refs: Vec<&str> = all_exts;

    let current_display = if current_path.is_empty() { None } else { Some(current_path.as_str()) };

    let drop_result = asset_drop_target(
        ui,
        egui::Id::new(("material_drop", entity)),
        current_display,
        &ext_refs,
        "Drop material or image",
        theme,
        payload,
    );

    // Thumbnail preview — when the entity has a material, look it up in the
    // shared thumbnail registry and render it as a small preview below the
    // drop target. If the thumbnail isn't cached yet (e.g. material was just
    // applied at runtime), queue a render request.
    if !current_path.is_empty() {
        let abs_path_opt = world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.resolve_path(&current_path));
        if let Some(abs_path) = abs_path_opt {
            let registry = world.get_resource::<MaterialThumbnailRegistry>();
            let tex_id = registry.and_then(|r| r.get(&abs_path));
            let size = egui::vec2(96.0, 96.0);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    egui::CornerRadius::same(4),
                    egui::Color32::from_rgb(14, 14, 18),
                );
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(4),
                    egui::Stroke::new(1.0, theme.widgets.border.to_color32()),
                    egui::StrokeKind::Inside,
                );
                if let Some(tid) = tex_id {
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    ui.painter().image(tid, rect, uv, egui::Color32::WHITE);
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "…",
                        egui::FontId::proportional(18.0),
                        theme.text.muted.to_color32(),
                    );
                    // Request a render so subsequent frames have the thumbnail.
                    cmds.push(move |world: &mut World| {
                        if let Some(mut reg) = world.get_resource_mut::<MaterialThumbnailRegistry>() {
                            reg.request(abs_path);
                        }
                    });
                }
            });
        }
    }

    if let Some(ref dropped) = drop_result.dropped_path {
        let ext = dropped
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let dropped_path = dropped.clone();
        if ext == "material" {
            cmds.push(move |world: &mut World| {
                let mat_path = if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
                    project.make_asset_relative(&dropped_path)
                } else {
                    dropped_path.to_string_lossy().to_string()
                };

                let fs_path = if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
                    project.resolve_path(&mat_path).to_string_lossy().to_string()
                } else {
                    mat_path.clone()
                };
                if let Ok(json) = std::fs::read_to_string(&fs_path) {
                    if let Ok(graph) = serde_json::from_str::<renzora_shader::material::graph::MaterialGraph>(&json) {
                        let result = renzora_shader::material::codegen::compile(&graph);
                        let mut state = world.resource_mut::<MaterialEditorState>();
                        state.editing_entity = Some(entity);
                        state.compiled_wgsl = Some(result.fragment_shader);
                        state.compile_errors = result.errors;
                        state.graph = graph;
                        state.edit_mode = MaterialEditMode::Existing { path: mat_path.clone(), entity };
                    }
                }

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
                        p.make_asset_relative(&dropped_path)
                    } else {
                        dropped_path.to_string_lossy().to_string()
                    };
                    let dir = project
                        .map(|p| p.path.join("materials"))
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    (tex, dir)
                };

                let mat_name = dropped_path
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

                let result = renzora_shader::material::codegen::compile(&graph);
                {
                    let mut state = world.resource_mut::<MaterialEditorState>();
                    state.editing_entity = Some(entity);
                    state.compiled_wgsl = Some(result.fragment_shader);
                    state.compile_errors = result.errors;
                    state.graph = graph;
                    state.edit_mode = MaterialEditMode::Existing { path: mat_asset_path.clone(), entity };
                }

                world.entity_mut(entity).remove::<renzora_shader::material::resolver::MaterialResolved>();
                if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
                    mr.0 = mat_asset_path;
                } else {
                    world.entity_mut(entity).insert(MaterialRef(mat_asset_path));
                }
            });
        }
    }

    if drop_result.cleared {
        cmds.push(move |world: &mut World| {
            world.entity_mut(entity).remove::<MaterialRef>();
            world.entity_mut(entity).remove::<renzora_shader::material::resolver::MaterialResolved>();
            world.entity_mut(entity).remove::<MeshMaterial3d<renzora_shader::material::runtime::GraphMaterial>>();
            let default_mat = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
            world.entity_mut(entity).insert(MeshMaterial3d(default_mat));
        });
    }
}

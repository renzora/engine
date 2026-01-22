use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, TextureId, Vec2};

use crate::core::{EditorEntity, EditorState, WorldEnvironmentMarker};
use crate::node_system::{
    render_camera_inspector, render_directional_light_inspector, render_point_light_inspector,
    render_script_inspector, render_spot_light_inspector, render_transform_inspector,
    render_world_environment_inspector, CameraNodeData,
};
use crate::scripting::{ScriptComponent, ScriptRegistry, RhaiScriptEngine};

// Phosphor icons for inspector
use egui_phosphor::regular::{
    SLIDERS, ARROWS_OUT_CARDINAL, GLOBE, LIGHTBULB, SUN, FLASHLIGHT,
    PLUS, MAGNIFYING_GLASS, CHECK_CIRCLE, CODE, VIDEO_CAMERA,
};

/// System parameter that bundles all inspector-related queries
#[derive(SystemParam)]
pub struct InspectorQueries<'w, 's> {
    pub transforms: Query<'w, 's, &'static mut Transform>,
    pub world_environments: Query<'w, 's, &'static mut WorldEnvironmentMarker>,
    pub point_lights: Query<'w, 's, &'static mut PointLight>,
    pub directional_lights: Query<'w, 's, &'static mut DirectionalLight>,
    pub spot_lights: Query<'w, 's, &'static mut SpotLight>,
    pub scripts: Query<'w, 's, &'static mut ScriptComponent>,
    pub cameras: Query<'w, 's, &'static mut CameraNodeData>,
}

pub fn render_inspector(
    ctx: &egui::Context,
    editor_state: &EditorState,
    entities: &Query<(Entity, &EditorEntity)>,
    queries: &mut InspectorQueries,
    script_registry: &ScriptRegistry,
    rhai_engine: &RhaiScriptEngine,
    _right_panel_width: f32,
    camera_preview_texture_id: Option<TextureId>,
) {
    egui::SidePanel::right("inspector")
        .default_width(320.0)
        .resizable(true)
        .show(ctx, |ui| {
            render_inspector_content(ui, editor_state, entities, queries, script_registry, rhai_engine, camera_preview_texture_id);
        });
}

/// Render inspector content (for use in docking)
pub fn render_inspector_content(
    ui: &mut egui::Ui,
    editor_state: &EditorState,
    entities: &Query<(Entity, &EditorEntity)>,
    queries: &mut InspectorQueries,
    script_registry: &ScriptRegistry,
    rhai_engine: &RhaiScriptEngine,
    camera_preview_texture_id: Option<TextureId>,
) {
    let panel_width = ui.available_width();

    ui.horizontal(|ui| {
        ui.label(RichText::new(SLIDERS).size(18.0).color(Color32::from_rgb(140, 191, 242)));
        ui.heading("Inspector");
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(selected) = editor_state.selected_entity {
            if let Ok((_, editor_entity)) = entities.get(selected) {
                // Entity name with accent bar
                ui.horizontal(|ui| {
                    // Accent bar
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(66, 150, 250));

                    ui.label(RichText::new(&editor_entity.name).size(16.0).strong());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(CHECK_CIRCLE).color(Color32::from_rgb(89, 191, 115)));
                        ui.label(RichText::new("Active").color(Color32::from_rgb(89, 191, 115)));
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Transform section
                if let Ok(mut transform) = queries.transforms.get_mut(selected) {
                    egui::CollapsingHeader::new(RichText::new(format!("{} Transform", ARROWS_OUT_CARDINAL)))
                        .default_open(true)
                        .show(ui, |ui| {
                            render_transform_inspector(ui, &mut transform);
                        });

                    ui.add_space(4.0);
                }

                // World Environment
                if let Ok(mut world_env) = queries.world_environments.get_mut(selected) {
                    egui::CollapsingHeader::new(RichText::new(format!("{} World Environment", GLOBE)))
                        .default_open(true)
                        .show(ui, |ui| {
                            render_world_environment_inspector(ui, &mut world_env);
                        });

                    ui.add_space(4.0);
                }

                // Lights
                if let Ok(mut point_light) = queries.point_lights.get_mut(selected) {
                    egui::CollapsingHeader::new(RichText::new(format!("{} Point Light", LIGHTBULB)))
                        .default_open(true)
                        .show(ui, |ui| {
                            render_point_light_inspector(ui, &mut point_light);
                        });

                    ui.add_space(4.0);
                }

                if let Ok(mut dir_light) = queries.directional_lights.get_mut(selected) {
                    egui::CollapsingHeader::new(RichText::new(format!("{} Directional Light", SUN)))
                        .default_open(true)
                        .show(ui, |ui| {
                            render_directional_light_inspector(ui, &mut dir_light);
                        });

                    ui.add_space(4.0);
                }

                if let Ok(mut spot_light) = queries.spot_lights.get_mut(selected) {
                    egui::CollapsingHeader::new(RichText::new(format!("{} Spot Light", FLASHLIGHT)))
                        .default_open(true)
                        .show(ui, |ui| {
                            render_spot_light_inspector(ui, &mut spot_light);
                        });

                    ui.add_space(4.0);
                }

                // Camera component
                if let Ok(mut camera_data) = queries.cameras.get_mut(selected) {
                    egui::CollapsingHeader::new(RichText::new(format!("{} Camera3D", VIDEO_CAMERA)))
                        .default_open(true)
                        .show(ui, |ui| {
                            render_camera_inspector(ui, &mut camera_data, camera_preview_texture_id);
                        });

                    ui.add_space(4.0);
                }

                // Script component
                if let Ok(mut script) = queries.scripts.get_mut(selected) {
                    egui::CollapsingHeader::new(RichText::new(format!("{} Script", CODE)))
                        .default_open(true)
                        .show(ui, |ui| {
                            render_script_inspector(ui, &mut script, script_registry, rhai_engine);
                        });

                    ui.add_space(4.0);
                }

                ui.add_space(16.0);

                // Add Component button
                if ui.add_sized(
                    Vec2::new(panel_width - 20.0, 28.0),
                    egui::Button::new(format!("{} Add Component", PLUS)),
                ).clicked() {
                    // TODO: Add component popup
                }
            }
        } else {
            // No selection state
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(MAGNIFYING_GLASS).size(32.0).color(Color32::from_rgb(80, 80, 90)));
                ui.add_space(8.0);
                ui.label(RichText::new("No Selection").weak());
                ui.add_space(8.0);
                ui.label(RichText::new("Select an entity in the Hierarchy\nor click on an object in the Scene.").weak());
            });
        }
    });
}

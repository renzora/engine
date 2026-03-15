//! Studio Preview Panel — displays the isolated animation preview viewport.

use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::EguiUserTextures;
use egui_phosphor::regular;

use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::studio_preview::{StudioPreviewImage, StudioPreviewOrbit};

pub struct StudioPreviewPanel;

impl EditorPanel for StudioPreviewPanel {
    fn id(&self) -> &str {
        "studio_preview"
    }

    fn title(&self) -> &str {
        "Studio Preview"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::VIDEO_CAMERA)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }

    fn min_size(&self) -> [f32; 2] {
        [280.0, 200.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let preview = world.get_resource::<StudioPreviewImage>();
        let user_textures = world.get_resource::<EguiUserTextures>();

        let texture_id = preview.and_then(|p| {
            p.texture_id.or_else(|| {
                user_textures.and_then(|ut| ut.image_id(p.handle.id()))
            })
        });

        if let Some(texture_id) = texture_id {
            let available = ui.available_size();
            let response = ui.add(egui::Image::new(egui::load::SizedTexture::new(
                texture_id,
                [available.x, available.y],
            )).sense(egui::Sense::click_and_drag()));

            // Orbit controls: drag to rotate, scroll to zoom
            if let Some(orbit) = world.get_resource::<StudioPreviewOrbit>() {
                let mut new_yaw = orbit.yaw;
                let mut new_pitch = orbit.pitch;
                let mut new_distance = orbit.distance;

                if response.dragged_by(egui::PointerButton::Primary) {
                    let delta = response.drag_delta();
                    new_yaw -= delta.x * 0.01;
                    new_pitch = (new_pitch - delta.y * 0.01).clamp(-1.4, 1.4);
                }

                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll.abs() > 0.0 {
                        new_distance = (new_distance - scroll * 0.01).clamp(0.5, 20.0);
                    }
                }

                if (new_yaw - orbit.yaw).abs() > 1e-5
                    || (new_pitch - orbit.pitch).abs() > 1e-5
                    || (new_distance - orbit.distance).abs() > 1e-5
                {
                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            let mut orbit = world.resource_mut::<StudioPreviewOrbit>();
                            orbit.yaw = new_yaw;
                            orbit.pitch = new_pitch;
                            orbit.distance = new_distance;
                        });
                    }
                }
            }
        } else {
            let (text_color, hint_color) = world
                .get_resource::<ThemeManager>()
                .map(|tm| (
                    tm.active_theme.text.muted.to_color32(),
                    tm.active_theme.text.secondary.to_color32(),
                ))
                .unwrap_or((
                    egui::Color32::from_white_alpha(80),
                    egui::Color32::from_white_alpha(50),
                ));

            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.35);
                ui.label(
                    egui::RichText::new(regular::VIDEO_CAMERA)
                        .size(32.0)
                        .color(text_color),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Select an animated entity")
                        .size(13.0)
                        .color(text_color),
                );
                ui.label(
                    egui::RichText::new("to preview animations here")
                        .size(11.0)
                        .color(hint_color),
                );
            });
        }
    }
}

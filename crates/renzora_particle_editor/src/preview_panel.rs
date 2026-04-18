//! Particle Preview Panel — displays the live particle effect preview texture.

use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::EguiUserTextures;
use egui_phosphor::regular::EYE;

use renzora_editor_framework::EditorPanel;
use renzora_theme::ThemeManager;

use crate::preview::ParticlePreviewImage;

pub struct ParticlePreviewPanel;

impl EditorPanel for ParticlePreviewPanel {
    fn id(&self) -> &str {
        "particle_preview"
    }

    fn title(&self) -> &str {
        "Particle Preview"
    }

    fn icon(&self) -> Option<&str> {
        Some(EYE)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let preview = world.get_resource::<ParticlePreviewImage>();
        let user_textures = world.get_resource::<EguiUserTextures>();

        let texture_id = preview.and_then(|p| {
            p.texture_id.or_else(|| {
                user_textures.and_then(|ut| ut.image_id(p.handle.id()))
            })
        });

        if let Some(texture_id) = texture_id {
            let available = ui.available_size();
            ui.add(egui::Image::new(egui::load::SizedTexture::new(
                texture_id,
                [available.x, available.y],
            )));
        } else {
            let text_color = world
                .get_resource::<ThemeManager>()
                .map(|tm| tm.active_theme.text.muted.to_color32())
                .unwrap_or(egui::Color32::from_white_alpha(80));

            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No preview available").color(text_color));
            });
        }
    }
}

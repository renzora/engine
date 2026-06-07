//! Bevy-native (ember) port of the egui `ParticlePreviewPanel`: displays the
//! live particle-effect preview render texture, or a "No preview available"
//! note when it isn't ready.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_with};
use renzora_ember::theme::*;

use crate::preview::ParticlePreviewImage;

pub struct NativeParticlePreview;

impl Plugin for NativeParticlePreview {
    fn build(&self, app: &mut App) {
        app.register_panel_content("particle_preview", false, build);
    }
}

fn ready(w: &World) -> bool {
    w.get_resource::<ParticlePreviewImage>().is_some_and(|p| p.handle != Handle::default())
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(window_bg())),
            Name::new("native-particle-preview"),
        ))
        .id();

    let note = commands.spawn((Text::new("No preview available"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())))).id();
    bind_display(commands, note, |w| !ready(w));

    let img = commands
        .spawn((
            ImageNode::default(),
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            Name::new("particle-preview-image"),
        ))
        .id();
    bind_display(commands, img, ready);
    bind_with(
        commands,
        img,
        |w| w.get_resource::<ParticlePreviewImage>().map(|p| p.handle.clone()),
        |w, e, h: &Option<Handle<Image>>| {
            if let (Some(h), Some(mut n)) = (h, w.get_mut::<ImageNode>(e)) {
                if n.image != *h {
                    n.image = h.clone();
                }
            }
        },
    );

    commands.entity(root).add_children(&[note, img]);
    renzora_editor_framework::mark_drop_zone(commands, root);
    root
}

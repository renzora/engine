//! Bevy-native (ember) port of the egui `ParticlePreviewPanel`: displays the
//! live particle-effect preview render texture, or a "No preview available"
//! note when it isn't ready.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, bind_with};
use renzora_ember::theme::*;
use renzora_ember::widgets::toggle_switch;

use crate::preview::{ParticlePreviewImage, ParticlePreviewSettings};

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
            // `Interaction` (picking-aware) lets the orbit-input system tell when
            // the cursor is genuinely over the preview vs. over a dock splitter.
            Interaction::default(),
            crate::preview::ParticlePreviewViewport,
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

    // Floating toolbar: floor (checkerboard plane) toggle.
    let toolbar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(6.0),
                left: Val::Px(6.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.08, 0.1, 0.7)),
            Name::new("particle-preview-toolbar"),
        ))
        .id();
    let floor_sw = toggle_switch(commands, true);
    bind_2way(
        commands,
        floor_sw,
        |w| w.get_resource::<ParticlePreviewSettings>().map(|s| s.show_floor).unwrap_or(true),
        |w, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ParticlePreviewSettings>() {
                s.show_floor = *v;
            }
        },
    );
    let floor_lbl = commands
        .spawn((Text::new("Floor"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(toolbar).add_children(&[floor_sw, floor_lbl]);

    commands.entity(root).add_children(&[note, img, toolbar]);
    renzora_editor_framework::mark_drop_zone(commands, root);
    root
}

//! Bevy-native (ember) port of the egui `CameraPreviewPanel`: shows the camera
//! preview render texture (`CameraPreviewState.image_handle`) filling the panel,
//! with a camera-name header + default-camera star, and an empty state when no
//! camera is being previewed. Reports its pixel rect to `PreviewResizeRequest`
//! so the preview renders at native resolution.

use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::ui::ComputedNode;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_with};
use renzora_ember::theme::*;
use renzora_editor_framework::SplashState;

use crate::camera_preview::{CameraPreviewState, PreviewResizeRequest};

/// Marks the native preview image — also used as the bevy_ui "panel mounted"
/// signal so the preview camera renders even without the egui `DockingState`.
#[derive(Component)]
pub struct NativeCamPreview;

pub fn register(app: &mut App) {
    app.register_panel_content("camera_preview", false, build);
    app.add_systems(Update, report_geometry.run_if(in_state(SplashState::Editor)));
}

fn previewing(w: &World) -> Option<Entity> {
    w.get_resource::<CameraPreviewState>().and_then(|s| s.previewing)
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-camera-preview"),
        ))
        .id();

    // Empty state.
    let empty = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() })
        .id();
    let empty_lbl = commands.spawn((Text::new(renzora::lang::t("viewport.no_cameras")), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())))).id();
    commands.entity(empty).add_child(empty_lbl);
    bind_display(commands, empty, |w| previewing(w).is_none());

    // Body: header (name + star) over the preview image.
    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    bind_display(commands, body, |w| previewing(w).is_some());

    let header = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(header_bg()))))
        .id();
    let name = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, name, |w| {
        previewing(w)
            .and_then(|e| w.get::<Name>(e).map(|n| n.as_str().to_string()))
            .unwrap_or_else(|| "Camera".to_string())
    });
    let star = icon_text(commands, &fonts.phosphor, "star", (255, 200, 80), 10.0);
    bind_display(commands, star, |w| {
        previewing(w).is_some_and(|e| w.get::<renzora::core::DefaultCamera>(e).is_some())
    });
    commands.entity(header).add_children(&[name, star]);

    let img = commands
        .spawn((
            ImageNode::default(),
            Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.10)),
            NativeCamPreview,
            Name::new("camera-preview-image"),
        ))
        .id();
    bind_with(
        commands,
        img,
        |w| w.get_resource::<CameraPreviewState>().map(|s| s.image_handle.clone()),
        |w, e, h: &Option<Handle<Image>>| {
            if let (Some(h), Some(mut n)) = (h, w.get_mut::<ImageNode>(e)) {
                if n.image != *h {
                    n.image = h.clone();
                }
            }
        },
    );
    commands.entity(body).add_children(&[header, img]);

    commands.entity(root).add_children(&[empty, body]);
    root
}

/// Report the preview image's physical-pixel size to `PreviewResizeRequest` so
/// the render texture matches the panel (crisp, not upscaled).
fn report_geometry(q: Query<&ComputedNode, With<NativeCamPreview>>, req: Option<Res<PreviewResizeRequest>>) {
    let Some(req) = req else { return };
    for cn in &q {
        let size = cn.size(); // physical px
        req.width.store((size.x.max(64.0)) as u32, Ordering::Relaxed);
        req.height.store((size.y.max(64.0)) as u32, Ordering::Relaxed);
    }
}

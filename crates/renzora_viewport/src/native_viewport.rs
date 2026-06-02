//! Bevy-native (ember) viewport panel.
//!
//! The 3D display + interaction are decoupled from the viewport's egui chrome:
//! the editor camera renders to an off-screen image (`Viewports.slots[i].image`)
//! and every interactive system (gizmo, drop, navigation) acts through screen
//! geometry published in [`ViewportResizeRequest`]. So the native panel only has
//! to (1) show that image via an `ImageNode`, and (2) report its on-screen rect +
//! hover — exactly what the egui `ViewportPanel::ui` did. The header bar / mode
//! switch / overlays are a later increment; this is the display + interaction
//! core (which is what makes the scene visible and drag-to-viewport work).

use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::core::viewport_types::Viewports;
use renzora_ember::font::EmberFonts;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::bind_with;

use crate::ViewportResizeRequest;

/// Dock panel id per viewport slot (slot 0 keeps the historical `"viewport"`).
const PANEL_IDS: [&str; 4] = ["viewport", "viewport-2", "viewport-3", "viewport-4"];

#[derive(Component)]
struct NativeViewport(usize);

pub fn register_native_viewport(app: &mut App) {
    use renzora_editor::SplashState;
    for (i, id) in PANEL_IDS.iter().enumerate() {
        // `scroll = false`: the camera image fills the panel.
        app.register_panel_content(id, false, move |commands, fonts| build_viewport(commands, fonts, i));
    }
    app.add_systems(
        Update,
        report_viewport_geometry.run_if(in_state(SplashState::Editor)),
    );
}

fn build_viewport(commands: &mut Commands, _fonts: &EmberFonts, index: usize) -> Entity {
    let img = commands
        .spawn((
            ImageNode::default(),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.10)),
            RelativeCursorPosition::default(),
            NativeViewport(index),
            Name::new("native-viewport"),
        ))
        .id();
    // Drive the displayed image from this slot's camera render target.
    bind_with(
        commands,
        img,
        move |w| {
            w.get_resource::<Viewports>()
                .and_then(|v| v.slots.get(index))
                .and_then(|s| s.image.clone())
        },
        |w, e, handle: &Option<Handle<Image>>| {
            if let (Some(handle), Some(mut node)) = (handle, w.get_mut::<ImageNode>(e)) {
                node.image = handle.clone();
            }
        },
    );
    img
}

/// Publish each native viewport's on-screen rect + hover to
/// [`ViewportResizeRequest`] (logical px, matching the egui panel) so the
/// resolver resizes the render image and the gizmo/drop/nav systems can map the
/// cursor into the scene.
fn report_viewport_geometry(
    viewports: Query<(
        &ComputedNode,
        &RelativeCursorPosition,
        &NativeViewport,
        Option<&GlobalTransform>,
    )>,
    req: Option<Res<ViewportResizeRequest>>,
) {
    let Some(req) = req else {
        return;
    };
    for (cn, rcp, vp, gt) in &viewports {
        let Some(slot) = req.slots.get(vp.0) else {
            continue;
        };
        let inv = cn.inverse_scale_factor();
        let size = cn.size() * inv;
        // Size + hover drive the render-image resize — always reported.
        slot.width.store(size.x.max(1.0) as u32, Ordering::Relaxed);
        slot.height.store(size.y.max(1.0) as u32, Ordering::Relaxed);
        slot.hovered.store(rcp.cursor_over, Ordering::Relaxed);
        // Screen top-left (logical px) is for cursor→scene raycasting.
        if let Some(gt) = gt {
            let center = gt.translation().truncate() * inv;
            let top_left = center - size * 0.5;
            slot.screen_x.store(top_left.x.to_bits(), Ordering::Relaxed);
            slot.screen_y.store(top_left.y.to_bits(), Ordering::Relaxed);
        }
    }
}

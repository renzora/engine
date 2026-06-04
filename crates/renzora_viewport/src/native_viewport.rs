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
use bevy::window::PrimaryWindow;

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
    crate::native_header::register(app);
}

fn build_viewport(commands: &mut Commands, fonts: &EmberFonts, index: usize) -> Entity {
    // Persistent content area — carries the `NativeViewport` marker (so the
    // reported viewport rect for gizmos/drops stays valid in every view mode)
    // and hosts the 3D image plus, on the primary slot in UI view, the embedded
    // UI editor.
    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.10)),
            RelativeCursorPosition::default(),
            NativeViewport(index),
            Name::new("native-viewport"),
        ))
        .id();

    let img = commands
        .spawn((
            ImageNode::default(),
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            Name::new("native-viewport-image"),
        ))
        .id();
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
    commands.entity(content).add_child(img);

    // The primary viewport (slot 0) owns the shared header + the UI editor; the
    // extra slots are bare camera-angle views.
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("native-viewport-root"),
        ))
        .id();
    if index == 0 {
        use renzora::core::viewport_types::{ViewportSettings, ViewportView};
        // In UI view the 3D image hides and the embedded UI editor (toolbar +
        // scene backdrop + UI render + selection handles) takes over.
        renzora_ember::reactive::bind_display(commands, img, |w| {
            w.get_resource::<ViewportSettings>().map(|s| s.viewport_view) != Some(ViewportView::Ui)
        });
        let editor = renzora_game_ui_editor::build_ui_canvas(commands, fonts);
        renzora_ember::reactive::bind_display(commands, editor, |w| {
            w.get_resource::<ViewportSettings>().map(|s| s.viewport_view) == Some(ViewportView::Ui)
        });
        commands.entity(content).add_child(editor);
        let header = crate::native_header::build_header(commands, fonts);
        commands.entity(root).add_children(&[header, content]);
    } else {
        commands.entity(root).add_child(content);
    }
    root
}

/// Publish each native viewport's on-screen rect + hover to
/// [`ViewportResizeRequest`] (logical px, matching the egui panel) so the
/// resolver resizes the render image and the gizmo/drop/nav systems can map the
/// cursor into the scene.
fn report_viewport_geometry(
    viewports: Query<(&ComputedNode, &RelativeCursorPosition, &NativeViewport)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    req: Option<Res<ViewportResizeRequest>>,
    overlays: Query<(), With<renzora_ember::widgets::Overlay>>,
) {
    let Some(req) = req else {
        return;
    };
    // A modal overlay swallows pointer input — don't let clicks/picking reach the
    // scene behind it.
    let modal_open = !overlays.is_empty();
    // Logical px from the window's top-left — the same space picking / camera
    // read `window.cursor_position()` in.
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    for (cn, rcp, vp) in &viewports {
        let Some(slot) = req.slots.get(vp.0) else {
            continue;
        };
        let inv = cn.inverse_scale_factor();
        let size = cn.size() * inv; // logical
        slot.width.store(size.x.max(1.0) as u32, Ordering::Relaxed);
        slot.height.store(size.y.max(1.0) as u32, Ordering::Relaxed);
        slot.hovered.store(rcp.cursor_over && !modal_open, Ordering::Relaxed);
        // Derive the node's screen top-left from the cursor + its normalized
        // position in the node ((-0.5,-0.5) = top-left). Scale-invariant, so it
        // lands in logical px regardless of DPI — and avoids UI `GlobalTransform`
        // coordinate-space ambiguity. Drives cursor→scene raycasting (picking).
        if let (Some(cursor), Some(norm)) = (cursor, rcp.normalized) {
            let top_left = cursor - (norm + Vec2::splat(0.5)) * size;
            slot.screen_x.store(top_left.x.to_bits(), Ordering::Relaxed);
            slot.screen_y.store(top_left.y.to_bits(), Ordering::Relaxed);
        }
    }
}

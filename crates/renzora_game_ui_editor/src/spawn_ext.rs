//! Editor-only spawn helpers relocated from `renzora_ember::game_ui::spawn`.
//!
//! These read `renzora::EditorSelection` / use the `image` crate, which the
//! lean runtime `renzora_game_ui` crate no longer carries.

use bevy::prelude::*;

use renzora_ember::game_ui::components::*;

/// Choose the parent to use for a "Add Widget" action in the editor.
///
/// Order of preference:
/// 1. Current `EditorSelection` if it's a UiCanvas or a UiWidget Container —
///    the user expects new widgets to land "inside what I'm working in."
/// 2. The provided `active_canvas` fallback — the canvas tab passes its
///    active canvas here.
/// 3. `None` — `spawn_widget` will fall back to "any canvas, or spawn one."
pub fn pick_spawn_parent(world: &World, active_canvas: Option<Entity>) -> Option<Entity> {
    if let Some(sel_res) = world.get_resource::<renzora::EditorSelection>() {
        if let Some(sel) = sel_res.get() {
            // Container/Panel: a layout-mode parent for nested widgets.
            let is_container = world
                .get::<UiWidget>(sel)
                .map(|w| matches!(w.widget_type, UiWidgetType::Container | UiWidgetType::Panel))
                .unwrap_or(false);
            if is_container {
                return Some(sel);
            }
            // Canvas: a free-placement parent.
            if world.get::<UiCanvas>(sel).is_some() {
                return Some(sel);
            }
        }
    }
    active_canvas
}

// ── Image-at-position (drag-drop from asset browser) ──────────────────────
//
// Converts the file path to an asset-relative path and spawns an Image
// widget at the drop coordinates, snapped to grid if enabled. Reads the
// dropped image's real dimensions via the `image` crate (editor-only dep).

/// Reference resolution helper for computing percent values.
struct Ref {
    w: f32,
    h: f32,
}

fn pct_w(px: f32, r: &Ref) -> Val {
    Val::Percent(px / r.w * 100.0)
}
fn pct_h(px: f32, r: &Ref) -> Val {
    Val::Percent(px / r.h * 100.0)
}

pub fn spawn_image_at(
    world: &mut World,
    asset_path: &std::path::Path,
    x: f32,
    y: f32,
    snap: bool,
    grid: f32,
    parent: Option<Entity>,
) {
    let canvas_entity = {
        let mut q = world.query_filtered::<Entity, With<UiCanvas>>();
        match parent.or_else(|| q.iter(world).next()) {
            Some(e) => e,
            None => world
                .spawn((
                    Name::new("UI Canvas"),
                    UiCanvas::default(),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                ))
                .id(),
        }
    };

    let r = parent
        .and_then(|p| world.get::<UiCanvas>(p))
        .map(|c| Ref {
            w: c.reference_width,
            h: c.reference_height,
        })
        .unwrap_or(Ref {
            w: 1280.0,
            h: 720.0,
        });

    let load_path = if let Some(project) = world.get_resource::<renzora::CurrentProject>() {
        project.make_asset_relative(asset_path)
    } else {
        asset_path.to_string_lossy().replace('\\', "/")
    };

    let image_handle: Handle<Image> = world.resource::<AssetServer>().load(load_path.clone());

    let (img_w, img_h) = ::image::image_dimensions(asset_path)
        .map(|(w, h)| (w as f32, h as f32))
        .unwrap_or((128.0, 128.0));

    let mut px = x;
    let mut py = y;
    if snap {
        px = (px / grid).round() * grid;
        py = (py / grid).round() * grid;
    }

    let name = asset_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Image")
        .to_string();

    let entity = world
        .spawn((
            Name::new(name),
            UiWidget {
                widget_type: UiWidgetType::Image,
                locked: false,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(px / r.w * 100.0),
                top: Val::Percent(py / r.h * 100.0),
                width: pct_w(img_w, &r),
                height: pct_h(img_h, &r),
                ..default()
            },
            ImageNode::new(image_handle),
            UiImagePath { path: load_path },
            UiThemed,
        ))
        .id();

    world.entity_mut(entity).set_parent_in_place(canvas_entity);

    if let Some(sel) = world.get_resource::<renzora::EditorSelection>() {
        sel.set(Some(entity));
    }
}

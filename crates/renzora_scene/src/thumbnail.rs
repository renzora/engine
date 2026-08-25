//! Save-time viewport snapshot for scene files.
//!
//! Every scene save also stores a picture of what the viewport was showing, at
//! `<project>/.cache/thumbnails/scenes/<rel>.png`, so the asset browser can
//! preview a scene without loading it.
//!
//! Save is the only cheap moment to take it. A material or a model thumbnail is
//! rendered on demand — the renderer loads that one asset into an offscreen
//! camera and snaps it. Doing the same for a scene would mean instantiating the
//! whole scene, which is exactly the work the browser exists to avoid. At save
//! time the scene is already on screen and already rendered, so the snapshot
//! costs one GPU readback and nothing else.
//!
//! The flow is two-step because the capture needs the render target, and the
//! save that knows the path is an exclusive `&mut World` system:
//! [`super::save_scene_system`] leaves a [`PendingSceneThumbnail`] behind, and
//! [`capture_scene_thumbnail`] picks it up and dispatches the `Screenshot`.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use renzora::core::viewport_types::ViewportState;
use renzora::core::{CurrentProject, EditorLocked, HideInHierarchy};
use renzora_editor_framework::{scene_thumb_path, SceneThumbnailRegistry};

/// Edge length of the written PNG. The browser draws tiles at ~96 px, so 256
/// leaves HiDPI headroom without turning the cache into a screenshot folder.
///
/// Square, and the capture is centre-cropped to reach it, because the viewport's
/// aspect is whatever the user's dock layout happens to make it — drag the
/// assets panel wider, or open a second panel beside the viewport, and a
/// thumbnail that inherited the viewport's aspect would be a different shape
/// from the one taken five minutes earlier. Tiles have a fixed box, so that
/// reads as the previews randomly changing size.
const THUMB_SIZE: u32 = 256;

/// Smallest viewport a snapshot is taken from. An *undocked* viewport slot
/// keeps rendering at a 64 px stub target (it still feeds the atmosphere / IBL
/// probe), and baking that into the cache would replace a good thumbnail with a
/// blurry square. Below this, the previous thumbnail is left alone.
const MIN_CAPTURE_EDGE: u32 = 128;

/// A scene was just written to this path and still needs its viewport snapshot.
/// Inserted by the save systems, consumed by [`capture_scene_thumbnail`].
#[derive(Resource)]
pub(crate) struct PendingSceneThumbnail(pub PathBuf);

/// Dispatches the viewport readback for a freshly saved scene.
///
/// The snapshot is of the *focused* viewport, which in a split layout is the
/// one the user was last working in — the view they'd expect to recognise on
/// the tile.
pub(crate) fn capture_scene_thumbnail(
    mut commands: Commands,
    pending: Option<Res<PendingSceneThumbnail>>,
    viewport: Res<ViewportState>,
    project: Option<Res<CurrentProject>>,
) {
    let Some(pending) = pending else {
        return;
    };
    // Take the request whatever happens below — a viewport we can't snapshot
    // now won't become snapshot-able by retrying next frame, and a stuck
    // request would fire a readback every frame forever.
    commands.remove_resource::<PendingSceneThumbnail>();

    let scene_path = pending.0.clone();
    let (Some(project), Some(render_image)) = (project, viewport.image_handle.clone()) else {
        return;
    };
    if !viewport.docked
        || viewport.current_size.x < MIN_CAPTURE_EDGE
        || viewport.current_size.y < MIN_CAPTURE_EDGE
    {
        debug!(
            "[scene] viewport too small ({}×{}) for a thumbnail of {}",
            viewport.current_size.x,
            viewport.current_size.y,
            scene_path.display()
        );
        return;
    }

    let thumb_path = scene_thumb_path(&scene_path, &project);

    commands
        .spawn((
            Screenshot::image(render_image),
            HideInHierarchy,
            EditorLocked,
            Name::new("Scene Thumbnail Screenshot"),
        ))
        .observe(
            move |trigger: On<ScreenshotCaptured>,
                  mut images: ResMut<Assets<Image>>,
                  mut registry: ResMut<SceneThumbnailRegistry>| {
                let Some(thumb) = square_thumbnail(&trigger.image) else {
                    warn!(
                        "[scene] viewport capture format {:?} can't be encoded as a thumbnail",
                        trigger.image.texture_descriptor.format
                    );
                    return;
                };
                if let Some(parent) = thumb_path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        warn!("[scene] couldn't create {}: {}", parent.display(), e);
                        return;
                    }
                }
                if let Err(e) = thumb.save(&thumb_path) {
                    warn!(
                        "[scene] couldn't write thumbnail {}: {}",
                        thumb_path.display(),
                        e
                    );
                    return;
                }
                // Publish the in-memory copy rather than letting the registry
                // re-load the file: the asset server keys on path, so a re-save
                // of the same scene would keep serving the previous snapshot's
                // bytes and the tile would never change.
                let (w, h) = thumb.dimensions();
                let handle = images.add(Image::new(
                    Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    thumb.into_raw(),
                    TextureFormat::Rgba8UnormSrgb,
                    default(),
                ));
                registry.complete(scene_path.clone(), handle);
            },
        );
}

/// Convert a viewport readback to a [`THUMB_SIZE`]-square RGBA8 buffer. `None`
/// if the capture's texture format isn't one Bevy can hand back as a
/// `DynamicImage`.
///
/// The square comes from a **centre crop**, not a squash: the middle of the
/// view is what the user framed, and stretching a 16:9 viewport into a square
/// would leave every object visibly too tall.
fn square_thumbnail(captured: &Image) -> Option<image::RgbaImage> {
    let dynamic = captured.clone().try_into_dynamic().ok()?;
    let rgba = dynamic.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let side = w.min(h);
    let cropped =
        image::imageops::crop_imm(&rgba, (w - side) / 2, (h - side) / 2, side, side).to_image();
    if side == THUMB_SIZE {
        return Some(cropped);
    }
    // Lanczos3 to match the texture thumbnail cache — the result is written
    // once and read for the life of the project, so the extra milliseconds are
    // worth it.
    Some(image::imageops::resize(
        &cropped,
        THUMB_SIZE,
        THUMB_SIZE,
        image::imageops::FilterType::Lanczos3,
    ))
}

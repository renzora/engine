//! Keeps every camera's idea of its render target's size honest.
//!
//! Bevy's `camera_system` re-measures a camera's target only when that target
//! announces a change (a window resize, an image asset event) or the camera or
//! its projection changes. It does not when the camera's `RenderTarget` is
//! swapped for a different image, and the editor swaps targets all the time:
//! parking the 3D camera on a token image while 2D owns the view, binding the
//! 2D viewport cameras to their slot images, redirecting a game camera into the
//! stretch image. A camera moved onto an image that did not itself change that
//! frame kept the size of the target it had left, indefinitely.
//!
//! That is harmless for one camera and fatal for two. Bevy caches the 2D depth
//! texture and the main colour texture per render target, each sized by
//! whichever camera reaches it first, so two cameras on one image that
//! disagree about its size produce a depth buffer and a colour target of
//! different sizes. wgpu rejects the pass ("Attachments have differing sizes"),
//! and the engine's render-error policy treats that as fatal: the editor
//! crashed on load with a 1040x740 depth against a 1600x900 colour target.
//!
//! The check here runs just before `camera_system`: any camera whose recorded
//! size differs from the image it actually targets gets its projection marked
//! changed, which makes `camera_system` measure the target again that frame.
//! One comparison per camera per frame, and nothing is written while sizes
//! agree, so a still scene marks nothing changed.

use bevy::camera::{CameraUpdateSystems, RenderTarget};
use bevy::prelude::*;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, refresh_stale_target_sizes.before(CameraUpdateSystems));
}

/// Window targets are checked too. The editor window opens at the launcher's
/// size (1040x740) and grows to the editor's (1600x900) as a project opens, and
/// a 2D camera drawing to the window kept the launcher size through that, which
/// is the crash the module describes, with exactly those two sizes.
fn refresh_stale_target_sizes(
    images: Res<Assets<Image>>,
    windows: Query<&Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<(Entity, &Camera, &RenderTarget, &mut Projection, Option<&Name>)>,
    mut reported: Local<bevy::platform::collections::HashSet<Entity>>,
) {
    for (entity, camera, target, mut projection, name) in &mut cameras {
        let actual = match target {
            RenderTarget::Image(image_target) => match images.get(&image_target.handle) {
                Some(image) => image.size(),
                None => continue,
            },
            RenderTarget::Window(window_ref) => {
                let window_entity = match window_ref {
                    bevy::window::WindowRef::Primary => match primary.iter().next() {
                        Some(e) => e,
                        None => continue,
                    },
                    bevy::window::WindowRef::Entity(e) => *e,
                };
                match windows.get(window_entity) {
                    Ok(window) => UVec2::new(window.physical_width(), window.physical_height()),
                    Err(_) => continue,
                }
            }
            _ => continue,
        };
        if actual.x == 0 || actual.y == 0 {
            continue;
        }
        let recorded = camera.computed.target_info.as_ref().map(|info| info.physical_size);
        if recorded.is_some_and(|r| r != actual) {
            projection.set_changed();
            // Once per camera: which one went stale is the useful fact if this
            // ever turns up a case the check does not cure.
            if reported.insert(entity) {
                info!(
                    "[camera] {} had a stale target size {:?}, target is {:?}; re-measuring",
                    name.map(|n| n.as_str().to_string()).unwrap_or_else(|| format!("{entity}")),
                    recorded,
                    actual
                );
            }
        }
    }
}

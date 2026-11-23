//! Each 3D viewport's overlay, drawn by a camera of its own.
//!
//! # Why
//!
//! The overlay is the transform gizmo, the light, collider and skeleton
//! gizmos, the on-top selection box and the labels: render layer 1 and the
//! slot's own `VIEWPORT_3D_GIZMO_LAYER_BASE + slot`. The slot's scene camera
//! used to draw those layers too, so a gizmo handle lighting up under the
//! cursor cost a full render of the scene, ray tracing, denoiser and all, to
//! change the colour of one line. With Solari on, moving the mouse across the
//! viewport ran it at 30 fps where it idled at 80.
//!
//! Now the scene camera draws layer 0 and nothing else, and an overlay camera
//! per slot draws the overlay layers into a transparent image of its own,
//! which the panel lays over the scene image (`panel.rs`). The redraw gate
//! (`redraw.rs`) wakes the two separately: the overlay redraws whenever the
//! scene does, and also on its own for a change only the overlay shows.
//!
//! # Why this is safe
//!
//! Everything on the overlay layers draws on top of the scene (gizmo groups at
//! `depth_bias: -1`, the handle meshes with depth testing off), so nothing
//! there needs the scene's depth buffer. The one overlay that can be told to
//! hide behind geometry, the selection box with its on-top setting off, moves
//! itself to layer 0 when it does (`renzora_gizmo::selection_visuals`), and so
//! stays with the scene camera.
//!
//! As a side effect the gizmo no longer passes through the scene's temporal
//! antialiasing, upscaler, exposure and tonemapping: its lines are sharp and
//! their colours are the colours they were given.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::render_resource::Extent3d;
use bevy::render::view::Msaa;

use renzora_viewport_api::{Viewports, VIEWPORT_3D_GIZMO_LAYER_BASE, VIEWPORT_COUNT};
use renzora_scene_api::{ViewportCamera, ViewportOverlayCamera};

/// Each slot's overlay image and camera.
#[derive(Resource, Default)]
pub struct ViewportOverlays {
    /// The transparent image each slot's overlay camera draws into, shown over
    /// the slot's scene image.
    pub images: [Option<Handle<Image>>; VIEWPORT_COUNT],
    /// The size each image was last given.
    sizes: [UVec2; VIEWPORT_COUNT],
    /// Each slot's overlay camera, once spawned.
    cameras: [Option<Entity>; VIEWPORT_COUNT],
    /// Whether each slot's overlay is on show: its 3D scene camera owns the
    /// slot image. Off in the 2D view, where an image of the last 3D gizmos
    /// would otherwise sit over the 2D scene. Written by
    /// `sync_viewport_camera_activation`, which decides the same thing for the
    /// scene cameras.
    pub shown: [bool; VIEWPORT_COUNT],
}

/// The panel's image node showing one slot's overlay.
#[derive(Component)]
pub(crate) struct ViewportOverlayImage(pub usize);

/// Give every slot its overlay image, at the size of the slot's scene image,
/// and its overlay camera.
///
/// The same size as the scene image, not the panel: the overlay is stretched
/// over the panel exactly as the scene image is, so the two stay aligned
/// through a render scale below 1 and through a splitter drag, where the scene
/// image keeps its old size until the button comes up.
pub(crate) fn sync_overlay_targets(
    mut commands: Commands,
    viewports: Res<Viewports>,
    mut overlays: ResMut<ViewportOverlays>,
    mut images: ResMut<Assets<Image>>,
    scene_cameras: Query<(Entity, &ViewportCamera)>,
    live_overlays: Query<(), With<ViewportOverlayCamera>>,
) {
    for i in 0..VIEWPORT_COUNT {
        let size = viewports.slots[i].current_size.max(UVec2::ONE);
        let image = match overlays.images[i].clone() {
            Some(handle) => {
                if overlays.sizes[i] != size {
                    if let Some(mut image) = images.get_mut(&handle) {
                        image.resize(Extent3d {
                            width: size.x,
                            height: size.y,
                            depth_or_array_layers: 1,
                        });
                    }
                    overlays.sizes[i] = size;
                }
                handle
            }
            None => {
                let handle = images.add(crate::make_render_target(size.x, size.y));
                overlays.images[i] = Some(handle.clone());
                overlays.sizes[i] = size;
                handle
            }
        };
        // Spawned under the slot's scene camera, so it goes when that camera
        // goes; a camera that has gone is spawned again under the new one.
        let alive = overlays.cameras[i].is_some_and(|e| live_overlays.contains(e));
        if !alive {
            let Some((scene, _)) = scene_cameras.iter().find(|(_, vc)| vc.0 == i) else {
                continue;
            };
            overlays.cameras[i] = Some(spawn_overlay_camera(&mut commands, i, image, scene));
        }
    }
}

/// The overlay camera for `slot`, a child of the slot's scene camera at its
/// exact pose.
///
/// A child, not a copy: transform propagation then places it in the same pass
/// that places the scene camera, before Bevy computes either camera's frustum
/// and visibility. Copying the pose after propagation left the overlay's
/// frustum a frame behind whenever the camera moved, so its handle meshes
/// were culled against where the camera had been, and the gizmo flickered
/// while orbiting.
fn spawn_overlay_camera(
    commands: &mut Commands,
    slot: usize,
    image: Handle<Image>,
    scene_camera: Entity,
) -> Entity {
    commands
        .spawn((
            ChildOf(scene_camera),
            Transform::IDENTITY,
            Camera3d::default(),
            Camera {
                // Off until `sync_viewport_camera_activation` decides it has
                // something to draw.
                is_active: false,
                // After the scene cameras, though with a target of its own the
                // order decides nothing.
                order: 1,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(image.into()),
            RenderLayers::layer(1).with(VIEWPORT_3D_GIZMO_LAYER_BASE + slot),
            // An LDR camera with no post-processing: the overlay is lines and
            // flat handles, drawn in the colours they were given.
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            ViewportOverlayCamera(slot),
            // Kept out of effect routing, the sky and contact shadows, which
            // all leave offscreen utility cameras alone; and out of the redraw
            // gate's scene scan, which skips isolated cameras.
            renzora_scene_api::IsolatedCamera,
            renzora_scene_api::HideInHierarchy,
            renzora_scene_api::EditorLocked,
            Name::new(format!("Editor Overlay Camera {slot}")),
        ))
        .id()
}

/// Give each overlay camera its scene camera's projection (the pose comes from
/// being its child).
///
/// In `PostUpdate` before Bevy's camera update, so the overlay's matrices are
/// built from this frame's projection in the same pass as the scene camera's.
/// Written only when the two differ.
pub(crate) fn follow_scene_projections(
    scenes: Query<&Projection, (With<ViewportCamera>, Without<ViewportOverlayCamera>)>,
    mut overlays: Query<(&ChildOf, &mut Projection), With<ViewportOverlayCamera>>,
) {
    for (parent, mut projection) in &mut overlays {
        let Ok(scene_projection) = scenes.get(parent.parent()) else {
            continue;
        };
        // Compared by the matrices they produce: that covers perspective,
        // orthographic and custom projections alike, aspect ratio included,
        // which matches because the two images are the same size.
        if projection.get_clip_from_view() != scene_projection.get_clip_from_view() {
            *projection = scene_projection.clone();
        }
    }
}

/// Point each panel's overlay node at its slot's image, and show it only while
/// the slot's overlay is on show.
pub(crate) fn sync_overlay_nodes(
    overlays: Res<ViewportOverlays>,
    mut nodes: Query<(&ViewportOverlayImage, &mut ImageNode, &mut Visibility)>,
) {
    for (node, mut image, mut visibility) in &mut nodes {
        let Some(handle) = overlays.images.get(node.0).cloned().flatten() else {
            continue;
        };
        if image.image != handle {
            image.image = handle;
        }
        let shown = overlays.shown.get(node.0).copied().unwrap_or(false);
        visibility.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
}

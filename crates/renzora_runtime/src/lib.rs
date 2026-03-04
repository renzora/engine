//! Renzora Runtime — game engine core without editor dependencies.
//!
//! Provides the game camera, test scene, and core systems.
//! When the editor is present, it renders to an offscreen image.
//! When standalone, it renders directly to the window.

pub mod camera;

use bevy::prelude::*;

/// Plugin that adds the game runtime: camera, scene, and core systems.
pub struct RuntimePlugin;

impl Plugin for RuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewportRenderTarget>()
            .add_systems(Startup, (camera::spawn_runtime_camera, camera::spawn_test_scene))
            .add_systems(Update, camera::sync_camera_render_target);
    }
}

/// Holds the optional render target for the game camera.
///
/// - `Some(handle)` — camera renders to this image (editor mode).
/// - `None` — camera renders to the window (standalone mode).
#[derive(Resource, Default)]
pub struct ViewportRenderTarget {
    pub image: Option<Handle<Image>>,
}

/// Marker component for the main game camera.
#[derive(Component)]
pub struct RuntimeCamera;

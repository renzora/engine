mod camera;
mod camera_preview;
mod texture;

pub use camera::camera_controller;
pub use camera_preview::{
    setup_camera_preview_texture, update_camera_preview, CameraPreviewImage,
};
pub use texture::{resize_viewport_texture, setup_viewport_texture};

use bevy::prelude::*;

#[derive(Resource)]
pub struct ViewportImage(pub Handle<Image>);

pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_viewport_texture, setup_camera_preview_texture));
    }
}

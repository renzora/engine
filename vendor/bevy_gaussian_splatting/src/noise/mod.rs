use bevy::{
    asset::uuid_handle,
    prelude::*,
};

const NOISE_SHADER_HANDLE: Handle<Shader> = uuid_handle!("4f73e89b-30f9-48de-b2b3-3d0f09f09f6f");

#[derive(Default)]
pub struct NoisePlugin;

impl Plugin for NoisePlugin {
    fn build(&self, app: &mut App) {
        crate::load_wesl!(app, NOISE_SHADER_HANDLE, "noise.wesl", "noise/noise");
    }
}

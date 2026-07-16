use bevy::{
    asset::{load_internal_asset, uuid_handle},
    prelude::*,
};

const DEPTH_SHADER_HANDLE: Handle<Shader> = uuid_handle!("72e596c7-6226-4366-af26-2acceb34c8a4");

pub struct DepthMaterialPlugin;

impl Plugin for DepthMaterialPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, DEPTH_SHADER_HANDLE, "depth.wgsl", Shader::from_wgsl);
    }
}

use bevy::{
    asset::{load_internal_asset, uuid_handle},
    prelude::*,
};

const POSITION_SHADER_HANDLE: Handle<Shader> = uuid_handle!("91ad4ad8-5e95-4f30-a262-7d3de4abd5a8");

pub struct PositionMaterialPlugin;

impl Plugin for PositionMaterialPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            POSITION_SHADER_HANDLE,
            "position.wgsl",
            Shader::from_wgsl
        );
    }
}

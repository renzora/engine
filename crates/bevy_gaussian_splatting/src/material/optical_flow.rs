use bevy::{
    asset::{load_internal_asset, uuid_handle},
    prelude::*,
};

const OPTICAL_FLOW_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("e25fefbf-dd95-46f2-89bb-91175f6bb4a6");

pub struct OpticalFlowMaterialPlugin;

impl Plugin for OpticalFlowMaterialPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            OPTICAL_FLOW_SHADER_HANDLE,
            "optical_flow.wgsl",
            Shader::from_wgsl
        );
    }
}

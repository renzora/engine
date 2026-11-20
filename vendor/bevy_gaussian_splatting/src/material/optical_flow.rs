use bevy::{
    asset::uuid_handle,
    prelude::*,
};

const OPTICAL_FLOW_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("e25fefbf-dd95-46f2-89bb-91175f6bb4a6");

pub struct OpticalFlowMaterialPlugin;

impl Plugin for OpticalFlowMaterialPlugin {
    fn build(&self, app: &mut App) {
        crate::load_wesl!(
            app,
            OPTICAL_FLOW_SHADER_HANDLE,
            "optical_flow.wesl",
            "material/optical_flow"
        );
    }
}
